//! Host-only policy adapter for the actual provider controller and scoped core.
//! Unwinding is not rollback: poison is stored outside the borrowed scope and
//! only explicit, validated checkpoint replacement can make the host available.
use super::super::{Descriptor, Error as RegionError, Region};
use ic_storage_native::{Refusal, Vector, with_vector};
use ic_storage_native_feasibility::checkpoint;
use std::{
    mem,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
};

const WORDS: usize = 16;

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Poisoned,
    Region(RegionError),
    Scope(Refusal),
    Checkpoint(checkpoint::Error),
    Capacity,
    Injected,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportFault {
    None,
    AfterValue(usize),
    BeforePublish,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameFault {
    None,
    AfterRelease,
    AfterPublish,
}

struct ProviderHost {
    backing: [u64; WORDS],
    root: Descriptor,
    counter: u64,
    poisoned: bool,
}

impl ProviderHost {
    fn new() -> Self {
        Self {
            backing: [0; WORDS],
            root: Descriptor {
                length: 0,
                capacity: 0,
            },
            counter: 41,
            poisoned: false,
        }
    }

    fn update<R>(
        &mut self,
        operation: impl for<'scope, 'region, 'backing> FnOnce(
            Vector<'scope, &'region Region<'backing>>,
        ) -> R,
    ) -> Result<R, Failure> {
        self.update_with(operation, FrameFault::None)
    }

    fn update_with<R>(
        &mut self,
        operation: impl for<'scope, 'region, 'backing> FnOnce(
            Vector<'scope, &'region Region<'backing>>,
        ) -> R,
        fault: FrameFault,
    ) -> Result<R, Failure> {
        if self.poisoned {
            return Err(Failure::Poisoned);
        }
        // Set before admission, allocation or application code. No guard Drop
        // clears it; an unwind or incomplete finalization leaves it sticky.
        self.poisoned = true;
        let (result, root) = {
            // SAFETY: &mut self exclusively owns this initialized, aligned array.
            // The controller and every client end before this block ends. Host
            // moves occur only outside a loan; no pointer or std header is saved.
            #[expect(
                unsafe_code,
                reason = "host owner lends real backing to the installed controller"
            )]
            let region =
                unsafe { Region::claim(NonNull::from(&mut self.backing).cast(), WORDS, self.root) }
                    .map_err(Failure::Region)?;
            let mut values = region.restore().map_err(Failure::Region)?;
            let result = with_vector(&mut values, WORDS, operation).map_err(Failure::Scope)?;
            let root = region
                .release(values)
                .map_err(|(error, _values)| Failure::Region(error))?;
            (result, root)
        };
        if fault == FrameFault::AfterRelease {
            return Err(Failure::Injected);
        }
        self.root = root;
        if fault == FrameFault::AfterPublish {
            return Err(Failure::Injected);
        }
        self.poisoned = false;
        // A business Err is part of R, so it still publishes completed mutations.
        Ok(result)
    }

    fn export(&mut self, epoch: u64) -> Result<Vec<u8>, Failure> {
        let counter = self.counter;
        self.update(|view| checkpoint::encode_v2(counter, view.values(), epoch))?
            .map_err(Failure::Checkpoint)
    }

    fn recover(&mut self, bytes: &[u8], epoch: u64) -> Result<(), Failure> {
        self.recover_with(bytes, epoch, ImportFault::None)
    }

    fn recover_with(
        &mut self,
        bytes: &[u8],
        epoch: u64,
        fault: ImportFault,
    ) -> Result<(), Failure> {
        let decoded = checkpoint::decode(bytes, epoch).map_err(Failure::Checkpoint)?;
        if decoded.values.len() > WORDS {
            return Err(Failure::Capacity);
        }
        // Build privately with fresh backing and allocator issuance. Only a
        // complete candidate replaces the old host; never reopen its old root.
        let mut candidate = Self::new();
        candidate.counter = decoded.counter;
        candidate.update(|mut view| {
            for (index, value) in decoded.values.into_iter().enumerate() {
                view.push(value).map_err(Failure::Scope)?;
                if fault == ImportFault::AfterValue(index + 1) {
                    return Err(Failure::Injected);
                }
            }
            Ok(())
        })??;
        if fault == ImportFault::BeforePublish {
            return Err(Failure::Injected);
        }
        *self = candidate;
        Ok(())
    }
}

