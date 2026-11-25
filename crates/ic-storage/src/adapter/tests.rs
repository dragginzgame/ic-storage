//! Integration of owned engines with named allocation custody.
use super::*;
use crate::{
    AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryRequest, MemoryRuntime,
    SealedDeclarationSnapshot, StableKey, VectorMemory,
};

fn runtime(backing: VectorMemory) -> MemoryRuntime<VectorMemory> {
    let mut runtime = MemoryRuntime::new(backing).unwrap();
    let source = SealedDeclarationSnapshot::new(
        ["app.map.v1", "app.cell.v1"]
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
fn owned_collections_reopen_and_keep_independent_allocations() {
    let backing = VectorMemory::default();
    {
        let host = runtime(backing.clone());
        let mut map =
            StableBTreeMap::<u64, Vec<u8>, VectorMemory>::create(lease(&host, "app.map.v1"));
        let mut cell = StableCell::<u64, VectorMemory>::create(lease(&host, "app.cell.v1"), 7);
        for key in 0..300 {
            map.insert(key, vec![key as u8; 1024]);
        }
        for key in (0..300).step_by(3) {
            map.remove(&key);
        }
        assert_eq!(cell.set(9), 7);
        assert!(
            host.open_memory(&StableKey::parse("app.map.v1").unwrap())
                .is_err()
        );
    }
    let host = runtime(backing);
    let mut map = StableBTreeMap::<u64, Vec<u8>, VectorMemory>::load(lease(&host, "app.map.v1"));
    let cell = StableCell::<u64, VectorMemory>::load(lease(&host, "app.cell.v1")).unwrap();
    assert_eq!(map.len(), 200);
    assert_eq!(*cell.get(), 9);
    for key in 0..300 {
        assert_eq!(map.get(&key), (key % 3 != 0).then(|| vec![key as u8; 1024]));
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

// These owned map constructors panic on foreign bytes. Inspecting the backing requires unwinding.
#[cfg(panic = "unwind")]
#[test]
fn foreign_map_load_and_creation_never_reset_bytes() {
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let memory = lease(&host, "app.map.v1");
    memory.grow(1).unwrap();
    memory.write(0, b"BTR\x02");
    drop(memory);
    let before = backing.borrow().clone();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            StableBTreeMap::<u64, u64, VectorMemory>::load(lease(&host, "app.map.v1"))
        }))
        .is_err()
    );
    assert_eq!(*backing.borrow(), before);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            StableBTreeMap::<u64, u64, VectorMemory>::create(lease(&host, "app.map.v1"))
        }))
        .is_err()
    );
    assert_eq!(*backing.borrow(), before);
}

#[test]
fn foreign_cell_load_preserves_bytes() {
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let cell_memory = lease(&host, "app.cell.v1");
    cell_memory.grow(1).unwrap();
    cell_memory.write(0, b"SCL\x01");
    let before = backing.borrow().clone();
    assert!(matches!(
        StableCell::<u64, VectorMemory>::load(cell_memory),
        Err(crate::StableCellPayloadError::NotStableCell)
    ));
    assert_eq!(*backing.borrow(), before);
}

#[test]
fn bulk_removal_preserves_exact_survivors_across_reopen() {
    use std::collections::{BTreeMap, BTreeSet};

    let backing = VectorMemory::default();
    let mut expected = BTreeMap::new();
    let host = runtime(backing.clone());
    let mut tree = StableBTreeMap::<_, _, VectorMemory>::create(lease(&host, "app.map.v1"));
    for key in 0_u64..512 {
        let value = vec![key as u8; if key % 13 == 0 { 8192 } else { 15 }];
        tree.insert(key, value.clone());
        expected.insert(key, value);
    }
    assert_eq!(tree.remove_many(BTreeSet::new()), 0);
    drop(tree);
    drop(host);

    for parity in [0, 1] {
        let host = runtime(backing.clone());
        let mut tree =
            StableBTreeMap::<u64, Vec<u8>, VectorMemory>::load(lease(&host, "app.map.v1"));
        assert!(
            tree.iter()
                .map(|e| (*e.key(), e.value()))
                .eq(expected.clone())
        );
        let keys: BTreeSet<u64> = (parity..514).step_by(2).collect();
        expected.retain(|key, _| !keys.contains(key));
        assert_eq!(tree.remove_many(keys.clone()), 256);
        assert_eq!(tree.remove_many(keys), 0);
    }

    let host = runtime(backing);
    let mut tree = StableBTreeMap::<u64, Vec<u8>, VectorMemory>::load(lease(&host, "app.map.v1"));
    assert!(tree.is_empty());
    tree.insert(42_u64, vec![7_u8; 8192]);
    assert_eq!(tree.get(&42), Some(vec![7; 8192]));
}

