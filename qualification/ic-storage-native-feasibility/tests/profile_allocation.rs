//! Fail each real native allocation boundary, including the second index.
use ic_storage::ordered::Error as StorageError;
use ic_storage_native_feasibility::profile::{Error, Schema, State};
use std::{
    alloc::{AllocError, Allocator, Global, Layout},
    cell::Cell,
    ptr::NonNull,
};

struct Budget {
    remaining: Cell<usize>,
    live: Cell<usize>,
    peak: Cell<usize>,
}
impl Budget {
    fn new(remaining: usize) -> Self {
        Self {
            remaining: Cell::new(remaining),
            live: Cell::new(0),
            peak: Cell::new(0),
        }
    }
}

// SAFETY: every successful block is issued by Global and is returned unchanged
// to Global with the caller's original layout. Budget refusal never acquires a
// block. Shared references only mutate accounting Cells, not allocated payload.
// Moving a Budget preserves all Global blocks; its reference outlives every map.
#[expect(unsafe_code, reason = "test-only fallible Global allocator forwarding")]
unsafe impl Allocator for Budget {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let remaining = self.remaining.get().checked_sub(1).ok_or(AllocError)?;
        let block = Global.allocate(layout)?;
        self.remaining.set(remaining);
        self.live.set(self.live.get() + layout.size());
        self.peak.set(self.peak.get().max(self.live.get()));
        Ok(block)
    }

    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        self.live.set(self.live.get() - layout.size());
        // SAFETY: the caller returns this allocator's Global block and layout.
        unsafe { Global.deallocate(pointer, layout) };
    }
}

#[test]
fn refusal_at_each_create_allocation_quarantines_both_roots_and_reclaims_on_discard() {
    let mut success_at = None;
    for allowance in 0..16 {
        let budget = Budget::new(allowance);
        let mut state = State::new_in(Schema::Wide, &budget);
        match state.put(&[11; 29], 1, 0, &"n".repeat(128)) {
            Ok(_) => {
                state.validate().unwrap();
                success_at = Some(allowance);
            }
            Err(error) => {
                assert_eq!(error, Error::Storage(StorageError::Allocation));
                assert_eq!(state.get(&[11; 29], 1), Err(Error::Poisoned));
                assert_eq!(state.checkpoint(1), Err(Error::Poisoned));
                assert!(matches!(state.usage(), Err(Error::Poisoned)));
            }
        }
        drop(state);
        assert_eq!(budget.live.get(), 0, "allowance {allowance}");
        if success_at.is_some() {
            break;
        }
    }
    // Two independently owned node/payload pairs are actually exercised.
    assert!(success_at.unwrap() >= 4);
}

#[test]
fn recovery_refusal_never_publishes_a_partial_pair_and_releases_every_candidate() {
    let mut source = State::new_in(Schema::Wide, Global);
    for id in 1..=16 {
        source
            .put(&[id as u8 + 10; 29], id, 0, &"n".repeat(128))
            .unwrap();
    }
    let checkpoint = source.checkpoint(8).unwrap();
    drop(source);
    let original = checkpoint.clone();
    let mut recovered = false;
    for allowance in 0..128 {
        let budget = Budget::new(allowance);
        match State::recover(&checkpoint, 8, Schema::Wide, Schema::Narrow, &budget) {
            Err(error) => assert_eq!(error, Error::Storage(StorageError::Allocation)),
            Ok(mut target) => {
                target.validate().unwrap();
                // Exact retries and deleting both entries require no allocation.
                budget.remaining.set(0);
                assert_eq!(
                    target
                        .put(&[11; 29], 1, 0, &"n".repeat(128))
                        .unwrap()
                        .revision,
                    1
                );
                target.remove(&[11; 29], 1, 1).unwrap();
                target.validate().unwrap();
                println!(
                    "profile conversion native_peak_requested_bytes={} allocations={allowance}",
                    budget.peak.get()
                );
                recovered = true;
            }
        }
        assert_eq!(budget.live.get(), 0, "candidate leak at {allowance}");
        assert_eq!(checkpoint, original);
        if recovered {
            break;
        }
    }
    assert!(recovered);
}
