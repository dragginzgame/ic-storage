use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
    ops::Bound,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use crate::{
    Memory,
    storable::{Bound as CodecBound, CodecError, Storable},
};

use super::OrderedMap;

/// Isolated unmanaged backing for engine tests, never a production integration.
#[derive(Clone, Default)]
struct TestMemory {
    bytes: Rc<RefCell<Vec<u8>>>,
    writes: Rc<Cell<u64>>,
    reads: Rc<Cell<u64>>,
    forbidden: Rc<Cell<Option<(u64, u64)>>>,
    growth_limit: Rc<Cell<Option<u64>>>,
}

impl Memory for TestMemory {
    fn size(&self) -> u64 {
        (self.bytes.borrow().len() / 65_536) as u64
    }
    fn grow(&self, pages: u64) -> i64 {
        let previous = self.size();
        let new = previous.checked_add(pages).expect("test size overflow");
        if self.growth_limit.get().is_some_and(|limit| new > limit) {
            return -1;
        }
        self.bytes
            .borrow_mut()
            .resize(usize::try_from(new * 65_536).unwrap(), 0);
        i64::try_from(previous).unwrap()
    }
    fn read(&self, offset: u64, dst: &mut [u8]) {
        let end = offset + dst.len() as u64;
        if let Some((start, limit)) = self.forbidden.get() {
            assert!(end <= start || offset >= limit, "forbidden payload read");
        }
        self.reads.set(self.reads.get() + 1);
        dst.copy_from_slice(
            &self.bytes.borrow()[usize::try_from(offset).unwrap()..usize::try_from(end).unwrap()],
        );
    }
    fn write(&self, offset: u64, src: &[u8]) {
        self.writes.set(self.writes.get() + 1);
        let start = usize::try_from(offset).unwrap();
        self.bytes.borrow_mut()[start..start + src.len()].copy_from_slice(src);
    }
}

/// Independent direct-read backing for raw initialization and unwind checks.
#[derive(Clone, Default)]
struct RawReadMemory {
    memory: TestMemory,
    raw_reads: Rc<Cell<usize>>,
    initialized: Rc<Cell<usize>>,
    fail_after: Rc<Cell<Option<usize>>>,
}

impl Memory for RawReadMemory {
    fn size(&self) -> u64 {
        self.memory.size()
    }

    fn grow(&self, pages: u64) -> i64 {
        self.memory.grow(pages)
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.memory.read(offset, dst);
    }

    fn write(&self, offset: u64, src: &[u8]) {
        self.memory.write(offset, src);
    }

    #[expect(
        unsafe_code,
        reason = "This isolated backing exercises raw initialization and partial-read unwind."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        self.raw_reads.set(self.raw_reads.get() + 1);
        let bytes = self.memory.bytes.borrow();
        let start = usize::try_from(offset).unwrap();
        let end = start.checked_add(count).unwrap();
        let source = &bytes[start..end];
        let fail_after = self.fail_after.get();
        let initialized = fail_after.unwrap_or(count).min(count);
        // SAFETY: the caller supplies fresh exclusive capacity disjoint from
        // this backing. The admitted source slice contains initialized bytes;
        // the copied prefix fits both the source and the requested destination.
        unsafe {
            std::ptr::copy_nonoverlapping(source.as_ptr(), dst, initialized);
        }
        self.initialized.set(initialized);
        assert!(fail_after.is_none(), "injected partial raw-read failure");
    }
}

fn check_external_blob_roundtrip(memory: impl Memory) {
    let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::new(memory);
    let key = vec![17; 35];
    let value: Vec<_> = (0_usize..100_000)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect();
    map.insert(key.clone(), value.clone());
    assert_eq!(map.get(&key), Some(value.clone()));
    let (stored_key, encoded) = map.iter().next().unwrap();
    assert_eq!(stored_key, key);
    assert_eq!(encoded.as_slice(), value);
    assert_eq!(encoded.decode::<Vec<u8>>(), value);
}

#[test]
fn external_blob_reads_support_default_and_direct_initialization() {
    check_external_blob_roundtrip(TestMemory::default());
    let memory = RawReadMemory::default();
    check_external_blob_roundtrip(memory.clone());
    assert_eq!(memory.raw_reads.get(), 4);
    assert_eq!(memory.initialized.get(), 100_000);
}

