use super::{Error, Suspension};
use crate::arena::{Allocator, Arena, Layout, Parked, Refusal, WORDS, ledger_image};

fn vector(arena: &Arena, value: u64) -> Vec<u64, &Arena> {
    let mut values = Vec::new_in(arena);
    values.try_reserve_exact(4).unwrap();
    values.push(value);
    values
}

fn suspend(root: Parked<'_>) -> Suspension<'_> {
    Suspension::new(root).unwrap_or_else(|(error, _)| panic!("suspension failed: {error:?}"))
}

#[test]
fn sole_root_freezes_allocation_and_resumes_exactly_once() {
    let arena = Arena::new();
    let values = vector(&arena, 42);
    let address = values.as_ptr();
    let root = Arena::park(values);
    let descriptor = root.descriptor;
    let before = arena.ledger.get();
    let mut frozen = suspend(root);
    assert!(arena.suspended.get());
    assert!(arena.allocate(Layout::new::<u64>()).is_err());
    // Deliberate test-only forgery cannot bypass the arena-wide suspension.
    assert_eq!(
        Parked {
            arena: &arena,
            descriptor
        }
        .reopen()
        .unwrap_err(),
        Refusal::Suspended
    );
    assert_eq!(arena.ledger.get(), before);
    let mut values = frozen.resume().unwrap();
    assert_eq!(values.as_ptr(), address);
    assert_eq!(values.as_slice(), [42]);
    assert!(!arena.suspended.get());
    assert_eq!(frozen.resume().unwrap_err(), Error::State);
    values.extend([1, 2, 3, 4, 5]);
    assert_eq!(values.remove(1), 1);
    values.shrink_to_fit();
    drop(values);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
    arena.ledger.get().validate().unwrap();
}

#[test]
fn unaccounted_live_owner_refuses_without_consuming_the_root() {
    let arena = Arena::new();
    let root = Arena::park(vector(&arena, 42));
    let other = vector(&arena, 99);
    let before = arena.ledger.get();
    let (error, root) = Suspension::new(root).err().unwrap();
    assert_eq!(error, Error::UnaccountedAllocation);
    assert!(!arena.suspended.get());
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(other.as_slice(), [99]);
    drop(other);
    let mut frozen = suspend(root);
    assert_eq!(frozen.resume().unwrap().as_slice(), [42]);
}

#[test]
fn an_extra_parked_owner_also_prevents_single_root_suspension() {
    let arena = Arena::new();
    let root = Arena::park(vector(&arena, 42));
    let other = Arena::park(vector(&arena, 99));
    let before = arena.ledger.get();
    let (error, root) = Suspension::new(root).err().unwrap();
    assert_eq!(error, Error::UnaccountedAllocation);
    assert_eq!(arena.ledger.get(), before);
    assert!(!arena.suspended.get());
    assert_eq!(root.reopen().unwrap().as_slice(), [42]);
    assert_eq!(other.reopen().unwrap().as_slice(), [99]);
}

#[test]
#[expect(
    clippy::forget_non_drop,
    reason = "exercise forgotten authority explicitly, independently of Drop"
)]
fn forgotten_live_or_parked_owners_are_not_silently_omitted() {
    for park_other in [false, true] {
        let arena = Arena::new();
        let root = Arena::park(vector(&arena, 42));
        let other = vector(&arena, 99);
        if park_other {
            std::mem::forget(Arena::park(other));
        } else {
            std::mem::forget(other);
        }
        let before = arena.ledger.get();
        let (error, root) = Suspension::new(root).err().unwrap();
        assert_eq!(error, Error::UnaccountedAllocation);
        assert_eq!(arena.ledger.get(), before);
        drop(root.reopen().unwrap());
        assert_eq!(arena.reusable_bytes(), (WORDS - 4) * 8);
    }
}

