//! Standalone application size probe for governed allocation operations.
pub mod support;
use host::RUNTIME;
use support::host;

const AUTHORITY: &str = "wasm_core_size_probe";

/// Bootstrap the probe allocation, returning its binding fingerprint or zero on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_core_size_probe_bootstrap() -> u64 {
    let Ok(declarations) = probe_declarations() else {
        return 0;
    };
    RUNTIME.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return 0;
        };
        if slot.is_none() {
            let config = ic_storage::StorageConfig::default();
            let Ok(runtime) =
                host::Runtime::new_with_config(ic_storage::DefaultMemoryImpl::default(), config)
            else {
                return 0;
            };
            *slot = Some(runtime);
        }
        let Some(runtime) = slot.as_mut() else {
            return 0;
        };
        runtime
            .bootstrap(&declarations, &pool(), &ic_storage::GenericAllocationPolicy)
            .unwrap_or(0)
    })
}

/// Open the admitted allocation and return its pages, or `u32::MAX` on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_core_size_probe_open() -> u32 {
    RUNTIME.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return u32::MAX;
        };
        slot.as_ref()
            .and_then(|runtime| {
                runtime
                    .open_memory(
                        &ic_storage::StableKey::parse("wasm_core_size_probe.rows.v1").ok()?,
                    )
                    .ok()
            })
            .map_or(u32::MAX, |memory| {
                u32::try_from(ic_storage::Memory::size(&memory)).unwrap_or(u32::MAX)
            })
    })
}

fn pool() -> ic_storage::AllocationGrants {
    ic_storage::AllocationGrants::new(vec![
        ic_storage::MemoryAuthority::new("wasm_core_size_probe", "wasm_core_size_probe.").unwrap(),
    ])
    .unwrap()
}

fn probe_declarations() -> Result<ic_storage::SealedDeclarationSnapshot, ic_storage::AdmissionError>
{
    let request = ic_storage::MemoryRequest::new(AUTHORITY, "wasm_core_size_probe.rows.v1")?;
    ic_storage::SealedDeclarationSnapshot::new(vec![request])
}
