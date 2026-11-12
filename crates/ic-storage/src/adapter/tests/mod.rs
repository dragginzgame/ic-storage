//! Public collection adapters require committed Storage allocations.
use super::*;
use crate::{
    MemoryRuntime, RuntimeOpenError, StorageConfig, VectorMemory, testing::fixture_runtime,
};
use std::{cell::Cell, rc::Rc};

mod framing;
mod improvements;
mod iteration;
mod owned;
mod regions;

const KEYS: [&str; 4] = ["test.map.v1", "test.set.v1", "test.cell.v1", "test.vec.v1"];

#[derive(Clone)]
struct ControlledMemory {
    backing: VectorMemory,
    refuse_growth: Rc<Cell<bool>>,
    growth_calls: Rc<Cell<u64>>,
}
impl crate::MemoryBacking for ControlledMemory {
    fn backing_identity(&self) -> crate::BackingIdentity {
        self.backing.backing_identity()
    }
}

impl Memory for ControlledMemory {
    fn size(&self) -> u64 {
        self.backing.size()
    }

    fn grow(&self, pages: u64) -> i64 {
        self.growth_calls.set(self.growth_calls.get() + 1);
        if self.refuse_growth.get() {
            -1
        } else {
            self.backing.grow(pages)
        }
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.backing.read(offset, dst);
    }

    fn write(&self, offset: u64, src: &[u8]) {
        self.backing.write(offset, src);
    }
}

pub(super) fn runtime<M: crate::MemoryBacking>(backing: M) -> MemoryRuntime<M> {
    fixture_runtime(backing, "test", "test.", &KEYS, StorageConfig::default()).unwrap()
}

#[test]
fn collections_reopen_through_committed_runtime_allocations() {
    let keys = KEYS.map(|key| crate::StableKey::parse(key).unwrap());
    let backing = VectorMemory::default();
    let runtime = runtime(backing.clone());
    let mut map =
        StableBTreeMap::<u64, u64>::create(runtime.open_memory(&keys[0]).unwrap()).unwrap();
    map.insert(7, 11);
    let mut set = StableBTreeSet::<u64>::create(runtime.open_memory(&keys[1]).unwrap()).unwrap();
    set.insert(9);
    let mut cell = StableCell::create(runtime.open_memory(&keys[2]).unwrap(), 1u64).unwrap();
    cell.set(3);
    let vector = StableVec::create(runtime.open_memory(&keys[3]).unwrap()).unwrap();
    vector.push(&5u64);
    for key in &keys {
        assert!(
            matches!(
                runtime.open_memory(key),
                Err(RuntimeOpenError::AlreadyOpen { .. })
            ),
            "{key}"
        );
    }
    drop((map, set, cell, vector));
    for key in &keys {
        drop(runtime.open_memory(key).unwrap());
    }
    drop(runtime);

    let runtime = self::runtime(backing);
    let map = StableBTreeMap::<u64, u64>::load(runtime.open_memory(&keys[0]).unwrap()).unwrap();
    let mut iter: MapIter<'_, u64, u64> = map.iter();
    assert_eq!(iter.next().unwrap().into_pair(), (7, 11));
    let set = StableBTreeSet::<u64>::load(runtime.open_memory(&keys[1]).unwrap()).unwrap();
    let mut iter: SetIter<'_, u64> = set.iter();
    assert_eq!(iter.next(), Some(9));
    let cell =
        StableCell::<u64>::load(runtime.open_memory(&keys[2]).unwrap(), 16 * 1024 * 1024).unwrap();
    assert_eq!(*cell.get(), 3);
    let vector = StableVec::<u64>::load(runtime.open_memory(&keys[3]).unwrap()).unwrap();
    assert_eq!(vector.get(0), Some(5));
}

