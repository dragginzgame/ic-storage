//! Storage contracts.

use super::*;

#[test]
fn store_error_retains_the_arithmetic_source() {
    use std::error::Error as _;
    let state = ic_metrics::persistence::MeasurementStateError::InvalidBounds;
    let error = StoreError::from(state);
    assert_eq!(error, StoreError::InvalidState(state));
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<ic_metrics::persistence::MeasurementStateError>(),
        Some(&state)
    );
    assert!(StoreError::StateRegression.source().is_none());
    assert!(StoreError::Codec.source().is_none());
}

#[test]
fn store_retains_allocation_lease_through_map_transfer() {
    let runtime = fixture_runtime(
        VectorMemory::default(),
        "metrics_test",
        "metrics.",
        &["metrics.store.v1"],
        StorageConfig::default(),
    )
    .unwrap();
    let mut store = MetricsStore::new(
        StableBTreeMap::create(
            runtime
                .open_memory(&ic_storage::StableKey::parse("metrics.store.v1").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    store
        .save_counter(&key("app"), &identity(), &Counter::ZERO)
        .unwrap();
    assert!(matches!(
        runtime.open_memory(&ic_storage::StableKey::parse("metrics.store.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    let map: crate::MetricsMap<_> = store.into_map();
    assert!(matches!(
        runtime.open_memory(&ic_storage::StableKey::parse("metrics.store.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    drop(map);
    let reopened = MetricsStore::new(
        StableBTreeMap::load(
            runtime
                .open_memory(&ic_storage::StableKey::parse("metrics.store.v1").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    assert!(matches!(
        runtime.open_memory(&ic_storage::StableKey::parse("metrics.store.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    assert_eq!(
        reopened.load_counter(&key("app"), &identity()),
        Ok(Some(Counter::ZERO))
    );
}

#[test]
fn one_map_reopens_mixed_types_and_namespaces_with_wide_totals() {
    let memory = VectorMemory::default();
    let mut store =
        MetricsStore::new(StableBTreeMap::create(admitted_memory(memory.clone())).unwrap());
    let mut aggregate = Aggregate::EMPTY;
    let mut summary = Summary::EMPTY;
    let mut histogram = Histogram::new([0, 10]).unwrap();
    for value in [0, u64::MAX, u64::MAX, 9] {
        aggregate.record(value);
        summary.record(value);
        histogram.record(value);
    }
    store
        .save_aggregate(&key("icydb"), &identity(), &aggregate)
        .unwrap();
    store
        .save_summary(&key("canic"), &identity(), &summary)
        .unwrap();
    store
        .save_histogram(&key("timers"), &identity(), &histogram)
        .unwrap();
    let bytes_identity = MetricIdentity::new("bytes", 0).unwrap();
    let counter =
        ic_metrics::persistence::restore_counter(u128::from(u64::MAX) + 42, false).unwrap();
    store
        .save_counter(&key("backup"), &bytes_identity, &counter)
        .unwrap();
    drop(store);
    let reopened = MetricsStore::new(StableBTreeMap::load(admitted_memory(memory)).unwrap());
    assert_eq!(
        reopened.load_aggregate(&key("icydb"), &identity()),
        Ok(Some(aggregate))
    );
    assert_eq!(
        reopened.load_summary(&key("canic"), &identity()),
        Ok(Some(summary))
    );
    assert_eq!(
        reopened.load_histogram(&key("timers"), &identity(), &[0, 10]),
        Ok(Some(histogram))
    );
    assert_eq!(
        reopened.load_counter(&key("backup"), &bytes_identity),
        Ok(Some(counter))
    );
    assert_eq!(
        reopened.load_aggregate(&key("missing"), &identity()),
        Ok(None)
    );
    assert!(aggregate.total() > u128::from(u64::MAX));
}

#[test]
fn histogram_configuration_checks_every_full_width_bound_without_writes() {
    fn check<const N: usize>() {
        let memory: MeteredMemory = MeteredMemory::default();
        let mut store =
            MetricsStore::new(StableBTreeMap::create(admitted_memory(memory.clone())).unwrap());
        let bounds = std::array::from_fn::<_, N, _>(|i| (i as u64 + 1) << 40);
        let value = Histogram::new(bounds).unwrap();
        let key = key("configuration");
        store.save_histogram(&key, &identity(), &value).unwrap();
        for index in 0..N {
            for delta in [1, 1_u64 << 32] {
                let mut other = bounds;
                other[index] += delta;
                let candidate = Histogram::new(other).unwrap();
                memory.reset();
                assert_eq!(
                    store.load_histogram(&key, &identity(), &other),
                    Err(StoreError::BoundsMismatch)
                );
                assert_eq!(
                    store.save_histogram(&key, &identity(), &candidate),
                    Err(StoreError::BoundsMismatch)
                );
                assert_eq!(memory.counts().writes, 0);
                assert_eq!(memory.counts().grows, 0);
            }
        }
        drop(store);
        let reopened = MetricsStore::new(StableBTreeMap::load(admitted_memory(memory)).unwrap());
        assert_eq!(
            reopened.load_histogram(&key, &identity(), &bounds),
            Ok(Some(value))
        );
    }
    check::<0>();
    check::<1>();
    check::<16>();
    check::<256>();
}

#[test]
fn histogram_equality_shortcut_preserves_full_state_comparison() {
    use crate::metric::StoredMetric;
    fn check<const N: usize>() {
        let bounds = std::array::from_fn::<_, N, _>(|i| i as u64);
        let empty = Histogram::new(bounds).unwrap();
        assert!(empty.same_state(&empty));
        let mut previous = empty;
        previous.record(0);
        assert!(previous.same_state(&previous));
        for sample in [0, u64::MAX] {
            let mut current = previous;
            current.record(sample);
            assert!(!current.same_state(&previous));
        }
        let other_bounds =
            Histogram::new(std::array::from_fn::<_, N, _>(|i| 1 + i as u64)).unwrap();
        assert_eq!(empty.same_state(&other_bounds), N == 0);
        // A clipped aggregate can remain unchanged while distribution changes.
        let aggregate = ic_metrics::persistence::restore_aggregate(
            u64::MAX,
            u128::MAX,
            Some(u64::MAX),
            true,
            true,
        )
        .unwrap();
        let clipped = ic_metrics::persistence::restore_histogram(
            bounds,
            [0; N],
            u64::MAX,
            [false; N],
            true,
            aggregate,
        )
        .unwrap();
        let mut continued = clipped;
        continued.record(0);
        assert_eq!(continued.aggregate(), clipped.aggregate());
        assert_eq!(continued.same_state(&clipped), N == 0);
        // Saturated aggregate fields cannot hide a change in any numeric lane,
        // including counts that differ only above the low 32 bits.
        for index in 0..N {
            let mut counts = [0; N];
            counts[index] = 1_u64 << 32;
            let changed_bucket = ic_metrics::persistence::restore_histogram(
                bounds,
                counts,
                u64::MAX,
                [false; N],
                true,
                aggregate,
            )
            .unwrap();
            assert_ne!(changed_bucket, clipped);
            assert!(!changed_bucket.same_state(&clipped));
            assert!(!clipped.same_state(&changed_bucket));
        }
        if N != 0 {
            let mut counts = [0; N];
            counts[0] = u64::MAX;
            let mut flags = [false; N];
            let exact_bucket = ic_metrics::persistence::restore_histogram(
                bounds,
                counts,
                u64::MAX,
                flags,
                true,
                aggregate,
            )
            .unwrap();
            flags[0] = true;
            let overflowed_bucket = ic_metrics::persistence::restore_histogram(
                bounds,
                counts,
                u64::MAX,
                flags,
                true,
                aggregate,
            )
            .unwrap();
            assert!(!exact_bucket.same_state(&overflowed_bucket));
        }
    }
    check::<0>();
    check::<1>();
    check::<4>();
    check::<8>();
    check::<256>();
}

#[test]
fn public_histogram_load_admits_expected_bounds_before_reading_a_row() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
    let key = key("app");
    let identity = identity();
    for retained in [None, Some(vec![0])] {
        if let Some(bytes) = retained.as_ref() {
            store.map.insert(key.0.clone(), bytes.clone());
        }
        writes.reset();
        for bounds in [[10, 10], [10, 0]] {
            assert_eq!(
                store.load_histogram(&key, &identity, &bounds),
                Err(StoreError::InvalidState(
                    ic_metrics::persistence::MeasurementStateError::InvalidBounds
                ))
            );
        }
        assert_eq!(
            store.load_histogram(&key, &identity, &[0; 257]),
            Err(StoreError::TooManyBounds)
        );
        assert_eq!(store.map.get(&key.0), retained);
        assert_eq!(writes.counts().writes, 0);
    }
}
