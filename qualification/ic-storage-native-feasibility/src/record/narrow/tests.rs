use super::*;
use ic_storage::record::{Error as CodecError, Field, Scalar};

#[test]
fn equal_outer_layout_does_not_hide_the_changed_scalar_kind() {
    assert_eq!(V3::STORE_ID, V2::STORE_ID);
    assert_eq!((V2::STORE_VERSION, V3::STORE_VERSION), (2, 3));
    assert_eq!((V3::STORE_SIZE, V3::STORE_ALIGN), (16, 8));
    assert_eq!((V2::STORE_SIZE, V2::STORE_ALIGN), (16, 8));
    assert_eq!(
        V3::STORE_FIELDS,
        [
            Field {
                id: 1,
                offset: 0,
                scalar: Scalar::U32
            },
            Field {
                id: 2,
                offset: 8,
                scalar: Scalar::U64
            },
        ]
    );
    assert_ne!(V2::STORE_FIELDS, V3::STORE_FIELDS);
    assert_eq!((V2::STORE_BYTES, V3::STORE_BYTES), (16, 12));
}

#[test]
fn checked_narrowing_and_widening_preserve_values_and_reject_out_of_range_sources() {
    for counter in [0, 1, 41, u64::from(u32::MAX)] {
        let source = V2::new(counter, u64::MAX).unwrap();
        let narrowed = V3::try_from(source).unwrap();
        assert_eq!(V2::from(narrowed), source);
    }
    for counter in [u64::from(u32::MAX) + 1, u64::MAX] {
        let source = V2::new(counter, u64::MAX).unwrap();
        assert_eq!(V3::try_from(source), Err(Error::Range));
        assert_eq!(source.counter(), counter);
        assert_eq!(source.ceiling(), u64::MAX);
    }
    assert_eq!(V3::new(42, 41), Err(Error::Ceiling));
    let mut value = V3::new(41, 42).unwrap();
    value.store_with(|mut view| view.add(1)).unwrap();
    let saved = value;
    assert_eq!(
        value.store_with(|mut view| view.add(1)),
        Err(Error::Ceiling)
    );
    assert_eq!(value, saved);
    let mut full = V3::new(u32::MAX, u64::MAX).unwrap();
    let saved = full;
    assert_eq!(
        full.store_with(|mut view| view.add(1)),
        Err(Error::Overflow)
    );
    assert_eq!(full, saved);
}

#[test]
fn portable_reconstruction_omits_native_padding_and_continues_checked_mutation() {
    for counter in [0, 41, u32::MAX] {
        let bytes = {
            let source = V2::new(u64::from(counter), u64::MAX).unwrap();
            V3::try_from(source).unwrap().store_encode().unwrap()
        };
        // Independently specified logical bytes: four counter bytes immediately
        // followed by eight ceiling bytes, with no repr(C) padding in between.
        let mut expected = [0; 12];
        expected[..4].copy_from_slice(&counter.to_le_bytes());
        expected[4..].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(bytes, expected);
        let mut restored = V3::store_decode(&bytes).unwrap();
        assert_eq!(
            restored.store_with(|view| {
                let counter: u32 = view.counter();
                let ceiling: u64 = view.ceiling();
                (counter, ceiling)
            }),
            (counter, u64::MAX)
        );
        if counter == u32::MAX {
            assert_eq!(
                restored.store_with(|mut view| view.add(1)),
                Err(Error::Overflow)
            );
        } else {
            restored.store_with(|mut view| view.add(1)).unwrap();
            assert_eq!(V2::from(restored).counter(), u64::from(counter) + 1);
        }
        assert_eq!(V2::store_decode(&bytes), Err(CodecError::Length));
    }
    let mut bytes = [0; 12];
    bytes[..4].copy_from_slice(&41u32.to_le_bytes());
    bytes[4..].copy_from_slice(&40u64.to_le_bytes());
    assert_eq!(V3::store_decode(&bytes), Err(CodecError::Invariant));
    assert_eq!(
        V3 {
            counter: 41,
            ceiling: 40
        }
        .store_encode(),
        Err(CodecError::Invariant)
    );
    for bytes in [&bytes[..11], &[0; 16][..]] {
        assert_eq!(V3::store_decode(bytes), Err(CodecError::Length));
    }
}

#[test]
fn reordered_mixed_width_fields_keep_id_order_and_typed_getters() {
    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, ic_storage::Store)]
    #[store(id = "MIXDTEST", version = 1)]
    struct Reordered {
        #[store(field = 2)]
        ceiling: u64,
        #[store(field = 1)]
        counter: u32,
    }
    let mut value = Reordered {
        counter: 41,
        ceiling: 100,
    };
    assert_eq!(
        Reordered::STORE_FIELDS[0],
        Field {
            id: 1,
            offset: 8,
            scalar: Scalar::U32
        }
    );
    assert_eq!(
        Reordered::STORE_FIELDS[1],
        Field {
            id: 2,
            offset: 0,
            scalar: Scalar::U64
        }
    );
    let bytes = value.store_encode().unwrap();
    assert_eq!(&bytes[..4], &41u32.to_le_bytes());
    assert_eq!(&bytes[4..], &100u64.to_le_bytes());
    assert_eq!(Reordered::store_decode(&bytes).unwrap(), value);
    assert_eq!(
        value.store_with(|view| (view.counter(), view.ceiling())),
        (41u32, 100u64)
    );
}
