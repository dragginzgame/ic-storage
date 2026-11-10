//! Public collection histories, independent of engine-private layouts.
use super::*;

#[test]
fn wide_key_search_handles_node_occupancy_edges_and_missing_insertion_points() {
    fn key(id: u64) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&id.to_be_bytes());
        bytes
    }
    for population in 0..=24 {
        let backing = VectorMemory::default();
        let host = runtime(backing.clone());
        let binding = crate::StableKey::parse("test.map.v1").unwrap();
        let mut map =
            StableBTreeMap::<[u8; 32], u64>::create(host.open_memory(&binding).unwrap()).unwrap();
        let mut model = std::collections::BTreeMap::new();
        for id in (0..population).rev() {
            assert_eq!(map.insert(key(id * 2), id), model.insert(key(id * 2), id));
        }
        drop(map);
        drop(host);
        let host = runtime(backing);
        let mut map =
            StableBTreeMap::<[u8; 32], u64>::load(host.open_memory(&binding).unwrap()).unwrap();
        for id in 0..=population * 2 + 1 {
            assert_eq!(map.get(&key(id)), model.get(&key(id)).copied());
            assert_eq!(
                map.insert(key(id), id + 100),
                model.insert(key(id), id + 100)
            );
        }
        for id in (0..=population * 2 + 1).rev() {
            assert_eq!(map.remove(&key(id)), model.remove(&key(id)));
            assert_eq!(map.get(&key(id)), None);
        }
        assert!(map.is_empty());
    }
}

#[test]
fn vector_model_mixed_updates_reopen_and_bidirectional_iteration() {
    let backing = VectorMemory::default();
    let mut expected = Vec::new();
    for generation in 0..6 {
        let runtime = runtime(backing.clone());
        let memory = runtime
            .open_memory(&crate::StableKey::parse("test.vec.v1").unwrap())
            .unwrap();
        let vector = if generation == 0 {
            StableVec::<u64>::create(memory).unwrap()
        } else {
            StableVec::load(memory).unwrap()
        };
        for i in 0..250 {
            let value = generation * 1000 + i;
            vector.push(&value);
            expected.push(value);
            if i % 3 == 0 {
                assert_eq!(vector.pop(), expected.pop());
            }
            if i % 7 == 0 && !expected.is_empty() {
                vector.set(0, &value);
                expected[0] = value;
            }
        }
        assert_eq!(vector.iter().collect::<Vec<_>>(), expected);
        assert_eq!(
            vector.iter().rev().collect::<Vec<_>>(),
            expected.iter().copied().rev().collect::<Vec<_>>()
        );
        assert_eq!(vector.get(u64::MAX), None);
    }
}