#[test]
fn partial_external_blob_reads_unwind_without_decoding_and_can_retry() {
    let memory = RawReadMemory::default();
    let mut map = OrderedMap::<u64, DecodeProbe, _>::new(memory.clone());
    map.insert(
        7,
        DecodeProbe {
            bytes: vec![29; 100_000],
            owned: false,
        },
    );
    let writes = memory.memory.writes.get();
    for prefix in [0, 1, 1023, 4096, 99_999] {
        memory.fail_after.set(Some(prefix));
        VALUE_DECODES.set(0);
        let reads = memory.raw_reads.get();
        assert!(catch_unwind(AssertUnwindSafe(|| map.get(&7))).is_err());
        assert_eq!(memory.raw_reads.get(), reads + 1);
        assert_eq!(memory.initialized.get(), prefix);
        assert_eq!(VALUE_DECODES.get(), 0);
        assert_eq!(memory.memory.writes.get(), writes);
        memory.fail_after.set(None);
        let value = map.get(&7).unwrap();
        assert_eq!(value.bytes, vec![29; 100_000]);
        assert!(value.owned);
        assert_eq!(VALUE_DECODES.get(), 1);
    }
}

#[test]
fn default_external_blob_read_unwinds_after_a_payload_prefix() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, DecodeProbe, _>::new(memory.clone());
    map.insert(
        7,
        DecodeProbe {
            bytes: vec![31; 100_000],
            owned: false,
        },
    );
    let node = map.store.node(map.store.root);
    let blocked = node.value + 16 + 4096;
    memory.forbidden.set(Some((blocked, blocked + 1)));
    memory.reads.set(0);
    VALUE_DECODES.set(0);
    let writes = memory.writes.get();
    assert!(catch_unwind(AssertUnwindSafe(|| map.get(&7))).is_err());
    // Routing and blob admission account for two reads; successful payload
    // reads before the blocked chunk prove the default path began filling it.
    assert!(memory.reads.get() > 2);
    assert_eq!(VALUE_DECODES.get(), 0);
    assert_eq!(memory.writes.get(), writes);
    memory.forbidden.set(None);
    assert_eq!(map.get(&7).unwrap().bytes, vec![31; 100_000]);
    assert_eq!(VALUE_DECODES.get(), 1);
}

#[test]
fn oversized_external_blob_length_is_rejected_before_raw_payload_io() {
    let memory = RawReadMemory::default();
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
    map.insert(7, vec![41; 32]);
    // This length still fits the overall allocation, but exceeds this blob's
    // admitted block capacity. It must fail before requesting raw payload IO.
    memory.write(map.store.root + 64, &49_u64.to_le_bytes());
    let writes = memory.memory.writes.get();
    assert!(catch_unwind(AssertUnwindSafe(|| map.get(&7))).is_err());
    assert_eq!(memory.raw_reads.get(), 0);
    assert_eq!(memory.memory.writes.get(), writes);
}

#[test]
fn mixed_mutations_match_an_ordered_model_and_reopen() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
    let mut model = BTreeMap::new();
    let mut state = 734_991_u64;
    for step in 0_u64..6000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let key = state % 311;
        match state >> 61 {
            0..=3 => {
                let value =
                    vec![u8::try_from(step % 251).unwrap(); usize::try_from(state % 71).unwrap()];
                assert_eq!(map.insert(key, value.clone()), model.insert(key, value));
            }
            4..=6 => {
                assert_eq!(map.remove(&key), model.remove(&key));
            }
            _ => {
                assert_eq!(map.get(&key), model.get(&key).cloned());
            }
        }
        if step % 79 == 0 {
            map = OrderedMap::load(memory.clone()).unwrap();
            assert_eq!(map.len(), model.len() as u64);
            assert_eq!(
                map.iter()
                    .map(|(key, bytes)| (key, bytes.decode::<Vec<u8>>()))
                    .collect::<Vec<_>>(),
                model
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                map.first_key_value(),
                model.first_key_value().map(|(k, v)| (*k, v.clone()))
            );
            assert_eq!(
                map.last_key_value(),
                model.last_key_value().map(|(k, v)| (*k, v.clone()))
            );
        }
    }
    for key in model.keys() {
        assert_eq!(map.remove(key), model.get(key).cloned());
    }
    assert!(map.is_empty());
    assert!(
        OrderedMap::<u64, Vec<u8>, _>::load(memory)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn monotone_insertions_and_deletions_keep_a_short_tree() {
    let mut map = OrderedMap::<u64, u64, _>::new(TestMemory::default());
    for key in 0..2048 {
        map.insert(key, key + 7);
    }
    assert!(map.store.node(map.store.root).link().height < 16);
    for key in (0..2048).step_by(2) {
        assert_eq!(map.remove(&key), Some(key + 7));
    }
    for key in (1..2048).rev().step_by(2) {
        assert_eq!(map.remove(&key), Some(key + 7));
    }
    assert!(map.is_empty());
}

#[test]
fn empty_and_singleton_extrema_preserve_value_and_io_behavior() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
    memory.reads.set(0);
    assert_eq!(map.first_key_value(), None);
    assert_eq!(map.last_key_value(), None);
    assert_eq!(memory.reads.get(), 0);
    map.insert(7, 17);
    assert_eq!(map.first_key_value(), Some((7, 17)));
    assert_eq!(map.last_key_value(), Some((7, 17)));
    map.remove(&7);
    memory.reads.set(0);
    assert_eq!(map.first_key_value(), None);
    assert_eq!(map.last_key_value(), None);
    assert_eq!(memory.reads.get(), 0);
}

