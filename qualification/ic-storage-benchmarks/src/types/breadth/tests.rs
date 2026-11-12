use super::codec::Encoding;
use super::*;
use crate::workloads::Workload;
use std::{borrow::Cow, collections::BTreeSet};

#[test]
fn identity_keys_keep_complete_principals_and_composite_order() {
    for id in [0, 1, 1000, 10_000, 100_000, u64::MAX] {
        let principal = <crate::BenchmarkPrincipal as BenchKey>::from_id(id);
        let single = IdentityKey::<false>::from_id(id);
        let composite = IdentityKey::<true>::from_id(id);
        assert_eq!(single.0.len(), 31);
        assert_eq!(composite.0.len(), 62);
        assert_eq!(&single.0[2..], principal.as_slice());
        assert_eq!(&composite.0[32..61], principal.as_slice());
        assert_eq!(single.id(), id);
        assert_eq!(composite.id(), id);
        checked_roundtrip(single);
        checked_roundtrip(composite);
    }
    let first = IdentityKey::<true>::from_id(1);
    let second = IdentityKey::<true>::from_id(2);
    assert_eq!(&first.0[..32], &second.0[..32]);
    assert!(first < second);
    for size in [0, 16, 30, 32, 61, 63, 128] {
        assert!(IdentityKey::<false>::decode(Cow::Owned(vec![0; size])).is_err());
        assert!(IdentityKey::<true>::decode(Cow::Owned(vec![0; size])).is_err());
    }
}

fn checked_roundtrip<T: Encoding + Clone + Eq + std::fmt::Debug>(value: T) {
    let bytes = value.encode();
    if T::FIXED {
        assert_eq!(bytes.len(), T::MAX as usize);
    } else {
        assert!(bytes.len() <= T::MAX as usize);
    }
    // Check the bound as well as round-trip equality: a codec can round-trip
    // successfully while declaring an incompatible fixed width (ICSS Option).
    let owned = value.clone().into_encoded();
    assert_eq!(bytes.as_ref(), owned);
    assert_eq!(T::decode(bytes).unwrap(), value);
}

#[test]
fn type_suite_spans_all_nine_key_value_bound_geometries() {
    let classes = ["fixed", "bounded-variable", "unbounded"];
    let expected: BTreeSet<_> = classes
        .into_iter()
        .flat_map(|k| classes.map(|v| (k, v)))
        .collect();
    let actual: BTreeSet<_> = Workload::Types
        .cases()
        .iter()
        .filter(|case| case.structure() == "Map")
        .map(|case| (case.key_bound(), case.value_bound()))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(Workload::Types.cases(), crate::MATRIX_CASES);
    assert_eq!(Workload::Types.rows(), 32);
    assert_eq!(Workload::Types.counts(), &[1, 8]);
}

#[test]
fn application_codecs_cover_variable_sizes_composite_components_and_optional_variants() {
    let sizes: BTreeSet<_> = (0..64)
        .map(|id| BoundedBytes::<255>::from_id(id).0.len())
        .collect();
    assert_eq!(sizes.len(), 64);
    let sizes: BTreeSet<_> = (0..64).map(|id| Vec::<u8>::from_id(id).len()).collect();
    assert_eq!(sizes.len(), 64);
    assert!(sizes.last().unwrap() > &1024);
    assert_eq!(TupleKey::from_id(255).0, 0);
    assert_eq!(TupleKey::from_id(256).0, 1);
    for size in [0, 8, 255] {
        checked_roundtrip(BoundedBytes::<255>::make(7, size, false));
    }
    for key in [0, 1, u64::MAX] {
        checked_roundtrip(TupleKey::from_id(key));
        for replacement in [false, true] {
            let value = OptionalU64::make(key, 9, replacement);
            checked_roundtrip(value.clone());
            let bytes = value.encode();
            assert_eq!(bytes.len(), if value.0.is_some() { 9 } else { 1 });
            assert_eq!(OptionalU64::decode(bytes).unwrap(), value);
            assert_eq!(value.canonical().len(), 9);
        }
    }
    for bytes in [vec![], vec![2], vec![0, 0], vec![1], vec![1; 10]] {
        assert!(OptionalU64::decode(Cow::Owned(bytes)).is_err());
    }
    assert!(TupleKey::decode(Cow::Owned(vec![0; 27])).is_err());
    assert!(BoundedBytes::<255>::decode(Cow::Owned(vec![0; 256])).is_err());
}

