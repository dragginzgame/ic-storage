//! Candidate reconstruction from logical bytes after source-domain teardown.

use std::mem::offset_of;

use ic_storage_native_feasibility::checkpoint::{self, Error};

use super::{
    Allocator, Arena, Layout, WORDS,
    suspension::Suspension,
    transfer::{self, Transfer},
};

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

fn freeze(values: Vec<u64, &Arena>) -> Suspension<'_> {
    Suspension::new(Arena::park(values))
        .unwrap_or_else(|(error, _)| panic!("fixture suspension failed: {error:?}"))
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

#[test]
fn checkpoint_recovers_after_frozen_source_and_its_resume_authority_are_lost() {
    let bytes = {
        let source_arena = Arena::new();
        let mut source = SourceRecord {
            counter: 42,
            values: Vec::new_in(&source_arena),
        };
        source.values.try_reserve_exact(16).unwrap();
        source.values.extend([1, 2, u64::MAX]);
        // Exclusive source access, no callbacks or mutation between export and freeze.
        let bytes = checkpoint::encode(source.counter, &source.values, 7).unwrap();
        let _frozen = freeze(source.values);
        assert!(source_arena.suspended.get());
        assert!(source_arena.allocate(Layout::new::<u64>()).is_err());
        assert_eq!(source_arena.reusable_bytes(), (WORDS - 16) * 8);
        // Both the suspension token and original Box end here. Neither is given
        // to the importer; the source is never resumed or inspected for recovery.
        bytes
    };
    let target_arena = Arena::new();
    let mut target = import_candidate(&bytes, 7, &target_arena).unwrap();
    assert_eq!(target.counter, 42);
    assert_eq!(target.values.as_slice(), [1, 2, u64::MAX]);
    target.values.push(9);
    assert_eq!(target.values.remove(1), 2);
    target.values.shrink_to_fit();
    assert_eq!(target.values.as_slice(), [1, u64::MAX, 9]);
    drop(target);
    assert_eq!(target_arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn failed_imports_preserve_frozen_source_existing_target_and_retryable_checkpoint() {
    let source_arena = Arena::new();
    let mut values = Vec::new_in(&source_arena);
    values.extend([1, 2, 3]);
    let bytes = checkpoint::encode(42, &values, 7).unwrap();
    let original_bytes = bytes.clone();
    let _frozen = freeze(values);
    let source_ledger = source_arena.ledger.get();

    let target_arena = Arena::new();
    let mut target = import_candidate(
        &checkpoint::encode(99, &[10, 11], 6).unwrap(),
        6,
        &target_arena,
    )
    .unwrap();
    let target_address = target.values.as_ptr();
    let mut damaged = bytes.clone();
    damaged[80] ^= 1;
    for (input, epoch, expected) in [
        (damaged.as_slice(), 7, Error::Integrity),
        (&bytes[..bytes.len() - 1], 7, Error::Framing),
        (bytes.as_slice(), 8, Error::Epoch),
    ] {
        let before = target_arena.ledger.get();
        assert_eq!(
            replace_root(&mut target, input, epoch, &target_arena),
            Err(ImportError::Checkpoint(expected))
        );
        assert_eq!(target_arena.ledger.get(), before);
        assert_eq!(target.values.as_ptr(), target_address);
        assert_eq!(target.counter, 99);
        assert_eq!(target.values.as_slice(), [10, 11]);
        assert!(source_arena.suspended.get());
        assert_eq!(source_arena.ledger.get(), source_ledger);
    }
    let mut blocker: Vec<u64, _> = Vec::new_in(&target_arena);
    blocker.try_reserve_exact(WORDS - 2).unwrap();
    let before = target_arena.ledger.get();
    assert_eq!(
        replace_root(&mut target, &bytes, 7, &target_arena),
        Err(ImportError::Allocation)
    );
    assert_eq!(target_arena.ledger.get(), before);
    assert_eq!(target.values.as_ptr(), target_address);
    assert_eq!(target.counter, 99);
    assert_eq!(target.values.as_slice(), [10, 11]);
    drop(blocker);

    replace_root(&mut target, &bytes, 7, &target_arena).unwrap();
    assert_eq!(target.counter, 42);
    assert_eq!(target.values.as_slice(), [1, 2, 3]);
    target.values.push(4);
    assert_eq!(bytes, original_bytes);
    assert!(source_arena.suspended.get());
    assert_eq!(source_arena.ledger.get(), source_ledger);
    drop(target);
    assert_eq!(target_arena.reusable_bytes(), WORDS * 8);
}

#[test]
fn cancellation_restores_source_but_requires_a_new_checkpoint_for_later_writes() {
    let arena = Arena::new();
    let mut source = SourceRecord {
        counter: 42,
        values: Vec::new_in(&arena),
    };
    source.values.extend([1, 2, 3]);
    let address = source.values.as_ptr();
    let old_bytes = checkpoint::encode(source.counter, &source.values, 7).unwrap();
    let mut frozen = freeze(source.values);
    // Cancel before target publication: the original authority is still present.
    source.values = frozen.resume().unwrap();
    assert_eq!(source.values.as_ptr(), address);
    assert_eq!(source.values.as_slice(), [1, 2, 3]);
    assert_eq!(source.counter, 42);
    source.counter += 1;
    source.values[1] = 99;
    source.values.push(4);
    let old = checkpoint::decode(&old_bytes, 7).unwrap();
    assert_eq!(old.counter, 42);
    assert_eq!(old.values, [1, 2, 3]);
    // The coordinator selects the new epoch; bytes cannot revoke themselves.
    let new_bytes = checkpoint::encode(source.counter, &source.values, 8).unwrap();
    let _frozen = freeze(source.values);
    let target_arena = Arena::new();
    assert!(matches!(
        import_candidate(&old_bytes, 8, &target_arena),
        Err(ImportError::Checkpoint(Error::Epoch))
    ));
    assert_eq!(target_arena.reusable_bytes(), WORDS * 8);
    let target = import_candidate(&new_bytes, 8, &target_arena).unwrap();
    assert_eq!(target.counter, 43);
    assert_eq!(target.values.as_slice(), [1, 99, 3, 4]);
}

#[test]
fn export_limit_refusal_leaves_source_live_for_correction_and_retry() {
    let arena = Arena::new();
    let mut source = SourceRecord {
        counter: 42,
        values: Vec::new_in(&arena),
    };
    source.values.try_reserve_exact(17).unwrap();
    source.values.extend(0..17);
    let before = arena.ledger.get();
    let address = source.values.as_ptr();
    assert_eq!(
        checkpoint::encode(source.counter, &source.values, 7),
        Err(Error::Limit)
    );
    assert_eq!(arena.ledger.get(), before);
    assert!(!arena.suspended.get());
    assert_eq!(source.values.as_ptr(), address);
    assert_eq!(source.counter, 42);
    assert!(source.values.iter().copied().eq(0..17));
    assert_eq!(source.values.pop(), Some(16));
    let bytes = checkpoint::encode(source.counter, &source.values, 7).unwrap();
    let _frozen = freeze(source.values);
    let target_arena = Arena::new();
    let target = import_candidate(&bytes, 7, &target_arena).unwrap();
    assert_eq!(target.counter, 42);
    assert!(target.values.iter().copied().eq(0..16));
}

fn checkpointed_transfer() -> (Transfer, Vec<u8>, usize) {
    let arena = Arena::new();
    let mut prefix: Vec<u64, _> = Vec::new_in(&arena);
    prefix.try_reserve_exact(48).unwrap();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(8).unwrap();
    values.extend([1, 0x0102_0304_0506_0708, u64::MAX]);
    drop(prefix);
    let address = values.as_ptr().addr();
    let bytes = checkpoint::encode(42, &values, 7).unwrap();
    let mut frozen = freeze(values);
    frozen.prepare_transfer(42).unwrap();
    let transfer =
        Transfer::take(arena).unwrap_or_else(|(error, _)| panic!("transfer failed: {error:?}"));
    (transfer, bytes, address)
}

#[test]
fn compatible_transfer_preserves_the_exported_logical_record() {
    let (transfer, bytes, address) = checkpointed_transfer();
    let mut destination = transfer
        .destination(WORDS)
        .unwrap_or_else(|(error, _)| panic!("destination failed: {error:?}"));
    let mut record = destination.restore().unwrap();
    assert_eq!(record.values.as_ptr().addr(), address);
    assert_eq!(
        checkpoint::encode(record.counter, &record.values, 7).unwrap(),
        bytes
    );
    record.counter += 1;
    record.values.push(9);
    assert_eq!(record.values.remove(1), 0x0102_0304_0506_0708);
    let next = checkpoint::decode(
        &checkpoint::encode(record.counter, &record.values, 8).unwrap(),
        8,
    )
    .unwrap();
    assert_eq!(next.counter, 43);
    assert_eq!(next.values, [1, u64::MAX, 9]);
}

#[test]
fn incompatible_native_extent_requires_explicit_portable_recovery() {
    let bytes = {
        let (transfer, bytes, _) = checkpointed_transfer();
        // The vector lives at words 48..56. A 32-word controller cannot issue
        // that allocation, although the three logical values fit in a new one.
        let (error, transfer) = transfer.destination(32).err().unwrap();
        assert_eq!(error, transfer::Error::IncompatibleExtent);
        drop(transfer); // Explicitly discard native custody before recovery.
        bytes
    };
    let arena = Arena::with_limit(32).unwrap();
    let mut target =
        import_candidate(&checkpoint::encode(99, &[10, 11], 6).unwrap(), 6, &arena).unwrap();
    let before = arena.ledger.get();
    let mut corrupt = bytes.clone();
    corrupt[80] ^= 1;
    for (input, epoch, expected) in [
        (corrupt.as_slice(), 7, Error::Integrity),
        (bytes.as_slice(), 8, Error::Epoch),
    ] {
        assert_eq!(
            replace_root(&mut target, input, epoch, &arena),
            Err(ImportError::Checkpoint(expected))
        );
        assert_eq!(arena.ledger.get(), before);
        assert_eq!(target.counter, 99);
        assert_eq!(target.values.as_slice(), [10, 11]);
    }
    let mut blocker: Vec<u64, _> = Vec::new_in(&arena);
    blocker.try_reserve_exact(30).unwrap();
    let occupied = arena.ledger.get();
    assert_eq!(
        replace_root(&mut target, &bytes, 7, &arena),
        Err(ImportError::Allocation)
    );
    assert_eq!(arena.ledger.get(), occupied);
    assert_eq!(target.counter, 99);
    assert_eq!(target.values.as_slice(), [10, 11]);
    drop(blocker);
    replace_root(&mut target, &bytes, 7, &arena).unwrap();
    assert_eq!(
        checkpoint::encode(target.counter, &target.values, 7).unwrap(),
        bytes
    );
    target.counter += 1;
    target.values.push(9);
    assert_eq!(target.values.remove(1), 0x0102_0304_0506_0708);
    target.values.shrink_to_fit();
    assert_eq!(target.counter, 43);
    assert_eq!(target.values.as_slice(), [1, u64::MAX, 9]);
    drop(target);
    assert_eq!(arena.reusable_bytes(), 32 * 8);
}
