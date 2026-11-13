//! This fixture binary owns its instance; the library owns no global runtime.
use ic_storage::{DefaultMemoryImpl, MemoryRuntime};
use std::cell::RefCell;

/// Fixture-owned runtime with the platform default backing.
pub type Runtime = MemoryRuntime<DefaultMemoryImpl>;

thread_local! {
    /// Exclusive runtime owned by this installed qualification canister.
    pub static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}
