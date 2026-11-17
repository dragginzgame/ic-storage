//! Actual node and byte loans outlive calls; both end before relocation.
use super::{Error, tests::Backing};
use ic_storage::ordered::{Error as OrderedError, Image, Map, Mutation, Node};
use std::panic::{AssertUnwindSafe, catch_unwind};

type DbNode = Node<512, 4096>;

#[test]
fn ordered_session_reuses_both_owners_then_readmits_after_both_loans_end() {
    let mut nodes = Backing::new(2048, DbNode::EMPTY);
    let mut bytes = Backing::new(1024 * 1024, 0u8);
    let (node_root, byte_root, descriptors, checkpoint, usage) = {
        let node_pool = nodes.claim(&[]).unwrap();
        let byte_pool = bytes.claim(&[]).unwrap();
        let mut map = Map::<512, 4096, _, _>::new_in_allocators(512, &node_pool, &byte_pool);
        for id in 0..256u64 {
            let mut row = [id as u8; 512];
            row[..8].copy_from_slice(&id.to_le_bytes());
            let mut key = [1; 9];
            key[1..].copy_from_slice(&id.to_be_bytes());
            let mut index = [2; 10];
            index[1] = (id % 8) as u8;
            index[2..].copy_from_slice(&id.to_be_bytes());
            map.with(|mut view| {
                view.apply(&[Mutation::Put(&key, &row), Mutation::Put(&index, &key)])
            })
            .unwrap()
            .unwrap();
        }
        let usage = map.allocation_usage().unwrap();
        for id in 0..256u64 {
            assert_eq!(node_pool.finish(), Err(Error::State));
            assert_eq!(byte_pool.finish(), Err(Error::State));
            assert!(matches!(node_pool.restore(0), Err(Error::State)));
            assert!(matches!(byte_pool.restore(0), Err(Error::State)));
            let mut key = [1; 9];
            key[1..].copy_from_slice(&id.to_be_bytes());
            map.with(|mut view| {
                let before = view.remove(&key).unwrap();
                assert_eq!(&before.as_slice()[..8], &id.to_le_bytes());
                view.insert(&key, before.as_slice()).unwrap();
            })
            .unwrap();
        }
        assert_eq!(map.allocation_usage().unwrap(), usage);
        let checkpoint = map.checkpoint(5).unwrap();
        let Image {
            nodes,
            payload,
            root,
            free,
            len,
        } = map.release().unwrap();
        let node_root = node_pool
            .release(nodes)
            .unwrap_or_else(|_| panic!("node settlement"))
            .unwrap();
        // Finishing one pool cannot authorize reuse of the other live owner.
        node_pool.finish().unwrap();
        assert_eq!(byte_pool.finish(), Err(Error::State));
        let byte_root = byte_pool
            .release(payload)
            .unwrap_or_else(|_| panic!("byte settlement"))
            .unwrap();
        byte_pool.finish().unwrap();
        (node_root, byte_root, (root, free, len), checkpoint, usage)
    };
    let mut moved_nodes = Backing::new(2048, DbNode::EMPTY);
    let mut moved_bytes = Backing::new(1024 * 1024, 0u8);
    assert_ne!(nodes.0.as_ptr(), moved_nodes.0.as_ptr());
    assert_ne!(bytes.0.as_ptr(), moved_bytes.0.as_ptr());
    moved_nodes.0.copy_from_slice(&nodes.0);
    moved_bytes.0.copy_from_slice(&bytes.0);
    drop((nodes, bytes));
    let node_pool = moved_nodes.claim(&[node_root]).unwrap();
    let byte_pool = moved_bytes.claim(&[byte_root]).unwrap();
    let mut map = Map::admit(
        Image {
            nodes: node_pool.restore(0).unwrap(),
            payload: byte_pool.restore(0).unwrap(),
            root: descriptors.0,
            free: descriptors.1,
            len: descriptors.2,
        },
        512,
    )
    .unwrap_or_else(|_| panic!("fresh admission"));
    assert_eq!(map.checkpoint(5).unwrap(), checkpoint);
    assert_eq!(map.allocation_usage().unwrap(), usage);
    map.with(|view| {
        for (_, owner) in view.prefix(&[2]) {
            assert_eq!(view.get(owner.as_slice()).unwrap().as_slice().len(), 512);
        }
    })
    .unwrap();
    drop(map);
    assert_eq!(node_pool.reusable_elements(), 2048);
    assert_eq!(byte_pool.reusable_elements(), 1024 * 1024);
    node_pool.finish().unwrap();
    byte_pool.finish().unwrap();
}

#[test]
fn ordered_session_unwind_keeps_both_allocations_unavailable_until_discarded() {
    let mut nodes = Backing::new(32, DbNode::EMPTY);
    let mut bytes = Backing::new(8192, 0u8);
    let node_pool = nodes.claim(&[]).unwrap();
    let byte_pool = bytes.claim(&[]).unwrap();
    let mut map = Map::<512, 4096, _, _>::new_in_allocators(8, &node_pool, &byte_pool);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = map.with(|mut view| {
                view.insert(b"row", b"new value").unwrap();
                panic!("after payload write");
            });
        }))
        .is_err()
    );
    assert_eq!(map.with(|_| ()), Err(OrderedError::Poisoned));
    assert_eq!(map.allocation_usage(), Err(OrderedError::Poisoned));
    assert_eq!(node_pool.finish(), Err(Error::State));
    assert_eq!(byte_pool.finish(), Err(Error::State));
    assert!(node_pool.reusable_elements() < 32);
    assert!(byte_pool.reusable_elements() < 8192);
    // Explicit discard of this failed host generation, not publication or rollback.
    drop(map);
    assert_eq!(node_pool.reusable_elements(), 32);
    assert_eq!(byte_pool.reusable_elements(), 8192);
    node_pool.finish().unwrap();
    byte_pool.finish().unwrap();
}
