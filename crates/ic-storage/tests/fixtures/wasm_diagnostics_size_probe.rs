//! Standalone application size probe for governed allocation operations.
pub mod support;
use host::RUNTIME;
use support::host;

const AUTHORITY: &str = "wasm_diagnostics_size_probe";

/// Bootstrap the probe allocation, returning its binding fingerprint or zero on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_diagnostics_size_probe_bootstrap() -> u64 {
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

/// Encode the allocation report and return its byte length, or `u32::MAX` on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_diagnostics_size_probe_export() -> u32 {
    RUNTIME
        .with(|slot| {
            let slot = slot.try_borrow().ok()?;
            let runtime = slot.as_ref()?;
            let report = runtime.memory_allocations().ok()?;
            let bytes = ic_wire::native::encode(
                &report,
                ic_wire::Limits {
                    bytes: 1024 * 1024,
                    allocation: 8 * 1024 * 1024,
                    nodes: 64 * 1024,
                    depth: 32,
                },
            )
            .ok()?;
            u32::try_from(bytes.len()).ok()
        })
        .unwrap_or(u32::MAX)
}

/// Encode the allocation report and return its byte length, or `u32::MAX` on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_diagnostics_size_probe_allocations() -> u32 {
    RUNTIME
        .with(|slot| {
            let slot = slot.try_borrow().ok()?;
            let runtime = slot.as_ref()?;
            let report = runtime.memory_allocations().ok()?;
            let bytes = ic_wire::native::encode(
                &report,
                ic_wire::Limits {
                    bytes: 1024 * 1024,
                    allocation: 8 * 1024 * 1024,
                    nodes: 64 * 1024,
                    depth: 32,
                },
            )
            .ok()?;
            u32::try_from(bytes.len()).ok()
        })
        .unwrap_or(u32::MAX)
}

fn pool() -> ic_storage::AllocationGrants {
    ic_storage::AllocationGrants::new(vec![
        ic_storage::MemoryAuthority::new(
            "wasm_diagnostics_size_probe",
            "wasm_diagnostics_size_probe.",
        )
        .unwrap(),
    ])
    .unwrap()
}

fn probe_declarations() -> Result<ic_storage::SealedDeclarationSnapshot, ic_storage::AdmissionError>
{
    let request = ic_storage::MemoryRequest::new(AUTHORITY, "wasm_diagnostics_size_probe.rows.v1")?;
    ic_storage::SealedDeclarationSnapshot::new(vec![request])
}
