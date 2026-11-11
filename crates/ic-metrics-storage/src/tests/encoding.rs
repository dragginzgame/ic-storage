//! Encoding contracts.

use super::*;

#[test]
fn frozen_wire_rows_reopen_through_the_governed_map() {
    let backing = VectorMemory::default();
    let mut records =
        StableBTreeMap::<Vec<u8>, Vec<u8>, _>::create(admitted_memory(backing.clone())).unwrap();
    let id = MetricIdentity::new("x", 3).unwrap();
    // Frozen empty Wire states, independent of the production encoder.
    for (name, kind, payload) in [
        ("aggregate", 1, 5),
        ("summary", 2, 6),
        ("histogram", 3, 8),
        ("counter", 4, 2),
    ] {
        let key = MetricKey::new("app", name).unwrap();
        let mut bytes = b"ICMW".to_vec();
        bytes.extend_from_slice(&[kind, 1, b'x', 3, payload]);
        bytes.resize(bytes.len() + usize::from(payload), 0);
        records.insert(key.0, bytes);
    }
    let before: Vec<_> = records
        .iter()
        .map(ic_storage::adapter::MapEntry::into_pair)
        .collect();
    drop(records);
    let store = MetricsStore::new(StableBTreeMap::load(admitted_memory(backing.clone())).unwrap());
    let key = |name| MetricKey::new("app", name).unwrap();
    assert_eq!(
        store.load_aggregate(&key("aggregate"), &id),
        Ok(Some(Aggregate::EMPTY))
    );
    assert_eq!(
        store.load_summary(&key("summary"), &id),
        Ok(Some(Summary::EMPTY))
    );
    assert_eq!(
        store.load_histogram(&key("histogram"), &id, &[]),
        Ok(Some(Histogram::new([]).unwrap()))
    );
    assert_eq!(
        store.load_counter(&key("counter"), &id),
        Ok(Some(Counter::ZERO))
    );
    assert_eq!(
        store
            .into_map()
            .iter()
            .map(ic_storage::adapter::MapEntry::into_pair)
            .collect::<Vec<_>>(),
        before
    );
    let mut store =
        MetricsStore::new(StableBTreeMap::load(admitted_memory(backing.clone())).unwrap());
    store
        .save_counter(
            &key("counter"),
            &id,
            &ic_metrics::persistence::restore_counter(9, false).unwrap(),
        )
        .unwrap();
    drop(store);
    let records = StableBTreeMap::<Vec<u8>, Vec<u8>, _>::load(admitted_memory(backing)).unwrap();
    let bytes = records.get(&key("counter").0).unwrap();
    assert_eq!(bytes, b"ICMW\x04\x01x\x03\x02\x09\x00");
}

#[test]
fn stored_kind_admission_rejects_every_unknown_tag_before_identity() {
    let id = identity();
    let mut bytes = encode(&id, &Counter::ZERO);
    for kind in 0..=u8::MAX {
        bytes[4] = kind;
        let result = codec::envelope(&bytes, &id);
        assert_eq!(result.is_ok(), (1..=4).contains(&kind));
        if !(1..=4).contains(&kind) {
            assert!(matches!(result, Err(StoreError::KindMismatch)));
        }
    }
}

#[test]
fn truncated_records_reject_all_metric_families_without_writes() {
    fn check<T: crate::metric::StoredMetric + std::fmt::Debug>(value: &T) {
        let memory = MeteredMemory::<VectorMemory>::default();
        let writes = memory.clone();
        let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
        let key = key("app");
        let identity = identity();
        let bytes = encode(&identity, value);
        for length in 0..bytes.len() {
            let truncated = bytes[..length].to_vec();
            store.map.insert(key.0.clone(), truncated.clone());
            assert_rejected_record(
                &mut store,
                &writes,
                &key,
                &identity,
                value,
                (truncated, StoreError::MalformedRecord),
            );
        }
    }

    check(&Aggregate::EMPTY);
    let mut aggregate = Aggregate::EMPTY;
    aggregate.record(7);
    check(&aggregate);
    check(&Summary::EMPTY);
    check(&ic_metrics::persistence::restore_summary(aggregate, Some(7)).unwrap());
    check(&Counter::ZERO);
    check(&ic_metrics::persistence::restore_counter(7, false).unwrap());
    let mut histogram = Histogram::new([0, 10]).unwrap();
    check(&histogram);
    histogram.record(5);
    check(&histogram);
    check(&Histogram::new([]).unwrap());
}

