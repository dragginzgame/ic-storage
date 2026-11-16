use super::*;
use alloc::vec;

fn reseal(bytes: &mut [u8]) {
    let end = bytes.len() - DIGEST;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}

#[test]
fn v2_bytes_preserve_full_width_and_require_explicit_narrowing() {
    let source = [V2::new(u64::from(u32::MAX), u64::MAX).unwrap()];
    let mut expected = b"RECVEC01RECU64V1".to_vec();
    expected.extend_from_slice(&2u32.to_le_bytes());
    expected.extend_from_slice(&1u32.to_le_bytes());
    expected.extend_from_slice(&7u64.to_le_bytes());
    expected.extend_from_slice(&u64::from(u32::MAX).to_le_bytes());
    expected.extend_from_slice(&u64::MAX.to_le_bytes());
    let digest = Sha256::digest(&expected);
    expected.extend_from_slice(&digest);
    let bytes = encode_v2(&source, 7).unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(decode_v2(&bytes, 7).unwrap().0[0], Some(source[0]));
    assert_eq!(decode(&bytes, 7), Err(Error::Schema));
    let (converted, count) = decode_narrow_v2(&bytes, 7).unwrap();
    assert_eq!(count, 1);
    let target = converted[0].unwrap();
    assert_eq!((target.counter(), target.ceiling()), (u32::MAX, u64::MAX));
    let current = encode(&[target], 8).unwrap();
    assert_eq!(decode_narrow_v2(&current, 8), Err(Error::Schema));
    assert_eq!(decode_v2(&current, 8), Err(Error::Schema));
}

#[test]
fn ordered_conversion_is_all_or_nothing_at_every_position() {
    for count in [0, 1, LIMIT] {
        let source: Vec<_> = (0..count)
            .map(|i| V2::new(i as u64, 100 + i as u64).unwrap())
            .collect();
        let bytes = encode_v2(&source, 9).unwrap();
        let (records, actual) = decode_narrow_v2(&bytes, 9).unwrap();
        assert_eq!(actual, count);
        for (input, output) in source.iter().zip(records) {
            assert_eq!(V2::from(output.unwrap()), *input);
        }
        assert_eq!(decode_narrow_v2(&bytes, 10), Err(Error::Epoch));
    }
    for index in 0..LIMIT {
        for counter in [u64::from(u32::MAX) + 1, u64::MAX] {
            let mut source = vec![V2::new(7, u64::MAX).unwrap(); LIMIT];
            source[index] = V2::new(counter, u64::MAX).unwrap();
            let bytes = encode_v2(&source, 11).unwrap();
            assert_eq!(
                decode_narrow_v2(&bytes, 11),
                Err(Error::Conversion {
                    index,
                    cause: narrow::Error::Range
                })
            );
            assert_eq!(decode_v2(&bytes, 11).unwrap().0[index], Some(source[index]));
        }
    }
}

#[test]
fn v2_framing_and_all_invariants_precede_conversion() {
    let bytes = encode_v2(&[V2::new(u64::MAX, u64::MAX).unwrap(); LIMIT], 7).unwrap();
    assert_eq!(bytes.len(), MAX_BYTES);
    for end in 0..bytes.len() {
        assert!(decode_narrow_v2(&bytes[..end], 7).is_err());
    }
    let mut invalid = bytes.clone();
    let last_ceiling = HEADER + (LIMIT - 1) * V2::STORE_BYTES + 8;
    invalid[last_ceiling..last_ceiling + 8].fill(0);
    reseal(&mut invalid);
    // A late invalid source record wins over an earlier valid-but-too-wide one:
    // the entire predecessor must decode before conversion can begin.
    assert_eq!(
        decode_narrow_v2(&invalid, 7),
        Err(Error::Record(CodecError::Invariant))
    );
    let mut wrong_schema = bytes.clone();
    wrong_schema[16] = 255;
    reseal(&mut wrong_schema);
    assert_eq!(decode_narrow_v2(&wrong_schema, 7), Err(Error::Schema));
    let mut corrupt = bytes;
    corrupt[HEADER] ^= 1;
    assert_eq!(decode_narrow_v2(&corrupt, 7), Err(Error::Integrity));
    assert_eq!(
        encode_v2(&[V2::new(1, 2).unwrap(); LIMIT + 1], 7),
        Err(Error::Limit)
    );
}

#[test]
fn widening_preserves_order_width_boundaries_and_unused_slots() {
    for count in [0, 1, LIMIT] {
        let source: Vec<_> = (0..count)
            .map(|i| V3::new(u32::MAX - i as u32, u64::MAX - i as u64).unwrap())
            .collect();
        let bytes = encode(&source, 19).unwrap();
        let (records, actual) = decode_widen_v3(&bytes, 19).unwrap();
        assert_eq!(actual, count);
        assert!(records[count..].iter().all(Option::is_none));
        let target: Vec<_> = records[..count].iter().map(|v| v.unwrap()).collect();
        for (old, new) in source.iter().zip(&target) {
            assert_eq!(new.counter(), u64::from(old.counter()));
            assert_eq!(new.ceiling(), old.ceiling());
        }
        assert_eq!(decode_v2(&bytes, 19), Err(Error::Schema));
        assert_eq!(decode_widen_v3(&bytes, 20), Err(Error::Epoch));
        let current = encode_v2(&target, 20).unwrap();
        assert_eq!(decode_widen_v3(&current, 20), Err(Error::Schema));
        assert_eq!(
            decode_narrow_v2(&current, 20).unwrap().0[..count],
            records_v3(&source)[..count]
        );
    }
}

fn records_v3(source: &[V3]) -> [Option<V3>; LIMIT] {
    let mut records = [None; LIMIT];
    for (slot, value) in records.iter_mut().zip(source) {
        *slot = Some(*value);
    }
    records
}

#[test]
fn widening_refuses_corrupt_or_invalid_predecessors_before_returning_records() {
    let bytes = encode(&[V3::new(u32::MAX, u64::MAX).unwrap(); LIMIT], 9).unwrap();
    for end in 0..bytes.len() {
        assert!(decode_widen_v3(&bytes[..end], 9).is_err());
    }
    for index in 0..LIMIT {
        let mut invalid = bytes.clone();
        let ceiling = HEADER + index * V3::STORE_BYTES + 4;
        invalid[ceiling..ceiling + 8].fill(0);
        reseal(&mut invalid);
        assert_eq!(
            decode_widen_v3(&invalid, 9),
            Err(Error::Record(CodecError::Invariant))
        );
    }
    let mut corrupt = bytes.clone();
    corrupt[HEADER] ^= 1;
    assert_eq!(decode_widen_v3(&corrupt, 9), Err(Error::Integrity));
    let mut wrong_count = bytes;
    wrong_count[20..24].copy_from_slice(&((LIMIT + 1) as u32).to_le_bytes());
    reseal(&mut wrong_count);
    assert_eq!(decode_widen_v3(&wrong_count, 9), Err(Error::Limit));
    let tight = encode(&[V3::new(7, 7).unwrap()], 10).unwrap();
    let mut record = decode_widen_v3(&tight, 10).unwrap().0[0].unwrap();
    assert!(record.add(1).is_err());
    assert_eq!((record.counter(), record.ceiling()), (7, 7));
}
