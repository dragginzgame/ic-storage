use super::*;
use crate::BenchKey;
use std::collections::BTreeSet;

#[test]
fn every_catalogue_type_has_every_collection_and_ordered_role() {
    let unique: BTreeSet<_> = MATRIX_CASES.iter().copied().collect();
    assert_eq!(unique.len(), MATRIX_CASES.len());
    for &kind in DATA_TYPES {
        for collection in Collection::ALL {
            let (key, value) = if collection == Collection::Set {
                (kind, DataType::Unit)
            } else {
                (DataType::U64, kind)
            };
            assert!(
                unique.contains(&Case {
                    collection,
                    key,
                    value
                }),
                "{kind:?} {collection:?}"
            );
            if matches!(collection, Collection::Map | Collection::Region) {
                assert!(unique.contains(&Case {
                    collection,
                    key: kind,
                    value: DataType::U64
                }));
            }
        }
    }
    for &key in &ICSS_KEYS {
        for &value in &ICSS_VALUES {
            for collection in [Collection::Map, Collection::Region] {
                assert!(unique.contains(&Case {
                    collection,
                    key,
                    value
                }));
            }
        }
    }
}
macro_rules! key_check {
    (true,$ty:ty) => {{
        let keys: Vec<_> = (0..32).map(<$ty as BenchKey>::from_id).collect();
        assert!(keys.windows(2).all(|p| p[0] < p[1]));
        assert_eq!(
            keys.iter().map(BenchKey::id).collect::<Vec<_>>(),
            (0..32).collect::<Vec<_>>()
        );
    }};
    (false,$ty:ty) => {};
}
macro_rules! check_types {
    ($_ctx:tt; $($name:ident=>($ty:ty,$label:literal,$source:literal,$ord:tt,$bound:literal,$payloads:expr,$opt:tt);)*) => {$ (
        if !DataType::$name.tiny() {key_check!($ord,$ty);}
        for &payload in DataType::$name.payloads() {
            for key in [0,1,7] {
                let before=DataType::$name.expected_value(key,payload,false);
                let after=DataType::$name.expected_value(key,payload,true);
                assert_eq!(before,<$ty as BenchValue>::make(key,payload,false).canonical());
                assert_eq!(after,<$ty as BenchValue>::make(key,payload,true).canonical());
            }
        }
    )*};
}
#[test]
fn concrete_catalogue_keys_and_complete_normalized_values_are_constructible() {
    type_catalogue!(check_types, ());
    for id in 0..2 {
        assert_eq!(bool::from_id(id).id(), id);
    }
    <()>::from_id(0);
    assert_eq!(().id(), 0);
}

#[test]
fn limitations_are_per_backend_and_per_collection() {
    let c = |collection, value| Case {
        collection,
        key: DataType::U64,
        value,
    };
    assert_eq!(
        unavailable(c(Collection::Vector, DataType::String), false)
            .unwrap()
            .0,
        "init"
    );
    assert!(unavailable(c(Collection::Map, DataType::OptionU64), false).is_none());
    assert_eq!(
        unavailable(c(Collection::Map, DataType::OptionU64), true)
            .unwrap()
            .0,
        "setup"
    );
    assert!(unavailable(c(Collection::Cell, DataType::OptionU64), true).is_none());
    assert_eq!(
        unavailable(
            Case {
                collection: Collection::Set,
                key: DataType::F64,
                value: DataType::Unit
            },
            false
        )
        .unwrap()
        .0,
        "trait"
    );
}

#[cfg(feature = "dfinity")]
mod reference_codecs {
    use super::*;
    use crate::upstream::Storable;
    fn roundtrip<T: BenchValue + Storable>(kind: DataType) {
        for &payload in kind.payloads() {
            for key in [0, 1, 7] {
                for replacement in [false, true] {
                    let value = T::make(key, payload, replacement);
                    let expected = value.clone().canonical();
                    let encoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        value.into_bytes_checked()
                    }));
                    if kind.fixed_option_refusal() && ((key & 1 == 0) ^ replacement) {
                        assert!(
                            encoded.is_err(),
                            "{kind:?} None must expose the native fixed-bound refusal"
                        );
                    } else {
                        let value = T::from_bytes(std::borrow::Cow::Owned(encoded.unwrap()));
                        assert_eq!(value.canonical(), expected, "{kind:?}");
                    }
                }
            }
        }
    }
    macro_rules! include_type {
        ("storage",$ty:ty,$kind:expr) => {};
        ($source:literal,$ty:ty,$kind:expr) => {
            roundtrip::<$ty>($kind);
        };
    }
    macro_rules! check_codecs {
        ($_ctx:tt; $($name:ident=>($ty:ty,$label:literal,$source:tt,$ord:tt,$bound:literal,$payloads:expr,$opt:tt);)*) => {$ (
            include_type!($source,$ty,DataType::$name);
        )*};
    }
    #[test]
    fn every_icss_test_type_checks_its_actual_codec_and_complete_decoded_fields() {
        type_catalogue!(check_codecs, ());
    }
}
