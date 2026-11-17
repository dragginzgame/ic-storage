extern crate std;

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    vec,
};

use super::{
    ARENA_BASE, CAPACITY, Error, Image, META_BASE, Node, ObjectReference, PAGE, PhantomData,
    inspect,
};

#[test]
fn fresh_matches_independent_golden_page_and_ignores_uncommitted_capacity() {
    let mut memory = vec![0; 2 * PAGE];
    memory[ARENA_BASE..].fill(0xcc); // Uncommitted capacity is not retained state.
    let image = Image::initialize_fresh(&mut memory, 129).unwrap();
    assert_eq!(
        &image.bytes()[META_BASE..],
        include_bytes!("../tests/fixtures/empty-metadata.bin")
    );
}

#[test]
fn fresh_and_open_refusals_preserve_all_bytes() {
    let mut memory = vec![0; 2 * PAGE];
    assert!(matches!(
        Image::open(&mut memory, 130),
        Err(Error::Admission(_))
    ));
    for at in [META_BASE, ARENA_BASE, memory.len() - 1] {
        memory[at] = 1;
        let before = memory.clone();
        assert!(matches!(
            Image::initialize_fresh(&mut memory, 130),
            Err(Error::NotFresh)
        ));
        assert_eq!(memory, before);
        memory[at] = 0;
    }
    for pages in [0, 128, 131, 257, u32::MAX] {
        assert!(matches!(
            Image::initialize_fresh(&mut memory, pages),
            Err(Error::Capacity)
        ));
        assert!(memory.iter().all(|&b| b == 0));
    }
    Image::initialize_fresh(&mut memory, 130).unwrap();
    let before = memory.clone();
    assert!(matches!(
        Image::initialize_fresh(&mut memory, 130),
        Err(Error::NotFresh)
    ));
    assert!(matches!(
        Image::open(&mut memory, 129),
        Err(Error::Admission(_))
    ));
    assert_eq!(memory, before);
}

