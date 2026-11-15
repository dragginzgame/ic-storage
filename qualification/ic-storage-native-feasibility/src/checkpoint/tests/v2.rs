use super::{Checkpoint, Error, reseal};
use crate::checkpoint::{MAX_CHECKPOINT_BYTES, V2_MAX_VALUES, decode, encode, encode_v2};

#[test]
fn v2_matches_independent_bytes_and_preserves_v1_writer() {
    let bytes: Vec<_> = include_str!("golden-v2.hex")
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    let values: Vec<_> = (0..17).map(|index| index * 0x0101_0101_0101_0101).collect();
    assert_eq!(encode_v2(42, &values, 7).unwrap(), bytes);
    assert_eq!(
        decode(&bytes, 7),
        Ok(Checkpoint {
            counter: 42,
            source_epoch: 7,
            values
        })
    );
    let old = encode(42, &[1, 2], 7).unwrap();
    let new = encode_v2(42, &[1, 2], 7).unwrap();
    assert_eq!(&old[8..10], &1_u16.to_le_bytes());
    assert_eq!(&new[8..10], &2_u16.to_le_bytes());
    assert_eq!(decode(&old, 7), decode(&new, 7));
    assert_eq!(encode(0, &[0; 17], 0), Err(Error::Limit));
}

#[test]
fn v2_full_extent_and_every_truncation_are_bounded() {
    for count in [0, 16, 17, V2_MAX_VALUES] {
        let values: Vec<_> = (0..count)
            .map(|i| if i % 2 == 0 { 0 } else { u64::MAX })
            .collect();
        let bytes = encode_v2(u64::MAX, &values, u64::MAX).unwrap();
        assert_eq!(bytes.len(), 120 + count * 8);
        assert_eq!(decode(&bytes, u64::MAX).unwrap().values, values);
        for end in 0..bytes.len() {
            assert!(
                decode(&bytes[..end], u64::MAX).is_err(),
                "count {count}, prefix {end}"
            );
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(decode(&extra, u64::MAX).is_err());
    }
    assert_eq!(MAX_CHECKPOINT_BYTES, 4216);
    assert_eq!(encode_v2(0, &[0; V2_MAX_VALUES + 1], 0), Err(Error::Limit));
}

#[test]
fn v2_cannot_smuggle_oversized_v1_or_invalid_headers() {
    let full = encode_v2(42, &[7; V2_MAX_VALUES], 9).unwrap();
    for (offset, replacement, expected) in [
        (8, 1_u16.to_le_bytes().to_vec(), Error::Limit),
        (8, 3_u16.to_le_bytes().to_vec(), Error::Format),
        (28, 513_u32.to_le_bytes().to_vec(), Error::Limit),
        (28, u32::MAX.to_le_bytes().to_vec(), Error::Limit),
        (64, u64::MAX.to_le_bytes().to_vec(), Error::Framing),
        (40, 2_u64.to_le_bytes().to_vec(), Error::Ownership),
        (24, 2_u32.to_le_bytes().to_vec(), Error::Schema),
    ] {
        let mut bytes = full.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        reseal(&mut bytes);
        assert_eq!(decode(&bytes, 9), Err(expected));
    }
    let mut corrupt = full.clone();
    corrupt[80 + 511 * 8] ^= 1;
    assert_eq!(decode(&corrupt, 9), Err(Error::Integrity));
    assert_eq!(decode(&full, 10), Err(Error::Epoch));
    let mut unsealed = full;
    unsealed[80 + 512 * 8] ^= 1;
    reseal(&mut unsealed);
    assert_eq!(decode(&unsealed, 9), Err(Error::Framing));
}
