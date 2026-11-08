//! Minimal installed size consumer; shared endpoints, backend-specific collections.
#[cfg(target_arch = "wasm32")]
#[path = "../../ic-storage-benchmarks/benches/size_consumer/app.rs"]
mod app;
#[cfg(target_arch = "wasm32")]
mod backend;
