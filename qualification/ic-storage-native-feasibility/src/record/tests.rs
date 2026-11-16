use super::*;
use std::mem::{align_of, offset_of, size_of};

#[test]
fn controlled_payloads_have_only_the_declared_scalar_fields() {
    assert_eq!((size_of::<V1>(), align_of::<V1>()), (8, 8));
    assert_eq!((size_of::<V2>(), align_of::<V2>()), (16, 8));
    assert_eq!(offset_of!(V1, counter), 0);
    assert_eq!(offset_of!(V2, counter), 0);
    assert_eq!(offset_of!(V2, ceiling), 8);
}

#[test]
fn conversion_and_mutation_validate_before_changing_values() {
    let old = V1 { counter: 41 };
    assert_eq!(old.upgrade(40), Err(Error::Ceiling));
    let mut next = old.upgrade(42).unwrap();
    next.add(1).unwrap();
    let saved = next;
    assert_eq!(next.add(1), Err(Error::Ceiling));
    assert_eq!(next, saved);
    let mut full = V1 { counter: u64::MAX }.upgrade(u64::MAX).unwrap();
    assert_eq!(full.add(1), Err(Error::Overflow));
    assert_eq!(full.counter(), u64::MAX);
}

#[test]
fn generated_metadata_and_codec_match_the_frozen_record_contract() {
    use ic_storage_native::record::{Error as CodecError, Field, Scalar};
    assert_eq!(V1::STORE_ID, b"RECU64V1");
    assert_eq!(V2::STORE_ID, V1::STORE_ID);
    assert_eq!((V1::STORE_VERSION, V2::STORE_VERSION), (1, 2));
    assert_eq!(
        (V1::STORE_BYTES, V1::STORE_SIZE, V1::STORE_ALIGN),
        (8, 8, 8)
    );
    assert_eq!(
        (V2::STORE_BYTES, V2::STORE_SIZE, V2::STORE_ALIGN),
        (16, 16, 8)
    );
    assert_eq!(
        V1::STORE_FIELDS,
        [Field {
            id: 1,
            offset: 0,
            scalar: Scalar::U64
        }]
    );
    assert_eq!(
        V2::STORE_FIELDS,
        [
            Field {
                id: 1,
                offset: 0,
                scalar: Scalar::U64
            },
            Field {
                id: 2,
                offset: 8,
                scalar: Scalar::U64
            }
        ]
    );
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&41u64.to_le_bytes());
    bytes[8..].copy_from_slice(&100u64.to_le_bytes());
    let mut value = V2::store_decode(&bytes).unwrap();
    assert_eq!(value, V2::new(41, 100).unwrap());
    assert_eq!(value.store_encode().unwrap(), bytes);
    assert_eq!(
        V1 { counter: 41 }.store_encode().unwrap(),
        41u64.to_le_bytes()
    );
    assert_eq!(V2::store_decode(&bytes[..15]), Err(CodecError::Length));
    assert_eq!(V1::store_decode(&bytes), Err(CodecError::Length));
    bytes[8..].copy_from_slice(&40u64.to_le_bytes());
    assert_eq!(V2::store_decode(&bytes), Err(CodecError::Invariant));
    assert_eq!(
        (V2 {
            counter: 41,
            ceiling: 40
        })
        .store_encode(),
        Err(CodecError::Invariant)
    );
    value.store_with(|mut view| view.add(1)).unwrap();
    assert_eq!(value.counter(), 42);
}

#[test]
fn logical_field_order_is_independent_of_native_field_offsets() {
    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, ic_storage_native::Store)]
    #[store(id = "TESTREOR", version = 1)]
    struct Reordered {
        #[store(field = 2)]
        ceiling: u64,
        #[store(field = 1)]
        counter: u64,
    }
    let mut value = Reordered {
        counter: 41,
        ceiling: 100,
    };
    let encoded = value.store_encode().unwrap();
    assert_eq!(&encoded[..8], &41u64.to_le_bytes());
    assert_eq!(&encoded[8..], &100u64.to_le_bytes());
    assert_eq!(Reordered::store_decode(&encoded).unwrap(), value);
    assert_eq!(Reordered::STORE_FIELDS[0].offset, 8);
    assert_eq!(Reordered::STORE_FIELDS[1].offset, 0);
    assert_eq!(
        value.store_with(|view| (view.counter(), view.ceiling())),
        (41, 100)
    );
}
