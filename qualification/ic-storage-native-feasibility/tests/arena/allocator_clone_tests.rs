//! Cloning an allocator need not confer authority over its existing blocks.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ic_storage_native_feasibility::{Fault, Host};

use super::{AllocError, Allocator, Arena, Cell, Layout, NonNull, WORDS};

// No AllocatorClone implementation: Clone deliberately selects another domain.
// Both arenas outlive every wrapper, and cloning never invalidates either one.
struct DistinctClone<'a> {
    current: &'a Arena,
    replacement: &'a Arena,
    fail_clone: &'a Cell<bool>,
}

impl Clone for DistinctClone<'_> {
    fn clone(&self) -> Self {
        assert!(!self.fail_clone.get(), "injected allocator clone failure");
        Self {
            current: self.replacement,
            replacement: self.replacement,
            fail_clone: self.fail_clone,
        }
    }
}

// SAFETY: each wrapper forwards allocation/deallocation to its unchanged live
// arena. Moving the wrapper preserves that reference. Clone chooses a different
// arena but makes no equivalence promise, never mutates the source wrapper and
// cannot invalidate its blocks. There is no Drop or unwinding allocator method.
#[expect(
    unsafe_code,
    reason = "private allocator forwarding for clone custody tests"
)]
unsafe impl Allocator for DistinctClone<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.current.allocate(layout)
    }

    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        // SAFETY: callers supply a block currently allocated by this wrapper;
        // it was issued by the same arena, with the same layout contract.
        unsafe { self.current.deallocate(pointer, layout) };
    }
}

#[test]
fn attachment_keeps_original_allocator_and_retirement_uses_the_new_empty_domain() {
    let original = Arena::new();
    let replacement = Arena::new();
    let fail_clone = Cell::new(false);
    let allocator = DistinctClone {
        current: &original,
        replacement: &replacement,
        fail_clone: &fail_clone,
    };
    let mut values = Vec::new_in(allocator);
    values.try_reserve_exact(4).unwrap();
    values.push(42);
    let address = values.as_ptr();
    let before = original.ledger.get();
    let mut host = Host::from_vec(7, values).ok().unwrap();
    host.with_update(|mut frame| {
        let ticket = frame.detach().unwrap();
        assert_eq!(original.ledger.get(), before);
        assert_eq!(replacement.reusable_bytes(), WORDS * 8);
        frame.attach(ticket);
        assert_eq!(frame.values().unwrap().as_ptr(), address);
        frame.push(43).unwrap();
        assert_eq!(frame.values().unwrap(), [42, 43]);
        let ticket = frame.detach().unwrap();
        frame.retire(ticket);
        assert_eq!(original.reusable_bytes(), WORDS * 8);
        frame.push(99).unwrap();
        assert_eq!(frame.values().unwrap(), [99]);
        assert!(replacement.reusable_bytes() < WORDS * 8);
        assert_eq!(original.reusable_bytes(), WORDS * 8);
    })
    .unwrap();
    drop(host);
    assert_eq!(original.reusable_bytes(), WORDS * 8);
    assert_eq!(replacement.reusable_bytes(), WORDS * 8);
}

#[test]
fn clone_panic_keeps_original_block_owned_and_poisons_before_detachment() {
    let original = Arena::new();
    let replacement = Arena::new();
    let fail_clone = Cell::new(true);
    let mut values = Vec::new_in(DistinctClone {
        current: &original,
        replacement: &replacement,
        fail_clone: &fail_clone,
    });
    values.try_reserve_exact(4).unwrap();
    values.push(42);
    let before = original.ledger.get();
    let mut host = Host::from_vec(7, values).ok().unwrap();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let _ = host.with_update(|mut frame| {
            frame.push(43).unwrap();
            let _ticket = frame.detach().unwrap();
            panic!("clone failure must precede ticket creation");
        });
    }));
    let failure = failure.expect_err("clone must panic");
    assert_eq!(
        failure.downcast_ref::<&str>(),
        Some(&"injected allocator clone failure")
    );
    fail_clone.set(false);
    assert_eq!(
        host.with_update(|_| panic!("poison must prevent callback entry")),
        Err(Fault::Poisoned)
    );
    assert_eq!(original.ledger.get(), before);
    assert_eq!(replacement.reusable_bytes(), WORDS * 8);
    drop(host);
    assert_eq!(original.reusable_bytes(), WORDS * 8);
    assert_eq!(replacement.reusable_bytes(), WORDS * 8);
}
