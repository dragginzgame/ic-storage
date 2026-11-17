//! Single owner of the checkpoint layout and backing for each fixture profile.
pub(crate) mod memory;
#[cfg(not(any(feature = "native-map-job", feature = "paged-store")))]
use ic_storage::checkpoint::Record;
use ic_storage::checkpoint::{Memory, Specification, Store as BackingStore};
#[cfg(not(any(feature = "native-map-job", feature = "paged-store")))]
use memory::StableMemory;

#[cfg(all(feature = "native-map-job", not(feature = "native-map-paged")))]
pub(crate) mod map_job;
#[cfg(any(
    feature = "paged-store",
    feature = "native-map-paged",
    all(test, feature = "native-map-job")
))]
pub(crate) mod paged;
#[cfg(all(test, feature = "native-map-job"))]
mod paged_tests;
#[cfg(not(any(
    feature = "native-job",
    feature = "native-record-vector",
    feature = "native-map",
    feature = "paged-store"
)))]
const KEY: u64 = 1;

#[cfg(not(any(
    feature = "native-job",
    feature = "native-record-vector",
    feature = "native-map",
    feature = "paged-store"
)))]
pub(crate) fn cell(create: bool) -> Record<Vec<u8>, StableMemory> {
    open(
        KEY,
        create,
        ic_storage_native_feasibility::checkpoint::MAX_CHECKPOINT_BYTES,
    )
}

#[cfg(all(feature = "native-map", not(feature = "native-map-job")))]
pub(crate) fn map_cell(create: bool) -> Record<Vec<u8>, StableMemory> {
    open(
        2,
        create,
        ic_storage_native_feasibility::checkpoint::maps::MAX_BYTES,
    )
}

#[cfg(all(
    feature = "native-job",
    not(any(feature = "native-record-vector", feature = "native-map"))
))]
pub(crate) fn job_cell(create: bool) -> Record<Vec<u8>, StableMemory> {
    open(
        3,
        create,
        ic_storage_native_feasibility::migration_job::MAX_BYTES,
    )
}

#[cfg(all(
    feature = "native-record-vector",
    any(
        not(feature = "native-record-vector-job"),
        feature = "native-record-vector-source",
        feature = "native-record-vector-v2",
        feature = "native-record-vector-convert"
    )
))]
#[cfg(not(feature = "native-map"))]
pub(crate) fn record_vector_cell(create: bool) -> Record<Vec<u8>, StableMemory> {
    open(
        4,
        create,
        ic_storage_native_feasibility::checkpoint::record_vector::MAX_BYTES,
    )
}

#[cfg(all(
    feature = "native-record-vector-job",
    not(any(
        feature = "native-record-vector-source",
        feature = "native-record-vector-v2",
        feature = "native-record-vector-convert"
    ))
))]
#[cfg(not(feature = "native-map"))]
pub(crate) fn job_cell(create: bool) -> Record<Vec<u8>, StableMemory> {
    open(
        5,
        create,
        ic_storage_native_feasibility::migration_job::RECORD_MAX_BYTES,
    )
}

#[cfg(not(any(feature = "native-map-job", feature = "paged-store")))]
fn open(key: u64, create: bool, limit: usize) -> Record<Vec<u8>, StableMemory> {
    let mut store = backing(StableMemory::default(), &[key], create);
    let region = store.take(key).unwrap();
    if create {
        Record::create(region, Vec::new()).unwrap()
    } else {
        Record::load(region, limit).unwrap()
    }
}

fn backing<M: Memory>(memory: M, keys: &[u64], create: bool) -> BackingStore<M> {
    let specs: Vec<_> = keys
        .iter()
        .map(|&id| Specification {
            id,
            bytes: if id == 21 {
                24 + 128 * ic_storage::map::paged::PAGE_BYTES as u64
            } else {
                16 + 16 * 1024
            },
        })
        .collect();
    if create {
        BackingStore::create(memory, &specs).unwrap()
    } else {
        BackingStore::open(memory, &specs).unwrap()
    }
}
