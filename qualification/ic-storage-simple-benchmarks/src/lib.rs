//! Fixed wire helpers for the installed ic-stable-structures benchmark canisters.
#![no_std]
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
pub mod wire;
