//! Generation-local ownership over the existing pool contract. Startup destroys
//! these private headers only after the source ended every native owner.
pub(super) use super::ordered_owner::Owner;
use super::ordered_owner::ROOTS;
use super::ordered_runtime::{auxiliary, header, publish, publish_auxiliary};
use super::region::pool::{Pool, Root};
use super::{BASE, ELEMENTS, Element, PAYLOAD_BYTES, PAYLOAD_OFFSET, checked};
use alloc::vec::Vec;
use core::{mem::MaybeUninit, ptr::NonNull};
use ic_storage::ordered::{Image, Map};

static mut NODES: MaybeUninit<Pool<'static, Element>> = MaybeUninit::uninit();
static mut BYTES: MaybeUninit<Pool<'static, u8>> = MaybeUninit::uninit();
static mut MAP: MaybeUninit<Owner> = MaybeUninit::uninit();
static mut PHASE: u64 = 0; // 0 fresh, 1 closed, 2 open, 3 executing/unavailable
static mut ADMISSIONS: u64 = 0;
static mut CALLS: u64 = 0;

fn roots(words: &[u64], at: usize) -> Option<Root> {
    let root = Root {
        start: usize::try_from(words[at]).unwrap(),
        length: usize::try_from(words[at + 1]).unwrap(),
        capacity: usize::try_from(words[at + 2]).unwrap(),
    };
    if root.capacity == 0 {
        assert_eq!(&words[at..at + 3], &[0; 3]);
        None
    } else {
        Some(root)
    }
}

// SAFETY for this module: admitted startup clears the entire compiler interval
// before target Rust executes. Single-threaded exports never suspend; the closed
// IC import set cannot reenter. These fixed disjoint pools are live for the entire
// generation, not borrowed from a stack and lifetime-extended. PHASE excludes
// overlapping frames. Closing consumes the map, ends both allocator references,
// finishes and drops both pools, then publishes complete descriptors last.
#[expect(
    unsafe_code,
    reason = "generation-owned static initialization and exclusive foreign loan"
)]
pub(super) fn open() {
    unsafe {
        assert!(matches!(PHASE, 0 | 1));
        let mut words = checked();
        assert_eq!(words[13], 1, "source owners must be closed");
        PHASE = 3;
        let descriptors = descriptors(&words);
        let mut node_roots = Vec::new();
        let mut byte_roots = Vec::new();
        let slots: [(Option<usize>, Option<usize>); 4] = core::array::from_fn(|i| {
            let descriptor = &descriptors[i];
            let node = roots(descriptor, 0);
            let bytes = roots(descriptor, 6);
            if node.is_none() {
                assert_eq!(&descriptor[..6], &[0; 6]);
                assert!(bytes.is_none());
            }
            let node = node.map(|root| {
                let slot = node_roots.len();
                node_roots.push(root);
                slot
            });
            let bytes = bytes.map(|root| {
                let slot = byte_roots.len();
                byte_roots.push(root);
                slot
            });
            (node, bytes)
        });
        let nodes = Pool::claim(
            NonNull::new(core::ptr::with_exposed_provenance_mut(BASE)).unwrap(),
            ELEMENTS,
            &node_roots,
        )
        .unwrap();
        let bytes = Pool::claim(
            NonNull::new(core::ptr::with_exposed_provenance_mut(
                BASE + PAYLOAD_OFFSET,
            ))
            .unwrap(),
            PAYLOAD_BYTES,
            &byte_roots,
        )
        .unwrap();
        (&raw mut NODES).write(MaybeUninit::new(nodes));
        (&raw mut BYTES).write(MaybeUninit::new(bytes));
        let nodes = (&raw const NODES).as_ref().unwrap().assume_init_ref();
        let bytes = (&raw const BYTES).as_ref().unwrap().assume_init_ref();
        let maps = core::array::from_fn(|i| {
            let (node, payload) = slots[i];
            let descriptor = descriptors[i];
            if let Some(slot) = node {
                Map::admit(
                    Image {
                        nodes: nodes.restore(slot).unwrap(),
                        payload: payload.map_or_else(
                            || Vec::new_in(bytes),
                            |slot| bytes.restore(slot).unwrap(),
                        ),
                        root: descriptor[3],
                        free: descriptor[4],
                        len: usize::try_from(descriptor[5]).unwrap(),
                    },
                    ROOTS[i].entries,
                )
                .unwrap_or_else(|_| panic!("native admission"))
            } else {
                Map::new_in_allocators(ROOTS[i].entries, nodes, bytes)
            }
        });
        (&raw mut MAP).write(MaybeUninit::new(Owner::new(maps)));
        words[13] = 0; // Scalar roots cannot be reissued while live owners exist.
        publish(words, false);
        ADMISSIONS += 1;
        PHASE = 2;
    }
}