#[test]
fn reservation_clear_and_reset_retain_exclusive_leases() {
    let runtime = runtime(VectorMemory::default());
    let mut map = StableBTreeMap::<u64, u64>::create(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    map.insert(7, 11);
    let (map, result) = map.reserve_pages(8);
    result.unwrap();
    let before = runtime.memory_allocations().unwrap();
    let (mut map, result) = map.reserve_pages(u64::MAX);
    assert!(matches!(
        result,
        Err(crate::RuntimeGrowError::ArithmeticOverflow | crate::RuntimeGrowError::CapacityExceeded)
    ));
    assert_eq!(map.get(&7), Some(11));
    assert_eq!(runtime.memory_allocations().unwrap(), before);
    assert!(matches!(
        runtime.open_memory(&crate::StableKey::parse("test.map.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    map.clear();
    assert!(map.is_empty());
    drop(map);
    let memory = runtime
        .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
        .unwrap();
    assert!(memory.size() >= 8);
    let map = StableBTreeMap::<u64, u64>::load(memory).unwrap();
    assert!(map.is_empty());
    let vector = StableVec::create(
        runtime
            .open_memory(&crate::StableKey::parse("test.vec.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    vector.push(&5u64);
    drop(vector);
    let vector = StableVec::<u64>::reset(
        runtime
            .open_memory(&crate::StableKey::parse("test.vec.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(vector.is_empty());
    assert!(matches!(
        runtime.open_memory(&crate::StableKey::parse("test.vec.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    drop(vector);
    let vector = StableVec::<u64>::load(
        runtime
            .open_memory(&crate::StableKey::parse("test.vec.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(vector.is_empty());
}

#[test]
fn reservation_is_observable_effect_free_when_satisfied_and_retryable_after_refusal() {
    let memory = ControlledMemory {
        backing: VectorMemory::default(),
        refuse_growth: Rc::new(Cell::new(false)),
        growth_calls: Rc::new(Cell::new(0)),
    };
    let runtime = runtime(memory.clone());
    let mut map = StableBTreeMap::<u64, u64, _>::create(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    map.insert(7, 11);
    let (map, result) = map.reserve_pages(8);
    result.unwrap();
    let before = memory.backing.borrow().clone();
    let allocations = runtime.memory_allocations().unwrap();
    let growth_calls = memory.growth_calls.get();
    let (map, pages) = map.allocated_pages();
    assert_eq!(pages, 8);
    let (map, result) = map.reserve_pages(pages);
    result.unwrap();
    let (map, result) = map.reserve_pages(0);
    result.unwrap();
    assert_eq!(memory.growth_calls.get(), growth_calls);
    assert_eq!(*memory.backing.borrow(), before);

    memory.refuse_growth.set(true);
    let (map, result) = map.reserve_pages(129);
    assert!(matches!(
        result,
        Err(crate::RuntimeGrowError::BackingRefused { .. })
    ));
    assert_eq!(map.get(&7), Some(11));
    assert_eq!(*memory.backing.borrow(), before);
    assert_eq!(runtime.memory_allocations().unwrap(), allocations);
    let (map, pages) = map.allocated_pages();
    assert_eq!(pages, 8);
    assert!(matches!(
        runtime.open_memory(&crate::StableKey::parse("test.map.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));

    memory.refuse_growth.set(false);
    let (mut map, result) = map.reserve_pages(129);
    result.unwrap();
    map.insert(8, 12);
    drop(map);
    drop(runtime);
    let runtime = self::runtime(memory);
    let map = StableBTreeMap::<u64, u64, _>::load(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    let (map, pages) = map.allocated_pages();
    assert_eq!(pages, 129);
    assert_eq!(map.get(&7), Some(11));
    assert_eq!(map.get(&8), Some(12));
}

#[test]
fn map_sequences_match_model_across_splits_removals_and_reopens() {
    use std::collections::BTreeMap;
    let backing = VectorMemory::default();
    let mut model = BTreeMap::new();
    let mut state = 0x1C50_u64;
    for generation in 0..8 {
        let host = runtime(backing.clone());
        let memory = host
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap();
        let mut map = if generation == 0 {
            StableBTreeMap::<u64, u64>::create(memory).unwrap()
        } else {
            StableBTreeMap::<u64, u64>::load(memory).unwrap()
        };
        for step in 0..500 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let key = (state >> 32) % 400;
            if step % 3 == 0 && generation > 0 {
                assert_eq!(map.remove(&key), model.remove(&key));
            } else {
                assert_eq!(map.insert(key, state), model.insert(key, state));
            }
            assert_eq!(map.get(&key), model.get(&key).copied());
        }
        assert_eq!(map.len(), model.len() as u64);
        assert_eq!(
            map.iter().map(MapEntry::into_pair).collect::<Vec<_>>(),
            model
                .iter()
                .map(|(&key, &value)| (key, value))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            map.range(100..200)
                .map(MapEntry::into_pair)
                .collect::<Vec<_>>(),
            model
                .range(100..200)
                .map(|(&key, &value)| (key, value))
                .collect::<Vec<_>>()
        );
        drop(map);
        drop(host);
    }
}

#[test]
fn native_growth_trap_poisons_the_shared_allocation_owner() {
    let memory = ControlledMemory {
        backing: VectorMemory::default(),
        refuse_growth: Rc::new(Cell::new(false)),
        growth_calls: Rc::new(Cell::new(0)),
    };
    let runtime = runtime(memory.clone());
    let mut map = StableBTreeMap::<u64, Vec<u8>, _>::create(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    memory.refuse_growth.set(true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || map.insert(1, vec![7; 1_048_576])
        ))
        .is_err()
    );
    assert!(
        runtime
            .open_memory(&crate::StableKey::parse("test.set.v1").unwrap())
            .is_err()
    );
}