#[cfg(all(feature = "owned", feature = "dfinity"))]
mod reference {
    use super::*;
    use crate::upstream as icss;
    use ic_storage::{
        StableKey, StorageConfig,
        adapter::StableVec,
        testing::{MeteredMemory, fixture_runtime},
    };

    fn same_codec<T>(value: T)
    where
        T: ic_storage::storable::Storable + icss::Storable + Clone + Eq + std::fmt::Debug,
    {
        let owned = <T as ic_storage::storable::Storable>::to_bytes_checked(&value).unwrap();
        let reference = <T as icss::Storable>::to_bytes_checked(&value);
        assert_eq!(owned, reference);
        let owned_bound = <T as ic_storage::storable::Storable>::BOUND;
        let reference_bound = <T as icss::Storable>::BOUND;
        match (owned_bound, reference_bound) {
            (
                ic_storage::storable::Bound::Bounded {
                    max_size: a,
                    is_fixed_size: b,
                },
                icss::storable::Bound::Bounded {
                    max_size: c,
                    is_fixed_size: d,
                },
            ) => assert_eq!((a, b), (c, d)),
            _ => panic!("these shared application codecs are bounded"),
        }
        assert_eq!(
            <T as ic_storage::storable::Storable>::from_bytes(reference).unwrap(),
            value
        );
        assert_eq!(<T as icss::Storable>::from_bytes(owned), value);
        assert_eq!(
            <T as ic_storage::storable::Storable>::into_bytes_checked(value.clone()).unwrap(),
            <T as icss::Storable>::into_bytes_checked(value)
        );
    }

    #[test]
    fn shared_application_codecs_match_published_icss_bytes_and_bounds() {
        for size in [0, 8, 255] {
            same_codec(BoundedBytes::<255>::make(7, size, false));
        }
        for key in [0, 1, 255, 256, u64::MAX] {
            same_codec(TupleKey::from_id(key));
            same_codec(OptionalU64::make(key, 9, false));
            same_codec(OptionalU64::make(key, 9, true));
        }
    }

    fn check_width<const MAX: u32>() {
        let backing = MeteredMemory::<ic_storage::VectorMemory>::default();
        let runtime = fixture_runtime(
            backing.clone(),
            "test",
            "test.",
            &["test.vector.v1"],
            StorageConfig::default(),
        )
        .unwrap();
        let key = StableKey::parse("test.vector.v1").unwrap();
        let vector =
            StableVec::<BoundedBytes<MAX>, _>::create(runtime.open_memory(&key).unwrap()).unwrap();
        let reference = icss::Vec::<BoundedBytes<MAX>, _>::new(icss::VectorMemory::default());
        for size in [0, 8, MAX] {
            let value = BoundedBytes::<MAX>::make(7, size, false);
            vector.push(&value);
            reference.push(&value);
        }
        drop(vector);
        let vector =
            StableVec::<BoundedBytes<MAX>, _>::load(runtime.open_memory(&key).unwrap()).unwrap();
        for (index, size) in [0, 8, MAX].into_iter().enumerate() {
            backing.reset();
            assert_eq!(vector.get(index as u64), reference.get(index as u64));
            assert_eq!(backing.counts().read_bytes, 4 + u64::from(size));
            assert_eq!((backing.counts().writes, backing.counts().grows), (0, 0));
        }
        assert_eq!(
            vector.iter().collect::<Vec<_>>(),
            reference.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn vector_width_boundaries_include_empty_and_full_width_records_after_reopen() {
        check_width::<255>();
        check_width::<256>();
        check_width::<65_535>();
        check_width::<65_536>();
    }

    #[test]
    fn native_icss_option_none_fixed_bound_refusal_is_not_hidden_by_the_shared_codec() {
        let bytes =
            <Option<u64> as ic_storage::storable::Storable>::to_bytes_checked(&None).unwrap();
        assert_eq!(bytes.as_ref(), &[0]);
        assert!(
            std::panic::catch_unwind(|| <Option<u64> as icss::Storable>::to_bytes_checked(&None))
                .is_err()
        );
        assert_eq!(
            <Option<u64> as icss::Storable>::BOUND,
            icss::storable::Bound::Bounded {
                max_size: 9,
                is_fixed_size: true
            }
        );
    }
}
