//! The product table uses the real loan/issuance controller, not Global alone.
use super::super::{Descriptor, Element, Region, sealed};
use ic_storage_native::map::{Error, ImportError, Map, Slot};
use std::{
    io::Read,
    mem::MaybeUninit,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
};

// Closed qualification: Slot is repr(C), three initialized u64s, Copy, no Drop
// or owning fields. Map::admit additionally validates its complete hash schema.
impl sealed::Sealed for Slot {}
impl Element for Slot {}

struct Backing([MaybeUninit<Slot>; 32]);

impl Backing {
    fn new() -> Self {
        Self([MaybeUninit::uninit(); 32])
    }

    /// The only callers preserve the initialized prefix returned by release.
    fn claim(&mut self, root: Descriptor) -> Region<'_, Slot> {
        // SAFETY: this owner exclusively lends the same stable allocation until
        // the returned controller ends. Source clients have ended; root is either
        // empty or the exact release result. Only Slot writers initialize it.
        #[expect(
            unsafe_code,
            reason = "exclusive host provider loan for the product native table"
        )]
        unsafe { Region::claim(NonNull::from(&mut self.0).cast(), 32, root) }.unwrap()
    }
}

#[test]
fn streamed_candidate_failure_reclaims_the_destination_loan() {
    let mut source = Map::new(32);
    source
        .with(|mut view| {
            for key in 0..16 {
                view.insert(key, key + 100).unwrap();
            }
        })
        .unwrap();
    let checkpoint = source.checkpoint(7).unwrap();
    let mut backing = Backing::new();
    let region = backing.claim(Descriptor {
        length: 0,
        capacity: 0,
    });
    drop(region.restore().unwrap());
    for stop in 0..3 {
        let mut input = checkpoint.as_slice();
        let mut calls = 0;
        let result = Map::from_reader_in(checkpoint.len(), 7, 32, &region, |buffer| {
            if calls == stop {
                return Err("read failure");
            }
            calls += 1;
            input.read_exact(buffer).unwrap();
            Ok(())
        });
        assert!(matches!(result, Err(ImportError::Source("read failure"))));
    }
    let mut bad = checkpoint.clone();
    *bad.last_mut().unwrap() ^= 1;
    let mut input = bad.as_slice();
    assert!(matches!(
        Map::from_reader_in(bad.len(), 7, 32, &region, |buffer| input.read_exact(buffer)),
        Err(ImportError::Map(Error::Integrity))
    ));
    let mut input = checkpoint.as_slice();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = Map::from_reader_in(checkpoint.len(), 7, 32, &region, |buffer| {
                assert_ne!(buffer.len(), 32, "source panic at final digest");
                input.read_exact(buffer)
            });
        }))
        .is_err()
    );
    // All failed private candidates ended: this one-block loan can issue again.
    let mut input = checkpoint.as_slice();
    let restored = Map::from_reader_in(checkpoint.len(), 7, 32, &region, |buffer| {
        input.read_exact(buffer)
    })
    .unwrap();
    assert_eq!(restored.checkpoint(7).unwrap(), checkpoint);
    assert_eq!(source.checkpoint(7).unwrap(), checkpoint);
    let (slots, count) = restored.release().unwrap();
    assert_eq!(count, 16);
    assert_eq!(
        region
            .release(slots)
            .unwrap_or_else(|_| panic!("release"))
            .capacity,
        32
    );
}

