//! Standalone application size probe for governed allocation operations.
pub mod support;
use host::RUNTIME;
use support::host;

const AUTHORITY: &str = "wasm_runtime_integration_size_probe";
const KEY: &str = "wasm_runtime_integration_size_probe.rows.v1";

/// Bootstrap the probe allocation, returning its binding fingerprint or zero on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_runtime_integration_size_probe_bootstrap() -> u64 {
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

/// Grow the allocation and return its previous page count, or `u32::MAX` on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_runtime_integration_size_probe_grow(pages: u64) -> u32 {
    RUNTIME
        .with(|slot| {
            slot.try_borrow()
                .ok()?
                .as_ref()?
                .open_memory(&ic_storage::StableKey::parse(KEY).ok()?)
                .ok()?
                .grow(pages)
                .ok()
                .and_then(|previous| u32::try_from(previous).ok())
        })
        .unwrap_or(u32::MAX)
}

/// Verify the existing binding and return its allocation ID, or `u32::MAX` on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_runtime_integration_size_probe_adopt() -> u32 {
    RUNTIME
        .with(|slot| {
            let slot = slot.try_borrow().ok()?;
            let runtime = slot.as_ref()?;
            runtime
                .verify_bootstrap(
                    &probe_declarations().ok()?,
                    &pool(),
                    &ic_storage::GenericAllocationPolicy,
                )
                .ok()?;
            runtime
                .allocation_id(&ic_storage::StableKey::parse(KEY).ok()?)
                .and_then(|id| u32::try_from(id.get()).ok())
        })
        .unwrap_or(u32::MAX)
}

/// Encode the allocation report and return its byte length, or `u32::MAX` on refusal.
#[expect(
    unsafe_code,
    reason = "Standalone probe owns these unique linker exports; no raw memory access."
)]
#[unsafe(no_mangle)]
pub extern "C" fn ic_memory_wasm_runtime_integration_size_probe_summary() -> u32 {
    RUNTIME
        .with(|slot| {
            let slot = slot.try_borrow().ok()?;
            let summary = slot.as_ref()?.memory_allocations().ok()?;
            let bytes = ic_wire::native::encode(
                &summary,
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
            "wasm_runtime_integration_size_probe",
            "wasm_runtime_integration_size_probe.",
        )
        .unwrap(),
    ])
    .unwrap()
}

fn probe_declarations() -> Result<ic_storage::SealedDeclarationSnapshot, ic_storage::AdmissionError>
{
    let request =
        ic_storage::MemoryRequest::new(AUTHORITY, "wasm_runtime_integration_size_probe.rows.v1")?;
    ic_storage::SealedDeclarationSnapshot::new(vec![request])
}
