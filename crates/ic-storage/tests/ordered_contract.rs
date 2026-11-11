//! Public ordered-collection acceptance, independent of physical tree layout.
use ic_storage::{
    AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryRequest, MemoryRuntime,
    SealedDeclarationSnapshot, StableKey, VectorMemory,
    adapter::{BTreeMapRegion, MapEntry, StableBTreeMap, StableBTreeSet},
    storable::{Bound, CodecError, Storable},
};
use std::{
    borrow::Cow,
    cell::Cell,
    collections::{BTreeMap, VecDeque},
    ops::Bound::{Excluded, Included, Unbounded},
    rc::Rc,
};

fn host(backing: VectorMemory) -> MemoryRuntime<VectorMemory> {
    let declarations = SealedDeclarationSnapshot::new(vec![
        MemoryRequest::new("ordered", "ordered.map.v1").unwrap(),
        MemoryRequest::new("ordered", "ordered.set.v1").unwrap(),
    ])
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("ordered", "ordered.").unwrap()]).unwrap();
    let mut runtime = MemoryRuntime::new(backing).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    runtime
}

fn key() -> StableKey {
    StableKey::parse("ordered.map.v1").unwrap()
}

#[test]
fn signed_key_model_survives_mutation_and_cold_reopen() {
    let backing = VectorMemory::default();
    let mut model = BTreeMap::new();
    let mut random = 0x8345_a879_21d0_3671_u64;
    for epoch in 0..5 {
        let runtime = host(backing.clone());
        let memory = runtime.open_memory(&key()).unwrap();
        let mut map = if epoch == 0 {
            StableBTreeMap::<i64, Vec<u8>, _>::create(memory).unwrap()
        } else {
            StableBTreeMap::<i64, Vec<u8>, _>::load(memory).unwrap()
        };
        for step in 0..1500 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let key = i64::try_from(random % 2048).unwrap() - 1024;
            if step % 4 == 0 {
                assert_eq!(map.remove(&key), model.remove(&key));
            } else {
                let value = vec![random.to_le_bytes()[0]; (step % 97) + 1];
                assert_eq!(map.insert(key, value.clone()), model.insert(key, value));
            }
            assert_eq!(map.get(&key), model.get(&key).cloned());
            assert_eq!(map.len(), model.len() as u64);
        }
        let expected: Vec<_> = model.iter().map(|(k, v)| (*k, v.clone())).collect();
        assert_eq!(
            map.iter().map(MapEntry::into_pair).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(map.first_key_value(), expected.first().cloned());
        assert_eq!(map.last_key_value(), expected.last().cloned());
    }
}

