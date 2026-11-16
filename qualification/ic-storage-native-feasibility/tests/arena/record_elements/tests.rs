use super::*;
use std::mem::{align_of, size_of};

fn value(counter: u32) -> V3 {
    V3::new(counter, u64::MAX).unwrap()
}

#[test]
fn fresh_record_elements_grow_mutate_remove_shrink_and_release_runs() {
    assert_eq!((size_of::<V2>(), align_of::<V2>()), (16, 8));
    assert_eq!((size_of::<V3>(), align_of::<V3>()), (16, 8));
    assert_eq!(V3::STORE_BYTES, 12);
    let arena = Arena::new();
    let available = arena.reusable_bytes();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(2).unwrap();
    values.extend([value(1), value(2)]);
    values.try_reserve_exact(3).unwrap();
    values.extend([value(3), value(4), value(5)]);
    let result = values[0].store_with(|mut view| {
        view.add(7)?;
        view.add(u32::MAX)
    });
    assert_eq!(result, Err(narrow::Error::Overflow));
    assert_eq!(values[0].counter(), 8); // Ordinary Err retains the earlier write.
    assert_eq!(values.remove(1), value(2));
    values.truncate(2);
    values.shrink_to_fit();
    assert_eq!(values.as_slice(), [value(8), value(3)]);
    assert_eq!(arena.reusable_bytes(), available - 2 * size_of::<V3>());
    values.clear();
    values.shrink_to_fit();
    assert_eq!(arena.reusable_bytes(), available);
    values.push(value(9));
    let pointer = values.as_ptr();
    let capacity = values.capacity();
    let mut blocker: Vec<u64, _> = Vec::new_in(&arena);
    blocker
        .try_reserve_exact(arena.reusable_bytes() / 8)
        .unwrap();
    let before = arena.ledger.get();
    assert!(values.try_reserve_exact(capacity).is_err());
    assert_eq!(values.as_ptr(), pointer);
    assert_eq!(values.capacity(), capacity);
    assert_eq!(values.as_slice(), [value(9)]);
    assert_eq!(arena.ledger.get(), before);
    drop(blocker);
    values.try_reserve_exact(capacity).unwrap();
    values.push(value(10));
    drop(values);
    assert_eq!(arena.reusable_bytes(), available);
}

#[test]
fn portable_bytes_use_the_declared_schema_and_logical_fields_without_native_padding() {
    let record = V3::new(0x0403_0201, 0x100f_0e0d_0c0b_0a09).unwrap();
    let mut expected = b"RECVEC01RECU64V1\x03\0\0\0\x01\0\0\0\x07\0\0\0\0\0\0\0".to_vec();
    expected.extend_from_slice(&[1, 2, 3, 4, 9, 10, 11, 12, 13, 14, 15, 16]);
    expected.extend_from_slice(&Sha256::digest(&expected));
    assert_eq!(encode(&[record], 7).unwrap(), expected);
    let arena = Arena::new();
    assert_eq!(
        import(&expected, 7, &arena, None).unwrap().as_slice(),
        [record]
    );
}

#[test]
fn checked_element_conversion_keeps_source_and_target_on_partial_failure() {
    let arena = Arena::new();
    let mut source = Vec::new_in(&arena);
    source.try_reserve_exact(3).unwrap();
    source.extend([
        V2::new(1, 10).unwrap(),
        V2::new(2, 20).unwrap(),
        V2::new(u64::MAX, u64::MAX).unwrap(),
    ]);
    let source_address = source.as_ptr();
    let saved_source = source.to_vec();
    let mut target = Vec::new_in(&arena);
    target.push(value(99));
    let target_address = target.as_ptr();
    let available = arena.reusable_bytes();
    assert_eq!(
        convert(&source, &arena, None),
        Err(Error::Conversion(narrow::Error::Range))
    );
    assert_eq!(convert(&source, &arena, Some(2)), Err(Error::Interrupted));
    assert_eq!(source.as_slice(), saved_source);
    assert_eq!(source.as_ptr(), source_address);
    assert_eq!(target.as_slice(), [value(99)]);
    assert_eq!(target.as_ptr(), target_address);
    assert_eq!(arena.reusable_bytes(), available);
    source[2] = V2::new(u64::from(u32::MAX), u64::MAX).unwrap();
    target = convert(&source, &arena, None).unwrap();
    assert_eq!(
        target
            .iter()
            .map(|v| (v.counter(), v.ceiling()))
            .collect::<Vec<_>>(),
        [(1, 10), (2, 20), (u32::MAX, u64::MAX)]
    );
    assert_eq!(source.as_ptr(), source_address);
    target[0].store_with(|mut view| view.add(1)).unwrap();
    assert_eq!(source[0].counter(), 1);
    drop(source);
    drop(target);
    assert_eq!(arena.reusable_bytes(), super::super::WORDS * 8);
}

