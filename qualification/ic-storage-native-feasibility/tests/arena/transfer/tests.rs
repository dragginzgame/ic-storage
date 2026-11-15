use super::{Destination, Error, Span, Transfer};
use crate::arena::{Allocator, Arena, Layout, WORDS, suspension::Suspension};

fn take(source: Arena) -> Transfer {
    Transfer::take(source).unwrap_or_else(|(error, _)| panic!("transfer failed: {error:?}"))
}

fn destination(transfer: Transfer) -> Destination {
    transfer
        .destination(WORDS)
        .unwrap_or_else(|(error, _)| panic!("destination failed: {error:?}"))
}

fn prepared(capacity: usize, values: &[u64]) -> (Transfer, usize) {
    let source = Arena::new();
    let mut vector = Vec::new_in(&source);
    vector.try_reserve_exact(capacity).unwrap();
    vector.extend_from_slice(values);
    if values.is_empty() && capacity != 0 {
        vector.push(u64::MAX);
        vector.clear();
    }
    let address = vector.as_ptr().addr();
    let mut frozen = Suspension::new(Arena::park(vector))
        .unwrap_or_else(|(error, _)| panic!("suspend failed: {error:?}"));
    frozen.prepare_transfer(42).unwrap();
    (take(source), address)
}

#[test]
fn fresh_controller_issues_nonzero_span_once_and_supports_reclamation() {
    let source = Arena::new();
    let mut prefix: Vec<u64, _> = Vec::new_in(&source);
    prefix.try_reserve_exact(8).unwrap();
    let mut values = Vec::new_in(&source);
    values.try_reserve_exact(4).unwrap();
    values.extend([0, u64::MAX, 0x0102_0304_0506_0708]);
    let address = values.as_ptr().addr();
    drop(prefix);
    let mut frozen = Suspension::new(Arena::park(values))
        .unwrap_or_else(|(error, _)| panic!("suspend failed: {error:?}"));
    frozen.prepare_transfer(42).unwrap();
    assert_eq!(
        frozen.prepare_transfer(99),
        Err(crate::arena::suspension::Error::State)
    );
    assert_eq!(
        frozen.resume().unwrap_err(),
        crate::arena::suspension::Error::State
    );
    assert!(source.allocate(Layout::new::<u64>()).is_err());

    let transfer = take(source); // Source controller is consumed before destination exists.
    let mut destination = destination(transfer);
    assert_eq!(
        destination.arena.reservation.get(),
        Some(Span { start: 8, words: 4 })
    );
    assert_eq!(destination.arena.reusable_bytes(), (WORDS - 4) * 8);
    assert!(
        destination
            .arena
            .ledger
            .get()
            .blocks
            .iter()
            .all(Option::is_none)
    );
    assert!(destination.arena.allocate(Layout::new::<u64>()).is_err());
    let mut record = destination.restore().unwrap();
    assert_eq!(record.counter, 42);
    assert_eq!(record.values.as_ptr().addr(), address);
    assert_eq!(
        record.values.as_slice(),
        [0, u64::MAX, 0x0102_0304_0506_0708]
    );
    assert_eq!(record.values.capacity(), 4);
    record.values.extend([5, 6, 7, 8, 9]);
    assert_eq!(record.values.remove(1), u64::MAX);
    record.values.shrink_to_fit();
    drop(record);
    assert_eq!(destination.restore().err(), Some(Error::State));
    assert_eq!(destination.arena.reusable_bytes(), WORDS * 8);
    let mut again = Vec::new_in(&destination.arena);
    again.try_reserve_exact(WORDS).unwrap();
    again.push(99u64);
    assert_eq!(again.as_slice(), [99]);
}

#[test]
fn empty_and_cleared_vectors_transfer_without_reading_spare_capacity() {
    for capacity in [0, 16] {
        let (transfer, address) = prepared(capacity, &[]);
        let mut destination = destination(transfer);
        let mut record = destination.restore().unwrap();
        assert_eq!(record.counter, 42);
        assert!(record.values.is_empty());
        assert_eq!(record.values.capacity(), capacity);
        if capacity != 0 {
            assert_eq!(record.values.as_ptr().addr(), address);
        }
        record.values.push(7);
        assert_eq!(record.values.as_slice(), [7]);
        drop(record);
        assert_eq!(destination.arena.reusable_bytes(), WORDS * 8);
    }
}