#[test]
fn range_cursors_match_model_when_alternating_ends_and_skipping() {
    let runtime = host(VectorMemory::default());
    let mut map =
        StableBTreeMap::<i64, i64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    let model: BTreeMap<_, _> = (-140..140).map(|k| (k * 3, k * 7)).collect();
    for (&key, &value) in &model {
        map.insert(key, value);
    }
    let starts = [
        Unbounded,
        Included(-171),
        Excluded(-171),
        Included(0),
        Excluded(17),
    ];
    let ends = [
        Unbounded,
        Included(273),
        Excluded(273),
        Included(0),
        Excluded(17),
    ];
    for start in starts {
        for end in ends {
            if matches!((start, end), (Excluded(17), Included(0) | Excluded(17))) {
                continue;
            }
            let mut expected: VecDeque<_> =
                model.range((start, end)).map(|(&k, &v)| (k, v)).collect();
            let mut actual = map.range((start, end));
            while !expected.is_empty() {
                assert_eq!(actual.next().map(MapEntry::into_pair), expected.pop_front());
                assert_eq!(
                    actual.next_back().map(MapEntry::into_pair),
                    expected.pop_back()
                );
            }
            assert!(actual.next().is_none());
            assert!(actual.next_back().is_none());

            // A bound-admitted cursor must remain valid after rank jumps from
            // either end, including gaps between stored keys and empty ranges.
            let mut actual = map.range((start, end));
            let mut actual_keys = map.keys_range((start, end));
            let mut expected = model.range((start, end));
            for (reverse, skip) in [
                (false, 3),
                (true, 7),
                (false, 0),
                (true, 0),
                (false, 25),
                (true, 9),
            ] {
                let wanted = if reverse {
                    expected.nth_back(skip)
                } else {
                    expected.nth(skip)
                }
                .map(|(&k, &v)| (k, v));
                let entry = if reverse {
                    actual.nth_back(skip)
                } else {
                    actual.nth(skip)
                };
                let key = if reverse {
                    actual_keys.nth_back(skip)
                } else {
                    actual_keys.nth(skip)
                };
                assert_eq!(entry.map(MapEntry::into_pair), wanted);
                assert_eq!(key, wanted.map(|(key, _)| key));
            }
            let remainder: Vec<_> = expected.map(|(&k, &v)| (k, v)).collect();
            assert_eq!(
                actual.map(MapEntry::into_pair).collect::<Vec<_>>(),
                remainder
            );
            assert_eq!(
                actual_keys.collect::<Vec<_>>(),
                remainder.iter().map(|&(k, _)| k).collect::<Vec<_>>()
            );
        }
    }
    let mut actual = map.iter();
    let mut expected = model.iter();
    assert_eq!(
        actual.nth(77).map(MapEntry::into_pair),
        expected.nth(77).map(|(&k, &v)| (k, v))
    );
    assert_eq!(
        actual.nth_back(55).map(MapEntry::into_pair),
        expected.nth_back(55).map(|(&k, &v)| (k, v))
    );
    assert_eq!(actual.count(), expected.count());
    let mut exhausted = map.iter();
    assert!(exhausted.nth(usize::MAX).is_none());
    assert!(exhausted.next_back().is_none());
}

thread_local! { static VALUE_DECODES: Cell<usize> = const { Cell::new(0) }; }
struct Value(Vec<u8>);
impl Storable for Value {
    const BOUND: Bound = Bound::Unbounded;
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
        Ok(Cow::Borrowed(&self.0))
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
        VALUE_DECODES.set(VALUE_DECODES.get() + 1);
        Ok(Self(bytes.into_owned()))
    }
}

#[test]
fn key_projection_and_owned_entries_do_not_decode_nonclone_values() {
    let runtime = host(VectorMemory::default());
    let mut map =
        StableBTreeMap::<u64, Value, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    for key in 0..300 {
        map.insert(key, Value(vec![42; 1024]));
    }
    VALUE_DECODES.set(0);
    assert!(map.contains_key(&143));
    assert!(!map.contains_key(&1000));
    assert_eq!(map.keys().rev().take(2).collect::<Vec<_>>(), [299, 298]);
    assert_eq!(map.keys_range(40..60).count(), 20);
    assert_eq!(map.iter().count(), 300);
    assert_eq!(map.iter().nth(91).unwrap().into_key(), 91);
    assert_eq!(VALUE_DECODES.get(), 0);
    let entry = map.iter().next_back().unwrap();
    map.insert(299, Value(vec![9; 8]));
    VALUE_DECODES.set(0);
    assert_eq!(entry.into_value().0, vec![42; 1024]);
    assert_eq!(VALUE_DECODES.get(), 1);
    assert_eq!(map.get(&299).unwrap().0, vec![9; 8]);
    assert_eq!(VALUE_DECODES.get(), 2);
}

#[test]
fn retained_entries_own_small_and_large_values_after_clear_and_drop() {
    let runtime = host(VectorMemory::default());
    let mut map =
        StableBTreeMap::<u64, Value, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    let lengths = [0, 1, 8, 24, 25, 256];
    for (key, length) in lengths.into_iter().enumerate() {
        map.insert(key as u64, Value(vec![u8::try_from(key).unwrap(); length]));
    }
    VALUE_DECODES.set(0);
    let entries: Vec<_> = map.iter().collect();
    assert_eq!(VALUE_DECODES.get(), 0);
    map.clear();
    drop(map);
    drop(runtime);
    for (key, entry) in entries.into_iter().enumerate() {
        let expected = vec![u8::try_from(key).unwrap(); lengths[key]];
        assert_eq!(entry.value().0, expected);
        let (stored_key, value) = entry.into_pair();
        assert_eq!(stored_key, key as u64);
        assert_eq!(value.0, expected);
    }
    assert_eq!(VALUE_DECODES.get(), lengths.len() * 2);
}

