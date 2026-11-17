use super::*;
use crate::arena::record_elements::{encode, import};
use ic_storage::record::Scalar;

fn parked(arena: &Arena, length: usize, capacity: usize) -> Parked<'_> {
    let mut values = Vec::new_in(arena);
    values.try_reserve_exact(capacity).unwrap();
    values.extend((0..length).map(|i| V3::new(i as u32, 100).unwrap()));
    Parked::new(values)
}

#[test]
fn same_domain_record_reopen_preserves_allocation_and_continues_mutating() {
    for (length, capacity) in [(0, 0), (0, 4), (3, 8), (16, 16)] {
        let arena = Arena::new();
        let available = arena.reusable_bytes();
        let token = parked(&arena, length, capacity);
        let descriptor = token.descriptor();
        let before = arena.ledger.get();
        let mut values = token.reopen(descriptor).unwrap();
        assert_eq!(values.len(), length);
        assert_eq!(values.capacity(), capacity);
        assert_eq!(arena.ledger.get(), before);
        if capacity != 0 {
            assert_eq!(
                values.as_ptr().addr(),
                arena.base().addr() + descriptor.start * WORD_BYTES
            );
        }
        for (index, value) in values.iter_mut().enumerate() {
            assert_eq!(value.counter(), index as u32);
            value.store_with(|mut view| view.add(1)).unwrap();
        }
        if length == 16 {
            // Default growth needs old and new blocks to coexist. The 512-byte
            // arena cannot hold both 16 and 17 records; refusal preserves custody.
            let before = arena.ledger.get();
            assert!(values.try_reserve_exact(1).is_err());
            assert_eq!(arena.ledger.get(), before);
            assert_eq!(values.len(), 16);
            values.remove(0);
        }
        values.try_reserve_exact(1).unwrap();
        values.push(V3::new(42, 100).unwrap());
        let checkpoint = encode(&values, 7).unwrap();
        let token = Parked::new(values);
        let descriptor = token.descriptor();
        let mut reopened = token.reopen(descriptor).unwrap();
        assert_eq!(encode(&reopened, 7).unwrap(), checkpoint);
        reopened.remove(0);
        reopened.shrink_to_fit();
        drop(reopened);
        assert_eq!(arena.reusable_bytes(), available);
    }
}

#[test]
fn altered_schema_layout_or_initialized_extent_cannot_use_the_live_witness() {
    let mutations: &[fn(&mut Descriptor)] = &[
        |d| d.contract.schema[0] ^= 1,
        |d| d.contract.version = 2,
        |d| d.contract.size = 8,
        |d| d.contract.align = 4,
        |d| d.contract.fields[0].id = 9,
        |d| d.contract.fields[0].offset = 8,
        |d| d.contract.fields[0].scalar = Scalar::U64,
        |d| {
            d.contract.version = 2;
            d.contract.fields[0].scalar = Scalar::U64;
        },
        |d| d.start += 1,
        |d| d.generation += 1,
        |d| d.length -= 1,
        |d| d.length = d.capacity,
        |d| d.length = usize::MAX,
        |d| d.capacity += 1,
        |d| d.capacity = usize::MAX,
    ];
    for mutate in mutations {
        let arena = Arena::new();
        let token = parked(&arena, 3, 8);
        let mut descriptor = token.descriptor();
        mutate(&mut descriptor);
        let expected = if descriptor.contract != Contract::V3 {
            Error::Schema
        } else {
            Error::Root
        };
        let before = arena.ledger.get();
        let available = arena.reusable_bytes();
        assert_eq!(token.reopen(descriptor), Err(expected));
        assert_eq!(arena.ledger.get(), before);
        assert_eq!(arena.reusable_bytes(), available);
    }
}

#[test]
fn empty_vector_metadata_is_admitted_instead_of_silently_reset() {
    for mutate in [
        (|d: &mut Descriptor| d.length = 1) as fn(&mut Descriptor),
        |d| d.capacity = 1,
        |d| d.contract.version = 99,
    ] {
        let arena = Arena::new();
        let token = parked(&arena, 0, 0);
        let mut descriptor = token.descriptor();
        mutate(&mut descriptor);
        assert!(token.reopen(descriptor).is_err());
        assert_eq!(arena.reusable_bytes(), crate::arena::WORDS * WORD_BYTES);
    }
}