#[test]
fn stored_unit_bytes_preserve_malformed_and_identity_errors() {
    let identity = MetricIdentity::new("x", 3).unwrap();
    let encoded = encode(&identity, &Aggregate::EMPTY);
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
    let key = key("app");
    for byte in 0..=u8::MAX {
        let mut bytes = encoded.clone();
        bytes[6] = byte;
        if byte == b'x' {
            assert_eq!(
                codec::decode::<Aggregate>(&bytes, &identity, &()),
                Ok(Aggregate::EMPTY)
            );
            continue;
        }
        let error = if byte.is_ascii_alphanumeric() || b"._/-:".contains(&byte) {
            StoreError::IdentityMismatch
        } else {
            StoreError::MalformedRecord
        };
        store.map.insert(key.0.clone(), bytes.clone());
        assert_rejected_record(
            &mut store,
            &writes,
            &key,
            &identity,
            &Aggregate::EMPTY,
            (bytes, error),
        );
    }
}

#[test]
fn identity_kind_bounds_and_regression_reject_without_mutation() {
    let mut store = store();
    let mut histogram = Histogram::new([10]).unwrap();
    histogram.record(7);
    let key = key("app");
    store.save(&key, &identity(), &histogram).unwrap();
    assert_eq!(
        store.save(&key, &MetricIdentity::new("bytes", 0).unwrap(), &histogram),
        Err(StoreError::IdentityMismatch)
    );
    assert_eq!(
        store.save(
            &key,
            &MetricIdentity::new("instructions", 1).unwrap(),
            &histogram
        ),
        Err(StoreError::IdentityMismatch)
    );
    assert_eq!(
        store.save(&key, &identity(), &Aggregate::EMPTY),
        Err(StoreError::KindMismatch)
    );
    assert_eq!(
        store.save(&key, &identity(), &Histogram::new([11]).unwrap()),
        Err(StoreError::BoundsMismatch)
    );
    assert_eq!(
        store.save(&key, &identity(), &Histogram::new([10]).unwrap()),
        Err(StoreError::StateRegression)
    );
    assert_eq!(
        store.load_histogram(&key, &identity(), histogram.upper_bounds()),
        Ok(Some(histogram))
    );
    assert!(store.remove(&key, &identity()).unwrap());
    let next = MetricIdentity::new("instructions", 1).unwrap();
    store.save(&key, &next, &Aggregate::EMPTY).unwrap();
    assert_eq!(
        store.load_aggregate(&key, &next),
        Ok(Some(Aggregate::EMPTY))
    );
}

#[test]
fn aggregate_wire_envelope_errors_and_explicit_removal() {
    let mut store = store();
    let key = key("app");
    let bytes = encode(&identity(), &Aggregate::EMPTY);
    let mut unknown = bytes.clone();
    unknown[3] = 99;
    assert_eq!(
        codec::decode::<Aggregate>(&unknown, &identity(), &()),
        Err(StoreError::UnsupportedFormat)
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        codec::decode::<Aggregate>(&trailing, &identity(), &()),
        Err(StoreError::MalformedRecord)
    );
    let mut flags = bytes;
    *flags.last_mut().unwrap() = 4;
    assert_eq!(
        codec::decode::<Aggregate>(&flags, &identity(), &()),
        Err(StoreError::MalformedRecord)
    );
    store
        .map
        .insert(key.0.clone(), wire_row(1, "instructions", 0, &[4]));
    assert!(store.remove(&key, &identity()).unwrap());
}