#[test]
fn extrema_reject_inconsistent_selected_children_before_value_decode() {
    for reverse in [false, true] {
        for (offset, replacement) in [(72, 2_u64), (32, 0x0102)] {
            let memory = TestMemory::default();
            let mut map = OrderedMap::<u64, DecodeProbe, _>::new(memory.clone());
            for key in [3, 1, 5, 0, 2, 4, 6] {
                map.insert(
                    key,
                    DecodeProbe {
                        bytes: vec![u8::try_from(key).unwrap()],
                        owned: false,
                    },
                );
            }
            let root = map.store.node(map.store.root);
            let child = if reverse { root.right } else { root.left };
            memory.write(child.address + offset, &replacement.to_le_bytes());
            let writes = memory.writes.get();
            VALUE_DECODES.set(0);
            memory.reads.set(0);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    if reverse {
                        map.last_key_value()
                    } else {
                        map.first_key_value()
                    }
                }))
                .is_err()
            );
            assert_eq!(VALUE_DECODES.get(), 0);
            assert_eq!(memory.reads.get(), 2);
            let (key, value) = if reverse {
                map.first_key_value()
            } else {
                map.last_key_value()
            }
            .unwrap();
            assert_eq!(key, if reverse { 0 } else { 6 });
            assert_eq!(value.bytes, vec![u8::try_from(key).unwrap()]);
            assert_eq!(VALUE_DECODES.get(), 1);
            assert_eq!(memory.writes.get(), writes);
        }
    }
}

#[test]
fn ranges_and_alternating_ends_are_fused() {
    let mut map = OrderedMap::<u64, u64, _>::new(TestMemory::default());
    for key in 0..31 {
        map.insert(key * 3, key);
    }
    for start in 0..25 {
        for end in start..32 {
            let expected: Vec<_> = (0..31)
                .map(|key| key * 3)
                .filter(|key| *key > start && *key <= end)
                .collect();
            let mut iter = map.keys_range((Bound::Excluded(start), Bound::Included(end)));
            let mut front = 0;
            let mut back = expected.len();
            while front < back {
                assert_eq!(iter.next(), Some(expected[front]));
                front += 1;
                if front < back {
                    back -= 1;
                    assert_eq!(iter.next_back(), Some(expected[back]));
                }
            }
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next_back(), None);
        }
    }
    let mut entries = map.iter();
    assert_eq!(entries.nth(3).unwrap().0, 9);
    assert_eq!(entries.nth_back(2).unwrap().0, 84);
    assert_eq!(entries.count(), 24);
    assert_eq!(map.range(9..9).count(), 0);
    assert_eq!(map.values().nth(7), Some(7));
}

