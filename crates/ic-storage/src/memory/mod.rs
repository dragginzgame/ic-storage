//! Upstream memory contract and Platform-backed canister storage.
cfg_select! {
    target_arch = "wasm32" => {
        mod wasm;
        pub use wasm::DefaultMemoryImpl;
    }
    _ => {
        /// Native default backing for fixture and host integration.
        pub type DefaultMemoryImpl = VectorMemory;
    }
}

pub use crate::substrate::{Memory, VectorMemory};
use std::{rc::Rc, sync::Arc};

///
/// BackingIdentity
/// Identity of a physical backing allocation, shared by all of its wrappers.
/// This is host custody metadata, not a virtual memory ID or durable record.
///

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BackingIdentity(usize);

impl BackingIdentity {
    /// Identify an Rc-owned backing. Its allocation must remain alive through
    /// every runtime/handle using this identity. Identify the underlying backing,
    /// not a newly allocated wrapper or an address inside a growable byte buffer.
    #[must_use]
    pub fn of_rc<T: ?Sized>(backing: &Rc<T>) -> Self {
        Self(Rc::as_ptr(backing).cast::<()>() as usize)
    }

    /// Identify an Arc-owned backing under the same contract as [`Self::of_rc`].
    #[must_use]
    pub fn of_arc<T: ?Sized>(backing: &Arc<T>) -> Self {
        Self(Arc::as_ptr(backing).cast::<()>() as usize)
    }
}