#[test]
fn oversized_keys_values_and_churn_reuse_backing() {
    let backing = VectorMemory::default();
    let runtime = host(backing.clone());
    let mut map =
        StableBTreeMap::<Vec<u8>, Vec<u8>, _>::create(runtime.open_memory(&key()).unwrap())
            .unwrap();
    let key_bytes = |index: u8| vec![index; 70_001];
    for cycle in 0..4 {
        for index in 0..8_u8 {
            assert_eq!(map.insert(key_bytes(index), vec![index; 131_073]), None);
        }
        for index in 0..8_u8 {
            assert_eq!(map.get(&key_bytes(index)).unwrap(), vec![index; 131_073]);
        }
        let (returned, full_pages) = map.allocated_pages();
        map = returned;
        for index in 0..8_u8 {
            assert_eq!(map.remove(&key_bytes(index)), Some(vec![index; 131_073]));
        }
        assert!(map.is_empty());
        if cycle == 3 {
            let before = backing.borrow().len();
            for index in 0..8_u8 {
                map.insert(key_bytes(index), vec![index; 131_073]);
            }
            let (returned, reused_pages) = map.allocated_pages();
            map = returned;
            assert_eq!(reused_pages, full_pages, "steady churn must reuse space");
            map.clear();
            assert!(map.is_empty());
            assert_eq!(backing.borrow().len(), before);
        }
    }
    drop(map);
    drop(runtime);
    let runtime = host(backing);
    assert!(
        StableBTreeMap::<Vec<u8>, Vec<u8>, _>::load(runtime.open_memory(&key()).unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn set_and_interleaved_region_views_keep_current_order() {
    let runtime = host(VectorMemory::default());
    let mut set = StableBTreeSet::<i64, _>::create(
        runtime
            .open_memory(&StableKey::parse("ordered.set.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    for value in (-200..200).rev() {
        assert!(set.insert(value));
        assert!(!set.insert(value));
    }
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        (-200..200).collect::<Vec<_>>()
    );
    for value in (-200..200).step_by(2) {
        assert!(set.remove(&value));
        assert!(!set.remove(&value));
    }
    let memory = Rc::new(runtime.open_memory(&key()).unwrap());
    let mut first = BTreeMapRegion::<u64, u64, _>::new(memory.clone(), 0..32);
    first.initialize().unwrap();
    let mut alias = BTreeMapRegion::<u64, u64, _>::new(memory.clone(), 0..32);
    let mut neighbor = BTreeMapRegion::<u64, u64, _>::new(memory, 32..64);
    neighbor.initialize().unwrap();
    for key in 0..128 {
        first.insert(key, key + 1).unwrap();
        assert_eq!(alias.get(&key).unwrap(), Some(key + 1));
        alias.insert(key, key + 2).unwrap();
        neighbor.insert(key, key + 1000).unwrap();
        assert_eq!(first.get(&key).unwrap(), Some(key + 2));
    }
    let snapshot = alias.keys_range_limited(20.., 3).unwrap();
    first.remove(&20).unwrap();
    assert_eq!(snapshot.collect::<Vec<_>>(), [20, 21, 22]);
    assert_eq!(
        alias
            .keys_range_limited(20.., 3)
            .unwrap()
            .collect::<Vec<_>>(),
        [21, 22, 23]
    );
    assert_eq!(neighbor.get(&20).unwrap(), Some(1020));
}

#[test]
fn foreign_framing_refuses_without_repair_or_allocation_changes() {
    let backing = VectorMemory::default();
    let runtime = host(backing.clone());
    let memory = runtime.open_memory(&key()).unwrap();
    memory.grow(1).unwrap();
    memory.write(0, &[0x5a; 1024]);
    drop(memory);
    let before = backing.borrow().clone();
    assert!(StableBTreeMap::<u64, u64, _>::load(runtime.open_memory(&key()).unwrap()).is_err());
    assert_eq!(*backing.borrow(), before);
    let map = StableBTreeMap::<u64, u64, _>::reset(runtime.open_memory(&key()).unwrap()).unwrap();
    assert!(map.is_empty());
}
