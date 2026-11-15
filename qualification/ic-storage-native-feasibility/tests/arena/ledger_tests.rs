//! Contradictory allocation metadata must refuse before raw-parts construction.

use super::{Arena, Ledger, Parked, Refusal, WORDS};

fn parked(arena: &Arena, value: u64) -> Parked<'_> {
    let mut values = Vec::new_in(arena);
    values.try_reserve_exact(4).unwrap();
    values.push(value);
    Arena::park(values)
}

#[test]
fn corrupted_ledger_refuses_without_changing_metadata() {
    // Both owners are parked before fault injection; no live Vec is dropped
    // through corrupt bookkeeping. Failed admission leaves this arena unusable.
    let cases: &[fn(&mut Ledger)] = &[
        |l| l.blocks[0].as_mut().unwrap().start = usize::MAX,
        |l| l.blocks[0].as_mut().unwrap().words = usize::MAX,
        |l| l.blocks[0].as_mut().unwrap().start = WORDS,
        |l| l.blocks[0].as_mut().unwrap().words = 0,
        |l| l.blocks[0].as_mut().unwrap().parked_len = Some(5),
        |l| l.blocks[0].as_mut().unwrap().generation = 0,
        |l| l.blocks[0].as_mut().unwrap().generation = l.next_generation,
        |l| l.blocks[1].as_mut().unwrap().generation = l.blocks[0].unwrap().generation,
        |l| l.blocks[1].as_mut().unwrap().start = 2,
        |l| l.blocks[1] = l.blocks[0],
        |l| l.occupied[0] = false,
        |l| l.occupied[WORDS - 1] = true,
        |l| l.blocks[1] = None,
        |l| l.next_generation = 0,
    ];
    for (index, corrupt) in cases.iter().enumerate() {
        let arena = Arena::new();
        let first = parked(&arena, 42);
        let _second = parked(&arena, 99);
        let mut ledger = arena.ledger.get();
        corrupt(&mut ledger);
        arena.ledger.set(ledger);
        assert_eq!(
            first.reopen().unwrap_err(),
            Refusal::MalformedLedger,
            "case {index}"
        );
        assert_eq!(arena.ledger.get(), ledger, "case {index}");
    }
}

#[test]
fn unrelated_corrupt_block_also_blocks_an_empty_owner() {
    let arena = Arena::new();
    let empty = Arena::park(Vec::new_in(&arena));
    let _other = parked(&arena, 7);
    let mut ledger = arena.ledger.get();
    ledger.blocks[0].as_mut().unwrap().parked_len = Some(usize::MAX);
    arena.ledger.set(ledger);
    assert_eq!(empty.reopen().unwrap_err(), Refusal::MalformedLedger);
    assert_eq!(arena.ledger.get(), ledger);
}

#[test]
fn admission_accepts_live_siblings_holes_and_exact_domain_end() {
    let arena = Arena::new();
    let first = parked(&arena, 42);
    let gap = parked(&arena, 99).reopen().unwrap();
    let mut last: Vec<u64, _> = Vec::new_in(&arena);
    last.try_reserve_exact(WORDS - 8).unwrap();
    last.extend([7, 11]);
    drop(gap);
    arena.ledger.get().validate().unwrap();
    let first = first.reopen().unwrap();
    assert_eq!(first.as_slice(), [42]);
    assert_eq!(last.as_slice(), [7, 11]);
    drop(first);
    drop(last);
    arena.ledger.get().validate().unwrap();
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn exhausted_generation_counter_does_not_invalidate_existing_owners() {
    let arena = Arena::new();
    let first = parked(&arena, 42);
    let mut ledger = arena.ledger.get();
    ledger.next_generation = u64::MAX;
    arena.ledger.set(ledger);
    let mut first = first.reopen().unwrap();
    assert!(first.try_reserve_exact(4).is_err());
    assert_eq!(first.as_slice(), [42]);
    drop(first);
    arena.ledger.get().validate().unwrap();
}
