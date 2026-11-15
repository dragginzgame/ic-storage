use super::{DIGEST, Error, HEADER, IMAGE_BYTES, SLOT, decode, encode, put};
use crate::arena::{Arena, WORDS};
use sha2::{Digest, Sha256};

fn reseal(bytes: &mut [u8]) {
    let digest = Sha256::digest(&bytes[..DIGEST]);
    bytes[DIGEST..].copy_from_slice(&digest);
}

fn one_block() -> [u8; IMAGE_BYTES] {
    let arena = Arena::new();
    let mut values: Vec<u64, _> = Vec::new_in(&arena);
    values.try_reserve_exact(4).unwrap();
    values.push(42);
    let _parked = Arena::park(values);
    encode(&arena.ledger.get()).unwrap()
}

#[test]
fn metadata_bytes_have_an_independent_golden_digest() {
    let bytes = one_block();
    // Independently assembled with Perl pack(Q<*) and sha256sum; see the format.
    let digest = bytes[DIGEST..]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        digest,
        "e996a6efadf614041430e1af6ec7fb563a4698b66ce66967dff0e66ab324f8b0"
    );
    assert_eq!(&bytes[..8], b"ICNLED01");
    let ledger = decode(&bytes).unwrap();
    assert_eq!(ledger.next_generation, 2);
    let block = ledger.blocks[0].unwrap();
    assert_eq!(
        (block.start, block.words, block.generation, block.parked_len),
        (0, 4, 1, Some(1))
    );
    assert!(ledger.blocks[1..].iter().all(Option::is_none));
    assert_eq!(encode(&ledger).unwrap(), bytes);
}

#[test]
fn every_truncated_prefix_trailing_byte_and_single_byte_corruption_refuses() {
    let bytes = one_block();
    for end in 0..bytes.len() {
        assert_eq!(decode(&bytes[..end]), Err(Error::Length), "prefix {end}");
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(decode(&trailing), Err(Error::Length));
    for offset in 0..bytes.len() {
        let mut corrupt = bytes;
        corrupt[offset] ^= 1;
        assert!(decode(&corrupt).is_err(), "byte {offset}");
    }
}

#[test]
fn resealed_unknown_formats_noncanonical_slots_and_invalid_extents_refuse() {
    for (offset, value, expected) in [
        (0, 0, Error::Format),
        (8, 2, Error::Format),
        (16, 65, Error::Format),
        (24, 0, Error::Ledger),
        (24, 1, Error::Ledger),
        (HEADER, 2, Error::Encoding),
        (HEADER, 0, Error::Encoding),
        (HEADER + 8, u64::MAX, Error::Ledger),
        (HEADER + 8, 64, Error::Ledger),
        (HEADER + 16, u64::MAX, Error::Ledger),
        (HEADER + 16, 0, Error::Ledger),
        (HEADER + 16, 65, Error::Ledger),
        (HEADER + 24, 0, Error::Ledger),
        (HEADER + 24, 2, Error::Ledger),
        (HEADER + 32, 5, Error::Ledger),
        (HEADER + SLOT + 8, 1, Error::Encoding),
    ] {
        let mut bytes = one_block();
        put(&mut bytes, offset, value);
        reseal(&mut bytes);
        assert_eq!(
            decode(&bytes),
            Err(expected),
            "offset {offset}, value {value}"
        );
    }
    // Two individually valid records still cannot alias a run or generation.
    for (start, generation) in [(2, 2), (4, 1)] {
        let mut bytes = one_block();
        put(&mut bytes, 24, 3);
        for (field, value) in [1, start, 4, generation, 1].into_iter().enumerate() {
            put(&mut bytes, HEADER + SLOT + field * 8, value);
        }
        reseal(&mut bytes);
        assert_eq!(decode(&bytes), Err(Error::Ledger));
    }
}

#[test]
fn export_refuses_active_owners_and_does_not_mutate_the_source() {
    let arena = Arena::new();
    let mut values: Vec<u64, _> = Vec::new_in(&arena);
    values.try_reserve_exact(4).unwrap();
    values.push(42);
    let before = arena.ledger.get();
    assert_eq!(encode(&before), Err(Error::ActiveOwner));
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(values.as_slice(), [42]);
    let parked = Arena::park(values);
    let before = arena.ledger.get();
    assert_eq!(decode(&encode(&before).unwrap()).unwrap(), before);
    assert_eq!(arena.ledger.get(), before);
    // The original token, not the decoded image, authorizes the actual reopen.
    assert_eq!(parked.reopen().unwrap().as_slice(), [42]);
}

#[test]
fn sparse_slots_and_full_domain_rebuild_exact_occupancy() {
    let arena = Arena::new();
    assert_eq!(
        decode(&encode(&arena.ledger.get()).unwrap()).unwrap(),
        arena.ledger.get()
    );
    let mut first: Vec<u64, _> = Vec::new_in(&arena);
    first.try_reserve_exact(2).unwrap();
    let mut last: Vec<u64, _> = Vec::new_in(&arena);
    last.try_reserve_exact(WORDS - 2).unwrap();
    last.push(u64::MAX);
    drop(first);
    let parked = Arena::park(last);
    let before = arena.ledger.get();
    assert_eq!(decode(&encode(&before).unwrap()).unwrap(), before);
    drop(parked.reopen().unwrap());
    let mut full: Vec<u64, _> = Vec::new_in(&arena);
    full.try_reserve_exact(WORDS).unwrap();
    let _parked = Arena::park(full); // Nonzero capacity, zero initialized elements.
    let before = arena.ledger.get();
    assert_eq!(decode(&encode(&before).unwrap()).unwrap(), before);
}

#[test]
fn corrupt_source_metadata_is_not_serialized_as_a_valid_image() {
    let mut ledger = decode(&one_block()).unwrap();
    ledger.occupied[0] = false;
    assert_eq!(encode(&ledger), Err(Error::Ledger));
}