#[test]
fn admitted_bound_paths_cover_missing_endpoints_and_mixed_jumps() {
    for population in [0, 31] {
        let mut map = OrderedMap::<u64, u64, _>::new(TestMemory::default());
        let mut model = BTreeMap::new();
        for key in 1..=population {
            map.insert(key * 2, key);
            model.insert(key * 2, key);
        }
        let mut bounds = vec![Bound::Unbounded];
        for key in [0, 1, 2, 3, 16, 17, 31, 32, 61, 62, 63, 64] {
            bounds.extend([Bound::Included(key), Bound::Excluded(key)]);
        }
        for &start in &bounds {
            for &end in &bounds {
                if let (
                    Bound::Included(a) | Bound::Excluded(a),
                    Bound::Included(b) | Bound::Excluded(b),
                ) = (start, end)
                    && (a > b
                        || (a == b
                            && matches!((start, end), (Bound::Excluded(_), Bound::Excluded(_)))))
                {
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| map.range((start, end)).count())).is_err()
                    );
                    continue;
                }
                let expected: VecDeque<_> = model.range((start, end)).map(|(k, _)| *k).collect();
                assert_eq!(map.range((start, end)).count(), expected.len());
                for reverse_first in [false, true] {
                    let mut remaining = expected.clone();
                    let mut iter = map.keys_range((start, end));
                    let mut reverse = reverse_first;
                    for skip in [0, 0, 2, 0, 1].into_iter().cycle().take(40) {
                        let mut answer = None;
                        for _ in 0..=skip {
                            answer = if reverse {
                                remaining.pop_back()
                            } else {
                                remaining.pop_front()
                            };
                            if answer.is_none() {
                                break;
                            }
                        }
                        let actual = if reverse {
                            iter.nth_back(skip)
                        } else {
                            iter.nth(skip)
                        };
                        assert_eq!(actual, answer, "{start:?}..{end:?}");
                        assert_eq!(iter.size_hint(), (remaining.len(), Some(remaining.len())));
                        reverse = !reverse;
                    }
                    assert!(remaining.is_empty());
                    assert_eq!(iter.next(), None);
                    assert_eq!(iter.next_back(), None);
                }
            }
        }
    }
}

#[test]
fn admitted_singleton_bounds_need_no_second_routing_descent() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
    for key in 0..4096 {
        map.insert(key * 2, key + 1);
    }
    for start in [Bound::Included(4096), Bound::Excluded(4095)] {
        for end in [Bound::Included(4096), Bound::Excluded(4097)] {
            for reverse in [false, true] {
                memory.reads.set(0);
                let mut keys = map.keys_range((start, end));
                let admission_reads = memory.reads.get();
                assert!(admission_reads > 0);
                let key = if reverse {
                    keys.next_back()
                } else {
                    keys.next()
                };
                assert_eq!(key, Some(4096));
                assert_eq!(memory.reads.get(), admission_reads);
                assert_eq!(keys.next(), None);
                assert_eq!(keys.next_back(), None);

                let mut entries = map.range((start, end));
                memory.reads.set(0);
                let entry = if reverse {
                    entries.next_back()
                } else {
                    entries.next()
                };
                let (key, bytes) = entry.unwrap();
                assert_eq!(key, 4096);
                assert_eq!(bytes.decode::<u64>(), 2049);
                // The only post-admission read is the inline value payload.
                assert_eq!(memory.reads.get(), 1);
            }
        }
    }
    let mut count_only = map.range(2048..6144);
    memory.reads.set(0);
    assert!(count_only.nth(usize::MAX).is_none());
    assert!(count_only.next_back().is_none());
    assert_eq!(memory.reads.get(), 0);
    let count_only = map.range(2048..6144);
    memory.reads.set(0);
    assert_eq!(count_only.count(), 2048);
    assert_eq!(memory.reads.get(), 0);
}

#[test]
fn empty_and_large_records_survive_reopen() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::new(memory.clone());
    let key = vec![19; 7000];
    let value = vec![71; 180_000];
    map.insert(Vec::new(), Vec::new());
    map.insert(key.clone(), value.clone());
    let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::load(memory).unwrap();
    assert_eq!(map.get(&key), Some(value));
    assert_eq!(map.remove(&Vec::new()), Some(Vec::new()));
    assert_eq!(map.keys().collect::<Vec<_>>(), vec![key]);
    let mut unit = OrderedMap::<(), (), _>::new(TestMemory::default());
    assert_eq!(unit.insert((), ()), None);
    assert_eq!(unit.insert((), ()), Some(()));
    assert_eq!(unit.remove(&()), Some(()));
}

#[test]
fn churn_reuses_classes_and_clear_reuses_dirty_capacity() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
    let sizes = [0, 13, 45, 200, 900, 2500];
    for round in 0..5 {
        for size in sizes {
            for key in 0..80 {
                map.insert(key, vec![37; size]);
            }
            for key in 0..80 {
                map.remove(&key);
            }
        }
        let pages = memory.size();
        if round == 0 {
            memory.growth_limit.set(Some(pages));
        }
        assert_eq!(Some(pages), memory.growth_limit.get());
    }
    map.clear();
    for key in 0..80 {
        map.insert(key, vec![99; 200]);
    }
    let reopened = OrderedMap::<u64, Vec<u8>, _>::load(memory).unwrap();
    assert_eq!(reopened.get(&17), Some(vec![99; 200]));
}

