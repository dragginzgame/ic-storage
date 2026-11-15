use super::{Checkpoint, Error, MAX_CHECKPOINT_BYTES, MAX_VALUES, decode, encode};
use sha2::{Digest, Sha256};

fn reseal(bytes: &mut [u8]) {
    let offset = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..offset]);
    bytes[offset..].copy_from_slice(&digest);
}

#[test]
fn golden_bytes_match_independently_assembled_vector() {
    let hex = include_str!("golden.hex");
    let bytes: Vec<u8> = hex
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    let values = [1, 0x0102_0304_0506_0708, u64::MAX];
    assert_eq!(encode(42, &values, 7).unwrap(), bytes);
    assert_eq!(
        decode(&bytes, 7),
        Ok(Checkpoint {
            counter: 42,
            source_epoch: 7,
            values: values.to_vec()
        })
    );
}

#[test]
fn empty_and_maximum_round_trip_with_integer_boundaries() {
    for count in [0, MAX_VALUES] {
        let values: Vec<_> = (0..count)
            .map(|i| if i % 2 == 0 { 0 } else { u64::MAX })
            .collect();
        let bytes = encode(u64::MAX, &values, 0).unwrap();
        assert_eq!(bytes.len(), 120 + count * 8);
        assert_eq!(
            decode(&bytes, 0),
            Ok(Checkpoint {
                counter: u64::MAX,
                source_epoch: 0,
                values
            })
        );
    }
    assert_eq!(encode(0, &[0; MAX_VALUES + 1], 0), Err(Error::Limit));
}

#[test]
fn every_truncated_prefix_trailing_data_and_oversize_refuse() {
    let bytes = encode(42, &[1, 2, 3], 7).unwrap();
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end], 7).is_err(), "accepted prefix {end}");
    }
    let mut extra = bytes;
    extra.push(0);
    assert_eq!(decode(&extra, 7), Err(Error::Framing));
    assert_eq!(decode(&[0; MAX_CHECKPOINT_BYTES + 1], 7), Err(Error::Limit));
}

#[test]
fn resealed_unknown_and_inconsistent_headers_still_refuse() {
    for (offset, value, expected) in [
        (0, 0, Error::Format),
        (8, 3, Error::Format),
        (10, 1, Error::Framing),
        (12, 79, Error::Framing),
        (16, 0, Error::Schema),
        (24, 2, Error::Schema),
        (28, 17, Error::Limit),
        (40, 2, Error::Ownership),
        (48, 1, Error::Ownership),
        (64, 0, Error::Framing),
        (72, 1, Error::Framing),
    ] {
        let mut bytes = encode(42, &[1, 2, 3], 7).unwrap();
        bytes[offset] = value;
        reseal(&mut bytes);
        assert_eq!(decode(&bytes, 7), Err(expected), "offset {offset}");
    }
}

mod v2;

#[test]
fn corruption_seal_and_epoch_mismatch_refuse() {
    for offset in [32, 56, 80, 112, 143] {
        let mut bytes = encode(42, &[1, 2, 3], 7).unwrap();
        bytes[offset] ^= 1;
        assert_eq!(decode(&bytes, 7), Err(Error::Integrity), "offset {offset}");
    }
    let mut unsealed = encode(42, &[1, 2, 3], 7).unwrap();
    unsealed[111] ^= 1;
    reseal(&mut unsealed);
    assert_eq!(decode(&unsealed, 7), Err(Error::Framing));
    let bytes = encode(42, &[1], 7).unwrap();
    assert_eq!(decode(&bytes, 8), Err(Error::Epoch));
    let mut changed = bytes;
    changed[32..40].copy_from_slice(&8u64.to_le_bytes());
    reseal(&mut changed);
    assert_eq!(decode(&changed, 7), Err(Error::Epoch));
}
