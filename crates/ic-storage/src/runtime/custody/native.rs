use super::BackingIdentity;
use std::{
    collections::BTreeSet,
    sync::{Mutex, PoisonError},
};

static CLAIMS: Mutex<BTreeSet<BackingIdentity>> = Mutex::new(BTreeSet::new());

pub(super) fn with_claims<T>(operation: impl FnOnce(&mut BTreeSet<BackingIdentity>) -> T) -> T {
    let mut claims = CLAIMS.lock().unwrap_or_else(PoisonError::into_inner);
    operation(&mut claims)
}
