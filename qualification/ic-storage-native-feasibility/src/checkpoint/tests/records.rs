use super::reseal;
use crate::{
    checkpoint::*,
    record::{self, Record},
};

#[test]
fn new_record_has_explicit_wire_fields_and_old_readers_refuse() {
    let value = Record::V2(record::V2::new(41, 100).unwrap());
    let bytes = encode_record(value, &[17, 23], 7).unwrap();
    // Assemble the envelope independently of the record writer and its offsets.
    let mut expected = Vec::from(*b"ICNCP001");
    expected.extend_from_slice(&[3, 0, 0, 0, 80, 0, 0, 0]);
    expected.extend_from_slice(b"RECU64V1");
    expected.extend_from_slice(&[2, 0, 0, 0, 2, 0, 0, 0]);
    for word in [7u64, 1, 2, 41, 16, 100, 17, 23] {
        expected.extend_from_slice(&word.to_le_bytes());
    }
    expected.extend_from_slice(b"SEALED01");
    expected.extend_from_slice(&[0; 32]);
    reseal(&mut expected);
    assert_eq!(bytes, expected);
    assert_eq!(decode(&bytes, 7), Err(Error::Format));
    assert_eq!(
        decode_record(&bytes, 7),
        Ok(RecordCheckpoint {
            source_epoch: 7,
            record: value,
            values: vec![17, 23],
        })
    );
    for old in [
        encode(41, &[17, 23], 7).unwrap(),
        encode_v2(41, &[17, 23], 7).unwrap(),
    ] {
        assert_eq!(
            decode_record(&old, 7).unwrap().record,
            Record::V1(record::V1 { counter: 41 })
        );
    }
    assert_eq!(
        encode_record(Record::V1(record::V1 { counter: 41 }), &[17, 23], 7).unwrap(),
        encode_v2(41, &[17, 23], 7).unwrap()
    );
}

#[test]
fn record_decoding_checks_schema_invariants_integrity_epochs_and_bounds() {
    let value = Record::V2(record::V2::new(41, 100).unwrap());
    let bytes = encode_record(value, &[17, 23], 7).unwrap();
    for (offset, replacement, error) in [
        (8, 4u16.to_le_bytes().to_vec(), Error::Format),
        (24, 1u32.to_le_bytes().to_vec(), Error::Schema),
        (24, 3u32.to_le_bytes().to_vec(), Error::Schema),
        (
            72,
            40u64.to_le_bytes().to_vec(),
            Error::Record(record::Error::Ceiling),
        ),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + replacement.len()].copy_from_slice(&replacement);
        reseal(&mut bad);
        assert_eq!(decode_record(&bad, 7), Err(error));
    }
    let mut corrupt = bytes.clone();
    corrupt[72] ^= 1;
    assert_eq!(decode_record(&corrupt, 7), Err(Error::Integrity));
    assert_eq!(decode_record(&bytes, 8), Err(Error::Epoch));
    for count in [0, V2_MAX_VALUES] {
        let values = vec![u64::MAX; count];
        let bytes = encode_record(value, &values, 7).unwrap();
        assert_eq!(decode_record(&bytes, 7).unwrap().values, values);
        for end in 0..bytes.len() {
            assert!(decode_record(&bytes[..end], 7).is_err());
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(decode_record(&extra, 7).is_err());
    }
    assert_eq!(
        encode_record(value, &[0; V2_MAX_VALUES + 1], 7),
        Err(Error::Limit)
    );
}