#[test]
fn wire_encoding_is_explicit_and_rejects_invalid_numeric_state() {
    let mut value = Aggregate::EMPTY;
    value.record(7);
    let identity = MetricIdentity::new("x", 3).unwrap();
    let aggregate = (1_u64, 7_u128, Some(7_u64), false, false);
    let expected = wire_state(1, &identity, &aggregate);
    assert_eq!(encode(&identity, &value), expected);
    assert_eq!(expected, b"ICMW\x01\x01x\x03\x06\x01\x07\x01\x07\x00\x00");
    let summary = ic_metrics::persistence::restore_summary(value, Some(7)).unwrap();
    assert_eq!(
        encode(&identity, &summary),
        wire_state(2, &identity, &(aggregate, Some(7_u64)))
    );
    let mut histogram = Histogram::new([10]).unwrap();
    histogram.record(7);
    assert_eq!(
        encode(&identity, &histogram),
        wire_state(
            3,
            &identity,
            &(aggregate, vec![(10_u64, 1_u64, false)], 0_u64, false)
        )
    );
    let invalid = wire_state(1, &identity, &(1_u64, 0_u128, Some(7_u64), false, false));
    assert!(matches!(
        codec::decode::<Aggregate>(&invalid, &identity, &()),
        Err(StoreError::InvalidState(_))
    ));
}

#[test]
fn counter_wire_records_reject_corruption_before_checkpoint_or_reset() {
    let value = ic_metrics::persistence::restore_counter(7, false).unwrap();
    let identity = MetricIdentity::new("x", 3).unwrap();
    let bytes = encode(&identity, &value);
    let expected = b"ICMW\x04\x01x\x03\x02\x07\x00".to_vec();
    assert_eq!(bytes, expected);
    let mut malformed = Vec::new();
    let mut trailing = bytes.clone();
    trailing.push(0);
    malformed.push((trailing, StoreError::MalformedRecord));
    let mut invalid_flag = bytes.clone();
    *invalid_flag.last_mut().unwrap() = 2;
    malformed.push((invalid_flag, StoreError::MalformedRecord));
    let mut impossible_overflow = bytes.clone();
    *impossible_overflow.last_mut().unwrap() = 1;
    malformed.push((
        impossible_overflow,
        StoreError::InvalidState(
            ic_metrics::persistence::MeasurementStateError::InvalidOverflowFlag,
        ),
    ));
    let mut unknown_kind = bytes.clone();
    unknown_kind[4] = 5;
    malformed.push((unknown_kind, StoreError::KindMismatch));
    let mut unknown_format = bytes;
    unknown_format[3] = 2;
    malformed.push((unknown_format, StoreError::UnsupportedFormat));
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
    let key = key("app");
    for record in malformed {
        store.map.insert(key.0.clone(), record.0.clone());
        assert_rejected_record(&mut store, &writes, &key, &identity, &value, record);
    }
    assert_eq!(
        store.remove(&key, &identity),
        Err(StoreError::UnsupportedFormat)
    );
    assert_eq!(writes.counts().writes, 0);
    // Explicit disposition can retire a known envelope with invalid payload.
    store
        .map
        .insert(key.0.clone(), wire_row(4, "x", 3, &[7, 2]));
    assert!(store.remove(&key, &identity).unwrap());
    assert!(!store.remove(&key, &identity).unwrap());
    // An aggregate's existing kind cannot be reinterpreted as a scalar counter.
    store.save(&key, &identity, &Aggregate::EMPTY).unwrap();
    writes.reset();
    assert_eq!(
        store.load_counter(&key, &identity),
        Err(StoreError::KindMismatch)
    );
    assert_eq!(
        store.save(&key, &identity, &value),
        Err(StoreError::KindMismatch)
    );
    assert_eq!(writes.counts().writes, 0);
}