fn populated() -> ProviderHost {
    let mut host = ProviderHost::new();
    host.update(|mut view| {
        for value in [17, 23, 41] {
            view.push(value).unwrap();
        }
    })
    .unwrap();
    host
}

#[test]
fn incomplete_finalization_poison_survives_release_and_descriptor_publication() {
    for fault in [FrameFault::AfterRelease, FrameFault::AfterPublish] {
        for forget in [false, true] {
            let mut host = populated();
            let saved = host.export(7).unwrap();
            let old_root = host.root;
            let result = host.update_with(
                |mut view| {
                    view.values_mut()[0] = 97;
                    view.push(123).unwrap();
                    if forget {
                        #[expect(
                            clippy::forget_non_drop,
                            reason = "forgotten views cannot clear finalization poison"
                        )]
                        mem::forget(view);
                    }
                    Err::<(), _>("declined")
                },
                fault,
            );
            assert_eq!(result, Err(Failure::Injected));
            assert!(host.poisoned);
            assert_eq!(&host.backing[..4], &[97, 23, 41, 123]);
            assert_eq!(
                host.root,
                if fault == FrameFault::AfterRelease {
                    old_root
                } else {
                    Descriptor {
                        length: 4,
                        capacity: 4,
                    }
                }
            );
            let poisoned_root = host.root;
            let poisoned_backing = host.backing;
            assert_eq!(
                host.update(|_| panic!("poisoned callback")),
                Err(Failure::Poisoned)
            );
            assert_eq!(host.export(7), Err(Failure::Poisoned));
            assert_eq!(
                host.recover(&saved, 8),
                Err(Failure::Checkpoint(checkpoint::Error::Epoch))
            );
            assert_eq!(
                host.recover_with(&saved, 7, ImportFault::BeforePublish),
                Err(Failure::Injected)
            );
            assert!(host.poisoned);
            assert_eq!(host.root, poisoned_root);
            assert_eq!(host.backing, poisoned_backing);
            host.recover(&saved, 7).unwrap();
            assert!(!host.poisoned);
            assert_eq!(host.root, old_root);
            host.update(|mut view| {
                assert_eq!(view.values(), [17, 23, 41]);
                view.clear();
                view.shrink_to_fit();
                view.push(55).unwrap();
            })
            .unwrap();
            assert_eq!(
                host.root,
                Descriptor {
                    length: 1,
                    capacity: 1
                }
            );
        }
    }
}

#[test]
fn business_errors_publish_growth_and_forgotten_views_leave_the_host_available() {
    let mut host = populated();
    let result = host.update(|mut view| {
        view.values_mut()[0] = 97;
        view.push(123).unwrap();
        Err::<(), _>("declined")
    });
    assert_eq!(result, Ok(Err("declined")));
    assert!(!host.poisoned);
    assert_eq!(
        host.root,
        Descriptor {
            length: 4,
            capacity: 4
        }
    );
    host.update(|mut view| {
        view.values_mut()[1] = 99;
        #[expect(
            clippy::forget_non_drop,
            reason = "forgetting a view must not bypass host finalization"
        )]
        mem::forget(view);
    })
    .unwrap();
    assert!(!host.poisoned);
    let bytes = host.export(7).unwrap();
    let decoded = checkpoint::decode(&bytes, 7).unwrap();
    assert_eq!(decoded.values, [97, 99, 41, 123]);
    assert_eq!(decoded.counter, 41);
}

