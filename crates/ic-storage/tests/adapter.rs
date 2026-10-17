//! Public collection adapters require committed IC Memory allocations.
use ic_memory::{
    GenericAllocationPolicy, MemoryAllocationPool, MemoryAuthority, MemoryRequest, MemoryRuntime,
    RuntimeOpenError, SealedDeclarationSnapshot, VectorMemory,
};
use ic_storage::{BTreeMap, Config, Lexicographic, adapter::*};

fn runtime(backing: VectorMemory) -> MemoryRuntime<VectorMemory> {
    let pool =
        MemoryAllocationPool::new(vec![MemoryAuthority::new("test", "test.").unwrap()], vec![])
            .unwrap();
    let declarations = SealedDeclarationSnapshot::new(
        [
            "tree",
            "map",
            "set",
            "cell",
            "vec",
            "heap",
            "log.index",
            "log.data",
        ]
        .map(|key| MemoryRequest::new("test", &format!("test.{key}.v1")).unwrap())
        .to_vec(),
    )
    .unwrap();
    let mut runtime = MemoryRuntime::new(backing).unwrap();
    runtime
        .bootstrap(&declarations, &pool, &GenericAllocationPolicy)
        .unwrap();
    runtime
}

#[test]
fn collections_reopen_through_committed_runtime_allocations() {
    let backing = VectorMemory::default();
    let runtime = runtime(backing.clone());
    assert!(matches!(
        runtime.open_memory("test.unknown.v1"),
        Err(RuntimeOpenError::StableKeyNotCommitted(_))
    ));
    let mut tree = BTreeMap::create(
        runtime.open_memory("test.tree.v1").unwrap(),
        Config::default(),
        Lexicographic,
    )
    .unwrap();
    tree.insert(b"key", b"row").unwrap();
    let mut map = StableBTreeMap::<u64, u64>::new(runtime.open_memory("test.map.v1").unwrap());
    map.insert(7, 11);
    let mut set = StableBTreeSet::<u64>::new(runtime.open_memory("test.set.v1").unwrap());
    set.insert(9);
    let mut cell = StableCell::new(runtime.open_memory("test.cell.v1").unwrap(), 1u64);
    cell.set(3);
    let vector = StableVec::new(runtime.open_memory("test.vec.v1").unwrap());
    vector.push(&5u64);
    let mut heap = StableMinHeap::new(runtime.open_memory("test.heap.v1").unwrap());
    heap.push(&8u64);
    heap.push(&2);
    let log = StableLog::new(
        runtime.open_memory("test.log.index.v1").unwrap(),
        runtime.open_memory("test.log.data.v1").unwrap(),
    );
    log.append(&13u64).unwrap();
    for key in [
        "test.tree.v1",
        "test.map.v1",
        "test.set.v1",
        "test.cell.v1",
        "test.vec.v1",
        "test.heap.v1",
        "test.log.index.v1",
        "test.log.data.v1",
    ] {
        assert!(
            matches!(
                runtime.open_memory(key),
                Err(RuntimeOpenError::AlreadyOpen { .. })
            ),
            "{key}"
        );
    }
    drop((tree, map, set, cell, vector, heap, log));
    for key in [
        "test.tree.v1",
        "test.map.v1",
        "test.set.v1",
        "test.cell.v1",
        "test.vec.v1",
        "test.heap.v1",
        "test.log.index.v1",
        "test.log.data.v1",
    ] {
        drop(runtime.open_memory(key).unwrap());
    }
    drop(runtime);

    let runtime = self::runtime(backing);
    let tree = BTreeMap::open(
        runtime.open_memory("test.tree.v1").unwrap(),
        Config::default(),
        Lexicographic,
    )
    .unwrap();
    assert_eq!(tree.get(b"key").unwrap(), Some(b"row".to_vec()));
    tree.validate().unwrap();
    let map = StableBTreeMap::<u64, u64>::load(runtime.open_memory("test.map.v1").unwrap());
    let mut iter: MapIter<'_, u64, u64> = map.iter();
    assert_eq!(iter.next().unwrap().into_pair(), (7, 11));
    let set = StableBTreeSet::<u64>::init(runtime.open_memory("test.set.v1").unwrap());
    let mut iter: SetIter<'_, u64> = set.iter();
    assert_eq!(iter.next(), Some(9));
    let cell = StableCell::init(runtime.open_memory("test.cell.v1").unwrap(), 99u64);
    assert_eq!(*cell.get(), 3);
    let vector = StableVec::<u64>::init(runtime.open_memory("test.vec.v1").unwrap());
    assert_eq!(vector.get(0), Some(5));
    let mut heap = StableMinHeap::<u64>::init(runtime.open_memory("test.heap.v1").unwrap());
    assert_eq!(heap.pop(), Some(2));
    assert_eq!(heap.pop(), Some(8));
    let log = StableLog::<u64>::init(
        runtime.open_memory("test.log.index.v1").unwrap(),
        runtime.open_memory("test.log.data.v1").unwrap(),
    );
    let mut iter: LogIter<'_, u64> = log.iter();
    assert_eq!(iter.next(), Some(13));
}