#[test]
fn lookup_and_key_iteration_never_read_value_payload() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
    map.insert(7, vec![99; 100_000]);
    let node = map.store.node(map.store.root);
    memory
        .forbidden
        .set(Some((node.value + 16, node.value + 16 + node.value_len)));
    assert!(map.contains_key(&7));
    assert_eq!(map.keys().collect::<Vec<_>>(), vec![7]);
    assert_eq!(map.keys_range(7..=7).collect::<Vec<_>>(), vec![7]);
    assert_eq!(map.keys_range(..=7).next_back(), Some(7));
    assert_eq!(map.range(7..=7).count(), 1);
    assert_eq!(map.iter().count(), 1);
    assert_eq!(map.values().count(), 1);
    assert!(map.values().nth(1).is_none());
    let writes = memory.writes.get();
    assert_eq!(map.remove(&8), None);
    assert_eq!(memory.writes.get(), writes);
}

#[test]
fn framing_refusal_is_read_only_and_reopen_is_bounded() {
    let memory = TestMemory::default();
    memory.grow(1);
    memory.bytes.borrow_mut().fill(0x7b);
    let before = memory.bytes.borrow().clone();
    assert!(OrderedMap::<u64, u64, _>::load(memory.clone()).is_none());
    assert_eq!(*memory.bytes.borrow(), before);
    let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
    for key in 0..500 {
        map.insert(key, key);
    }
    memory.reads.set(0);
    let map = OrderedMap::<u64, u64, _>::load(memory.clone()).unwrap();
    assert!(memory.reads.get() <= 2);
    assert_eq!(map.len(), 500);
    memory.write(8, &9_u64.to_le_bytes());
    let writes = memory.writes.get();
    assert!(OrderedMap::<u64, u64, _>::load(memory.clone()).is_none());
    assert_eq!(memory.writes.get(), writes);
}

#[test]
fn corrupt_links_and_failed_growth_trap_without_reset() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
    map.insert(7, 8);
    let root = map.store.root;
    memory.write(root + 16, &root.to_le_bytes());
    let writes = memory.writes.get();
    assert!(catch_unwind(AssertUnwindSafe(|| map.get(&0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| map.range(0..).count())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| map.range(..=7).count())).is_err());
    assert_eq!(memory.writes.get(), writes);
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
    memory.growth_limit.set(Some(1));
    assert!(catch_unwind(AssertUnwindSafe(|| map.insert(1, vec![8; 100_000]))).is_err());
    assert_eq!(&memory.bytes.borrow()[..8], b"STORMAP1");
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct BadWidth;

impl Storable for BadWidth {
    const BOUND: CodecBound = CodecBound::Bounded {
        max_size: 1,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
        Ok(Cow::Borrowed(&[]))
    }
    fn into_bytes(self) -> Result<Vec<u8>, CodecError> {
        Ok(Vec::new())
    }
    fn from_bytes(_: Cow<'_, [u8]>) -> Result<Self, CodecError> {
        Ok(Self)
    }
}

#[test]
fn engine_checks_codec_bounds_before_mutation() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<BadWidth, (), _>::new(memory.clone());
    let writes = memory.writes.get();
    assert!(catch_unwind(AssertUnwindSafe(|| map.insert(BadWidth, ()))).is_err());
    assert_eq!(memory.writes.get(), writes);
}

#[test]
fn cursor_jumps_and_alternating_paths_match_a_deque() {
    let mut map = OrderedMap::<u64, u64, _>::new(TestMemory::default());
    for key in 0..400 {
        map.insert(key * 2, key + 1);
    }
    for seed in 1..80 {
        let start = seed * 3;
        let end = 780 - seed * 2;
        let mut expected: VecDeque<_> = (0..400)
            .map(|key| key * 2)
            .filter(|key| *key >= start && *key < end)
            .collect();
        let mut cursor = map.keys_range(start..end);
        let mut state = seed;
        for _ in 0..80 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let reverse = state & 1 != 0;
            let skip = usize::try_from((state >> 8) % 9).unwrap();
            let mut answer = None;
            for _ in 0..=skip {
                answer = if reverse {
                    expected.pop_back()
                } else {
                    expected.pop_front()
                };
                if answer.is_none() {
                    break;
                }
            }
            let actual = if reverse {
                cursor.nth_back(skip)
            } else {
                cursor.nth(skip)
            };
            assert_eq!(actual, answer);
            assert_eq!(cursor.size_hint(), (expected.len(), Some(expected.len())));
        }
        assert!(cursor.nth(usize::MAX).is_none());
        assert!(cursor.next_back().is_none());
        assert!(cursor.next().is_none());
    }
    let mut entries = map.range(50..701);
    assert_eq!(entries.next().unwrap().0, 50);
    assert_eq!(entries.next_back().unwrap().0, 700);
    assert_eq!(entries.nth(10).unwrap().0, 72);
    assert_eq!(entries.next_back().unwrap().0, 698);
    assert_eq!(entries.nth_back(10).unwrap().0, 676);
    assert_eq!(entries.next().unwrap().0, 74);
    assert_eq!(entries.next_back().unwrap().0, 674);
    assert_eq!(entries.count(), 299);
}