#[test]
#[expect(
    clippy::forget_non_drop,
    clippy::drop_non_drop,
    reason = "test both ways of losing suspension authority without a destructor"
)]
fn dropping_or_forgetting_suspension_keeps_the_domain_quarantined() {
    for forget in [false, true] {
        for empty in [false, true] {
            let arena = Arena::new();
            let values = if empty {
                Vec::new_in(&arena)
            } else {
                vector(&arena, 42)
            };
            let frozen = suspend(Arena::park(values));
            let before = arena.ledger.get();
            if forget {
                std::mem::forget(frozen);
            } else {
                drop(frozen);
            }
            assert!(arena.suspended.get());
            assert!(arena.allocate(Layout::new::<u64>()).is_err());
            assert_eq!(arena.ledger.get(), before);
        }
    }
}

#[test]
fn damaged_or_consistently_changed_images_never_unfreeze_or_publish_a_vector() {
    for consistent in [false, true] {
        let arena = Arena::new();
        let mut frozen = suspend(Arena::park(vector(&arena, 42)));
        let before = arena.ledger.get();
        let expected = if consistent {
            let mut altered = before;
            altered.next_generation += 1;
            frozen.image = ledger_image::encode(&altered).unwrap();
            Error::ChangedLedger
        } else {
            frozen.image[80] ^= 1;
            Error::Image(ledger_image::Error::Integrity)
        };
        assert_eq!(frozen.resume().unwrap_err(), expected);
        assert!(frozen.root.is_some());
        assert!(arena.suspended.get());
        assert!(arena.allocate(Layout::new::<u64>()).is_err());
        assert_eq!(arena.ledger.get(), before);
    }
}

#[test]
fn identical_image_bytes_cannot_choose_another_allocation_domain() {
    let a = Arena::new();
    let b = Arena::new();
    let mut first = suspend(Arena::park(vector(&a, 42)));
    let mut second = suspend(Arena::park(vector(&b, 99)));
    assert_eq!(first.image, second.image); // Metadata contains no payload/domain identity.
    std::mem::swap(&mut first.image, &mut second.image);
    let first = first.resume().unwrap();
    let second = second.resume().unwrap();
    assert_eq!(first.as_slice(), [42]);
    assert_eq!(second.as_slice(), [99]);
    assert_ne!(first.as_ptr(), second.as_ptr());
}

#[test]
fn coherent_image_and_ledger_changes_cannot_rebind_the_original_token() {
    let arena = Arena::new();
    let mut frozen = suspend(Arena::park(vector(&arena, 42)));
    let mut altered = arena.ledger.get();
    altered.blocks[0].as_mut().unwrap().generation += 1;
    altered.next_generation += 1;
    arena.ledger.set(altered); // Deliberate internal fault, never a public operation.
    frozen.image = ledger_image::encode(&altered).unwrap();
    assert_eq!(
        frozen.resume().unwrap_err(),
        Error::Root(Refusal::UnknownAllocation)
    );
    assert!(arena.suspended.get());
    assert!(frozen.root.is_some());
    assert_eq!(arena.ledger.get(), altered);
}

#[test]
fn empty_root_resumes_and_can_allocate_normally() {
    let arena = Arena::new();
    let mut frozen = suspend(Arena::park(Vec::new_in(&arena)));
    let mut values = frozen.resume().unwrap();
    assert!(values.is_empty());
    values.push(42);
    assert_eq!(values.as_slice(), [42]);
    drop(values);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn an_invalid_root_descriptor_refuses_before_freezing() {
    let arena = Arena::new();
    let mut root = Arena::park(vector(&arena, 42));
    let original = root.descriptor;
    root.descriptor.as_mut().unwrap().length += 1;
    let before = arena.ledger.get();
    let (error, mut root) = Suspension::new(root).err().unwrap();
    assert_eq!(error, Error::Root(Refusal::Initialization));
    assert!(!arena.suspended.get());
    assert_eq!(arena.ledger.get(), before);
    root.descriptor = original; // Restore test-only fault injection, not a public repair API.
    assert_eq!(root.reopen().unwrap().as_slice(), [42]);
}