#[test]
fn generated_map_histories_match_an_independent_model_after_reopen() {
    use std::collections::BTreeMap;
    for seed in 1u64..=16 {
        let backing = VectorMemory::default();
        let runtime = runtime(backing);
        let key = crate::StableKey::parse("test.map.v1").unwrap();
        let mut map =
            StableBTreeMap::<u64, Vec<u8>>::create(runtime.open_memory(&key).unwrap()).unwrap();
        let mut model = BTreeMap::new();
        let mut random = seed;
        for step in 0..500 {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let id = (random >> 32) % 191;
            match random % 16 {
                0 => {
                    drop(map);
                    map = StableBTreeMap::load(runtime.open_memory(&key).unwrap()).unwrap();
                }
                1 if step % 13 == 0 => {
                    map.clear();
                    model.clear();
                }
                1..=3 => assert_eq!(map.remove(&id), model.remove(&id)),
                4 => assert_eq!(map.get(&id), model.get(&id).cloned()),
                5 => {
                    let mut actual = map.range(id / 2..=id);
                    let mut expected = model.range(id / 2..=id);
                    let mut forward = true;
                    loop {
                        let (a, b) = if forward {
                            (actual.next(), expected.next())
                        } else {
                            (actual.next_back(), expected.next_back())
                        };
                        assert_eq!(a.map(MapEntry::into_pair), b.map(|(k, v)| (*k, v.clone())));
                        if b.is_none() {
                            break;
                        }
                        forward = !forward;
                    }
                }
                _ => {
                    let n = [0, 31, 32, 33, 236, 237, 4076, 4077, 4096]
                        [usize::try_from((random >> 16) % 9).unwrap()];
                    let bytes = vec![random.to_le_bytes()[0]; n];
                    assert_eq!(map.insert(id, bytes.clone()), model.insert(id, bytes));
                }
            }
            assert_eq!(map.len(), model.len() as u64);
            if step % 64 == 0 {
                assert_eq!(
                    map.iter().map(MapEntry::into_pair).collect::<Vec<_>>(),
                    model
                        .iter()
                        .map(|(k, v)| (*k, v.clone()))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}

#[derive(Clone)]
struct UncheckedHelpers(Vec<u8>);

thread_local! { static UNCHECKED_DECODES: Cell<usize> = const { Cell::new(0) }; }

impl Storable for UncheckedHelpers {
    const BOUND: Bound = Bound::Bounded {
        max_size: 2,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Result<std::borrow::Cow<'_, [u8]>, CodecError> {
        Ok(std::borrow::Cow::Borrowed(&self.0))
    }
    fn into_bytes(self) -> Result<Vec<u8>, CodecError> {
        Ok(self.0)
    }
    fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Result<Self, CodecError> {
        UNCHECKED_DECODES.with(|calls| calls.set(calls.get() + 1));
        Ok(Self(bytes.into_owned()))
    }
    fn check_bounds(_: &[u8]) -> Result<(), CodecError> {
        Ok(())
    }
    fn to_bytes_checked(&self) -> Result<std::borrow::Cow<'_, [u8]>, CodecError> {
        self.to_bytes()
    }
    fn into_bytes_checked(self) -> Result<Vec<u8>, CodecError> {
        self.into_bytes()
    }
}

fn rejects_without_writing(
    backing: &crate::testing::MeteredMemory<VectorMemory>,
    operation: impl FnOnce(),
) {
    let before = backing.backing().borrow().clone();
    backing.reset();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).is_err());
    let counts = backing.counts();
    assert_eq!((counts.writes, counts.grows), (0, 0));
    assert_eq!(*backing.backing().borrow(), before);
}

#[test]
fn collection_bounds_cannot_be_bypassed_by_overridden_codec_helpers() {
    for len in [0, 1, 3] {
        for operation in 0..5 {
            let backing = crate::testing::MeteredMemory::<VectorMemory>::default();
            let host = runtime(backing.clone());
            let invalid = UncheckedHelpers(vec![9; len]);
            // These deliberately dishonest overrides must never be the
            // collection's authority for its physical record geometry.
            assert!(invalid.to_bytes_checked().is_ok());
            assert!(invalid.clone().into_bytes_checked().is_ok());
            assert!(UncheckedHelpers::check_bounds(&invalid.0).is_ok());
            match operation {
                0 => {
                    let memory = host
                        .open_memory(&crate::StableKey::parse(KEYS[0]).unwrap())
                        .unwrap();
                    let mut map =
                        StableBTreeMap::<u64, UncheckedHelpers, _>::create(memory).unwrap();
                    map.insert(1, UncheckedHelpers(vec![1, 2]));
                    rejects_without_writing(&backing, || {
                        map.insert(1, invalid);
                    });
                }
                1 | 2 => {
                    let memory = host
                        .open_memory(&crate::StableKey::parse(KEYS[3]).unwrap())
                        .unwrap();
                    let vector = StableVec::<UncheckedHelpers, _>::create(memory).unwrap();
                    vector.push(&UncheckedHelpers(vec![1, 2]));
                    rejects_without_writing(&backing, || {
                        if operation == 1 {
                            vector.push(&invalid);
                        } else {
                            vector.set(0, &invalid);
                        }
                    });
                }
                3 => {
                    let memory = host
                        .open_memory(&crate::StableKey::parse(KEYS[2]).unwrap())
                        .unwrap();
                    let mut cell =
                        StableCell::create(memory, UncheckedHelpers(vec![1, 2])).unwrap();
                    rejects_without_writing(&backing, || {
                        cell.set(invalid);
                    });
                }
                _ => {
                    let memory = host
                        .open_memory(&crate::StableKey::parse(KEYS[2]).unwrap())
                        .unwrap();
                    // Reset may reserve capacity before encoding. Preallocate
                    // here to isolate the collection write boundary.
                    memory.grow(1).unwrap();
                    rejects_without_writing(&backing, || {
                        let _ = StableCell::reset(memory, invalid);
                    });
                }
            }
        }
        UNCHECKED_DECODES.with(|calls| calls.set(0));
        assert!(
            std::panic::catch_unwind(|| {
                crate::ordered_map::decode::<UncheckedHelpers>(std::borrow::Cow::Owned(vec![
                    9;
                    len
                ]))
            })
            .is_err()
        );
        assert_eq!(UNCHECKED_DECODES.with(Cell::get), 0);
    }
}