#[test]
fn tuple_routes_and_values_reopen() {
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let mut map =
        StableBTreeMap::<(u8, [u8; 30]), (u64, u8), _>::create(lease(&host, "app.map.v1"));
    map.insert((7, [8; 30]), (41, 255));
    drop(map);
    drop(host);
    let host = runtime(backing);
    let map = StableBTreeMap::<(u8, [u8; 30]), (u64, u8), _>::load(lease(&host, "app.map.v1"));
    assert_eq!(map.get(&(7, [8; 30])), Some((41, 255)));
}

#[test]
fn mixed_btree_updates_and_bulk_deletes_match_an_ordered_map() {
    use std::collections::{BTreeMap, BTreeSet};
    let backing = VectorMemory::default();
    let mut oracle = BTreeMap::new();
    let mut random = 37_u64;
    let mut number = || {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        (random >> 32) % 400
    };
    for epoch in 0..6 {
        let host = runtime(backing.clone());
        let memory = lease(&host, "app.map.v1");
        let mut map = if epoch == 0 {
            StableBTreeMap::create(memory)
        } else {
            StableBTreeMap::load(memory)
        };
        for i in 0..300 {
            let key = number();
            let value = vec![i as u8; if key % 11 == 0 { 4097 } else { 17 }];
            assert_eq!(map.insert(key, value.clone()), oracle.insert(key, value));
        }
        let keys: BTreeSet<_> = (0..160).map(|_| number()).collect();
        let before = oracle.len();
        oracle.retain(|key, _| !keys.contains(key));
        assert_eq!(map.remove_many(keys), (before - oracle.len()) as u64);
        assert!(
            map.iter()
                .rev()
                .map(|e| e.into_pair())
                .eq(oracle.clone().into_iter().rev())
        );
    }
}

#[test]
fn map_options_survive_reopen_clear_and_free_block_reuse() {
    type Map = StableBTreeMap<u64, Vec<u8>, VectorMemory>;
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let options = BTreeMapOptions {
        node_target_bytes: 512,
        allocation_unit_bytes: 257,
    };
    let mut chunks = Map::create_with_options(lease(&host, "app.map.v1"), options);
    let default = Map::create(lease(&host, "app.cell.v1"));
    assert_eq!(default.options(), BTreeMapOptions::default());
    for key in 0..24 {
        chunks.insert(key, vec![key as u8; 8192]);
    }
    let (mut chunks, pages) = chunks.allocated_pages();
    assert!(pages <= 4);
    // Leave external blocks and nodes on the free lists before reopening.
    for key in 0..18 {
        chunks.remove(&key);
    }
    drop((chunks, default, host));
    let host = runtime(backing);
    let mut chunks = Map::load(lease(&host, "app.map.v1"));
    assert_eq!(chunks.options(), options);
    for key in 18..24 {
        assert_eq!(chunks.get(&key).unwrap(), vec![key as u8; 8192]);
    }
    for key in 0..18 {
        chunks.insert(key, vec![7; 1024]);
    }
    let (mut chunks, reused_pages) = chunks.allocated_pages();
    assert_eq!(reused_pages, pages);
    for key in 0..18 {
        assert_eq!(chunks.get(&key).unwrap(), vec![7; 1024]);
    }
    chunks.clear();
    drop(chunks);
    let chunks = Map::load(lease(&host, "app.map.v1"));
    assert!(chunks.is_empty());
    assert_eq!(chunks.options(), options);
}

#[test]
fn invalid_map_options_leave_existing_storage_intact() {
    type Map = StableBTreeMap<u64, u64, VectorMemory>;
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let mut map = Map::create(lease(&host, "app.map.v1"));
    map.insert(1, 2);
    drop(map);
    let before = backing.borrow().clone();
    for options in [
        BTreeMapOptions {
            node_target_bytes: 0,
            ..Default::default()
        },
        BTreeMapOptions {
            allocation_unit_bytes: 255,
            ..Default::default()
        },
    ] {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Map::reset_with_options(lease(&host, "app.map.v1"), options)
            }))
            .is_err()
        );
        assert_eq!(*backing.borrow(), before);
    }
    let map = Map::load(lease(&host, "app.map.v1"));
    assert_eq!(map.get(&1), Some(2));
}

#[cfg(panic = "unwind")]
#[test]
fn previous_and_future_map_headers_are_refused_without_writes() {
    // An empty ICMAP003 map with bounded, fixed-width u64 keys and values.
    let mut payload = Vec::new();
    payload.extend_from_slice(&0_u64.to_le_bytes());
    payload.extend_from_slice(&record::HEADER_BYTES.to_le_bytes());
    payload.extend_from_slice(&[0; 9]); // absent root
    for _ in 0..2 {
        payload.push(2);
        payload.extend_from_slice(&8_u32.to_le_bytes());
    }
    payload.extend_from_slice(&[0; 32 * 9]); // empty free lists
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let memory = lease(&host, "app.map.v1");
    memory.grow(1).unwrap();
    drop(memory);
    let options = [0, 4, 1, 1];
    let configured = [options.as_slice(), &payload].concat();
    let images = [
        record::encode_payload(*b"ICMAP003", &payload).unwrap(),
        record::encode_payload(*b"ICMAP003", &[payload.as_slice(), &options].concat()).unwrap(),
        record::encode_payload(*b"ICMAP004", &configured).unwrap(),
        record::encode_payload(*b"ICPMAP002", &configured).unwrap(),
    ];
    for bytes in images {
        let memory = lease(&host, "app.map.v1");
        memory.write(0, &bytes);
        drop(memory);
        let before = backing.borrow().clone();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                StableBTreeMap::<u64, u64, VectorMemory>::load(lease(&host, "app.map.v1"))
            }))
            .is_err()
        );
        assert_eq!(*backing.borrow(), before);
    }
}

