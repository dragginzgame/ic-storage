//! Shared byte-defined vector provider fixtures; no Rust allocation authority.

pub mod records;

/// Build the closed u64 provider fixture with explicit test configuration.
#[must_use]
pub fn source(
    version: u64,
    root: (u64, u64),
    extent: u64,
    trap: bool,
    fault: &str,
    source_guard: bool,
) -> String {
    let mut source = include_str!("provider.wat")
        .replace("__VERSION__", &version.to_string())
        .replace("__LENGTH__", &root.0.to_string())
        .replace("__CAPACITY__", &root.1.to_string())
        .replace("__EXTENT__", &extent.to_string())
        .replace("__TRAP__", if trap { "1" } else { "0" })
        .replace("__FAULT__", fault);
    if !source_guard {
        source = source.replace("(export \"canister_pre_upgrade\")", "");
    }
    source
}