#[test]
fn upstream_and_custom_formats_are_not_interchangeable() {
    let runtime = runtime(VectorMemory::default());
    let mut map = StableBTreeMap::<u64, u64>::new(runtime.open_memory("test.map.v1").unwrap());
    map.insert(1, 2);
    drop(map);
    assert!(matches!(
        BTreeMap::open(
            runtime.open_memory("test.map.v1").unwrap(),
            Config::default(),
            Lexicographic
        ),
        Err(ic_storage::Error::Format)
    ));
}

#[test]
fn constructor_errors_release_allocation_leases() {
    let runtime = runtime(VectorMemory::default());
    assert!(matches!(
        BTreeMap::create(
            runtime.open_memory("test.tree.v1").unwrap(),
            Config {
                max_key_bytes: 0,
                ..Config::default()
            },
            Lexicographic,
        ),
        Err(ic_storage::Error::Configuration)
    ));
    let tree = BTreeMap::create(
        runtime.open_memory("test.tree.v1").unwrap(),
        Config::default(),
        Lexicographic,
    )
    .unwrap();
    assert!(matches!(
        runtime.open_memory("test.tree.v1"),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    drop(tree);
    assert!(matches!(
        BTreeMap::open(
            runtime.open_memory("test.tree.v1").unwrap(),
            Config {
                max_value_bytes: 1,
                ..Config::default()
            },
            Lexicographic
        ),
        Err(ic_storage::Error::Configuration)
    ));
    let tree = BTreeMap::open(
        runtime.open_memory("test.tree.v1").unwrap(),
        Config::default(),
        Lexicographic,
    )
    .unwrap();
    assert!(tree.is_empty());
}

#[test]
fn map_clear_persists_empty_state_and_reuses_its_allocation() {
    let backing = VectorMemory::default();
    {
        let runtime = runtime(backing.clone());
        let mut map = StableBTreeMap::<u64, u64>::init(runtime.open_memory("test.map.v1").unwrap());
        for key in 0..256 {
            map.insert(key, key + 1);
        }
        let capacity = backing.borrow().len();
        map.clear_new();
        assert_eq!(backing.borrow().len(), capacity);
        assert!(matches!(
            runtime.open_memory("test.map.v1"),
            Err(RuntimeOpenError::AlreadyOpen { .. })
        ));
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert!(map.iter().next().is_none());
        assert_eq!(map.get(&128), None);
        map.clear_new();
    }
    {
        let runtime = runtime(backing.clone());
        let mut map = StableBTreeMap::<u64, u64>::init(runtime.open_memory("test.map.v1").unwrap());
        assert!(map.is_empty());
        assert_eq!(map.insert(42, 99), None);
        assert_eq!(map.get(&42), Some(99));
    }
    let runtime = runtime(backing);
    let map = StableBTreeMap::<u64, u64>::init(runtime.open_memory("test.map.v1").unwrap());
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&42), Some(99));
    assert_eq!(map.get(&128), None);
}
