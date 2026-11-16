//! Single owner of the governed locator and reader for both fixture profiles.

use ic_storage_legacy::{
    AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryBacking, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StableKey,
};
#[cfg(not(feature = "native-map-job"))]
use ic_storage_legacy::{DefaultMemoryImpl, adapter::StableCell};

const OWNER: &str = "native-checkpoint";
const PREFIX: &str = "native.checkpoint.";
#[cfg(feature = "native-map-job")]
pub(crate) mod map_job;
#[cfg(not(any(
    feature = "native-job",
    feature = "native-record-vector",
    feature = "native-map"
)))]
const KEY: &str = "native.checkpoint.scalar.v1";

#[cfg(not(any(
    feature = "native-job",
    feature = "native-record-vector",
    feature = "native-map"
)))]
pub(crate) fn cell(create: bool) -> StableCell<Vec<u8>> {
    open(
        KEY,
        create,
        ic_storage_native_feasibility::checkpoint::MAX_CHECKPOINT_BYTES,
    )
}

#[cfg(all(feature = "native-map", not(feature = "native-map-job")))]
pub(crate) fn map_cell(create: bool) -> StableCell<Vec<u8>> {
    open(
        "native.checkpoint.maps.v1",
        create,
        ic_storage_native_feasibility::checkpoint::maps::MAX_BYTES,
    )
}

#[cfg(all(
    feature = "native-job",
    not(any(feature = "native-record-vector", feature = "native-map"))
))]
pub(crate) fn job_cell(create: bool) -> StableCell<Vec<u8>> {
    open(
        "native.checkpoint.job.v1",
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
pub(crate) fn record_vector_cell(create: bool) -> StableCell<Vec<u8>> {
    open(
        "native.checkpoint.record_vector.v1",
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
pub(crate) fn job_cell(create: bool) -> StableCell<Vec<u8>> {
    open(
        "native.checkpoint.record_vector.job.v1",
        create,
        ic_storage_native_feasibility::migration_job::RECORD_MAX_BYTES,
    )
}

#[cfg(not(feature = "native-map-job"))]
fn open(key: &str, create: bool, limit: usize) -> StableCell<Vec<u8>> {
    let runtime = runtime(DefaultMemoryImpl::default(), &[key], create);
    let memory = runtime
        .open_memory(&StableKey::parse(key).unwrap())
        .unwrap();
    if create {
        StableCell::create(memory, Vec::new()).unwrap()
    } else {
        StableCell::load(memory, limit).unwrap()
    }
}

fn runtime<M: MemoryBacking>(memory: M, keys: &[&str], create: bool) -> MemoryRuntime<M> {
    // Every open constructs new runtime/lease ownership. Import never creates
    // a missing locator or relies on a handle retained from the previous heap.
    if !create {
        assert!(memory.size() != 0, "missing checkpoint provider");
    }
    let mut runtime = MemoryRuntime::new(memory).unwrap();
    if !create {
        for key in keys {
            assert!(
                runtime
                    .allocation_id(&StableKey::parse(key).unwrap())
                    .is_some(),
                "missing checkpoint locator"
            );
        }
    }
    let declarations = SealedDeclarationSnapshot::new(
        keys.iter()
            .map(|key| MemoryRequest::new(OWNER, key).unwrap())
            .collect(),
    )
    .unwrap();
    let grants = AllocationGrants::new(vec![MemoryAuthority::new(OWNER, PREFIX).unwrap()]).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    runtime
}
