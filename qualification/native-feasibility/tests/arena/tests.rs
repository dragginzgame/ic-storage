use super::{Allocator, Arena, Layout, Parked, Refusal, WORD_BYTES, WORDS};

#[test]
fn moving_an_owned_allocator_keeps_its_backing_allocation_stable() {
    let mut vector: Vec<u64, _> = Vec::new_in(Arena::new());
    vector.try_reserve_exact(4).unwrap();
    vector.extend([55, 89]);
    let pointer = vector.as_ptr();
    let mut moved = Box::new(vector);
    assert_eq!(moved.as_ptr(), pointer);
    assert_eq!(moved.as_slice(), [55, 89]);
    moved.push(144);
    assert_eq!(moved.allocator().reusable_bytes(), (WORDS - 4) * WORD_BYTES);
}

#[test]
fn fragmented_free_space_refuses_until_an_adjacent_run_is_available() {
    let arena = Arena::new();
    let mut clients: Vec<_> = (0..WORDS)
        .map(|index| {
            let mut vector: Vec<u64, _> = Vec::new_in(&arena);
            vector.try_reserve_exact(1).unwrap();
            vector.push(index as u64);
            Some(vector)
        })
        .collect();
    assert_eq!(arena.reusable_bytes(), 0);
    for index in (0..WORDS).step_by(2) {
        drop(clients[index].take());
    }
    assert_eq!(arena.reusable_bytes(), WORDS / 2 * WORD_BYTES);
    let mut replacement: Vec<u64, _> = Vec::new_in(&arena);
    let before = arena.ledger.get();
    assert!(replacement.try_reserve_exact(2).is_err());
    assert_eq!(arena.ledger.get(), before);
    drop(clients[1].take());
    replacement.try_reserve_exact(3).unwrap();
    replacement.extend([100, 101, 102]);
    for (index, vector) in clients.iter().enumerate() {
        if let Some(vector) = vector {
            assert_eq!(vector.as_slice(), [index as u64]);
        }
    }
    drop(replacement);
    drop(clients);
    assert_eq!(arena.reusable_bytes(), WORDS * WORD_BYTES);
}

#[test]
fn vector_round_trip_preserves_allocation_and_initialized_values() {
    let arena = Arena::new();
    let mut vector = Vec::new_in(&arena);
    vector.try_reserve_exact(8).unwrap();
    vector.extend([3, 5, 8]);
    let (address, capacity) = (vector.as_ptr(), vector.capacity());
    let parked = Arena::park(vector);
    assert_eq!(arena.reusable_bytes(), (WORDS - capacity) * WORD_BYTES);
    let mut reopened = parked.reopen().unwrap();
    assert_eq!(reopened.as_ptr(), address);
    assert_eq!(reopened.as_slice(), [3, 5, 8]);
    reopened.push(13);
    assert_eq!(reopened.pop(), Some(13));
    drop(reopened);
    assert_eq!(arena.reusable_bytes(), WORDS * WORD_BYTES);
}

#[test]
fn simultaneous_vectors_are_disjoint_and_growth_preserves_both() {
    let arena = Arena::new();
    let mut first: Vec<u64, _> = Vec::new_in(&arena);
    first.try_reserve_exact(2).unwrap();
    first.extend([10, 11]);
    let mut second: Vec<u64, _> = Vec::new_in(&arena);
    second.try_reserve_exact(2).unwrap();
    second.extend([20, 21]);
    assert_ne!(first.as_ptr(), second.as_ptr());
    first.try_reserve_exact(8).unwrap();
    assert_eq!(first.as_slice(), [10, 11]);
    assert_eq!(second.as_slice(), [20, 21]);
    drop(first);
    drop(second);
    assert_eq!(arena.reusable_bytes(), WORDS * WORD_BYTES);
}

#[test]
fn exhausted_growth_preserves_original_allocation_and_ledger() {
    let arena = Arena::new();
    let mut first: Vec<u64, _> = Vec::new_in(&arena);
    first.try_reserve_exact(32).unwrap();
    first.extend([1, 2]);
    let mut second: Vec<u64, _> = Vec::new_in(&arena);
    second.try_reserve_exact(32).unwrap();
    let before = arena.ledger.get();
    let pointer = first.as_ptr();
    assert!(first.try_reserve_exact(31).is_err());
    assert_eq!(first.as_ptr(), pointer);
    assert_eq!(first.as_slice(), [1, 2]);
    assert_eq!(arena.ledger.get(), before);
}

#[test]
fn shrink_remove_release_and_reuse_do_not_claim_page_reclamation() {
    let arena = Arena::new();
    let mut vector: Vec<u64, _> = Vec::new_in(&arena);
    vector.try_reserve_exact(16).unwrap();
    vector.extend([1, 2, 3, 4]);
    assert_eq!(vector.remove(1), 2);
    vector.shrink_to_fit();
    assert_eq!(vector.capacity(), 3);
    assert_eq!(vector.as_slice(), [1, 3, 4]);
    assert_eq!(arena.reusable_bytes(), (WORDS - 3) * WORD_BYTES);
    drop(vector);
    let mut replacement: Vec<u64, _> = Vec::new_in(&arena);
    replacement.try_reserve_exact(WORDS).unwrap();
    assert_eq!(arena.reusable_bytes(), 0);
    drop(replacement);
    assert_eq!(arena.reusable_bytes(), WORDS * WORD_BYTES);
    assert_eq!(std::mem::size_of_val(&*arena.backing), WORDS * WORD_BYTES);
}

