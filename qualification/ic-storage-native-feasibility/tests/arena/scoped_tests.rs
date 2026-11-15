//! The existing safe ownership model exercised with the real private allocator.
#![forbid(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use ic_storage_native_feasibility::{Fault, Host, MAX_VALUES, Refusal, checkpoint};

use super::{Arena, WORDS};

#[test]
fn supplied_vector_keeps_its_allocation_and_business_errors_preserve_writes() {
    let arena = Arena::new();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(4).unwrap();
    values.extend([1, 2]);
    let address = values.as_ptr();
    let before = arena.ledger.get();
    let mut host = Host::from_vec(7, values).ok().unwrap();
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(
        host.with_update(|mut frame| {
            assert_eq!(frame.values().unwrap().as_ptr(), address);
            frame.set_counter(8);
            frame.push(3).unwrap();
            let ticket = frame.detach().unwrap();
            assert_eq!(frame.values(), Err(Refusal::Detached));
            frame.attach(ticket);
            assert_eq!(frame.values().unwrap().as_ptr(), address);
            Err::<(), _>("business refusal")
        }),
        Ok(Err("business refusal"))
    );
    let bytes = host
        .with_update(|frame| {
            checkpoint::encode(frame.counter(), frame.values().unwrap(), 7).unwrap()
        })
        .unwrap();
    let decoded = checkpoint::decode(&bytes, 7).unwrap();
    assert_eq!(decoded.counter, 8);
    assert_eq!(decoded.values, [1, 2, 3]);
    drop(host);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn arena_capacity_refusal_preserves_state_and_retirement_releases_the_run() {
    let arena = Arena::with_limit(4).unwrap();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(4).unwrap();
    values.extend([1, 2, 3, 4]);
    let mut host = Host::from_vec(42, values).ok().unwrap();
    let before = arena.ledger.get();
    host.with_update(|mut frame| {
        assert_eq!(frame.push(5), Err(Refusal::Allocation));
        assert_eq!(frame.values().unwrap(), [1, 2, 3, 4]);
        assert_eq!(arena.ledger.get(), before);
        let ticket = frame.detach().unwrap();
        frame.retire(ticket);
        assert_eq!(arena.reusable_bytes(), 4 * 8);
        frame.push(9).unwrap();
        assert_eq!(frame.values().unwrap(), [9]);
    })
    .unwrap();
    drop(host);
    assert_eq!(arena.reusable_bytes(), 4 * 8);
}

#[test]
#[expect(
    clippy::forget_non_drop,
    reason = "exercise forgotten frame-bound authority with real arena allocations"
)]
fn forgotten_ticket_keeps_the_actual_run_until_the_poisoned_host_is_discarded() {
    let arena = Arena::new();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(4).unwrap();
    values.push(42);
    let mut host = Host::from_vec(7, values).ok().unwrap();
    let before = arena.ledger.get();
    assert_eq!(
        host.with_update(|mut frame| {
            std::mem::forget(frame.detach().unwrap());
            Err::<(), _>("cannot hide lost authority")
        }),
        Err(Fault::OutstandingOwner)
    );
    assert_eq!(
        host.with_update(|_| panic!("must not enter")),
        Err(Fault::Poisoned)
    );
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(arena.reusable_bytes(), (WORDS - 4) * 8);
    drop(host);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn panic_after_detachment_quarantines_and_host_destruction_reclaims() {
    let arena = Arena::new();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(4).unwrap();
    values.push(42);
    let mut host = Host::from_vec(7, values).ok().unwrap();
    let before = arena.ledger.get();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let _ = host.with_update(|mut frame| {
            frame.push(43).unwrap();
            let _ticket = frame.detach().unwrap();
            panic!("failure after native mutation");
        });
    }));
    assert!(failure.is_err());
    assert_eq!(host.with_update(|_| ()), Err(Fault::Poisoned));
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(arena.reusable_bytes(), (WORDS - 4) * 8);
    drop(host);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn oversized_constructor_returns_the_original_owner_without_ledger_changes() {
    let arena = Arena::new();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(MAX_VALUES + 1).unwrap();
    values.extend(0..=MAX_VALUES as u64);
    let address = values.as_ptr();
    let before = arena.ledger.get();
    let (error, values) = Host::from_vec(42, values).err().unwrap();
    assert_eq!(error, Refusal::Capacity);
    assert_eq!(values.as_ptr(), address);
    assert!(values.iter().copied().eq(0..=MAX_VALUES as u64));
    assert_eq!(arena.ledger.get(), before);
    drop(values);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}
