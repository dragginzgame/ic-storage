//! Allocation-free startup fixture using Platform's synchronous system-raw binding path.
//!
//! The dependency graph links std. No general heap or persistent Rust references
//! are used. The host admits the final module and installs its Wasm bootstrap.

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};

use ic_platform_runtime::raw as ic0;

#[cfg(feature = "managed-graph")]
mod graph;

#[cfg(not(feature = "version-b"))]
static INITIALIZED: AtomicU64 = AtomicU64::new(101);
#[cfg(feature = "version-b")]
static INITIALIZED: AtomicU64 = AtomicU64::new(202);
static ZEROED: AtomicU64 = AtomicU64::new(0);
static LIFECYCLE: AtomicU64 = AtomicU64::new(0);

// Distinct B data shifts the linker layout; the host bounds every data segment.
#[cfg(feature = "version-b")]
static EXTRA: [u8; 513] = [37; 513];

#[inline(never)]
fn stack_probe(depth: u8) -> u64 {
    let bytes = black_box([depth; 256]);
    let child = if depth == 0 {
        0
    } else {
        stack_probe(depth - 1)
    };
    child + bytes.iter().map(|&b| u64::from(b)).sum::<u64>()
}

#[expect(
    unsafe_code,
    reason = "unique IC lifecycle export owned by this fixture"
)]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    #[cfg(feature = "managed-graph")]
    graph::initialize();
    LIFECYCLE.store(1, Ordering::Relaxed);
}

#[expect(
    unsafe_code,
    reason = "unique IC lifecycle export owned by this fixture"
)]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn post_upgrade() {
    #[cfg(feature = "managed-graph")]
    graph::admit();
    // The host-generated wrapper validates retained bytes before this function.
    LIFECYCLE.store(2, Ordering::Relaxed);
}

#[expect(unsafe_code, reason = "unique IC query export owned by this fixture")]
#[unsafe(export_name = "canister_query transient")]
extern "C" fn transient() {
    #[cfg(feature = "version-b")]
    black_box(&EXTRA);
    for value in [
        INITIALIZED.load(Ordering::Relaxed),
        ZEROED.load(Ordering::Relaxed),
        LIFECYCLE.load(Ordering::Relaxed),
        stack_probe(4),
    ] {
        ic0::msg_reply_data_append(&value.to_le_bytes());
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "unique IC update export owned by this fixture")]
#[unsafe(export_name = "canister_update dirty")]
extern "C" fn dirty() {
    INITIALIZED.store(999, Ordering::Relaxed);
    ZEROED.store(888, Ordering::Relaxed);
    LIFECYCLE.store(stack_probe(4), Ordering::Relaxed);
    ic0::msg_reply();
}
