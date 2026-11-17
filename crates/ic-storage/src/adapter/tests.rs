//! Integration of upstream engines with named allocation custody.
use super::*;
use crate::{
    AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryRequest, MemoryRuntime,
    SealedDeclarationSnapshot, StableKey, VectorMemory,
};
fn runtime(backing: VectorMemory) -> MemoryRuntime<VectorMemory> {
    let mut runtime = MemoryRuntime::new(backing).unwrap();
    let source = SealedDeclarationSnapshot::new(
        [
            "app.map.v1",
            "app.set.v1",
            "app.cell.v1",
            "app.vec.v1",
            "app.heap.v1",
            "app.logindex.v1",
            "app.logdata.v1",
        ]
        .into_iter()
        .map(|name| MemoryRequest::new("app", name).unwrap())
        .collect(),
    )
    .unwrap();
    runtime
        .bootstrap(
            &source,
            &AllocationGrants::new(vec![MemoryAuthority::new("app", "app.").unwrap()]).unwrap(),
            &GenericAllocationPolicy,
        )
        .unwrap();
    runtime
}
fn lease(runtime: &MemoryRuntime<VectorMemory>, name: &str) -> crate::RuntimeMemory<VectorMemory> {
    runtime
        .open_memory(&StableKey::parse(name).unwrap())
        .unwrap()
}
#[test]
fn upstream_collections_reopen_and_keep_independent_allocations() {
    let backing = VectorMemory::default();
    {
        let host = runtime(backing.clone());
        let mut map = StableBTreeMap::<u64, Vec<u8>>::create(lease(&host, "app.map.v1"));
        let mut set = StableBTreeSet::<u64>::create(lease(&host, "app.set.v1"));
        let mut cell = StableCell::<u64>::create(lease(&host, "app.cell.v1"), 7);
        let vector = StableVec::<u64>::create(lease(&host, "app.vec.v1"));
        for key in 0..300 {
            map.insert(key, vec![key as u8; 1024]);
            set.insert(key);
            vector.push(&key);
        }
        for key in (0..300).step_by(3) {
            map.remove(&key);
            set.remove(&key);
        }
        assert_eq!(cell.set(9), 7);
        assert!(
            host.open_memory(&StableKey::parse("app.map.v1").unwrap())
                .is_err()
        );
    }
    let host = runtime(backing);
    let mut map = StableBTreeMap::<u64, Vec<u8>>::load(lease(&host, "app.map.v1"));
    let set = StableBTreeSet::<u64>::load(lease(&host, "app.set.v1"));
    let cell = StableCell::<u64>::load(lease(&host, "app.cell.v1")).unwrap();
    let vector = StableVec::<u64>::load(lease(&host, "app.vec.v1"));
    assert_eq!(map.len(), 200);
    assert_eq!(set.len(), 200);
    assert_eq!(*cell.get(), 9);
    assert_eq!(vector.len(), 300);
    assert_eq!(vector.get(299), Some(299));
    for key in 0..300 {
        assert_eq!(map.get(&key), (key % 3 != 0).then(|| vec![key as u8; 1024]));
        assert_eq!(set.contains(&key), key % 3 != 0);
    }
    assert_eq!(
        map.range(10..15)
            .map(|entry| *entry.key())
            .collect::<Vec<_>>(),
        vec![10, 11, 13, 14]
    );
    map.insert(301, vec![8; 4096]);
    assert_eq!(map.remove(&301), Some(vec![8; 4096]));
}
#[test]
fn foreign_collection_load_and_creation_never_reset_bytes() {
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let memory = lease(&host, "app.map.v1");
    memory.grow(1).unwrap();
    memory.write(0, b"ICMAP001");
    drop(memory);
    let before = backing.borrow().clone();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            StableBTreeMap::<u64, u64>::load(lease(&host, "app.map.v1"))
        }))
        .is_err()
    );
    assert_eq!(*backing.borrow(), before);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            StableBTreeMap::<u64, u64>::create(lease(&host, "app.map.v1"))
        }))
        .is_err()
    );
    assert_eq!(*backing.borrow(), before);
    let cell_memory = lease(&host, "app.cell.v1");
    cell_memory.grow(1).unwrap();
    cell_memory.write(0, b"ICCELL01");
    let before = backing.borrow().clone();
    assert!(StableCell::<u64>::load(cell_memory).is_err());
    assert_eq!(*backing.borrow(), before);
}

#[test]
fn upstream_heap_log_and_regions_reopen_without_custom_engines() {
    use std::rc::Rc;
    let backing = VectorMemory::default();
    {
        let host = runtime(backing.clone());
        let mut heap = StableMinHeap::<u64>::create(lease(&host, "app.heap.v1"));
        for item in [9, 2, 7] {
            heap.push(&item);
        }
        assert_eq!(heap.pop(), Some(2));
        let log = StableLog::<Vec<u8>>::create(
            lease(&host, "app.logindex.v1"),
            lease(&host, "app.logdata.v1"),
        );
        log.append(&vec![1; 70000]).unwrap();
        log.append(&vec![2; 90000]).unwrap();
        let memory = Rc::new(lease(&host, "app.map.v1"));
        let mut a = BTreeMapRegion::<u64, u64>::new(memory.clone(), 0..4);
        let mut b = BTreeMapRegion::<u64, u64>::new(memory, 4..8);
        a.initialize();
        b.initialize();
        a.insert(3, 30);
        b.insert(3, 300);
        assert_eq!(a.get(&3), Some(30));
        assert_eq!(b.get(&3), Some(300));
    }
    let host = runtime(backing);
    let mut heap = StableMinHeap::<u64>::load(lease(&host, "app.heap.v1"));
    assert_eq!(heap.pop(), Some(7));
    assert_eq!(heap.pop(), Some(9));
    let log = StableLog::<Vec<u8>>::load(
        lease(&host, "app.logindex.v1"),
        lease(&host, "app.logdata.v1"),
    );
    assert_eq!(log.get(0), Some(vec![1; 70000]));
    assert_eq!(log.get(1), Some(vec![2; 90000]));
    let memory = Rc::new(lease(&host, "app.map.v1"));
    let a = BTreeMapRegion::<u64, u64>::new(memory.clone(), 0..4);
    let b = BTreeMapRegion::<u64, u64>::new(memory, 4..8);
    assert_eq!(a.get(&3), Some(30));
    assert_eq!(b.get(&3), Some(300));
}