#[test]
fn cursor_entries_preserve_payloads_across_mixed_jumps_and_reuse() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
    let mut model = BTreeMap::new();
    for index in 0_u64..127 {
        let key = index * 37 % 127;
        let size = [0, 8, 24, 25, 300][usize::try_from(key % 5).unwrap()];
        let value = vec![u8::try_from(key).unwrap(); size];
        map.insert(key, value.clone());
        model.insert(key, value);
    }
    let mut expected: VecDeque<_> = model
        .range(17..=110)
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    let mut cursor = map.range(17..=110);
    let mut retained = Vec::new();
    for (step, skip) in [0, 0, 2, 0, 4].into_iter().cycle().take(24).enumerate() {
        let reverse = step % 2 != 0;
        let mut answer = None;
        for _ in 0..=skip {
            answer = if reverse {
                expected.pop_back()
            } else {
                expected.pop_front()
            };
            if answer.is_none() {
                break;
            }
        }
        let actual = if reverse {
            cursor.nth_back(skip)
        } else {
            cursor.nth(skip)
        };
        match (actual, answer) {
            (Some((key, bytes)), Some((expected_key, expected_bytes))) => {
                assert_eq!(key, expected_key);
                assert_eq!(bytes.as_slice(), expected_bytes);
                retained.push((bytes, expected_bytes));
            }
            (None, None) => {}
            _ => panic!("directional cursor disagrees with model"),
        }
        assert_eq!(cursor.size_hint(), (expected.len(), Some(expected.len())));
    }
    let reads = memory.reads.get();
    assert_eq!(cursor.count(), expected.len());
    assert_eq!(memory.reads.get(), reads);
    map.clear();
    for key in 0..127 {
        map.insert(key, vec![99; 300]);
    }
    for (bytes, expected) in retained {
        assert_eq!(bytes.decode::<Vec<u8>>(), expected);
    }
}

#[test]
fn inline_values_remain_unread_during_key_scans_and_entries_own_their_bytes() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
    map.insert(7, 900);
    let address = map.store.root;
    memory.forbidden.set(Some((address + 104, address + 128)));
    assert!(map.contains_key(&7));
    assert_eq!(map.keys().collect::<Vec<_>>(), vec![7]);
    assert_eq!(map.iter().count(), 1);
    memory.forbidden.set(None);
    let (_, encoded) = map.iter().next().unwrap();
    map.insert(7, 12);
    assert_eq!(encoded.decode::<u64>(), 900);
}

thread_local! {
    static VALUE_DECODES: Cell<usize> = const { Cell::new(0) };
}

struct DecodeProbe {
    bytes: Vec<u8>,
    owned: bool,
}

impl Storable for DecodeProbe {
    const BOUND: CodecBound = CodecBound::Unbounded;

    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
        Ok(Cow::Borrowed(&self.bytes))
    }

    fn into_bytes(self) -> Result<Vec<u8>, CodecError> {
        Ok(self.bytes)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
        VALUE_DECODES.set(VALUE_DECODES.get() + 1);
        let owned = matches!(bytes, Cow::Owned(_));
        Ok(Self {
            bytes: bytes.into_owned(),
            owned,
        })
    }
}

