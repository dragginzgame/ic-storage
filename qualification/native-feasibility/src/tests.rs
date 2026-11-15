use std::panic::{AssertUnwindSafe, catch_unwind};

use super::{Fault, Host, MAX_VALUES, Refusal};

#[test]
fn ordinary_error_preserves_completed_writes_and_finalizes_attachment() {
    let mut host = Host::default();
    let outcome = host.with_update(|mut frame| {
        frame.set_counter(7);
        frame.push(11).unwrap();
        let ticket = frame.detach().unwrap();
        frame.attach(ticket);
        Err::<(), _>("business error")
    });
    assert_eq!(outcome, Ok(Err("business error")));
    host.with_update(|frame| {
        assert_eq!(frame.counter(), 7);
        assert_eq!(frame.values(), Ok([11].as_slice()));
    })
    .unwrap();
}

#[test]
fn detached_owner_blocks_access_and_returns_to_the_same_allocation() {
    let mut host = Host::default();
    host.with_update(|mut frame| {
        frame.push(23).unwrap();
        let address = frame.values().unwrap().as_ptr();
        let ticket = frame.detach().unwrap();
        assert_eq!(frame.values(), Err(Refusal::Detached));
        assert_eq!(frame.push(3), Err(Refusal::Detached));
        assert_eq!(frame.pop(), Err(Refusal::Detached));
        assert!(matches!(frame.detach(), Err(Refusal::Detached)));
        frame.attach(ticket);
        assert_eq!(frame.values().unwrap().as_ptr(), address);
        assert_eq!(frame.values(), Ok([23].as_slice()));
    })
    .unwrap();
}

#[test]
#[expect(
    clippy::forget_non_drop,
    reason = "exercise deliberate forgetting of the ownership ticket"
)]
fn forgotten_ticket_quarantines_without_losing_or_reclaiming_the_owner() {
    let mut host = Host::default();
    assert_eq!(
        host.with_update(|mut frame| {
            frame.push(42).unwrap();
            std::mem::forget(frame.detach().unwrap());
        }),
        Err(Fault::OutstandingOwner)
    );
    // Inspect only fixture internals: the public API grants no poisoned access.
    assert_eq!(host.detached.as_deref(), Some([42].as_slice()));
    assert_eq!(
        host.with_update(|_| panic!("must not enter")),
        Err(Fault::Poisoned)
    );
    assert_eq!(host.detached.as_deref(), Some([42].as_slice()));
}

#[test]
fn dropped_ticket_also_leaves_an_outstanding_obligation() {
    let mut host = Host::default();
    assert_eq!(
        host.with_update(|mut frame| {
            let _ticket = frame.detach().unwrap();
            Err::<(), _>("business error cannot hide lost ownership")
        }),
        Err(Fault::OutstandingOwner)
    );
}

#[test]
#[expect(
    clippy::forget_non_drop,
    reason = "neither the frame nor ticket destructor may own finalization"
)]
fn forgetting_both_frame_and_ticket_still_quarantines() {
    let mut host = Host::default();
    assert_eq!(
        host.with_update(|mut frame| {
            frame.push(13).unwrap();
            std::mem::forget(frame.detach().unwrap());
            std::mem::forget(frame);
        }),
        Err(Fault::OutstandingOwner)
    );
    assert_eq!(host.detached.as_deref(), Some([13].as_slice()));
    assert_eq!(host.with_update(|_| ()), Err(Fault::Poisoned));
}

#[test]
#[expect(
    clippy::forget_non_drop,
    reason = "frame finalization must not rely on its destructor"
)]
fn forgetting_the_view_without_detaching_is_safe() {
    let mut host = Host::default();
    host.with_update(|mut frame| {
        frame.push(5).unwrap();
        std::mem::forget(frame);
    })
    .unwrap();
    assert_eq!(
        host.with_update(|frame| frame.values().unwrap().to_vec()),
        Ok(vec![5])
    );
}

#[test]
fn panic_preserves_quarantine_and_does_not_claim_rollback() {
    let mut host = Host::default();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = host.with_update(|mut frame| {
            frame.set_counter(99);
            frame.push(2).unwrap();
            let _ticket = frame.detach().unwrap();
            panic!("fault after mutation");
        });
    }));
    assert!(panic.is_err());
    assert_eq!(host.counter, 99);
    assert_eq!(host.detached.as_deref(), Some([2].as_slice()));
    assert_eq!(host.with_update(|_| ()), Err(Fault::Poisoned));
}

#[test]
fn capacity_refusal_removal_retirement_and_later_allocation() {
    let mut host = Host::default();
    host.with_update(|mut frame| {
        for value in 0..MAX_VALUES as u64 {
            frame.push(value).unwrap();
        }
        let before = frame.values().unwrap().to_vec();
        assert_eq!(frame.push(100), Err(Refusal::Capacity));
        assert_eq!(frame.values().unwrap(), before);
        assert_eq!(frame.pop(), Ok(Some(MAX_VALUES as u64 - 1)));
        frame.push(200).unwrap();
        let ticket = frame.detach().unwrap();
        frame.retire(ticket);
        assert!(frame.values().unwrap().is_empty());
        assert_eq!(frame.pop(), Ok(None));
        frame.push(300).unwrap();
    })
    .unwrap();
    assert_eq!(
        host.with_update(|frame| frame.values().unwrap().to_vec()),
        Ok(vec![300])
    );
}

#[test]
fn transient_copy_can_escape_without_retaining_an_owner_or_borrow() {
    let mut host = Host::default();
    let mut copy = host
        .with_update(|mut frame| {
            frame.push(10).unwrap();
            frame.values().unwrap().to_vec()
        })
        .unwrap();
    copy[0] = 11;
    assert_eq!(
        host.with_update(|frame| frame.values().unwrap().to_vec()),
        Ok(vec![10])
    );
}
