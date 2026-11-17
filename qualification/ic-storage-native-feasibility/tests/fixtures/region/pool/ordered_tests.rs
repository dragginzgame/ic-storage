//! Compact DB nodes and separately owned payloads over the existing allocator/custody owner.
use super::tests::Backing;
use ic_storage::ordered::{Error as OrderedError, Image, Map, Mutation, Node};

type DbNode = Node<512, 4096>;
type DbMap<A, P> = Map<512, 4096, A, P>;
impl super::super::sealed::Sealed for DbNode {}
impl super::super::Element for DbNode {}

#[test]
fn ordered_db_nodes_reopen_and_reconstruct_after_source_pool_destruction() {
    assert_eq!(size_of::<DbNode>(), 56);
    assert_eq!(align_of::<DbNode>(), 8);
    let mut backing = Backing::new(512, DbNode::EMPTY);
    let mut payload_backing = Backing::new(2 * 1024 * 1024, 0u8);
    let (root, payload_root, descriptor, checkpoint) = {
        let pool = backing.claim(&[]).unwrap();
        let payload_pool = payload_backing.claim(&[]).unwrap();
        let mut map = DbMap::new_in_allocators(256, &pool, &payload_pool);
        for id in 0..64u64 {
            let mut primary = vec![1];
            primary.extend_from_slice(&id.to_be_bytes());
            let mut index = vec![2, (id % 8) as u8];
            index.extend_from_slice(&id.to_be_bytes());
            let row = vec![id as u8; 4096];
            map.with(|mut view| {
                view.apply(&[
                    Mutation::Put(&primary, &row),
                    Mutation::Put(&index, &primary),
                ])
            })
            .unwrap()
            .unwrap();
        }
        // Keep holes through settlement to exercise the free list on readmission.
        map.with(|mut view| view.remove(&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0]))
            .unwrap();
        let checkpoint = map.checkpoint(4).unwrap();
        let Image {
            nodes,
            payload,
            root,
            free,
            len,
        } = map.release().unwrap();
        let descriptor = (root, free, len);
        let root = pool
            .release(nodes)
            .unwrap_or_else(|_| panic!("owned allocation"))
            .unwrap();
        let payload_root = payload_pool
            .release(payload)
            .unwrap_or_else(|_| panic!("payload settlement"))
            .unwrap();
        payload_pool.finish().unwrap();
        pool.finish().unwrap();
        (root, payload_root, descriptor, checkpoint)
    };
    {
        let pool = backing.claim(&[root]).unwrap();
        let payload_pool = payload_backing.claim(&[payload_root]).unwrap();
        let image = Image {
            nodes: pool.restore(0).unwrap(),
            payload: payload_pool.restore(0).unwrap(),
            root: descriptor.0,
            free: descriptor.1,
            len: descriptor.2,
        };
        let mut reopened =
            DbMap::admit(image, 256).unwrap_or_else(|_| panic!("valid native image"));
        assert_eq!(reopened.checkpoint(4).unwrap(), checkpoint);
        reopened
            .with(|mut view| {
                for (_, owner) in view.prefix(&[2]) {
                    assert_eq!(view.get(owner.as_slice()).unwrap().as_slice().len(), 4096);
                }
                view.insert(&[255; 512], &[7; 4096]).unwrap();
            })
            .unwrap();
        drop(reopened);
        assert_eq!(pool.reusable_elements(), 512);
        assert_eq!(payload_pool.reusable_elements(), 2 * 1024 * 1024);
        payload_pool.finish().unwrap();
        pool.finish().unwrap();
    }
    drop(backing);
    drop(payload_backing);
    let mut destination = Backing::new(512, DbNode::EMPTY);
    let pool = destination.claim(&[]).unwrap();
    let mut payload_destination = Backing::new(2 * 1024 * 1024, 0u8);
    let payload_pool = payload_destination.claim(&[]).unwrap();
    let mut recovered =
        DbMap::from_checkpoint_in_allocators(&checkpoint, 4, 256, &pool, &payload_pool).unwrap();
    assert_eq!(recovered.checkpoint(4).unwrap(), checkpoint);
    recovered
        .with(|mut view| {
            view.insert(b"new", b"continued").unwrap();
        })
        .unwrap();
    drop(recovered);
    assert_eq!(pool.reusable_elements(), 512);
    assert_eq!(payload_pool.reusable_elements(), 2 * 1024 * 1024);
    payload_pool.finish().unwrap();
    pool.finish().unwrap();
}

