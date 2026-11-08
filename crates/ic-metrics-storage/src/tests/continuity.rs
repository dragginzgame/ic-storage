//! Continuity contracts.

use super::*;

#[test]
fn exact_caps_and_overflow_flags_survive_storage_and_continue() {
    let mut store = store();
    let key = key("app");
    let mut value =
        ic_metrics::persistence::restore_aggregate(u64::MAX, 0, Some(0), false, false).unwrap();
    store.save(&key, &identity(), &value).unwrap();
    let exact = store.load_aggregate(&key, &identity()).unwrap().unwrap();
    assert!(!exact.samples_overflowed());
    value.record(0);
    store.save(&key, &identity(), &value).unwrap();
    assert_eq!(
        store.save(&key, &identity(), &exact),
        Err(StoreError::StateRegression)
    );
    assert_eq!(store.load_aggregate(&key, &identity()), Ok(Some(value)));
    let wide =
        ic_metrics::persistence::restore_aggregate(u64::MAX, u128::MAX, Some(u64::MAX), true, true)
            .unwrap();
    store.save(&key, &identity(), &wide).unwrap();
    assert_eq!(store.load_aggregate(&key, &identity()), Ok(Some(wide)));
    let histogram =
        ic_metrics::persistence::restore_histogram([0], [u64::MAX], 0, [true], false, value)
            .unwrap();
    store
        .save(
            &crate::MetricKey::new("app", "distribution").unwrap(),
            &identity(),
            &histogram,
        )
        .unwrap();
}

#[test]
fn counter_checkpoints_preserve_wide_caps_identity_and_explicit_resets() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store =
        MetricsStore::new(StableBTreeMap::create(admitted_memory(memory.clone())).unwrap());
    let key = key("app");
    let identity = MetricIdentity::new("bytes", 7).unwrap();
    assert_eq!(store.load_counter(&key, &identity), Ok(None));
    for value in [
        Counter::ZERO,
        ic_metrics::persistence::restore_counter(u128::from(u64::MAX) + 1, false).unwrap(),
        ic_metrics::persistence::restore_counter(u128::MAX, false).unwrap(),
        ic_metrics::persistence::restore_counter(u128::MAX, true).unwrap(),
    ] {
        store.save(&key, &identity, &value).unwrap();
        assert_eq!(store.load_counter(&key, &identity), Ok(Some(value)));
    }
    drop(store);
    let mut store = MetricsStore::new(StableBTreeMap::load(admitted_memory(memory)).unwrap());
    let retained = ic_metrics::persistence::restore_counter(u128::MAX, true).unwrap();
    assert_eq!(store.load_counter(&key, &identity), Ok(Some(retained)));
    writes.reset();
    for regression in [
        Counter::ZERO,
        ic_metrics::persistence::restore_counter(u128::MAX, false).unwrap(),
    ] {
        assert_eq!(
            store.save(&key, &identity, &regression),
            Err(StoreError::StateRegression)
        );
    }
    for wrong in [
        MetricIdentity::new("items", 7).unwrap(),
        MetricIdentity::new("bytes", 8).unwrap(),
    ] {
        assert_eq!(
            store.load_counter(&key, &wrong),
            Err(StoreError::IdentityMismatch)
        );
        assert_eq!(
            store.save(&key, &wrong, &retained),
            Err(StoreError::IdentityMismatch)
        );
        assert_eq!(
            store.remove(&key, &wrong),
            Err(StoreError::IdentityMismatch)
        );
    }
    assert_eq!(
        store.load_aggregate(&key, &identity),
        Err(StoreError::KindMismatch)
    );
    assert_eq!(
        store.save(&key, &identity, &Aggregate::EMPTY),
        Err(StoreError::KindMismatch)
    );
    assert_eq!(writes.counts().writes, 0);
    assert_eq!(store.load_counter(&key, &identity), Ok(Some(retained)));
    assert!(store.remove(&key, &identity).unwrap());
    let next = MetricIdentity::new("bytes", 8).unwrap();
    store.save(&key, &next, &Counter::ZERO).unwrap();
    assert_eq!(store.load_counter(&key, &next), Ok(Some(Counter::ZERO)));
    assert_eq!(
        store.load_counter(&key, &identity),
        Err(StoreError::IdentityMismatch)
    );
}

