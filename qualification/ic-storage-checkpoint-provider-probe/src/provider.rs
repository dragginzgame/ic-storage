//! Single owner of the governed locator and reader for both fixture profiles.

use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StableKey, adapter::StableCell,
};

const OWNER: &str = "native-checkpoint";
const PREFIX: &str = "native.checkpoint.";
#[cfg(not(any(feature = "native-job", feature = "native-record-vector")))]
const KEY: &str = "native.checkpoint.scalar.v1";

#[cfg(not(any(feature = "native-job", feature = "native-record-vector")))]
pub(crate) fn cell(create: bool) -> StableCell<Vec<u8>> {
    open(
        KEY,
        create,
        ic_storage_native_feasibility::checkpoint::MAX_CHECKPOINT_BYTES,
    )
}

#[cfg(all(feature = "native-job", not(feature = "native-record-vector")))]
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
pub(crate) fn job_cell(create: bool) -> StableCell<Vec<u8>> {
    open(
        "native.checkpoint.record_vector.job.v1",
        create,
        ic_storage_native_feasibility::migration_job::RECORD_MAX_BYTES,
    )
}

fn open(key: &str, create: bool, limit: usize) -> StableCell<Vec<u8>> {
    // Every open constructs new runtime/lease ownership. Import never creates
    // a missing locator or relies on a handle retained from the previous heap.
    let mut runtime = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
    let parsed = StableKey::parse(key).unwrap();
    if !create {
        assert!(
            runtime.allocation_id(&parsed).is_some(),
            "missing checkpoint locator"
        );
    }
    let declarations =
        SealedDeclarationSnapshot::new(vec![MemoryRequest::new(OWNER, key).unwrap()]).unwrap();
    let grants = AllocationGrants::new(vec![MemoryAuthority::new(OWNER, PREFIX).unwrap()]).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    let memory = runtime.open_memory(&parsed).unwrap();
    if create {
        StableCell::create(memory, Vec::new()).unwrap()
    } else {
        StableCell::load(memory, limit).unwrap()
    }
}