#[test]
fn new_map_writes_the_v1_marker() {
    let host = runtime(VectorMemory::default());
    drop(StableBTreeMap::<u64, u64, VectorMemory>::create(lease(
        &host,
        "app.map.v1",
    )));
    let mut marker = [0; 9];
    lease(&host, "app.map.v1").read(0, &mut marker);
    assert_eq!(&marker, b"ICPMAP001");
}

#[test]
fn canic_project_registry_row_survives_reopen() {
    type Row = (u32, [u8; 32], [u8; 30]);
    type Projects = StableBTreeMap<[u8; 32], Row, VectorMemory>;
    let host = runtime(VectorMemory::default());
    let row = (37, [42; 32], [19; 30]);
    let key = [11; 32];
    let mut projects = Projects::create(lease(&host, "app.map.v1"));
    assert_eq!(projects.insert(key, row), None);
    drop(projects);
    let projects = Projects::load(lease(&host, "app.map.v1"));
    assert_eq!(projects.get(&key), Some(row));
    assert_eq!(
        Row::BOUND,
        Bound::Bounded {
            max_size: 74,
            is_fixed_size: true
        }
    );
    let mixed = (String::from(""), vec![0; 256], Some(7u8));
    assert_eq!(
        <(String, Vec<u8>, Option<u8>)>::from_bytes(mixed.to_bytes()),
        mixed
    );
}

#[test]
fn bounded_keys_interleave_across_separators_and_respect_endpoints() {
    use std::ops::Bound::{Excluded, Included};
    let host = runtime(VectorMemory::default());
    let mut map = StableBTreeMap::<u64, Vec<u8>, _>::create(lease(&host, "app.map.v1"));
    for i in 0..100 {
        map.insert(i, vec![i as u8; 64]);
    }
    for (bounds, expected) in [
        ((Excluded(11), Included(75)), 12..76),
        ((Included(11), Excluded(75)), 11..75),
        ((Included(12), Included(12)), 12..13),
        ((Excluded(12), Included(12)), 12..12),
    ] {
        let mut iter = map.keys_range(bounds);
        let mut oracle = expected;
        while !oracle.is_empty() {
            assert_eq!(iter.next(), oracle.next());
            assert_eq!(iter.next_back(), oracle.next_back());
        }
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
    }
}

#[cfg(feature = "test-support")]
#[test]
fn shared_reads_match_point_lookups_including_branch_hits_and_adjacent_misses() {
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let mut map = StableBTreeMap::<u64, Vec<u8>, _>::create(lease(&host, "app.map.v1"));
    let compare = |map: &StableBTreeMap<u64, Vec<u8>, VectorMemory>, keys: &[u64]| {
        let before = backing.borrow().clone();
        let mut seen = Vec::new();
        map.visit_sorted_keys(keys, |key, value| seen.push((*key, value)));
        assert_eq!(
            seen,
            keys.iter()
                .map(|key| (*key, map.get(key)))
                .collect::<Vec<_>>()
        );
        assert_eq!(*backing.borrow(), before);
    };
    compare(&map, &[]);
    compare(&map, &[1, 2]);
    for i in 0..512 {
        map.insert(2 * i, vec![i as u8; if i % 2 == 0 { 7 } else { 1024 }]);
    }
    for keys in (0..1025).collect::<Vec<_>>().chunks(32) {
        compare(&map, keys);
    }
    compare(&map, &[0, 1022, u64::MAX]);
}

#[cfg(panic = "unwind")]
#[test]
fn key_range_checks_node_integrity_without_reading_external_records() {
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let mut map = StableBTreeMap::<u64, Vec<u8>, _>::create(lease(&host, "app.map.v1"));
    map.insert(7, vec![25; 1024]);
    let corrupt_digest = |magic: &[u8]| {
        let mut bytes = backing.borrow_mut();
        let offset = bytes
            .windows(magic.len())
            .position(|window| window == magic)
            .unwrap();
        bytes[offset + 12] ^= 1;
    };
    corrupt_digest(b"ICBTVAL1");
    assert_eq!(map.keys_range(7..=7).next(), Some(7));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| map.get(&7))).is_err());
    corrupt_digest(b"ICBTND01");
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| map
            .keys_range(7..=7)
            .next()))
        .is_err()
    );
}