#[test]
fn checkpoint_continuity_admits_bounded_recorded_extensions() {
    use crate::metric::StoredMetric;

    let observations = [0, 1, 3, u64::MAX];
    for prefix_length in 0..=3 {
        for mut prefix in 0..4_usize.pow(prefix_length) {
            let mut previous = Summary::EMPTY;
            let mut previous_histogram = Histogram::new([1, 10]).unwrap();
            for _ in 0..prefix_length {
                let value = observations[prefix % 4];
                previous.record(value);
                previous_histogram.record(value);
                prefix /= 4;
            }
            for added_length in 0..=3 {
                for mut added in 0..4_usize.pow(added_length) {
                    let mut current = previous;
                    let mut histogram = previous_histogram;
                    for _ in 0..added_length {
                        let value = observations[added % 4];
                        current.record(value);
                        histogram.record(value);
                        added /= 4;
                    }
                    assert!(current.aggregate().continues(&previous.aggregate()));
                    assert!(current.continues(&previous));
                    assert!(histogram.continues(&previous_histogram));
                }
            }
        }
    }
}

#[test]
fn wire_encoding_admits_bounded_histograms_and_numeric_extremes() {
    fn check<T: crate::metric::StoredMetric>(value: &T) {
        let identity = MetricIdentity::new(&"x".repeat(64), u64::MAX).unwrap();
        let bytes = encode(&identity, value);
        assert!(bytes.len() <= codec::MAX_RECORD_BYTES);
        assert!(codec::decode::<T>(&bytes, &identity, value.configuration()).unwrap() == *value);
    }
    check(&Aggregate::EMPTY);
    check(&Counter::ZERO);
    check(&ic_metrics::persistence::restore_counter(u128::MAX, true).unwrap());
    check(&Summary::EMPTY);
    check(&Histogram::new([]).unwrap());
    let bounds = std::array::from_fn::<_, 256, _>(|i| u64::MAX - 255 + i as u64);
    check(&Histogram::new(bounds).unwrap());
    let wide =
        ic_metrics::persistence::restore_aggregate(u64::MAX, u128::MAX, Some(u64::MAX), true, true)
            .unwrap();
    check(&wide);
    check(&ic_metrics::persistence::restore_summary(wide, Some(u64::MAX)).unwrap());
    check(
        &ic_metrics::persistence::restore_histogram(
            std::array::from_fn::<_, 256, _>(|i| i as u64),
            [u64::MAX; 256],
            u64::MAX,
            [true; 256],
            true,
            wide,
        )
        .unwrap(),
    );
    assert_eq!(
        <Histogram<257> as crate::metric::StoredMetric>::ADMISSION,
        Err(StoreError::TooManyBounds)
    );
    assert_eq!(
        <Histogram<{ usize::MAX }> as crate::metric::StoredMetric>::ADMISSION,
        Err(StoreError::TooManyBounds)
    );
}

#[test]
fn wire_byte_mutations_preserve_state_admission_and_rejected_checkpoints() {
    let mut aggregate = Aggregate::EMPTY;
    let mut summary = Summary::EMPTY;
    let mut histogram = Histogram::new([0, 10, u64::MAX]).unwrap();
    for sample in [0, 5, 11] {
        aggregate.record(sample);
        summary.record(sample);
        histogram.record(sample);
    }
    let overflowed =
        ic_metrics::persistence::restore_aggregate(u64::MAX, u128::MAX, Some(u64::MAX), true, true)
            .unwrap();
    let outcomes = [
        check_wire_mutations(&Counter::ZERO),
        check_wire_mutations(&ic_metrics::persistence::restore_counter(u128::MAX, false).unwrap()),
        check_wire_mutations(&ic_metrics::persistence::restore_counter(u128::MAX, true).unwrap()),
        check_wire_mutations(&Aggregate::EMPTY),
        check_wire_mutations(&aggregate),
        check_wire_mutations(&overflowed),
        check_wire_mutations(&Summary::EMPTY),
        check_wire_mutations(&summary),
        check_wire_mutations(
            &ic_metrics::persistence::restore_summary(overflowed, Some(u64::MAX)).unwrap(),
        ),
        check_wire_mutations(&Histogram::new([]).unwrap()),
        check_wire_mutations(&histogram),
    ];
    assert!(outcomes.iter().all(|(_, refused)| *refused > 0));
    assert!(outcomes.iter().any(|(admitted, _)| *admitted > 0));
}