#[test]
fn reopen_preserves_sharing_cycles_and_allocator_continuation() {
    let mut memory = vec![0; 3 * PAGE];
    {
        let mut image = Image::initialize_fresh(&mut memory, 129).unwrap();
        image
            .with_update(|s| -> Result<(), Error> {
                assert_eq!(s.grow(1)?, 129);
                let node = s.allocate_node(7, None)?;
                assert_eq!(
                    node.reference(),
                    ObjectReference {
                        offset: 0,
                        identity: 1
                    }
                );
                s.set_node_next(node, Some(node))?;
                let counter = s.allocate_counter(41, Some(node), Some(node))?;
                s.publish_root(counter)?;
                assert_eq!(s.summary()?.used_bytes, 160);
                Ok(())
            })
            .unwrap()
            .unwrap();
    }
    let mut image = Image::open(&mut memory, 130).unwrap();
    let metadata = image.bytes()[META_BASE..ARENA_BASE].to_vec();
    image
        .with_update(|s| -> Result<(), Error> {
            let counter = s.root()?.unwrap();
            assert_eq!(
                counter.reference(),
                ObjectReference {
                    offset: 72,
                    identity: 2
                }
            );
            let left = s.left(counter)?.unwrap();
            let right = s.right(counter)?.unwrap();
            assert_eq!(left, right);
            assert_eq!(s.node_next(left)?, Some(left));
            assert_eq!(s.counter_value(counter)?, 41);
            s.set_counter_value(counter, 42)?;
            s.set_node_value(left, 99)?;
            assert_eq!(s.node_value(right)?, 99);
            s.set_node_next(left, None)?;
            assert_eq!(s.node_next(right)?, None);
            s.set_node_next(left, Some(left))?;
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(&image.bytes()[META_BASE..ARENA_BASE], metadata);
    image
        .with_update(|s| -> Result<(), Error> {
            let root = s.root()?.unwrap();
            let node = s.allocate_node(8, s.left(root)?)?;
            assert_eq!(
                node.reference(),
                ObjectReference {
                    offset: 160,
                    identity: 3
                }
            );
            assert_eq!(s.grow(1)?, 130);
            assert_eq!(s.summary()?.used_bytes, 232);
            assert_eq!(s.summary()?.next_identity, 4);
            assert_eq!(s.counter_value(root)?, 42);
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert!(image.bytes()[2 * PAGE..].iter().all(|&b| b == 0));
    assert_ne!(
        &image.bytes()[META_BASE + 160..META_BASE + 192],
        &metadata[160..192]
    );
    assert!(!image.is_poisoned());
}

#[test]
fn resource_refusals_do_not_consume_bytes_or_identities() {
    let mut memory = vec![0; 2 * PAGE];
    let mut image = Image::initialize_fresh(&mut memory, 129).unwrap();
    let before = image.bytes().to_vec();
    image
        .with_update(|s| {
            assert_eq!(s.allocate_node(0, None), Err(Error::ArenaFull));
            assert_eq!(s.grow(u32::MAX), Err(Error::Capacity));
            assert_eq!(s.grow(2), Err(Error::Capacity));
            assert_eq!(s.grow(0), Ok(129));
        })
        .unwrap();
    assert_eq!(image.bytes(), before);
    image
        .with_update(|s| {
            s.grow(1).unwrap();
            let node = s.allocate_node(5, None).unwrap();
            assert_eq!(node.reference().identity, 1);
            assert_eq!(s.grow(1), Err(Error::Capacity));
            assert_eq!(s.node_value(node), Ok(5));
        })
        .unwrap();
}

#[test]
fn allocation_after_reopen_crosses_a_page_only_after_explicit_growth() {
    let mut memory = vec![0; 3 * PAGE];
    memory[2 * PAGE..].fill(0xcc);
    {
        let mut image = Image::initialize_fresh(&mut memory, 130).unwrap();
        image
            .with_update(|s| {
                for value in 0..910 {
                    s.allocate_node(value, None).unwrap();
                }
                assert_eq!(s.summary().unwrap().used_bytes, 65520);
            })
            .unwrap();
    }
    let mut image = Image::open(&mut memory, 130).unwrap();
    let before = image.bytes().to_vec();
    image
        .with_update(|s| {
            assert_eq!(s.allocate_node(910, None), Err(Error::ArenaFull));
        })
        .unwrap();
    assert_eq!(image.bytes(), before);
    image
        .with_update(|s| {
            assert_eq!(s.grow(1), Ok(130));
            let node = s.allocate_node(910, None).unwrap();
            assert_eq!(
                node.reference(),
                ObjectReference {
                    offset: 65520,
                    identity: 911
                }
            );
            assert_eq!(s.node_value(node), Ok(910));
            assert_eq!(s.summary().unwrap().used_bytes, 65592);
        })
        .unwrap();
    assert_eq!(
        &image.bytes()[ARENA_BASE..ARENA_BASE + 65520],
        &before[ARENA_BASE..ARENA_BASE + 65520]
    );
    assert!(image.bytes()[ARENA_BASE + 65592..].iter().all(|&b| b == 0));
}

#[test]
fn index_exhaustion_is_an_unchanged_refusal() {
    let mut memory = vec![0; 3 * PAGE];
    let mut image = Image::initialize_fresh(&mut memory, 131).unwrap();
    image
        .with_update(|s| {
            for value in 0..CAPACITY as u64 {
                s.allocate_node(value, None).unwrap();
            }
        })
        .unwrap();
    let before = image.bytes().to_vec();
    image
        .with_update(|s| {
            assert_eq!(s.allocate_node(0, None), Err(Error::IndexFull));
            assert_eq!(s.allocate_counter(0, None, None), Err(Error::IndexFull));
            assert_eq!(s.summary().unwrap().next_identity, 1025);
        })
        .unwrap();
    assert_eq!(image.bytes(), before);
}

#[test]
fn ordinary_callback_error_keeps_successful_writes_and_access() {
    let mut memory = vec![0; 2 * PAGE];
    let mut image = Image::initialize_fresh(&mut memory, 130).unwrap();
    assert_eq!(
        image.with_update(|s| {
            let root = s.allocate_counter(41, None, None).unwrap();
            s.publish_root(root).unwrap();
            s.set_counter_value(root, 42).unwrap();
            Err::<(), _>("business refusal")
        }),
        Ok(Err("business refusal"))
    );
    assert!(!image.is_poisoned());
    image
        .with_update(|s| {
            assert_eq!(s.counter_value(s.root().unwrap().unwrap()), Ok(42));
            assert_eq!(s.summary().unwrap().next_identity, 2);
        })
        .unwrap();
}

#[test]
fn private_forged_handles_are_refused_before_writes() {
    let mut memory = vec![0; 2 * PAGE];
    let mut image = Image::initialize_fresh(&mut memory, 130).unwrap();
    image
        .with_update(|s| {
            let node = s.allocate_node(7, None).unwrap();
            let counter = s.allocate_counter(0, None, None).unwrap();
            let before = s.backing.to_vec();
            for reference in [
                ObjectReference {
                    offset: 0,
                    identity: 0,
                },
                ObjectReference {
                    offset: 1,
                    identity: 1,
                },
                ObjectReference {
                    offset: 0,
                    identity: u64::MAX,
                },
                counter.reference(),
            ] {
                // Only this child test module can forge the private brand/identity.
                let forged = Node {
                    reference,
                    brand: PhantomData,
                };
                assert_eq!(s.set_node_value(forged, 9), Err(Error::Handle));
                assert_eq!(s.set_node_next(node, Some(forged)), Err(Error::Handle));
                assert_eq!(
                    s.allocate_counter(0, Some(forged), None),
                    Err(Error::Handle)
                );
            }
            assert_eq!(s.backing, before);
        })
        .unwrap();
}

#[test]
fn callback_panic_poisons_without_claiming_rollback() {
    let mut memory = vec![0; 2 * PAGE];
    let mut image = Image::initialize_fresh(&mut memory, 130).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = image.with_update(|s| {
                s.allocate_node(7, None).unwrap();
                panic!("application fault after a completed allocation");
            });
        }))
        .is_err()
    );
    assert!(image.is_poisoned());
    assert_eq!(inspect(image.bytes()).unwrap().object_count, 1);
    assert_eq!(image.with_update(|_| ()), Err(Error::Poisoned));
}

