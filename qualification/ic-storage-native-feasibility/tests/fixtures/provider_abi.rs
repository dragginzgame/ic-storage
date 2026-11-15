//! Private scalar caller for the fixed-region foreign provider experiment.
//!
//! Execute only after complete artifact admission under scalar-provider-contract.md.
//! The host combiner supplies the provider and trusted settlement entrypoints;
//! these raw functions must not be exposed as application entrypoints.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs)]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    core::arch::wasm32::unreachable()
}

#[expect(
    unsafe_code,
    reason = "foreign provider contract implemented by the admitted Wasm fixture"
)]
#[link(wasm_import_module = "native_provider_diagnostic")]
unsafe extern "C" {
    // Return a live, exclusively claimed, writable 4096-byte external allocation,
    // aligned to 8 and disjoint from compiler allocations, with an initialized
    // first u64. The actual provider traps on refusal. Custody lasts until the
    // trusted caller settles after this Rust function returns. See the complete
    // implementation argument in scalar-provider-contract.md.
    fn claim_region() -> *mut u8;
}

/// Loads the first initialized scalar from the admitted foreign region.
///
/// # Safety
/// Requires the complete admitted provider contract above and trusted caller
/// settlement after return. The raw compiler output alone does not supply it.
#[expect(
    unsafe_code,
    reason = "bounded scalar access under the admitted foreign allocation contract"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn provider_probe_read() -> u64 {
    // SAFETY: the provider supplies initialized, aligned, exclusively claimed
    // memory. Only this word is read; no pointer or reference escapes this call.
    unsafe {
        let base = claim_region();
        if base.is_null() {
            return 0;
        }
        base.cast::<u64>().read()
    }
}

/// Stores one scalar in the admitted foreign region.
///
/// # Safety
/// Requires the same complete provider contract as provider_probe_read.
#[expect(
    unsafe_code,
    reason = "bounded scalar access under the admitted foreign allocation contract"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn provider_probe_write(value: u64) {
    // SAFETY: the provider supplies aligned, exclusively claimed writable
    // memory. Only this word is written; no pointer or reference escapes.
    unsafe {
        let base = claim_region();
        if !base.is_null() {
            base.cast::<u64>().write(value);
        }
    }
}
