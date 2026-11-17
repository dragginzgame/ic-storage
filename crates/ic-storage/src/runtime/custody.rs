//! Process-local backing claims, retained by every issued runtime handle.
use super::RuntimeConstructionError;
use crate::{Memory, VectorMemory};
use std::{collections::BTreeSet, rc::Rc, sync::Arc};

///
/// BackingIdentity
///
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

///
/// MemoryBacking
///
/// Host boundary for a backing whose aliases can be identified consistently.
///
/// All wrappers and clones over the same physical backing must return the same
/// stable identity; independent backings must have different identities. Keep
/// its identity allocation alive for the backing's complete runtime/handle
/// lifetime. A wrapper delegates to its underlying backing's implementation.
/// Obtaining the identity must not read, grow or write backing memory or create
/// a manager/runtime; use a retained heap identity, not persisted contents.
/// Implementing Memory alone does not establish this custody contract. Raw IO
/// and unmanaged managers over a claimed backing remain prohibited; this is not
/// an isolation boundary against arbitrary linked code or a dishonest backing.
///
pub trait MemoryBacking: Memory {
    /// Return the physical backing identity under the trait's host contract.
    fn backing_identity(&self) -> BackingIdentity;
}

impl MemoryBacking for VectorMemory {
    fn backing_identity(&self) -> BackingIdentity {
        BackingIdentity::of_rc(self)
    }
}

impl<M: MemoryBacking> MemoryBacking for Rc<M> {
    fn backing_identity(&self) -> BackingIdentity {
        self.as_ref().backing_identity()
    }
}

#[cfg(target_arch = "wasm32")]
impl MemoryBacking for crate::DefaultMemoryImpl {
    fn backing_identity(&self) -> BackingIdentity {
        // Every default handle addresses the same canister stable memory.
        BackingIdentity(0)
    }
}

#[cfg(not(target_arch = "wasm32"))]
static CLAIMS: std::sync::Mutex<BTreeSet<BackingIdentity>> = std::sync::Mutex::new(BTreeSet::new());

#[cfg(target_arch = "wasm32")]
thread_local! {
    static CLAIMS: std::cell::RefCell<BTreeSet<BackingIdentity>> = const {
        std::cell::RefCell::new(BTreeSet::new())
    };
}

fn with_claims<T>(operation: impl FnOnce(&mut BTreeSet<BackingIdentity>) -> T) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut claims = CLAIMS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        operation(&mut claims)
    }
    #[cfg(target_arch = "wasm32")]
    CLAIMS.with(|claims| operation(&mut claims.borrow_mut()))
}

pub(super) struct BackingClaim(BackingIdentity);

impl BackingClaim {
    pub fn acquire(memory: &impl MemoryBacking) -> Result<Self, RuntimeConstructionError> {
        let identity = memory.backing_identity();
        if with_claims(|claims| claims.insert(identity)) {
            Ok(Self(identity))
        } else {
            Err(RuntimeConstructionError::BackingAlreadyClaimed)
        }
    }
}

impl Drop for BackingClaim {
    fn drop(&mut self) {
        with_claims(|claims| claims.remove(&self.0));
    }
}