#[test]
#[expect(
    clippy::forget_non_drop,
    clippy::drop_non_drop,
    reason = "lost suspension must not grant transfer authority"
)]
fn dropped_or_forgotten_suspension_cannot_authorize_transfer() {
    for forget in [false, true] {
        let source = Arena::new();
        let mut values = Vec::new_in(&source);
        values.push(42);
        let frozen = Suspension::new(Arena::park(values))
            .unwrap_or_else(|(error, _)| panic!("suspend failed: {error:?}"));
        if forget {
            std::mem::forget(frozen);
        } else {
            drop(frozen);
        }
        let before = source.ledger.get();
        let (error, source) = Transfer::take(source).err().unwrap();
        assert_eq!(error, Error::State);
        assert_eq!(source.ledger.get(), before);
        assert!(source.suspended.get());
        assert!(source.allocate(Layout::new::<u64>()).is_err());
    }
}

#[test]
fn live_source_without_release_is_returned_unchanged() {
    let source = Arena::new();
    let before = source.ledger.get();
    let (error, source) = Transfer::take(source).err().unwrap();
    assert_eq!(error, Error::State);
    assert_eq!(source.ledger.get(), before);
    assert!(!source.suspended.get());
    let mut values = Vec::new_in(&source);
    values.push(7u64);
    assert_eq!(values.as_slice(), [7]);
}

#[test]
fn source_metadata_corruption_refuses_transfer_without_resetting_it() {
    let source = Arena::new();
    let mut values = Vec::new_in(&source);
    values.push(42);
    let mut frozen = Suspension::new(Arena::park(values))
        .unwrap_or_else(|(error, _)| panic!("suspend failed: {error:?}"));
    frozen.prepare_transfer(42).unwrap();
    let mut changed = source.ledger.get();
    changed
        .blocks
        .iter_mut()
        .flatten()
        .next()
        .unwrap()
        .parked_len = Some(2);
    source.ledger.set(changed); // Test-only corruption after explicit release.
    let (error, source) = Transfer::take(source).err().unwrap();
    assert_eq!(
        error,
        Error::Source(crate::arena::suspension::Error::Root(
            crate::arena::Refusal::Initialization
        ))
    );
    assert_eq!(source.ledger.get(), changed);
    assert!(source.suspended.get());
}

#[test]
fn wrong_layout_cannot_issue_or_clear_the_reserved_span() {
    let (transfer, _) = prepared(4, &[42]);
    let mut destination = destination(transfer);
    let before = destination.arena.ledger.get();
    let reservation = destination.arena.reservation.get();
    // Exercise only the internal issuance window; normal clients see a frozen
    // destination. None of these requests can acquire a block.
    destination.arena.suspended.set(false);
    for layout in [
        Layout::new::<u64>(),
        Layout::from_size_align(32, 4).unwrap(),
    ] {
        assert!(destination.arena.allocate(layout).is_err());
        assert_eq!(destination.arena.ledger.get(), before);
        assert_eq!(destination.arena.reservation.get(), reservation);
    }
    destination.arena.suspended.set(true);
    assert_eq!(destination.restore().unwrap().values.as_slice(), [42]);
}

#[test]
fn failed_issuance_keeps_root_and_span_quarantined() {
    let (transfer, _) = prepared(4, &[42]);
    let mut destination = destination(transfer);
    let mut exhausted = destination.arena.ledger.get();
    exhausted.next_generation = u64::MAX; // Fault at the actual allocate boundary.
    destination.arena.ledger.set(exhausted);
    let reservation = destination.arena.reservation.get();
    assert_eq!(destination.restore().err(), Some(Error::Allocation));
    assert!(destination.root.is_some());
    assert!(destination.arena.suspended.get());
    assert_eq!(destination.arena.ledger.get(), exhausted);
    assert_eq!(destination.arena.reservation.get(), reservation);
    assert!(destination.arena.allocate(Layout::new::<u64>()).is_err());
    // Discarding the whole destination is safe; no partial Vec was published.
}