#[test]
fn inconsistent_summary_latest_rejects_load_and_save_without_overwrite() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store =
        MetricsStore::new(StableBTreeMap::create(admitted_memory(memory.clone())).unwrap());
    let key = key("app");
    let identity = identity();
    let mut summary = Summary::EMPTY;
    summary.record(10);
    summary.record(10);
    let mut bytes = encode(&identity, &summary);
    // A present zero latest contradicts two tens.
    *bytes.last_mut().unwrap() = 0;
    store.map.insert(key.0.clone(), bytes.clone());
    drop(store);
    let mut reopened = MetricsStore::new(StableBTreeMap::load(admitted_memory(memory)).unwrap());
    assert_rejected_record(
        &mut reopened,
        &writes,
        &key,
        &identity,
        &summary,
        (
            bytes,
            StoreError::InvalidState(ic_metrics::persistence::MeasurementStateError::InvalidLatest),
        ),
    );
}

#[test]
fn bounded_inputs_empty_zero_and_idempotent_checkpoints() {
    assert_eq!(MetricKey::new("", "x"), Err(StoreError::InvalidIdentifier));
    assert_eq!(
        MetricKey::new("a\0b", "x"),
        Err(StoreError::InvalidIdentifier)
    );
    assert_eq!(
        MetricKey::new("a", &"x".repeat(129)),
        Err(StoreError::InvalidIdentifier)
    );
    assert_eq!(
        MetricIdentity::new("µs", 0),
        Err(StoreError::InvalidIdentifier)
    );
    assert_ne!(MetricKey::new("a", "b/c"), MetricKey::new("a/b", "c"));
    let mut store = store();
    let key = key("app");
    let mut value = Summary::EMPTY;
    store.save(&key, &identity(), &value).unwrap();
    assert_eq!(
        store.load_summary(&key, &identity()),
        Ok(Some(Summary::EMPTY))
    );
    value.record(0);
    store.save(&key, &identity(), &value).unwrap();
    store.save(&key, &identity(), &value).unwrap();
    assert_eq!(
        store
            .load_summary(&key, &identity())
            .unwrap()
            .unwrap()
            .latest(),
        Some(0)
    );
    let bounds = std::array::from_fn::<_, 257, _>(|index| u64::try_from(index).unwrap());
    let unsupported = Histogram::new(bounds).unwrap();
    let oversized_key = crate::MetricKey::new("app", "oversize").unwrap();
    assert_eq!(
        store.load_histogram(&oversized_key, &identity(), unsupported.upper_bounds()),
        Err(StoreError::TooManyBounds)
    );
    assert_eq!(
        store.save_histogram(&oversized_key, &identity(), &unsupported),
        Err(StoreError::TooManyBounds)
    );
    assert!(store.map.get(&oversized_key.0).is_none());
    store.map.insert(oversized_key.0.clone(), vec![0]);
    assert_eq!(
        store.load_histogram(&oversized_key, &identity(), unsupported.upper_bounds()),
        Err(StoreError::TooManyBounds)
    );
    assert_eq!(
        store.save_histogram(&oversized_key, &identity(), &unsupported),
        Err(StoreError::TooManyBounds)
    );
    assert_eq!(store.map.get(&oversized_key.0), Some(vec![0]));
    let bytes = vec![0; codec::MAX_RECORD_BYTES + 1];
    assert_eq!(
        codec::decode::<Aggregate>(&bytes, &identity(), &()),
        Err(StoreError::RecordTooLarge)
    );
}

#[test]
fn histogram_checkpoints_cover_maximum_and_zero_bound_counts() {
    let identity = identity();
    let bounds = std::array::from_fn::<_, 256, _>(|index| u64::try_from(index).unwrap());
    let mut value = Histogram::new(bounds).unwrap();
    for observation in [0, 127, 255, 256] {
        value.record(observation);
    }
    let bytes = encode(&identity, &value);
    assert_eq!(
        codec::decode::<Histogram<256>>(&bytes, &identity, value.upper_bounds()),
        Ok(value)
    );
    let mut zero = Histogram::new([]).unwrap();
    zero.record(0);
    let bytes = encode(&identity, &zero);
    assert_eq!(
        codec::decode::<Histogram<0>>(&bytes, &identity, zero.upper_bounds()),
        Ok(zero)
    );
}

