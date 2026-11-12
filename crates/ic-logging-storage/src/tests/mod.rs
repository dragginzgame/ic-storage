use std::num::NonZeroUsize;

use ic_logging::{AppendError, EntryInput, Filter, Level, PageRequest};
use ic_storage::{
    MemoryBacking, RuntimeOpenError, StorageConfig, VectorMemory,
    testing::{MeteredMemory, fixture_runtime},
};

use super::*;
use ic_logging::LogSnapshot;

fn limits() -> Limits {
    Limits {
        max_entries: NonZeroUsize::new(4).unwrap(),
        max_message_bytes: 64,
        max_label_bytes: 16,
        max_text_bytes: MAX_TEXT_BYTES,
    }
}
fn key() -> LogKey {
    LogKey::new("app", "worker").unwrap()
}
fn identity() -> LogIdentity {
    LogIdentity { epoch: 7 }
}
fn input(time: u64, message: &str) -> EntryInput<'_> {
    EntryInput {
        created_at_secs: time,
        level: Level::Info,
        source: "w",
        topic: None,
        message,
    }
}
fn admitted<M: MemoryBacking>(memory: M) -> ic_storage::RuntimeMemory<M> {
    fixture_runtime(
        memory,
        "logging_test",
        "logs.",
        &["logs.store.v1"],
        StorageConfig::default(),
    )
    .unwrap()
    .open_memory(&ic_storage::StableKey::parse("logs.store.v1").unwrap())
    .unwrap()
}
fn store() -> LoggingStore<VectorMemory> {
    LoggingStore::new(StableBTreeMap::create(admitted(VectorMemory::default())).unwrap())
}

mod continuity;
mod encoding;
mod storage;
