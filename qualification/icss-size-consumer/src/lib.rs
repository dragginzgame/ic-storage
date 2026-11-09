//! Minimal installed size consumer; matched endpoints, backend-specific collections.
#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod backend;