#[test]
fn another_live_record_allocation_cannot_substitute_for_the_original_root() {
    let arena = Arena::new();
    let first = parked(&arena, 2, 4);
    let second = parked(&arena, 2, 4);
    let other = second.descriptor();
    let before = arena.ledger.get();
    assert_eq!(first.reopen(other), Err(Error::Root));
    assert_eq!(arena.ledger.get(), before);
    let values = second.reopen(other).unwrap();
    assert_eq!(values.len(), 2);
    drop(values);
    // The refused first token leaves its run unavailable; it cannot be inferred free.
    assert_eq!(
        arena.reusable_bytes(),
        crate::arena::WORDS * WORD_BYTES - 4 * V3::STORE_SIZE
    );
}

#[test]
fn suspended_or_changed_ledger_refuses_without_issuing_a_record_owner() {
    for fault in 0..5 {
        let arena = Arena::new();
        let token = parked(&arena, 2, 4);
        let descriptor = token.descriptor();
        let mut ledger = arena.ledger.get();
        match fault {
            0 => arena.suspended.set(true),
            1 => ledger.occupied[0] = false,
            2 => {
                ledger.blocks[0].as_mut().unwrap().generation = ledger.next_generation;
                ledger.next_generation += 1;
            }
            3 => ledger.blocks[0].as_mut().unwrap().parked_len = Some(2),
            4 => ledger.occupied[crate::arena::WORDS - 1] = true,
            _ => unreachable!(),
        }
        arena.ledger.set(ledger);
        let expected = match fault {
            0 => Refusal::Suspended,
            1 | 4 => Refusal::MalformedLedger,
            _ => Refusal::UnknownAllocation,
        };
        assert_eq!(token.reopen(descriptor), Err(Error::Ledger(expected)));
        assert_eq!(arena.ledger.get(), ledger);
    }
}

#[test]
fn u64_reopen_cannot_claim_record_initialization_or_read_padding() {
    let arena = Arena::new();
    let token = parked(&arena, 2, 4);
    let descriptor = token.descriptor();
    // Deliberately forge numeric u64 metadata for the same allocated bytes.
    // Those numbers cannot turn a V3 witness into initialized u64 authority.
    let forged = crate::arena::Parked {
        arena: &arena,
        descriptor: Some(crate::arena::Descriptor {
            start: descriptor.start,
            generation: descriptor.generation,
            length: descriptor.length * 2,
            capacity: descriptor.capacity * 2,
        }),
    };
    let before = arena.ledger.get();
    assert_eq!(forged.reopen(), Err(Refusal::NotParked));
    assert_eq!(arena.ledger.get(), before);
    drop(token.reopen(descriptor).unwrap());
    assert_eq!(arena.reusable_bytes(), crate::arena::WORDS * WORD_BYTES);
}

#[test]
fn forgotten_record_token_keeps_its_run_unavailable() {
    let arena = Arena::new();
    let token = parked(&arena, 2, 4);
    let descriptor = token.descriptor();
    let available = arena.reusable_bytes();
    // This token has no destructor; forgetting it must not release its run.
    #[expect(
        clippy::forget_non_drop,
        reason = "exercise lost record custody without cleanup"
    )]
    core::mem::forget(token);
    let next = parked(&arena, 2, 4);
    assert_ne!(next.descriptor().start, descriptor.start);
    let next_descriptor = next.descriptor();
    drop(next.reopen(next_descriptor).unwrap());
    assert_eq!(arena.reusable_bytes(), available);
}

#[test]
fn portable_recovery_remains_available_after_native_descriptor_refusal_and_teardown() {
    let bytes = {
        let arena = Arena::new();
        let mut values = Vec::new_in(&arena);
        values.extend([V3::new(7, 100).unwrap(), V3::new(8, 200).unwrap()]);
        let bytes = encode(&values, 9).unwrap();
        let token = Parked::new(values);
        let mut incompatible = token.descriptor();
        incompatible.contract.version = 2;
        assert_eq!(token.reopen(incompatible), Err(Error::Schema));
        bytes
    };
    let arena = Arena::new();
    let mut restored = import(&bytes, 9, &arena, None).unwrap();
    restored[0].store_with(|mut view| view.add(1)).unwrap();
    restored.push(V3::new(9, 300).unwrap());
    assert_eq!(
        restored
            .iter()
            .map(|record| (record.counter(), record.ceiling()))
            .collect::<Vec<_>>(),
        [(8, 100), (8, 200), (9, 300)]
    );
    drop(restored);
    assert_eq!(arena.reusable_bytes(), crate::arena::WORDS * WORD_BYTES);
}
