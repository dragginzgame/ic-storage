//! Explicit host fixtures using the production namespace/bootstrap boundary.
use crate::{
    AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryBacking, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StorageConfig,
};

mod metered_memory;
pub use metered_memory::{MemoryIoCounts, MeteredMemory};

/// Construct a host runtime with one namespace and a complete request roster.
///
/// # Errors
/// Returns errors from backing construction, grant/key admission, declaration sealing, or bootstrap.
pub fn fixture_runtime<M: MemoryBacking>(
    memory: M,
    authority: &str,
    prefix: &str,
    keys: &[&str],
    config: StorageConfig,
) -> Result<MemoryRuntime<M>, Box<dyn std::error::Error>> {
    let mut runtime = MemoryRuntime::new_with_config(memory, config)?;
    let source = SealedDeclarationSnapshot::new(
        keys.iter()
            .map(|key| MemoryRequest::new(authority, key))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let grants = AllocationGrants::new(vec![MemoryAuthority::new(authority, prefix)?])?;
    runtime.bootstrap(&source, &grants, &GenericAllocationPolicy)?;
    Ok(runtime)
}