#[test]
fn checkpoints_reject_same_count_rewrites_and_admit_order_after_overflow() {
    let mut store = store();
    let key = key("app");
    let initial = ic_metrics::persistence::restore_aggregate(2, 8, Some(5), false, false).unwrap();
    store.save(&key, &identity(), &initial).unwrap();
    let rewrite = ic_metrics::persistence::restore_aggregate(2, 9, Some(5), false, false).unwrap();
    assert_eq!(
        store.save(&key, &identity(), &rewrite),
        Err(StoreError::StateRegression)
    );
    let summary_key = MetricKey::new("app", "ordered").unwrap();
    let aggregate =
        ic_metrics::persistence::restore_aggregate(u64::MAX, u128::MAX, Some(u64::MAX), true, true)
            .unwrap();
    let mut summary = ic_metrics::persistence::restore_summary(aggregate, Some(2)).unwrap();
    store.save(&summary_key, &identity(), &summary).unwrap();
    summary.record(3);
    store.save(&summary_key, &identity(), &summary).unwrap();
    assert_eq!(
        store.load_summary(&summary_key, &identity()),
        Ok(Some(summary))
    );
}

#[test]
fn checkpoints_reject_impossible_exact_appends_without_writes() {
    // Only 1 was added to the total, which cannot introduce maximum 3.
    assert_rejected_checkpoint(
        &ic_metrics::persistence::restore_aggregate(2, 2, Some(1), false, false).unwrap(),
        &ic_metrics::persistence::restore_aggregate(3, 3, Some(3), false, false).unwrap(),
    );
    // One added observation cannot exceed the new maximum of 1.
    assert_rejected_checkpoint(
        &ic_metrics::persistence::restore_aggregate(1, 0, Some(0), false, false).unwrap(),
        &ic_metrics::persistence::restore_aggregate(2, 2, Some(1), false, false).unwrap(),
    );
    let summary = |samples: &[u64]| {
        let mut value = Summary::EMPTY;
        for sample in samples {
            value.record(*sample);
        }
        value
    };
    for (previous, current) in [
        // One addition must equal the latest, not just fall below the maximum.
        (summary(&[2, 1]), summary(&[1, 1, 2])),
        (summary(&[2, 0]), summary(&[1, 2, 1])),
        // A distinct new maximum and latest must both fit in the added total.
        (summary(&[1]), summary(&[0, 0, 3, 1])),
    ] {
        assert_rejected_checkpoint(&previous, &current);
    }
    let histogram = |samples: &[u64]| {
        let mut value = Histogram::new([1, 10]).unwrap();
        for sample in samples {
            value.record(*sample);
        }
        value
    };
    for (previous, current) in [
        // An addition in (1, 10] cannot add just 1 to the total.
        (histogram(&[1, 1, 3]), histogram(&[0, 1, 2, 3])),
        // An addition in [0, 1] cannot add 2 to the total.
        (histogram(&[0, 3]), histogram(&[1, 1, 3])),
        // The bucket containing a new maximum must have an added observation.
        (histogram(&[0, 5]), histogram(&[1, 1, 1, 1, 1, 0, 0, 6])),
        // Added bucket ranges must allow actually attaining the new maximum.
        (histogram(&[0, 5]), histogram(&[0, 2, 3, 9])),
    ] {
        assert_rejected_checkpoint(&previous, &current);
    }
}

