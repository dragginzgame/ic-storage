use ic_metrics::{
    MeasurementAggregate as Aggregate, MeasurementCounter as Counter,
    MeasurementHistogram as Histogram, MeasurementSummary as Summary,
};
use ic_storage::{
    RuntimeMemory, RuntimeOpenError, StorageConfig, VectorMemory,
    testing::{MeteredMemory, fixture_runtime},
};

use ic_storage::adapter::StableBTreeMap;

use crate::{MetricIdentity, MetricKey, MetricsStore, StoreError, codec};

// Isolated native host: declare a permanent key and commit allocation before opening.
fn admitted_memory<M: ic_storage::MemoryBacking>(backing: M) -> RuntimeMemory<M> {
    let runtime = fixture_runtime(
        backing,
        "metrics_test",
        "metrics.",
        &["metrics.store.v1"],
        StorageConfig::default(),
    )
    .unwrap();
    runtime
        .open_memory(&ic_storage::StableKey::parse("metrics.store.v1").unwrap())
        .unwrap()
}

fn identity() -> MetricIdentity {
    MetricIdentity::new("instructions", 0).unwrap()
}
fn key(namespace: &str) -> MetricKey {
    MetricKey::new(namespace, "work").unwrap()
}
fn store() -> MetricsStore<VectorMemory> {
    MetricsStore::new(StableBTreeMap::create(admitted_memory(VectorMemory::default())).unwrap())
}

fn encode<T: crate::metric::StoredMetric>(identity: &MetricIdentity, value: &T) -> Vec<u8> {
    codec::encode(identity, value).unwrap()
}

fn assert_rejected_record<T: crate::metric::StoredMetric + std::fmt::Debug>(
    store: &mut MetricsStore<MeteredMemory>,
    writes: &MeteredMemory,
    key: &MetricKey,
    identity: &MetricIdentity,
    value: &T,
    (bytes, error): (Vec<u8>, StoreError),
) {
    // Callers install the record before this check, including before reopening.
    writes.reset();
    assert_eq!(
        store.load::<T>(key, identity, value.configuration()),
        Err(error)
    );
    assert_eq!(store.save(key, identity, value), Err(error));
    assert_eq!(store.map.get(&key.0), Some(bytes));
    assert_eq!(writes.counts().writes, 0);
}

// The independent fixture uses plain tuples with the declared field order.
fn wire_state<T: serde::Serialize>(kind: u8, identity: &MetricIdentity, state: &T) -> Vec<u8> {
    let state = ic_wire::native::encode(state, fixture_limits()).unwrap();
    wire_row(kind, &identity.unit, identity.epoch, &state)
}

fn fixture_limits() -> ic_wire::Limits {
    ic_wire::Limits {
        bytes: codec::MAX_RECORD_BYTES,
        allocation: 1024 * 1024,
        nodes: 16 * 1024,
        depth: 16,
    }
}

fn wire_row(kind: u8, unit: &str, epoch: u64, state: &[u8]) -> Vec<u8> {
    let payload = ic_wire::BytesRef::<{ codec::MAX_RECORD_BYTES }>::new(state).unwrap();
    let encoded = ic_wire::native::encode(&(kind, unit, epoch, payload), fixture_limits()).unwrap();
    let mut bytes = b"ICMW".to_vec();
    bytes.extend_from_slice(&encoded);
    bytes
}

fn assert_rejected_checkpoint<T: crate::metric::StoredMetric>(previous: &T, current: &T) {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
    let key = key("append");
    store.save(&key, &identity(), previous).unwrap();
    let retained = store.map.get(&key.0);
    writes.reset();
    assert_eq!(
        store.save(&key, &identity(), current),
        Err(StoreError::StateRegression)
    );
    assert_eq!(writes.counts().writes, 0);
    assert_eq!(store.map.get(&key.0), retained);
}

// Mutate persisted bytes independently of the decoder's field/range formulas.
// A changed byte can be another valid state; admission is not corruption detection.
fn check_wire_mutations<T: crate::metric::StoredMetric + std::fmt::Debug>(
    value: &T,
) -> (usize, usize) {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = MetricsStore::new(StableBTreeMap::create(admitted_memory(memory)).unwrap());
    let key = key("mutation");
    let identity = identity();
    let original = encode(&identity, value);
    let mut admitted = 0;
    let mut refused = 0;
    for offset in 0..original.len() {
        for mask in [1, 2, 4, 8, 16, 32, 64, 128, 0x55, 0xaa, 0xff] {
            for paired in [false, true] {
                let mut bytes = original.clone();
                bytes[offset] ^= mask;
                if paired {
                    let adjacent = (offset + 1) % bytes.len();
                    bytes[adjacent] ^= mask;
                }
                store.map.insert(key.0.clone(), bytes.clone());
                writes.reset();
                let loaded = store.load::<T>(&key, &identity, value.configuration());
                assert_eq!(writes.counts().writes, 0);
                match loaded {
                    Err(error) => {
                        refused += 1;
                        assert_rejected_record(
                            &mut store,
                            &writes,
                            &key,
                            &identity,
                            value,
                            (bytes, error),
                        );
                    }
                    Ok(Some(restored)) => {
                        admitted += 1;
                        // Every admitted record must retain its exact canonical bytes.
                        assert_eq!(encode(&identity, &restored), bytes);
                        store.save(&key, &identity, &restored).unwrap();
                        assert_eq!(writes.counts().writes, 0);
                        assert_eq!(store.map.get(&key.0), Some(bytes.clone()));
                        match store.save(&key, &identity, value) {
                            Ok(()) => {
                                assert_eq!(store.map.get(&key.0), Some(original.clone()));
                                assert_eq!(
                                    store
                                        .load::<T>(&key, &identity, value.configuration())
                                        .unwrap()
                                        .as_ref(),
                                    Some(value)
                                );
                            }
                            Err(error) => {
                                assert_eq!(error, StoreError::StateRegression);
                                assert_eq!(writes.counts().writes, 0);
                                assert_eq!(store.map.get(&key.0), Some(bytes));
                            }
                        }
                    }
                    Ok(None) => panic!("installed mutation became an absent row"),
                }
            }
        }
    }
    (admitted, refused)
}

mod continuity;
mod encoding;
mod storage;
