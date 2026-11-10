use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::BTreeMap,
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
                map.iter().collect::<Vec<_>>(),
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
    use std::collections::VecDeque;

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
    assert_eq!(super::decode::<u64>(Cow::Owned(encoded)), 900);
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
        assert!(catch_unwind(AssertUnwindSafe(|| map.insert(0, 50))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map.remove(&0))).is_err());
        assert_eq!(memory.writes.get(), writes);
        let mut reverse = map.keys();
        assert_eq!(reverse.next_back(), Some(6));
        assert_eq!(reverse.next_back(), Some(5));
        assert_eq!(reverse.next_back(), Some(4));
        assert!(catch_unwind(AssertUnwindSafe(|| reverse.next_back())).is_err());
    }
}