#[test]
fn encoded_values_keep_inline_decoding_borrowed_and_entry_decoding_lazy() {
    for size in [0, 24, 25, 100_000] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<u64, DecodeProbe, _>::new(memory.clone());
        map.insert(
            7,
            DecodeProbe {
                bytes: vec![39; size],
                owned: false,
            },
        );
        let map = OrderedMap::<u64, DecodeProbe, _>::load(memory).unwrap();
        VALUE_DECODES.set(0);
        assert_eq!(map.keys().collect::<Vec<_>>(), vec![7]);
        assert_eq!(map.range(7..=7).count(), 1);
        let (key, encoded) = map.iter().next().unwrap();
        assert_eq!(key, 7);
        assert_eq!(encoded.as_slice(), vec![39; size]);
        assert_eq!(VALUE_DECODES.get(), 0);
        let value = encoded.decode::<DecodeProbe>();
        assert_eq!(VALUE_DECODES.get(), 1);
        assert_eq!(value.bytes, vec![39; size]);
        assert_eq!(value.owned, size > super::storage::INLINE_VALUE);
        let value = map.get(&7).unwrap();
        assert_eq!(VALUE_DECODES.get(), 2);
        assert_eq!(value.bytes, vec![39; size]);
        assert_eq!(value.owned, size > super::storage::INLINE_VALUE);
    }
}

#[test]
fn encoded_values_remain_owned_after_replacement_removal_and_clear() {
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(TestMemory::default());
    let sizes = [0, 24, 25, 100_000];
    for (key, size) in sizes.into_iter().enumerate() {
        map.insert(key as u64, vec![u8::try_from(key).unwrap(); size]);
    }
    let entries = map.iter().collect::<Vec<_>>();
    map.insert(1, vec![91; 25]);
    map.remove(&2);
    map.clear();
    map.insert(0, vec![92; 100_000]);
    for (key, bytes) in entries {
        let expected = vec![u8::try_from(key).unwrap(); sizes[usize::try_from(key).unwrap()]];
        assert_eq!(bytes.as_slice(), expected);
        assert_eq!(bytes.decode::<Vec<u8>>(), expected);
    }
}

#[test]
fn invalid_value_codec_width_is_rejected_before_payload_reads() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
    map.insert(7, 8);
    memory.write(map.store.root + 64, &7_u64.to_le_bytes());
    for entries in [false, true] {
        memory.reads.set(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            if entries {
                let _ = map.iter().next();
            } else {
                let _ = map.get(&7);
            }
        }));
        assert!(result.is_err());
        assert_eq!(memory.reads.get(), 1);
    }
}

#[test]
fn small_records_pack_tightly_and_scans_read_each_path_once() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
    for key in 0..4096 {
        map.insert(key, key + 1);
    }
    assert_eq!(memory.size(), 9);
    memory.reads.set(0);
    assert_eq!(map.keys().fold(0, |count, _| count + 1), 4096);
    assert!(memory.reads.get() <= 4128);
    memory.reads.set(0);
    let reopened = OrderedMap::<u64, u64, _>::load(memory.clone()).unwrap();
    assert_eq!(reopened.len(), 4096);
    assert!(memory.reads.get() <= 2);
}

#[test]
fn replacements_write_only_the_payload_when_routing_fields_agree() {
    for size in [0, 1, 24, 25, 48, 49, 100_000] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
        for key in [3, 1, 5] {
            map.insert(key, vec![7; size]);
        }
        let pages = memory.size();
        memory.writes.set(0);
        assert_eq!(map.insert(1, vec![9; size]), Some(vec![7; size]));
        assert_eq!(memory.writes.get(), 1, "payload size {size}");
        assert_eq!(memory.size(), pages);
        let reopened = OrderedMap::<u64, Vec<u8>, _>::load(memory).unwrap();
        assert_eq!(reopened.len(), 3);
        assert_eq!(reopened.get(&1), Some(vec![9; size]));
        assert_eq!(reopened.get(&3), Some(vec![7; size]));
        assert_eq!(reopened.get(&5), Some(vec![7; size]));
    }
}

#[test]
fn replacement_length_and_location_changes_publish_routing_fields() {
    for (before, after, same_class) in [
        (1, 2, true),
        (24, 23, true),
        (25, 47, true),
        (48, 25, true),
        (24, 25, false),
        (25, 24, false),
        (48, 49, false),
        (49, 48, false),
    ] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
        map.insert(7, vec![11; before]);
        memory.writes.set(0);
        assert_eq!(map.insert(7, vec![19; after]), Some(vec![11; before]));
        if same_class {
            // Reused payload storage still needs its changed length published.
            assert_eq!(memory.writes.get(), 2);
        } else {
            assert!(memory.writes.get() > 1);
        }
        let reopened = OrderedMap::<u64, Vec<u8>, _>::load(memory).unwrap();
        assert_eq!(reopened.len(), 1);
        assert_eq!(reopened.get(&7), Some(vec![19; after]));
    }
}