#[test]
fn bounded_portable_records_recover_after_source_teardown() {
    for count in [0, 1, LIMIT] {
        let bytes = {
            let arena = Arena::new();
            let mut source = Vec::new_in(&arena);
            source.try_reserve_exact(count).unwrap();
            source.extend((0..count).map(|i| value(i as u32)));
            encode(&source, 7).unwrap()
        }; // The original owner, allocator and backing allocation are gone.
        assert_eq!(bytes.len(), HEADER + count * 12 + DIGEST);
        let arena = Arena::new();
        let mut target = import(&bytes, 7, &arena, None).unwrap();
        assert_eq!(target.len(), count);
        assert_eq!(encode(&target, 7).unwrap(), bytes);
        for (index, record) in target.iter_mut().enumerate() {
            assert_eq!(*record, value(index as u32));
            record.store_with(|mut view| view.add(1)).unwrap();
        }
        target.pop();
        target.push(value(99));
        target.shrink_to_fit();
        let next = encode(&target, 8).unwrap();
        drop(target);
        assert_eq!(arena.reusable_bytes(), super::super::WORDS * 8);
        let restored = import(&next, 8, &arena, None).unwrap();
        assert_eq!(restored.last(), Some(&value(99)));
        drop(restored);
        assert_eq!(arena.reusable_bytes(), super::super::WORDS * 8);
    }
}

fn reseal(bytes: &mut [u8]) {
    let end = bytes.len() - DIGEST;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}

#[test]
fn malformed_record_batches_refuse_before_destination_allocation() {
    let bytes = encode(&[value(1), value(2)], 7).unwrap();
    let arena = Arena::new();
    let before = arena.ledger.get();
    let mut cases = Vec::new();
    for length in 0..bytes.len() {
        cases.push((bytes[..length].to_vec(), Error::Framing));
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    cases.push((trailing, Error::Framing));
    for (offset, byte, expected) in [
        (0, b'X', Error::Framing),
        (8, b'X', Error::Schema),
        (16, 2, Error::Schema),
        (20, 17, Error::Limit),
        (20, 1, Error::Framing),
        (24, 8, Error::Epoch),
    ] {
        let mut invalid = bytes.clone();
        invalid[offset] = byte;
        reseal(&mut invalid);
        cases.push((invalid, expected));
    }
    let mut damaged = bytes.clone();
    damaged[HEADER] ^= 1;
    cases.push((damaged, Error::Integrity));
    let mut invalid = bytes;
    invalid[HEADER + V3::STORE_BYTES + 4..HEADER + 2 * V3::STORE_BYTES].fill(0);
    reseal(&mut invalid); // Last record violates counter <= ceiling.
    cases.push((invalid, Error::Record(CodecError::Invariant)));
    cases.push((vec![0; MAX_BYTES + 1], Error::Limit));
    for (input, expected) in cases {
        assert_eq!(import(&input, 7, &arena, None), Err(expected));
        assert_eq!(arena.ledger.get(), before);
    }
    assert_eq!(encode(&[value(0); LIMIT + 1], 7), Err(Error::Limit));
}

#[test]
fn allocation_and_partial_import_failure_preserve_existing_root_and_allow_retry() {
    let arena = Arena::new();
    let mut root = Vec::new_in(&arena);
    root.try_reserve_exact(1).unwrap();
    root.push(value(99));
    let pointer = root.as_ptr();
    let available = arena.reusable_bytes();
    let bytes = encode(&[value(1), value(2), value(3)], 7).unwrap();
    for index in 0..3 {
        assert_eq!(
            import(&bytes, 7, &arena, Some(index)),
            Err(Error::Interrupted)
        );
        assert_eq!(arena.reusable_bytes(), available);
        assert_eq!(root.as_ptr(), pointer);
        assert_eq!(root.as_slice(), [value(99)]);
    }
    let mut blocker: Vec<u64, _> = Vec::new_in(&arena);
    blocker.try_reserve_exact(available / 8).unwrap();
    let before = arena.ledger.get();
    assert_eq!(import(&bytes, 7, &arena, None), Err(Error::Allocation));
    assert_eq!(arena.ledger.get(), before);
    assert_eq!(root.as_ptr(), pointer);
    assert_eq!(root.as_slice(), [value(99)]);
    drop(blocker);
    root = import(&bytes, 7, &arena, None).unwrap();
    assert_eq!(root.as_slice(), [value(1), value(2), value(3)]);
    drop(root);
    assert_eq!(arena.reusable_bytes(), super::super::WORDS * 8);
}

#[test]
fn forgetting_a_record_owner_does_not_make_its_run_reusable() {
    let arena = Arena::new();
    let mut values = Vec::new_in(&arena);
    values.try_reserve_exact(2).unwrap();
    values.extend([value(1), value(2)]);
    let pointer = values.as_ptr();
    let available = arena.reusable_bytes();
    core::mem::forget(values);
    assert_eq!(arena.reusable_bytes(), available);
    let mut next = Vec::new_in(&arena);
    next.try_reserve_exact(2).unwrap();
    next.push(value(3));
    assert_ne!(next.as_ptr(), pointer);
    drop(next);
    assert_eq!(arena.reusable_bytes(), available);
}