#[test]
fn map_reopens_after_fresh_issuance_and_growth_refusal_preserves_custody() {
    let mut backing = Backing::new();
    let (root, count, checkpoint) = {
        let region = backing.claim(Descriptor {
            length: 0,
            capacity: 0,
        });
        let slots = region.restore().unwrap();
        let mut map = Map::admit(slots, 0, 32).unwrap_or_else(|_| panic!("empty admission"));
        map.with(|mut view| {
            for key in 0..6 {
                assert_eq!(view.insert(key, key + 10), Ok(None));
            }
            // This allocator can issue only one block. A second allocation for
            // rehash refuses even though the physical loan has spare bytes.
            assert_eq!(view.insert(6, 16), Err(Error::Allocation));
            for key in 0..6 {
                assert_eq!(view.get(key), Some(key + 10));
            }
        })
        .unwrap();
        let saved = map.checkpoint(7).unwrap();
        assert_eq!(map.recover(&saved, 7), Err(Error::Allocation));
        assert_eq!(map.checkpoint(7).unwrap(), saved);
        let (slots, count) = map.release().unwrap();
        let root = region.release(slots).unwrap_or_else(|_| panic!("release"));
        (root, count, saved)
    };
    assert_eq!(
        root,
        Descriptor {
            length: 8,
            capacity: 8
        }
    );
    assert_eq!(count, 6);
    {
        let region = backing.claim(root);
        let slots = region.restore().unwrap();
        let mut map = Map::admit(slots, count, 32).unwrap_or_else(|_| panic!("readmission"));
        assert_eq!(map.checkpoint(7).unwrap(), checkpoint);
        map.with(|mut view| {
            assert_eq!(view.remove(2), Some(12));
            assert_eq!(view.insert(99, 100), Ok(None));
            assert_eq!(view.get(2), None);
            assert_eq!(view.get(99), Some(100));
        })
        .unwrap();
        let (slots, count) = map.release().unwrap();
        assert_eq!(count, 6);
        assert_eq!(
            region
                .release(slots)
                .unwrap_or_else(|_| panic!("settlement")),
            root
        );
    }
    // Portable recovery constructs a new table in a different backing, with no
    // source pointer or allocation header crossing into the target controller.
    let mut fresh = Backing::new();
    let region = fresh.claim(Descriptor {
        length: 0,
        capacity: 0,
    });
    drop(region.restore().unwrap());
    let mut restored = Map::from_checkpoint_in(&checkpoint, 7, 32, &region).unwrap();
    restored
        .with(|view| {
            for key in 0..6 {
                assert_eq!(view.get(key), Some(key + 10));
            }
        })
        .unwrap();
}

#[test]
fn native_rebuild_keeps_the_source_live_and_reopens_a_selected_candidate() {
    let mut source_backing = Backing::new();
    let mut target_backing = Backing::new();
    let (target_root, target_count, checkpoint) = {
        let source_region = source_backing.claim(Descriptor {
            length: 0,
            capacity: 0,
        });
        let target_region = target_backing.claim(Descriptor {
            length: 0,
            capacity: 0,
        });
        let mut source = Map::admit(source_region.restore().unwrap(), 0, 32)
            .unwrap_or_else(|_| panic!("source admission"));
        source
            .with(|mut view| {
                for key in 0..6 {
                    view.insert(key, key + 10).unwrap();
                }
            })
            .unwrap();
        let before = source.checkpoint(7).unwrap();
        drop(target_region.restore().unwrap());
        // 25 entries need 64 slots; this target can issue only 32. Refusal
        // preserves both the source and the target's ability to allocate later.
        assert!(matches!(
            source.rebuild_in(25, &target_region),
            Err(Error::Allocation)
        ));
        assert_eq!(source.checkpoint(7).unwrap(), before);
        let mut candidate = source.rebuild_in(7, &target_region).unwrap();
        candidate
            .with(|mut view| {
                for key in 0..6 {
                    assert_eq!(view.get(key), Some(key + 10));
                }
                view.insert(99, 100).unwrap();
            })
            .unwrap();
        assert_eq!(source.checkpoint(7).unwrap(), before);
        let checkpoint = candidate.checkpoint(8).unwrap();
        // Both real owning headers remain live until the runtime selects and
        // releases them. Neither an application view nor a raw descriptor owns it.
        let (old_slots, old_count) = source.release().unwrap();
        let (new_slots, new_count) = candidate.release().unwrap();
        assert_eq!((old_count, new_count), (6, 7));
        assert_ne!(old_slots.as_ptr(), new_slots.as_ptr());
        assert_eq!(
            source_region
                .release(old_slots)
                .unwrap_or_else(|_| panic!("source release")),
            Descriptor {
                length: 8,
                capacity: 8
            }
        );
        let root = target_region
            .release(new_slots)
            .unwrap_or_else(|_| panic!("candidate release"));
        (root, new_count, checkpoint)
    };
    assert_eq!(
        target_root,
        Descriptor {
            length: 16,
            capacity: 16
        }
    );
    let region = target_backing.claim(target_root);
    let mut reopened = Map::admit(region.restore().unwrap(), target_count, 32)
        .unwrap_or_else(|_| panic!("candidate readmission"));
    assert_eq!(reopened.checkpoint(8).unwrap(), checkpoint);
    reopened
        .with(|mut view| {
            assert_eq!(view.remove(99), Some(100));
            view.insert(100, 101).unwrap();
        })
        .unwrap();
    let mut portable = Map::from_checkpoint(&checkpoint, 8, 32).unwrap();
    portable
        .with(|view| {
            assert_eq!(view.get(99), Some(100));
            for key in 0..6 {
                assert_eq!(view.get(key), Some(key + 10));
            }
        })
        .unwrap();
}