#[test]
fn checkpoint_continuity_preserves_exact_caps_and_clipped_appends() {
    use crate::metric::StoredMetric;

    let maximum = u64::MAX;
    let mut aggregate = ic_metrics::persistence::restore_aggregate(
        maximum - 1,
        u128::from(maximum - 1) * u128::from(maximum),
        Some(maximum),
        false,
        false,
    )
    .unwrap();
    let mut summary = ic_metrics::persistence::restore_summary(aggregate, Some(maximum)).unwrap();
    let mut histogram = ic_metrics::persistence::restore_histogram(
        [0, maximum],
        [0, maximum - 1],
        0,
        [false; 2],
        false,
        aggregate,
    )
    .unwrap();
    let mut store = store();
    for value in [maximum, 0, maximum, maximum, 1] {
        let previous = (aggregate, summary, histogram);
        aggregate.record(value);
        summary.record(value);
        histogram.record(value);
        assert!(aggregate.continues(&previous.0));
        assert!(summary.continues(&previous.1));
        assert!(histogram.continues(&previous.2));
        store
            .save_aggregate(&key("aggregate"), &identity(), &previous.0)
            .unwrap();
        store
            .save_aggregate(&key("aggregate"), &identity(), &aggregate)
            .unwrap();
        store
            .save_summary(&key("summary"), &identity(), &previous.1)
            .unwrap();
        store
            .save_summary(&key("summary"), &identity(), &summary)
            .unwrap();
        store
            .save_histogram(&key("histogram"), &identity(), &previous.2)
            .unwrap();
        store
            .save_histogram(&key("histogram"), &identity(), &histogram)
            .unwrap();
    }
    assert!(aggregate.samples_overflowed());
    assert!(aggregate.total_overflowed());
    assert!(histogram.bucket_overflowed()[1]);
}

#[test]
fn identical_checkpoints_validate_without_stable_writes() {
    fn check<T: crate::metric::StoredMetric>(initial: &T, changed: &T) {
        let memory = MeteredMemory::<VectorMemory>::default();
        let writes = memory.clone();
        let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
        let key = key("app");
        writes.reset();
        store.save(&key, &identity(), initial).unwrap();
        assert!(writes.counts().writes > 0); // Absent is different from a retained empty row.
        for (value, changed) in [(initial, false), (changed, true)] {
            writes.reset();
            store.save(&key, &identity(), value).unwrap();
            assert_eq!(writes.counts().writes > 0, changed);
            writes.reset();
            store.save(&key, &identity(), value).unwrap();
            assert_eq!(writes.counts().writes, 0);
            assert!(
                store
                    .load::<T>(&key, &identity(), value.configuration())
                    .unwrap()
                    .as_ref()
                    == Some(value)
            );
        }
        writes.reset();
        assert_eq!(
            store.save(&key, &MetricIdentity::new("bytes", 0).unwrap(), changed),
            Err(StoreError::IdentityMismatch)
        );
        assert_eq!(writes.counts().writes, 0);
        store.map.insert(key.0.clone(), vec![0]);
        writes.reset();
        assert_eq!(
            store.save(&key, &identity(), changed),
            Err(StoreError::MalformedRecord)
        );
        assert_eq!(writes.counts().writes, 0);
        assert_eq!(store.map.get(&key.0), Some(vec![0]));
    }
    let mut aggregate = Aggregate::EMPTY;
    aggregate.record(0);
    check(&Aggregate::EMPTY, &aggregate);
    let mut counter = Counter::ZERO;
    counter.add(u128::from(u64::MAX) + 1);
    check(&Counter::ZERO, &counter);
    let exact_cap = ic_metrics::persistence::restore_counter(u128::MAX, false).unwrap();
    let overflowed = ic_metrics::persistence::restore_counter(u128::MAX, true).unwrap();
    check(&exact_cap, &overflowed);
    let mut summary = Summary::EMPTY;
    summary.record(1);
    check(&Summary::EMPTY, &summary);
    let mut histogram = Histogram::new([0; 0]).unwrap();
    histogram.record(0);
    check(&Histogram::new([]).unwrap(), &histogram);
    let bounds = std::array::from_fn::<_, 256, _>(|index| u64::try_from(index).unwrap());
    let mut histogram = Histogram::new(bounds).unwrap();
    histogram.record(255);
    check(&Histogram::new(bounds).unwrap(), &histogram);
}