#[test]
fn empty_vector_parks_without_an_allocation() {
    let arena = Arena::new();
    let parked = Arena::park(Vec::new_in(&arena));
    assert!(parked.descriptor.is_none());
    assert!(parked.reopen().unwrap().is_empty());
    assert_eq!(arena.reusable_bytes(), WORDS * WORD_BYTES);
}

#[test]
fn cleared_vector_reopens_empty_without_reading_spare_capacity() {
    let arena = Arena::new();
    let mut vector = Vec::new_in(&arena);
    vector.try_reserve_exact(8).unwrap();
    vector.push(42);
    vector.clear();
    let vector = Arena::park(vector).reopen().unwrap();
    assert!(vector.is_empty());
    assert_eq!(vector.capacity(), 8);
    drop(vector);
    assert_eq!(arena.reusable_bytes(), WORDS * WORD_BYTES);
}

#[test]
fn partial_construction_unwind_releases_only_its_live_block() {
    let arena = Arena::new();
    let mut surviving: Vec<u64, _> = Vec::new_in(&arena);
    surviving.try_reserve_exact(8).unwrap();
    surviving.extend([21, 34]);
    let free_before = arena.reusable_bytes();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut partial: Vec<u64, _> = Vec::new_in(&arena);
        partial.try_reserve_exact(16).unwrap();
        partial.extend([1, 2, 3]);
        panic!("injected constructor failure after initializing three elements");
    }));
    assert!(outcome.is_err());
    assert_eq!(surviving.as_slice(), [21, 34]);
    assert_eq!(arena.reusable_bytes(), free_before);
    drop(surviving);
    assert_eq!(arena.reusable_bytes(), WORDS * WORD_BYTES);
}

#[test]
fn forgotten_live_vector_also_keeps_its_run_unavailable() {
    let arena = Arena::new();
    let mut vector: Vec<u64, _> = Vec::new_in(&arena);
    vector.try_reserve_exact(WORDS).unwrap();
    std::mem::forget(vector);
    assert!(arena.allocate(Layout::new::<u64>()).is_err());
    assert_eq!(arena.reusable_bytes(), 0);
}

#[test]
fn malformed_length_capacity_offset_and_generation_refuse_before_typed_access() {
    for field in 0..4 {
        let arena = Arena::new();
        let mut vector = Vec::new_in(&arena);
        vector.try_reserve_exact(8).unwrap();
        vector.push(42);
        let mut parked = Arena::park(vector);
        let descriptor = parked.descriptor.as_mut().unwrap();
        let expected = match field {
            0 => {
                descriptor.length = 2;
                Refusal::Initialization
            }
            1 => {
                descriptor.capacity = 9;
                Refusal::Extent
            }
            2 => {
                descriptor.start = WORDS;
                Refusal::UnknownAllocation
            }
            _ => {
                descriptor.generation += 1;
                Refusal::UnknownAllocation
            }
        };
        let ledger = arena.ledger.get();
        assert_eq!(parked.reopen().unwrap_err(), expected);
        assert_eq!(arena.ledger.get(), ledger);
    }
}

#[test]
fn stale_descriptor_cannot_reopen_a_reused_run_or_duplicate_a_live_owner() {
    let arena = Arena::new();
    let mut original = Vec::new_in(&arena);
    original.try_reserve_exact(4).unwrap();
    original.push(1);
    let parked = Arena::park(original);
    let stale = parked.descriptor;
    let reopened = parked.reopen().unwrap();
    // Deliberate test-only descriptor forgery; there is no such public API.
    assert_eq!(
        Parked {
            arena: &arena,
            descriptor: stale
        }
        .reopen()
        .unwrap_err(),
        Refusal::NotParked
    );
    drop(reopened);
    let mut replacement = Vec::new_in(&arena);
    replacement.try_reserve_exact(4).unwrap();
    replacement.push(2);
    let replacement = Arena::park(replacement);
    assert_eq!(stale.unwrap().start, replacement.descriptor.unwrap().start);
    assert_eq!(
        Parked {
            arena: &arena,
            descriptor: stale
        }
        .reopen()
        .unwrap_err(),
        Refusal::UnknownAllocation
    );
    assert_eq!(replacement.reopen().unwrap().as_slice(), [2]);
}

#[test]
#[expect(
    clippy::forget_non_drop,
    reason = "parked ownership accounting cannot depend on token Drop"
)]
fn forgotten_token_keeps_its_run_unavailable() {
    let arena = Arena::new();
    let mut vector = Vec::new_in(&arena);
    vector.try_reserve_exact(WORDS).unwrap();
    vector.push(123);
    std::mem::forget(Arena::park(vector));
    let mut other: Vec<u64, _> = Vec::new_in(&arena);
    assert!(other.try_reserve_exact(1).is_err());
    assert_eq!(arena.reusable_bytes(), 0);
}

#[test]
fn unsupported_layout_and_generation_exhaustion_leave_ledger_unchanged() {
    let arena = Arena::new();
    let before = arena.ledger.get();
    for (size, align) in [(0, 8), (1, 1), (8, 16), (WORDS * WORD_BYTES + 8, 8)] {
        assert!(
            arena
                .allocate(Layout::from_size_align(size, align).unwrap())
                .is_err()
        );
        assert_eq!(arena.ledger.get(), before);
    }
    let mut exhausted = before;
    exhausted.next_generation = u64::MAX;
    arena.ledger.set(exhausted);
    assert!(arena.allocate(Layout::new::<u64>()).is_err());
    assert_eq!(arena.ledger.get(), exhausted);
}