#[test]
fn populated_wire_rows_reopen_without_reencoding() {
    // Independent field layout: deliberately do not use the production encoder.
    let id = identity();
    let backing = VectorMemory::default();
    let mut map =
        StableBTreeMap::<Vec<u8>, Vec<u8>, _>::create(admitted_memory(backing.clone())).unwrap();
    let maximum = u64::MAX;
    let total = u128::from(maximum) * 2;
    let aggregate = (2_u64, total, Some(maximum), false, false);
    let mut originals = Vec::new();
    for (name, bytes) in [
        ("aggregate", wire_state(1, &id, &aggregate)),
        ("summary", wire_state(2, &id, &(aggregate, Some(maximum)))),
        (
            "histogram",
            wire_state(
                3,
                &id,
                &(aggregate, vec![(maximum, 2_u64, false)], 0_u64, false),
            ),
        ),
        ("counter", wire_state(4, &id, &(u128::MAX, true))),
    ] {
        let key = key(name);
        map.insert(key.0.clone(), bytes.clone());
        originals.push((key, bytes));
    }
    drop(map);
    let store = MetricsStore::new(StableBTreeMap::load(admitted_memory(backing)).unwrap());
    let expected =
        ic_metrics::persistence::restore_aggregate(2, total, Some(maximum), false, false).unwrap();
    assert_eq!(
        store.load_aggregate(&key("aggregate"), &id),
        Ok(Some(expected))
    );
    assert_eq!(
        store.load_summary(&key("summary"), &id),
        Ok(Some(
            ic_metrics::persistence::restore_summary(expected, Some(maximum)).unwrap()
        ))
    );
    assert_eq!(
        store.load_histogram(&key("histogram"), &id, &[maximum]),
        Ok(Some(
            ic_metrics::persistence::restore_histogram([maximum], [2], 0, [false], false, expected)
                .unwrap()
        ))
    );
    assert_eq!(
        store.load_counter(&key("counter"), &id),
        Ok(Some(
            ic_metrics::persistence::restore_counter(u128::MAX, true).unwrap()
        ))
    );
    let map = store.into_map();
    for (key, bytes) in originals {
        assert_eq!(map.get(&key.0), Some(bytes));
    }
}

#[test]
fn wire_collection_and_frame_limits_refuse_without_replacement() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
    let key = key("limits");
    let identity = identity();
    let value = Histogram::new([]).unwrap();
    let state = (
        (0_u64, 0_u128, None::<u64>, false, false),
        vec![(0_u64, 0_u64, false); 257],
        0_u64,
        false,
    );
    let too_many = wire_state(3, &identity, &state);
    let too_large = vec![0; codec::MAX_RECORD_BYTES + 1];
    for (bytes, error) in [
        (too_many, StoreError::MalformedRecord),
        (too_large, StoreError::RecordTooLarge),
    ] {
        store.map.insert(key.0.clone(), bytes.clone());
        assert_rejected_record(&mut store, &writes, &key, &identity, &value, (bytes, error));
    }
    // The largest syntactic field widths must pass codec budgets even if their
    // impossible distribution is subsequently refused by arithmetic admission.
    let maximum = u64::MAX;
    let state = (
        (maximum, u128::MAX, Some(maximum), true, true),
        (0..256_u64)
            .map(|i| (maximum - 255 + i, maximum, true))
            .collect::<Vec<_>>(),
        maximum,
        true,
    );
    let bytes = wire_state(3, &identity, &state);
    let bounds = std::array::from_fn::<_, 256, _>(|i| maximum - 255 + i as u64);
    assert!(matches!(
        codec::decode::<Histogram<256>>(&bytes, &identity, &bounds),
        Err(StoreError::InvalidState(_))
    ));
}