#[test]
fn replacements_move_between_inline_and_external_classes() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
    for key in 0..80 {
        map.insert(key, vec![u8::try_from(key).unwrap(); 23]);
    }
    for size in [24, 25, 90, 0, 16, 70_000, 2] {
        for key in 0..80 {
            let previous = map.get(&key).unwrap();
            assert_eq!(map.insert(key, vec![9; size]), Some(previous));
        }
        map = OrderedMap::load(memory.clone()).unwrap();
        for key in 0..80 {
            assert_eq!(map.get(&key), Some(vec![9; size]));
        }
    }
    for key in (0..80).rev() {
        assert_eq!(map.remove(&key), Some(vec![9; 2]));
    }
    assert!(map.is_empty());
}

#[test]
fn cursor_only_reads_successors_and_predecessors_when_requested() {
    for reverse in [false, true] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
        for key in [3, 1, 5, 0, 2, 4, 6] {
            map.insert(key, key + 10);
        }
        let root = map.store.node(map.store.root);
        let child = if reverse { root.left } else { root.right };
        memory
            .forbidden
            .set(Some((child.address, child.address + 104)));
        memory.reads.set(0);
        let mut keys = map.keys();
        let key = if reverse {
            keys.nth_back(3)
        } else {
            keys.nth(3)
        };
        assert_eq!(key, Some(3));
        assert_eq!(memory.reads.get(), 1);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if reverse {
                    keys.next_back()
                } else {
                    keys.next()
                }
            }))
            .is_err()
        );
        memory.forbidden.set(None);

        let mut keys = map.keys();
        if reverse {
            assert_eq!(keys.nth_back(3), Some(3));
        } else {
            assert_eq!(keys.nth(3), Some(3));
        }
        memory.reads.set(0);
        let key = if reverse {
            keys.next_back()
        } else {
            keys.next()
        };
        assert_eq!(key, Some(if reverse { 2 } else { 4 }));
        assert_eq!(memory.reads.get(), 2);

        for count_only in [false, true] {
            let mut keys = map.keys();
            if reverse {
                assert_eq!(keys.nth_back(3), Some(3));
            } else {
                assert_eq!(keys.nth(3), Some(3));
            }
            memory.reads.set(0);
            if count_only {
                assert_eq!(keys.count(), 3);
            } else {
                assert_eq!(keys.nth(usize::MAX), None);
                assert_eq!(keys.next_back(), None);
            }
            assert_eq!(memory.reads.get(), 0);
        }
    }
}

#[test]
fn deferred_cursor_children_are_checked_before_yield() {
    for reverse in [false, true] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
        for key in [3, 1, 5, 0, 2, 4, 6] {
            map.insert(key, key + 10);
        }
        let root = map.store.node(map.store.root);
        let child = if reverse { root.left } else { root.right };
        memory.write(child.address + 72, &2_u64.to_le_bytes());
        let writes = memory.writes.get();
        let mut keys = map.keys();
        let key = if reverse {
            keys.nth_back(3)
        } else {
            keys.nth(3)
        };
        assert_eq!(key, Some(3));
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if reverse {
                    keys.next_back()
                } else {
                    keys.next()
                }
            }))
            .is_err()
        );
        assert_eq!(memory.writes.get(), writes);
    }
}

#[test]
fn inconsistent_cached_child_counts_and_heights_are_rejected_on_descent() {
    // Each corrupted child remains locally well-shaped. Only comparison with
    // the admitted parent link reveals that its routing metadata is inconsistent.
    for (offset, replacement) in [(72, 2_u64), (32, 0x0102)] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
        for key in [3, 1, 5, 0, 2, 4, 6] {
            map.insert(key, key + 10);
        }
        let root = map.store.node(map.store.root);
        let left = root.left.address;
        memory.write(left + offset, &replacement.to_le_bytes());
        // Reopen need not scan all descendants; actual use must reject them.
        let mut map = OrderedMap::<u64, u64, _>::load(memory.clone()).unwrap();
        let writes = memory.writes.get();
        assert!(catch_unwind(AssertUnwindSafe(|| map.iter().nth(2))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map.keys().next())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map.get(&0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map.range(0..2).count())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map.range(..=2).count())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map.insert(0, 50))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map.remove(&0))).is_err());
        assert_eq!(memory.writes.get(), writes);
        let mut reverse = map.keys();
        assert_eq!(reverse.next_back(), Some(6));
        assert_eq!(reverse.next_back(), Some(5));
        assert_eq!(reverse.next_back(), Some(4));
        assert_eq!(reverse.next_back(), Some(3));
        assert!(catch_unwind(AssertUnwindSafe(|| reverse.next_back())).is_err());
    }
}
