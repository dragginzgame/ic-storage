//! Single owner of the governed locator and reader for both fixture profiles.

use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StableKey, adapter::StableCell,
};
use ic_storage_native_feasibility::checkpoint;

const OWNER: &str = "native-checkpoint";
const PREFIX: &str = "native.checkpoint.";
const KEY: &str = "native.checkpoint.scalar.v1";

pub(crate) fn cell(create: bool) -> StableCell<Vec<u8>> {
    // Every open constructs new runtime/lease ownership. Import never creates
    // a missing locator or relies on a handle retained from the previous heap.
    let mut runtime = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
    let key = StableKey::parse(KEY).unwrap();
    if !create {
        assert!(
            runtime.allocation_id(&key).is_some(),
            "missing checkpoint locator"
        );
    }
    let declarations =
        SealedDeclarationSnapshot::new(vec![MemoryRequest::new(OWNER, KEY).unwrap()]).unwrap();
    let grants = AllocationGrants::new(vec![MemoryAuthority::new(OWNER, PREFIX).unwrap()]).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    let memory = runtime.open_memory(&key).unwrap();
    if create {
        StableCell::create(memory, Vec::new()).unwrap()
    } else {
        StableCell::load(memory, checkpoint::MAX_CHECKPOINT_BYTES).unwrap()
    }
}
