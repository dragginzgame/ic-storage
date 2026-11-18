use super::{BackingIdentity, MemoryBacking};
use std::{cell::RefCell, collections::BTreeSet};

impl MemoryBacking for crate::DefaultMemoryImpl {
    fn backing_identity(&self) -> BackingIdentity {
        // Every default handle addresses the same canister stable memory.
        BackingIdentity(0)
    }
}

thread_local! {
    static CLAIMS: RefCell<BTreeSet<BackingIdentity>> = const { RefCell::new(BTreeSet::new()) };
}

pub(super) fn with_claims<T>(operation: impl FnOnce(&mut BTreeSet<BackingIdentity>) -> T) -> T {
    CLAIMS.with(|claims| operation(&mut claims.borrow_mut()))
}
