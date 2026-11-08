//! Small owned-vector allocation workload; run explicitly for measurements.
use ic_storage::{
    AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryRequest, MemoryRuntime,
    SealedDeclarationSnapshot, StableKey, VectorMemory, adapter::StableVec,
};
fn main() {
    let mut host = MemoryRuntime::new(VectorMemory::default()).unwrap();
    let declarations = SealedDeclarationSnapshot::new(vec![
        MemoryRequest::new("measure", "measure.rows.v1").unwrap(),
    ])
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("measure", "measure.").unwrap()]).unwrap();
    host.bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    let rows = StableVec::<[u8; 1024], _>::create(
        host.open_memory(&StableKey::parse("measure.rows.v1").unwrap())
            .unwrap(),
    )
    .expect("create measurement vector");
    for _ in 0..8192 {
        rows.push(&[7; 1024]);
    }
    println!("{:#?}", host.memory_allocations().unwrap());
}
