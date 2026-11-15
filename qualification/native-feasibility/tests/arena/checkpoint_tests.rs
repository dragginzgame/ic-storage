//! Candidate reconstruction from logical bytes after source-domain teardown.

use std::mem::offset_of;

use ic_storage_native_feasibility::checkpoint::{self, Error};

use super::{Arena, WORDS};

#[repr(C)]
struct SourceRecord<'a> {
    counter: u64,
    values: Vec<u64, &'a Arena>,
}

#[repr(C)]
struct TargetRecord<'a> {
    values: Vec<u64, &'a Arena>,
    counter: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum ImportError {
    Checkpoint(Error),
    Allocation,
}

fn import_candidate<'a>(
    bytes: &[u8],
    epoch: u64,
    arena: &'a Arena,
) -> Result<TargetRecord<'a>, ImportError> {
    let decoded = checkpoint::decode(bytes, epoch).map_err(ImportError::Checkpoint)?;
    let mut values = Vec::new_in(arena);
    values
        .try_reserve_exact(decoded.values.len())
        .map_err(|_| ImportError::Allocation)?;
    for value in decoded.values {
        values.push(value);
    }
    Ok(TargetRecord {
        counter: decoded.counter,
        values,
    })
}

fn replace_root<'a>(
    root: &mut TargetRecord<'a>,
    bytes: &[u8],
    epoch: u64,
    arena: &'a Arena,
) -> Result<(), ImportError> {
    let candidate = import_candidate(bytes, epoch, arena)?;
    *root = candidate;
    Ok(())
}

#[test]
fn checkpoint_survives_source_teardown_and_reconstructs_a_different_record_layout() {
    let bytes = {
        let arena = Arena::new();
        let mut source = SourceRecord {
            counter: 42,
            values: Vec::new_in(&arena),
        };
        source.values.try_reserve_exact(16).unwrap();
        source.values.extend([1, 0x0102_0304_0506_0708, u64::MAX]);
        let bytes = checkpoint::encode(source.counter, &source.values, 7).unwrap();
        drop(source);
        drop(arena);
        bytes
    };
    // Only the logical bytes cross the boundary; the old arena no longer exists.
    let arena = Arena::new();
    assert_ne!(
        offset_of!(SourceRecord<'_>, counter),
        offset_of!(TargetRecord<'_>, counter)
    );
    let mut target = import_candidate(&bytes, 7, &arena).unwrap();
    assert_eq!(target.counter, 42);
    assert_eq!(
        target.values.as_slice(),
        [1, 0x0102_0304_0506_0708, u64::MAX]
    );
    target.values.try_reserve_exact(5).unwrap();
    target.values.push(13);
    assert_eq!(target.values.remove(1), 0x0102_0304_0506_0708);
    let round_trip = checkpoint::encode(target.counter, &target.values, 8).unwrap();
    assert_eq!(
        checkpoint::decode(&round_trip, 8).unwrap().values,
        [1, u64::MAX, 13]
    );
    drop(target);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn corrupt_or_unallocatable_candidate_never_replaces_the_existing_root() {
    let arena = Arena::new();
    let mut root = TargetRecord {
        counter: 99,
        values: Vec::new_in(&arena),
    };
    root.values.try_reserve_exact(2).unwrap();
    root.values.extend([10, 11]);
    let bytes = checkpoint::encode(42, &[1, 2, 3], 7).unwrap();
    let mut corrupt = bytes.clone();
    corrupt[80] ^= 1;
    let before = arena.ledger.get();
    assert_eq!(
        replace_root(&mut root, &corrupt, 7, &arena),
        Err(ImportError::Checkpoint(Error::Integrity))
    );
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(root.counter, 99);
    assert_eq!(root.values.as_slice(), [10, 11]);

    let mut blocker: Vec<u64, _> = Vec::new_in(&arena);
    blocker.try_reserve_exact(WORDS - 2).unwrap();
    let before = arena.ledger.get();
    assert_eq!(
        replace_root(&mut root, &bytes, 7, &arena),
        Err(ImportError::Allocation)
    );
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(root.counter, 99);
    assert_eq!(root.values.as_slice(), [10, 11]);
    drop(blocker);
    replace_root(&mut root, &bytes, 7, &arena).unwrap();
    assert_eq!(root.counter, 42);
    assert_eq!(root.values.as_slice(), [1, 2, 3]);
    drop(root);
    assert_eq!(arena.reusable_bytes(), WORDS * 8);
}