#[test]
fn caught_unwinds_poison_before_callback_and_only_checkpoint_replacement_recovers() {
    for fault in 0..3 {
        let mut host = populated();
        let saved = host.export(7).unwrap();
        let old_root = host.root;
        let old_backing = host.backing;
        // AssertUnwindSafe permits observing the poisoned fixture, not resuming
        // its allocation or claiming that the unwind restored its bytes.
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = host.update(|mut view| {
                if fault == 0 {
                    panic!("before mutation");
                }
                view.values_mut()[0] = 97;
                view.push(123).unwrap();
                if fault == 2 {
                    #[expect(
                        clippy::forget_non_drop,
                        reason = "poison cannot depend on view destruction"
                    )]
                    mem::forget(view);
                }
                panic!("after mutation");
            });
        }));
        assert!(panic.is_err());
        assert!(host.poisoned);
        assert_eq!(host.root, old_root);
        if fault == 0 {
            assert_eq!(host.backing, old_backing);
        } else {
            assert_eq!(&host.backing[..4], &[97, 23, 41, 123]);
        }
        let damaged = host.backing;
        assert_eq!(
            host.update(|_| panic!("poisoned callback ran")),
            Err(Failure::Poisoned)
        );
        assert_eq!(host.export(7), Err(Failure::Poisoned));
        assert_eq!(host.backing, damaged);

        let mut corrupt = saved.clone();
        corrupt[80] ^= 1;
        assert_eq!(
            host.recover(&corrupt, 7),
            Err(Failure::Checkpoint(checkpoint::Error::Integrity))
        );
        assert_eq!(
            host.recover(&saved, 8),
            Err(Failure::Checkpoint(checkpoint::Error::Epoch))
        );
        let oversized = checkpoint::encode_v2(41, &[1; WORDS + 1], 7).unwrap();
        assert_eq!(host.recover(&oversized, 7), Err(Failure::Capacity));
        for import_fault in [
            ImportFault::AfterValue(1),
            ImportFault::AfterValue(3),
            ImportFault::BeforePublish,
        ] {
            assert_eq!(
                host.recover_with(&saved, 7, import_fault),
                Err(Failure::Injected)
            );
            assert!(host.poisoned);
            assert_eq!(host.root, old_root);
            assert_eq!(host.backing, damaged);
            assert_eq!(host.export(7), Err(Failure::Poisoned));
        }
        assert!(host.poisoned);
        assert_eq!(host.root, old_root);
        assert_eq!(host.backing, damaged);

        host.recover(&saved, 7).unwrap();
        assert!(!host.poisoned);
        assert_eq!(host.export(7).unwrap(), saved);
        host.update(|mut view| {
            view.clear();
            view.shrink_to_fit();
            view.push(55).unwrap();
        })
        .unwrap();
        assert_eq!(
            host.root,
            Descriptor {
                length: 1,
                capacity: 1
            }
        );
        assert_eq!(host.update(|view| view.values().to_vec()).unwrap(), [55]);
    }
}

#[test]
fn failed_admission_does_not_run_callback_or_clear_poison() {
    let mut host = populated();
    let saved = host.export(7).unwrap();
    host.root.capacity = WORDS + 1;
    let before = host.backing;
    assert_eq!(
        host.update(|_| panic!("invalid root callback ran")),
        Err(Failure::Region(RegionError::Root))
    );
    assert!(host.poisoned);
    assert_eq!(host.backing, before);
    assert_eq!(host.update(|_| ()), Err(Failure::Poisoned));
    host.recover(&saved, 7).unwrap();
    assert_eq!(host.export(7).unwrap(), saved);
}

#[test]
fn failed_candidates_preserve_available_state_and_full_or_empty_replacements_continue_allocating() {
    let mut host = populated();
    let saved = host.export(7).unwrap();
    let root = host.root;
    let backing = host.backing;
    let replacement = checkpoint::encode_v2(99, &[55; WORDS], 8).unwrap();
    for fault in [
        ImportFault::AfterValue(1),
        ImportFault::AfterValue(WORDS),
        ImportFault::BeforePublish,
    ] {
        assert_eq!(
            host.recover_with(&replacement, 8, fault),
            Err(Failure::Injected)
        );
        assert!(!host.poisoned);
        assert_eq!(host.root, root);
        assert_eq!(host.backing, backing);
        assert_eq!(host.export(7).unwrap(), saved);
    }
    host.recover(&replacement, 8).unwrap();
    assert_eq!(host.export(8).unwrap(), replacement);
    assert_eq!(
        host.root,
        Descriptor {
            length: WORDS,
            capacity: WORDS
        }
    );
    let result = host.update(|mut view| {
        view.values_mut()[0] = 77;
        view.push(88)
    });
    assert_eq!(result, Ok(Err(Refusal::Limit)));
    assert!(!host.poisoned);
    assert_eq!(host.backing[0], 77);
    let empty = checkpoint::encode_v2(123, &[], 9).unwrap();
    host.recover(&empty, 9).unwrap();
    assert_eq!(host.export(9).unwrap(), empty);
    assert_eq!(
        host.root,
        Descriptor {
            length: 0,
            capacity: 0
        }
    );
    host.update(|mut view| view.push(42)).unwrap().unwrap();
    let decoded = checkpoint::decode(&host.export(9).unwrap(), 9).unwrap();
    assert_eq!(decoded.values, [42]);
    assert_eq!(decoded.counter, 123);
}
