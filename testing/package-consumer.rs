//! Independent archive consumer: shared lease identity, custody and reopen bytes.
use ic_logging::{EntryInput, Level, Limits, Log};
use ic_logging_storage::{LogIdentity, LogKey, LoggingStore};
use ic_metrics::MeasurementCounter;
use ic_metrics_storage::{MetricIdentity, MetricKey, MetricsStore};
use ic_storage::adapter::StableBTreeMap;
use ic_storage::{RuntimeOpenError, StorageConfig, VectorMemory, testing::fixture_runtime};
use std::num::NonZeroUsize;

fn main() {
    let backing = VectorMemory::default();
    let open = || {
        fixture_runtime(
            backing.clone(),
            "archive",
            "archive.",
            &["archive.map.v1", "archive.metrics.v1", "archive.logging.v1"],
            StorageConfig::default(),
        )
        .unwrap()
    };
    {
        let runtime = open();
        let lease = runtime
            .open_memory(&ic_storage::StableKey::parse("archive.map.v1").unwrap())
            .unwrap();
        let mut map = StableBTreeMap::<u64, u64, _>::create(lease).unwrap();
        assert_eq!(map.insert(7, 11), None);
        assert!(matches!(
            runtime.open_memory(&ic_storage::StableKey::parse("archive.map.v1").unwrap()),
            Err(RuntimeOpenError::AlreadyOpen { .. })
        ));
        drop(map);
        assert!(
            runtime
                .open_memory(&ic_storage::StableKey::parse("archive.map.v1").unwrap())
                .is_ok()
        );
    }
    let runtime = open();
    let map = StableBTreeMap::<u64, u64, _>::load(
        runtime
            .open_memory(&ic_storage::StableKey::parse("archive.map.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(map.get(&7), Some(11));

    let key = MetricKey::new("archive", "bytes").unwrap();
    let identity = MetricIdentity::new("bytes", 0).unwrap();
    let mut counter = MeasurementCounter::ZERO;
    counter.add(42);
    let mut store = MetricsStore::new(
        StableBTreeMap::create(
            runtime
                .open_memory(&ic_storage::StableKey::parse("archive.metrics.v1").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    store.save_counter(&key, &identity, &counter).unwrap();
    assert!(matches!(
        runtime.open_memory(&ic_storage::StableKey::parse("archive.metrics.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    drop(store.into_map());
    let reopened = MetricsStore::new(
        StableBTreeMap::load(
            runtime
                .open_memory(&ic_storage::StableKey::parse("archive.metrics.v1").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        reopened.load_counter(&key, &identity).unwrap(),
        Some(counter)
    );

    let key = LogKey::new("archive", "worker").unwrap();
    let identity = LogIdentity { epoch: 7 };
    let limits = Limits {
        max_entries: NonZeroUsize::new(4).unwrap(),
        max_message_bytes: 64,
        max_label_bytes: 32,
        max_text_bytes: 512,
    };
    let mut log = Log::new(limits);
    log.append(EntryInput {
        created_at_secs: 20,
        level: Level::Info,
        source: "worker",
        topic: None,
        message: "archive checkpoint",
    })
    .unwrap();
    let mut store = LoggingStore::new(
        StableBTreeMap::create(
            runtime
                .open_memory(&ic_storage::StableKey::parse("archive.logging.v1").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    store.save(&key, identity, &log).unwrap();
    assert!(matches!(
        runtime.open_memory(&ic_storage::StableKey::parse("archive.logging.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    drop(store.into_map());
    let mut reopened = LoggingStore::new(
        StableBTreeMap::load(
            runtime
                .open_memory(&ic_storage::StableKey::parse("archive.logging.v1").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    let mut restored = reopened.load(&key, identity, limits).unwrap().unwrap();
    assert_eq!(restored.snapshot(), log.snapshot());
    assert_eq!(restored.prune_expired(21, 0, 4).removed, 1);
    reopened.save(&key, identity, &restored).unwrap();
    drop(reopened.into_map());
    let reopened = LoggingStore::new(
        StableBTreeMap::load(
            runtime
                .open_memory(&ic_storage::StableKey::parse("archive.logging.v1").unwrap())
                .unwrap(),
        )
        .unwrap(),
    );
    let empty = reopened
        .load(&key, identity, limits)
        .unwrap()
        .unwrap()
        .snapshot();
    assert!(empty.entries.is_empty());
    assert_eq!(empty.next_sequence, Some(1));
    assert_eq!(empty.last_created_at_secs, Some(20));
}