#[test]
fn histogram_checkpoints_preserve_each_bucket_count_and_flag() {
    let exact = ic_metrics::persistence::restore_aggregate(2, 1, Some(1), false, false).unwrap();
    let redistributed =
        ic_metrics::persistence::restore_aggregate(3, 3, Some(1), false, false).unwrap();
    let zero =
        ic_metrics::persistence::restore_aggregate(u64::MAX, 1, Some(1), true, false).unwrap();
    let total = u128::from(u64::MAX) + 12;
    let overflow =
        ic_metrics::persistence::restore_aggregate(u64::MAX, total, Some(11), true, false).unwrap();
    let later_overflow =
        ic_metrics::persistence::restore_aggregate(u64::MAX, total + 11, Some(11), true, false)
            .unwrap();
    // Every candidate passes core restoration. Other increasing fields cannot
    // compensate for a bucket decrease or a cleared flag in either array slot.
    let pairs = [
        (
            ic_metrics::persistence::restore_histogram(
                [0, 10],
                [1, 1],
                0,
                [false; 2],
                false,
                exact,
            )
            .unwrap(),
            ic_metrics::persistence::restore_histogram(
                [0, 10],
                [0, 3],
                0,
                [false; 2],
                false,
                redistributed,
            )
            .unwrap(),
        ),
        (
            ic_metrics::persistence::restore_histogram(
                [0, 10],
                [u64::MAX, 1],
                0,
                [true, false],
                false,
                zero,
            )
            .unwrap(),
            ic_metrics::persistence::restore_histogram(
                [0, 10],
                [u64::MAX, 1],
                0,
                [false; 2],
                false,
                zero,
            )
            .unwrap(),
        ),
        (
            ic_metrics::persistence::restore_histogram(
                [0, 10],
                [0, u64::MAX],
                1,
                [false, true],
                false,
                overflow,
            )
            .unwrap(),
            ic_metrics::persistence::restore_histogram(
                [0, 10],
                [0, u64::MAX],
                2,
                [false; 2],
                false,
                later_overflow,
            )
            .unwrap(),
        ),
    ];
    for (previous, regressing) in pairs {
        let memory = MeteredMemory::<VectorMemory>::default();
        let writes = memory.clone();
        let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
        let key = key("app");
        store.save(&key, &identity(), &previous).unwrap();
        writes.reset();
        assert_eq!(
            store.save(&key, &identity(), &regressing),
            Err(StoreError::StateRegression)
        );
        assert_eq!(writes.counts().writes, 0);
        assert_eq!(
            store.load_histogram(&key, &identity(), previous.upper_bounds()),
            Ok(Some(previous))
        );
    }
}

#[test]
fn checkpoint_continuity_matches_independent_reachable_states() {
    use crate::metric::StoredMetric;

    // Complete alphabet 0..=3: no omitted intermediate observation can explain
    // a transition whose maximum is at most 3. Compare states, not histories.
    let mut sequences = vec![Vec::<u64>::new()];
    for length in 1..=3 {
        for mut code in 0..4_usize.pow(length) {
            let mut sequence = Vec::new();
            for _ in 0..length {
                sequence.push((code % 4) as u64);
                code /= 4;
            }
            sequences.push(sequence);
        }
    }
    let project = |sequence: &[u64]| {
        let mut summary = Summary::EMPTY;
        let mut histogram = Histogram::new([0, 2]).unwrap();
        for &sample in sequence {
            summary.record(sample);
            histogram.record(sample);
        }
        (summary.aggregate(), summary, histogram)
    };
    for prefix in &sequences {
        let previous = project(prefix);
        let reachable: Vec<_> = sequences
            .iter()
            .filter(|suffix| prefix.len() + suffix.len() <= 3)
            .map(|suffix| {
                let mut joined = prefix.clone();
                joined.extend_from_slice(suffix);
                project(&joined)
            })
            .collect();
        for candidate in &sequences {
            let current = project(candidate);
            assert_eq!(
                current.0.continues(&previous.0),
                reachable.iter().any(|state| state.0 == current.0),
                "aggregate {prefix:?} -> {candidate:?}"
            );
            assert_eq!(
                current.1.continues(&previous.1),
                reachable.iter().any(|state| state.1 == current.1),
                "summary {prefix:?} -> {candidate:?}"
            );
            assert_eq!(
                current.2.continues(&previous.2),
                reachable.iter().any(|state| state.2 == current.2),
                "histogram {prefix:?} -> {candidate:?}"
            );
        }
    }
}
