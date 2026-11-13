//! Independent consumer of Storage and the canonical Platform Runtime (#152).
//! Graph admission proves one executor; installed backing tests remain separate.
#![forbid(unsafe_code)]
use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryBacking,
    MemoryRequest, MemoryRuntime, SealedDeclarationSnapshot, StableKey, adapter::StableCell,
};

/// Compile the deployed backing and execute its native counterpart in tests.
/// This fixture requires fresh backing and explicitly reopens its own cell.
pub fn composed_backing_roundtrip() {
    roundtrip(&DefaultMemoryImpl::default());
}

fn roundtrip<M: MemoryBacking + Clone>(backing: &M) {
    let expected = ic_platform::Principal::anonymous().as_slice().to_vec();
    for create in [true, false] {
        let mut host = MemoryRuntime::new(backing.clone()).unwrap();
        let requests = SealedDeclarationSnapshot::new(vec![
            MemoryRequest::new("composition", "composition.cell.v1").unwrap(),
        ])
        .unwrap();
        let grants = AllocationGrants::new(vec![
            MemoryAuthority::new("composition", "composition.").unwrap(),
        ])
        .unwrap();
        host.bootstrap(&requests, &grants, &GenericAllocationPolicy)
            .unwrap();
        let memory = host
            .open_memory(&StableKey::parse("composition.cell.v1").unwrap())
            .unwrap();
        let cell = if create {
            StableCell::create(memory, expected.clone()).unwrap()
        } else {
            StableCell::<Vec<u8>, _>::load(memory, 29).unwrap()
        };
        assert_eq!(cell.get(), &expected);
    }
}

#[cfg(test)]
mod tests;