#[test]
fn ordered_batch_and_candidate_allocation_failures_preserve_existing_state() {
    // Seven slots hold the first four-node allocation, then refuse either
    // growth to eight or a separate four-node recovery candidate.
    let mut backing = Backing::new(7, DbNode::EMPTY);
    let pool = backing.claim(&[]).unwrap();
    let mut payload_backing = Backing::new(16 * 1024, 0u8);
    let payload_pool = payload_backing.claim(&[]).unwrap();
    let mut map = DbMap::new_in_allocators(32, &pool, &payload_pool);
    for key in 0..4u8 {
        map.with(|mut view| view.insert(&[key], &[key]))
            .unwrap()
            .unwrap();
    }
    let before = map.checkpoint(1).unwrap();
    let unused = pool.reusable_elements();
    assert_eq!(
        map.with(
            |mut view| view.apply(&[Mutation::Put(&[0], b"changed"), Mutation::Put(&[4], b"new")])
        )
        .unwrap(),
        Err(OrderedError::Allocation)
    );
    assert_eq!(map.checkpoint(1).unwrap(), before);
    assert_eq!(pool.reusable_elements(), unused);
    // Recovery must hold old and candidate allocations simultaneously.
    assert_eq!(map.restore(&before, 1), Err(OrderedError::Allocation));
    assert_eq!(map.checkpoint(1).unwrap(), before);
    assert_eq!(pool.reusable_elements(), unused);
    map.with(|mut view| view.apply(&[Mutation::Remove(&[0]), Mutation::Put(&[4], b"reused")]))
        .unwrap()
        .unwrap();
    assert_eq!(pool.reusable_elements(), unused);
    drop(map);
    assert_eq!(pool.reusable_elements(), 7);
    assert_eq!(payload_pool.reusable_elements(), 16 * 1024);
    payload_pool.finish().unwrap();
    pool.finish().unwrap();
}

#[test]
fn ordered_payload_refusal_and_candidate_cleanup_preserve_both_owners() {
    let mut nodes = Backing::new(64, DbNode::EMPTY);
    let mut bytes = Backing::new(2048, 0u8);
    let pool = nodes.claim(&[]).unwrap();
    let payload_pool = bytes.claim(&[]).unwrap();
    let mut map = DbMap::new_in_allocators(32, &pool, &payload_pool);
    map.with(|mut v| v.insert(b"a", b"one")).unwrap().unwrap();
    let before = map.checkpoint(1).unwrap();
    let unused = (pool.reusable_elements(), payload_pool.reusable_elements());
    for batch in [
        vec![
            Mutation::Put(b"a", b"changed"),
            Mutation::Put(b"b", &[0; 4096]),
        ],
        vec![Mutation::Remove(b"a"), Mutation::Put(b"b", &[0; 4096])],
        vec![Mutation::Put(b"a", &[0; 4096])],
    ] {
        assert_eq!(
            map.with(|mut v| v.apply(&batch)).unwrap(),
            Err(OrderedError::Allocation)
        );
        assert_eq!(map.checkpoint(1).unwrap(), before);
        assert_eq!(
            (pool.reusable_elements(), payload_pool.reusable_elements()),
            unused
        );
    }
    assert_eq!(map.restore(&before, 1), Err(OrderedError::Allocation));
    assert_eq!(map.checkpoint(1).unwrap(), before);
    assert_eq!(
        (pool.reusable_elements(), payload_pool.reusable_elements()),
        unused
    );
    // Same-class replacement and deletion/reuse need no new payload allocation.
    map.with(|mut v| {
        v.apply(&[
            Mutation::Put(b"a", b"two"),
            Mutation::Remove(b"a"),
            Mutation::Put(b"b", b"reused"),
        ])
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        (pool.reusable_elements(), payload_pool.reusable_elements()),
        unused
    );
    drop(map);
    assert_eq!(pool.reusable_elements(), 64);
    assert_eq!(payload_pool.reusable_elements(), 2048);
    pool.finish().unwrap();
    payload_pool.finish().unwrap();
}
