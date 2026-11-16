//! Exercise generated declarations as an application using the optional feature.
#![cfg(feature = "derive")]
#![no_std]

use ic_storage_native::record::{Error, Field, Scalar};

mod without_prelude {
    #![no_implicit_prelude]

    #[repr(C)]
    #[derive(::core::clone::Clone, ::core::marker::Copy, ::ic_storage_native::Store)]
    #[store(id = "NOPRELUD", version = 1)]
    struct Value {
        #[store(field = 1)]
        word: ::core::primitive::u64,
    }

    #[test]
    fn expansion_needs_no_application_imports() {
        let mut value = Value { word: 42 };
        let bytes = value.store_encode().unwrap();
        ::core::assert_eq!(Value::store_decode(&bytes).unwrap().word, 42);
        ::core::assert_eq!(value.store_with(|view| view.word()), 42);
    }
}

mod qualified {
    use ::core::primitive::{u16 as u32, u16 as u64};

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, ic_storage_native::Store)]
    #[store(id = "APPRECRD", version = 1)]
    struct Record {
        #[store(field = 9)]
        ceiling: ::core::primitive::u64,
        #[store(field = 2)]
        count: ::core::primitive::u32,
    }

    #[test]
    fn explicit_primitive_paths_bypass_aliases_without_changing_the_contract() {
        assert_eq!(::core::mem::size_of::<(u32, u64)>(), 4);
        let value = Record {
            count: 0x0403_0201,
            ceiling: 0x100f_0e0d_0c0b_0a09,
        };
        let bare = super::Record {
            count: value.count,
            ceiling: value.ceiling,
        };
        assert_eq!(Record::STORE_ID, super::Record::STORE_ID);
        assert_eq!(Record::STORE_VERSION, super::Record::STORE_VERSION);
        assert_eq!(Record::STORE_FIELDS, super::Record::STORE_FIELDS);
        assert_eq!(Record::STORE_SIZE, super::Record::STORE_SIZE);
        assert_eq!(Record::STORE_ALIGN, super::Record::STORE_ALIGN);
        assert_eq!(value.store_encode(), bare.store_encode());
        assert_eq!(
            Record::store_decode(&bare.store_encode().unwrap()).unwrap(),
            value
        );
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, ic_storage_native::Store)]
#[store(id = "APPRECRD", version = 1, validate = Self::valid)]
struct Record {
    #[store(field = 9)]
    ceiling: u64,
    #[store(field = 2)]
    count: u32,
}

impl Record {
    fn valid(&self) -> bool {
        u64::from(self.count) <= self.ceiling
    }
}

#[test]
fn declaration_binds_identity_layout_and_padding_free_logical_order() {
    assert_eq!(Record::STORE_ID, b"APPRECRD");
    assert_eq!(Record::STORE_VERSION, 1);
    assert_eq!(Record::STORE_BYTES, 12);
    assert_eq!(Record::STORE_SIZE, core::mem::size_of::<Record>());
    assert_eq!(Record::STORE_ALIGN, core::mem::align_of::<Record>());
    assert_eq!(
        Record::STORE_FIELDS,
        [
            Field {
                id: 2,
                offset: core::mem::offset_of!(Record, count),
                scalar: Scalar::U32
            },
            Field {
                id: 9,
                offset: core::mem::offset_of!(Record, ceiling),
                scalar: Scalar::U64
            },
        ]
    );
    let mut value = Record {
        count: 0x0403_0201,
        ceiling: 0x100f_0e0d_0c0b_0a09,
    };
    let expected = [1, 2, 3, 4, 9, 10, 11, 12, 13, 14, 15, 16];
    assert_eq!(value.store_encode().unwrap(), expected);
    assert_eq!(Record::store_decode(&expected).unwrap(), value);
    assert_eq!(
        value.store_with(|view| (view.count(), view.ceiling())),
        (value.count, value.ceiling)
    );
}

#[test]
fn codec_rejects_truncation_trailing_bytes_and_invalid_values() {
    let valid = Record {
        count: 7,
        ceiling: 8,
    }
    .store_encode()
    .unwrap();
    for length in 0..valid.len() {
        assert_eq!(Record::store_decode(&valid[..length]), Err(Error::Length));
    }
    let mut trailing = [0; 13];
    trailing[..12].copy_from_slice(&valid);
    assert_eq!(Record::store_decode(&trailing), Err(Error::Length));
    let mut invalid = valid;
    invalid[..4].copy_from_slice(&9u32.to_le_bytes());
    assert_eq!(Record::store_decode(&invalid), Err(Error::Invariant));
    assert_eq!(
        Record {
            count: 9,
            ceiling: 8
        }
        .store_encode(),
        Err(Error::Invariant)
    );
    for count in [0, 1, u32::MAX] {
        let value = Record {
            count,
            ceiling: u64::MAX,
        };
        assert_eq!(
            Record::store_decode(&value.store_encode().unwrap()).unwrap(),
            value
        );
    }
}

mod shadowed {
    // These are legitimate application names, not names owned by the expansion.
    use ::core::primitive::{u16 as bool, u16 as u8, u16 as usize};
    use ApplicationResult::{Err, Ok};

    enum ApplicationResult {
        Ok,
        Err,
    }
    mod core {}
    mod ic_storage_native {}

    macro_rules! concat {
        () => {
            "application concat"
        };
    }
    macro_rules! stringify {
        () => {
            "application stringify"
        };
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, ::ic_storage_native::Store)]
    #[store(id = "SHADOWED", version = 1, validate = Self::valid)]
    struct Value {
        #[store(field = 1)]
        r#type: u32,
        #[store(field = 2)]
        record: u64,
    }

    impl Value {
        fn valid(&self) -> ::core::primitive::bool {
            self.record >= u64::from(self.r#type)
        }
    }

    #[test]
    fn application_names_cannot_change_generated_types_or_result_constructors() {
        assert_eq!(concat!(), "application concat");
        assert_eq!(stringify!(), "application stringify");
        let _: (u8, usize, bool) = (256, 256, 256);
        assert!(matches!(
            (Ok, Err),
            (ApplicationResult::Ok, ApplicationResult::Err)
        ));
        let mut value = Value {
            r#type: 7,
            record: 9,
        };
        let bytes: [::core::primitive::u8; 12] = value.store_encode().unwrap();
        assert_eq!(Value::store_decode(&bytes).unwrap(), value);
        let _: &'static [::core::primitive::u8; 8] = Value::STORE_ID;
        let _: ::core::primitive::u32 = Value::STORE_VERSION;
        let _: ::core::primitive::usize = Value::STORE_BYTES;
        assert_eq!(
            value.store_with(|view| (view.r#type(), view.record())),
            (7, 9)
        );
        assert_eq!(
            Value::store_decode(&[]),
            ::core::result::Result::Err(::ic_storage_native::record::Error::Length)
        );
        assert_eq!(
            Value {
                r#type: 10,
                record: 9
            }
            .store_encode(),
            ::core::result::Result::Err(::ic_storage_native::record::Error::Invariant)
        );
    }
}