#[test]
fn altered_destination_span_refuses_before_allocation() {
    let (transfer, _) = prepared(4, &[42]);
    let mut destination = destination(transfer);
    destination.arena.reservation.set(Some(Span {
        start: WORDS,
        words: 4,
    }));
    let before = destination.arena.ledger.get();
    assert_eq!(destination.restore().err(), Some(Error::State));
    assert!(destination.root.is_some());
    assert!(destination.arena.suspended.get());
    assert_eq!(destination.arena.ledger.get(), before);
}

#[test]
fn forgetting_the_restored_owner_cannot_issue_it_again_or_authorize_transfer() {
    let (transfer, _) = prepared(4, &[42]);
    let mut destination = destination(transfer);
    let record = destination.restore().unwrap();
    std::mem::forget(record);
    assert_eq!(destination.restore().err(), Some(Error::State));
    assert_eq!(destination.arena.reusable_bytes(), (WORDS - 4) * 8);
    let before = destination.arena.ledger.get();
    let (error, arena) = Transfer::take(destination.arena).err().unwrap();
    assert_eq!(error, Error::State);
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(arena.reusable_bytes(), (WORDS - 4) * 8);
}

#[test]
fn domain_refusal_preserves_authority_and_an_exact_fit_enforces_its_limit() {
    let (mut transfer, address) = prepared(4, &[42]);
    for (limit, expected) in [
        (0, Error::Domain),
        (WORDS + 1, Error::Domain),
        (3, Error::IncompatibleExtent), // Initialized values fit; capacity does not.
    ] {
        let (error, returned) = transfer.destination(limit).err().unwrap();
        assert_eq!(error, expected);
        transfer = returned;
    }
    let mut destination = transfer
        .destination(4)
        .unwrap_or_else(|(error, _)| panic!("exact fit failed: {error:?}"));
    let mut record = destination.restore().unwrap();
    assert_eq!(record.counter, 42);
    assert_eq!(record.values.as_ptr().addr(), address);
    assert_eq!(record.values.as_slice(), [42]);
    assert!(record.values.try_reserve_exact(4).is_err());
    assert_eq!(record.values.as_slice(), [42]);
    assert_eq!(record.values.capacity(), 4);
    drop(record);
    assert_eq!(destination.arena.reusable_bytes(), 4 * 8);
    let mut reuse = Vec::new_in(&destination.arena);
    reuse.try_reserve_exact(4).unwrap();
    reuse.push(99u64);
    assert!(reuse.try_reserve_exact(4).is_err());
    assert_eq!(reuse.as_slice(), [99]);
}

#[test]
fn restricted_arena_rejects_ledger_extents_outside_its_usable_region() {
    assert!(Arena::with_limit(0).is_err());
    assert!(Arena::with_limit(WORDS + 1).is_err());
    let arena = Arena::with_limit(4).unwrap();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(4).unwrap();
    values.push(42);
    let root = Arena::park(values);
    let before = arena.ledger.get();
    let mut changed = before;
    changed.blocks[0].as_mut().unwrap().start = 4;
    changed.occupied.fill(false);
    changed.occupied[4..8].fill(true);
    changed.validate().unwrap(); // Fits physical backing, but not this controller.
    arena.ledger.set(changed);
    assert_eq!(
        root.admit(&changed),
        Err(crate::arena::Refusal::MalformedLedger)
    );
    assert_eq!(arena.ledger.get(), changed);
    arena.ledger.set(before); // Undo test-only fault; no recovery API is exposed.
    assert_eq!(root.reopen().unwrap().as_slice(), [42]);
    assert_eq!(arena.reusable_bytes(), 4 * 8);
}

#[test]
fn reconstructed_record_enters_scoped_ownership_without_reallocating() {
    let (transfer, address) = prepared(4, &[1, 2]);
    let mut destination = destination(transfer);
    let super::Record { counter, values } = destination.restore().unwrap();
    let mut host = ic_storage_native_feasibility::Host::from_vec(counter, values)
        .ok()
        .unwrap();
    host.with_update(|mut frame| {
        assert_eq!(frame.counter(), 42);
        assert_eq!(frame.values().unwrap().as_ptr().addr(), address);
        let ticket = frame.detach().unwrap();
        frame.attach(ticket);
        frame.push(3).unwrap();
        assert_eq!(frame.values().unwrap(), [1, 2, 3]);
    })
    .unwrap();
    drop(host);
    assert_eq!(destination.arena.reusable_bytes(), WORDS * 8);
}