#[test]
fn swallowed_post_write_failure_cannot_reenable_the_session() {
    for unwind in [false, true] {
        let mut memory = vec![0; 2 * PAGE];
        let mut image = Image::initialize_fresh(&mut memory, 130).unwrap();
        assert_eq!(
            image.with_update(|s| {
                let node = s.allocate_node(7, None).unwrap();
                if unwind {
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| {
                            let _ = s.commit(*s.length, false, |bytes| {
                                bytes[ARENA_BASE + 32] = 1;
                                panic!("interrupted write");
                            });
                        }))
                        .is_err()
                    );
                } else {
                    assert!(matches!(
                        s.commit(*s.length, false, |bytes| {
                            bytes[ARENA_BASE + 32] = 1;
                        }),
                        Err(Error::Admission(_))
                    ));
                }
                assert_eq!(s.summary(), Err(Error::Poisoned));
                assert_eq!(s.root(), Err(Error::Poisoned));
                assert_eq!(s.node_value(node), Err(Error::Poisoned));
                assert_eq!(s.node_next(node), Err(Error::Poisoned));
                assert_eq!(s.allocate_node(8, None), Err(Error::Poisoned));
            }),
            Err(Error::Poisoned)
        );
        assert!(image.is_poisoned());
        assert!(inspect(image.bytes()).is_err());
        assert_eq!(image.with_update(|_| ()), Err(Error::Poisoned));
    }
}

#[test]
fn session_accounting_tracks_growth_allocations_and_replaced_roots() {
    let mut memory = vec![0; 3 * PAGE];
    let mut image = Image::initialize_fresh(&mut memory, 129).unwrap();
    let replacement = image
        .with_update(|s| {
            assert_eq!(s.summary().unwrap().committed_bytes, 0);
            assert_eq!(s.root().unwrap(), None);
            s.grow(1).unwrap();
            let node = s.allocate_node(7, None).unwrap();
            let first = s.allocate_counter(11, Some(node), Some(node)).unwrap();
            s.publish_root(first).unwrap();
            assert_eq!(s.root().unwrap(), Some(first));
            let second = s.allocate_counter(22, Some(node), None).unwrap();
            s.publish_root(second).unwrap();
            assert_eq!(s.root().unwrap(), Some(second));
            s.set_node_next(node, Some(node)).unwrap();
            s.set_node_value(node, 9).unwrap();
            assert_eq!(s.node_value(s.left(first).unwrap().unwrap()), Ok(9));
            assert_eq!(s.node_next(node), Ok(Some(node)));
            s.grow(1).unwrap();
            let expected = super::Summary {
                object_count: 3,
                committed_bytes: (2 * PAGE) as u64,
                used_bytes: 248,
                next_identity: 4,
                root: Some(second.reference()),
            };
            assert_eq!(s.summary(), Ok(expected));
            assert_eq!(s.grow(1), Err(Error::Capacity));
            assert_eq!(s.summary(), Ok(expected));
            second.reference()
        })
        .unwrap();
    image
        .with_update(|s| {
            assert_eq!(s.root().unwrap().unwrap().reference(), replacement);
            assert_eq!(s.summary().unwrap().used_bytes, 248);
        })
        .unwrap();
    assert_eq!(inspect(image.bytes()).unwrap().root, Some(replacement));
}

#[test]
fn admission_is_not_reused_across_sessions_and_exit_still_checks_bytes() {
    for at_exit in [false, true] {
        let mut memory = vec![0; PAGE];
        let mut image = Image::initialize_fresh(&mut memory, 129).unwrap();
        if at_exit {
            // Private fault injection bypasses the public writer to prove that
            // final admission remains independent of the session's summary.
            assert!(matches!(
                image.with_update(|s| s.backing[16] ^= 1),
                Err(Error::Admission(_))
            ));
            assert!(image.is_poisoned());
        } else {
            image
                .with_update(|s| assert_eq!(s.summary().unwrap().object_count, 0))
                .unwrap();
            // No public caller can mutate this buffer while Image owns it.
            image.backing[16] ^= 1;
            assert!(matches!(
                image.with_update(|_| panic!("must refuse before callback")),
                Err(Error::Admission(_))
            ));
        }
        assert!(inspect(image.bytes()).is_err());
    }
}