#[expect(
    unsafe_code,
    reason = "exclusive non-reentrant access to an already admitted owner"
)]
pub(super) fn with<R>(operation: impl FnOnce(&mut Owner) -> R) -> R {
    unsafe {
        assert!(PHASE == 2);
        assert_eq!(header()[13], 0);
        PHASE = 3;
        let result = operation((&raw mut MAP).as_mut().unwrap().assume_init_mut());
        CALLS += 1;
        PHASE = 2;
        result
    }
}

#[expect(
    unsafe_code,
    reason = "complete settlement before publishing either allocation"
)]
pub(super) fn close(fail: bool) {
    unsafe {
        assert!(PHASE == 2);
        PHASE = 3;
        let maps = (&raw const MAP).read().assume_init().release();
        let node_pool = (&raw const NODES).as_ref().unwrap().assume_init_ref();
        let byte_pool = (&raw const BYTES).as_ref().unwrap().assume_init_ref();
        let descriptors = maps.map(|map| {
            let Image {
                nodes,
                payload,
                root,
                free,
                len,
            } = map.release().unwrap();
            let nodes = node_pool
                .release(nodes)
                .unwrap_or_else(|_| panic!("node settlement"));
            let bytes = byte_pool
                .release(payload)
                .unwrap_or_else(|_| panic!("payload settlement"));
            let mut descriptor = [0; 9];
            descriptor[..6].copy_from_slice(&nodes.map_or([0; 6], |n| {
                [
                    n.start as u64,
                    n.length as u64,
                    n.capacity as u64,
                    root,
                    free,
                    len as u64,
                ]
            }));
            descriptor[6..].copy_from_slice(&bytes.map_or([0; 3], |b| {
                [b.start as u64, b.length as u64, b.capacity as u64]
            }));
            descriptor
        });
        node_pool.finish().unwrap();
        byte_pool.finish().unwrap();
        (&raw mut NODES).as_mut().unwrap().assume_init_drop();
        (&raw mut BYTES).as_mut().unwrap().assume_init_drop();
        let mut words = checked();
        assert_eq!(words[13], 0);
        words[2..8].copy_from_slice(&descriptors[0][..6]);
        words[10..13].copy_from_slice(&descriptors[0][6..]);
        publish_auxiliary(descriptors[1..].try_into().unwrap());
        words[13] = 1;
        publish(words, fail);
        PHASE = 1;
    }
}

#[expect(
    unsafe_code,
    reason = "read-only idle diagnostics without an active mutable frame"
)]
pub(super) fn statistics() -> [u64; 6] {
    unsafe {
        assert!(matches!(PHASE, 1 | 2));
        let (initialized, allocated) = if PHASE == 2 {
            let usage = (&raw const MAP)
                .as_ref()
                .unwrap()
                .assume_init_ref()
                .allocation_usage()
                .unwrap();
            (usage.initialized_bytes as u64, usage.allocated_bytes as u64)
        } else {
            descriptors(&header())
                .iter()
                .fold((0, 0), |(initialized, allocated), root| {
                    (
                        initialized + root[1] * size_of::<Element>() as u64 + root[7],
                        allocated + root[2] * size_of::<Element>() as u64 + root[8],
                    )
                })
        };
        [
            PHASE,
            ADMISSIONS,
            CALLS,
            initialized,
            allocated,
            super::ordered_runtime::metadata_bytes(),
        ]
    }
}

#[expect(
    unsafe_code,
    reason = "phase observation before explicit lifecycle transition"
)]
pub(super) fn is_open() -> bool {
    unsafe { PHASE == 2 }
}

fn descriptors(words: &[u64; 14]) -> [[u64; 9]; 4] {
    let mut roots = [[0; 9]; 4];
    roots[0][..6].copy_from_slice(&words[2..8]);
    roots[0][6..].copy_from_slice(&words[10..13]);
    roots[1..].copy_from_slice(&auxiliary());
    roots
}
