//! One-root suspension backed by the original live domain and ownership token.
#![forbid(unsafe_code)]

use super::{Ledger, Parked, Refusal, ledger_image};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    State,
    Root(Refusal),
    UnaccountedAllocation,
    Image(ledger_image::Error),
    ChangedLedger,
}

pub(super) struct Suspension<'arena> {
    root: Option<Parked<'arena>>,
    image: [u8; ledger_image::IMAGE_BYTES],
}

pub(super) fn complete_root(root: &Parked<'_>, ledger: &Ledger) -> Result<(), Error> {
    let index = root.admit(ledger).map_err(Error::Root)?;
    if ledger.blocks.iter().flatten().count() != usize::from(index.is_some()) {
        return Err(Error::UnaccountedAllocation);
    }
    Ok(())
}

impl<'arena> Suspension<'arena> {
    pub(super) fn new(root: Parked<'arena>) -> Result<Self, (Error, Parked<'arena>)> {
        // Refusal returns the original token and leaves all arena state intact.
        let prepare = || {
            if root.arena.suspended.get() {
                return Err(Error::State);
            }
            let ledger = root.arena.ledger.get();
            complete_root(&root, &ledger)?;
            ledger_image::encode(&ledger).map_err(Error::Image)
        };
        let image = match prepare() {
            Ok(image) => image,
            Err(error) => return Err((error, root)),
        };
        root.arena.suspended.set(true);
        Ok(Self {
            root: Some(root),
            image,
        })
    }

    fn admitted_root(&self) -> Result<&Parked<'arena>, Error> {
        let root = self.root.as_ref().ok_or(Error::State)?;
        if !root.arena.suspended.get() {
            return Err(Error::State);
        }
        let decoded = ledger_image::decode(&self.image).map_err(Error::Image)?;
        let live = root.arena.ledger.get();
        if decoded != live {
            return Err(Error::ChangedLedger);
        }
        complete_root(root, &live)?;
        Ok(root)
    }

    pub(super) fn prepare_transfer(&mut self, counter: u64) -> Result<(), Error> {
        let root = self.admitted_root()?;
        if root.arena.release.get().is_some() {
            return Err(Error::State);
        }
        root.arena.release.set(Some(super::transfer::Release {
            descriptor: root.descriptor,
            counter,
        }));
        // Consume authority, but keep the source frozen. Moving the Arena later
        // requires all its outstanding safe borrows to have ended.
        self.root = None;
        Ok(())
    }

    pub(super) fn resume(&mut self) -> Result<Vec<u64, &'arena super::Arena>, Error> {
        self.admitted_root()?;
        // The image does not restore custody: it is checked against the still
        // live arena, and only this original, consumed token creates the Vec.
        // No callback, allocation or new input occurs between these checks and
        // reopening. Its repeated admission is an invariant check; an unexpected
        // refusal re-quarantines the arena even though the token was consumed.
        let root = self.root.take().expect("checked root");
        let arena = root.arena;
        arena.suspended.set(false);
        match root.reopen() {
            Ok(values) => Ok(values),
            Err(error) => {
                arena.suspended.set(true);
                Err(Error::Root(error))
            }
        }
    }
}

// No Drop implementation: losing this object must not clear the arena's
// independent suspended flag or reclaim an allocation whose token was lost.

#[cfg(test)]
mod tests;
