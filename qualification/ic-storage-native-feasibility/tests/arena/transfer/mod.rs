//! Explicit ownership transfer of a live Box, never an IC region provider.

use super::{
    Allocator, Arena, Cell, Descriptor, Layout, Ledger, MaybeUninit, Parked, UnsafeCell, WORDS,
    suspension,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) start: usize,
    pub(super) words: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Release {
    pub(super) descriptor: Option<Descriptor>,
    pub(super) counter: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    State,
    Domain,
    IncompatibleExtent,
    Source(suspension::Error),
    Allocation,
}

// No Clone, serialization, public constructor or raw-address adoption. Owning
// this actual Box, plus the consumed source authority, carries the proof.
pub(super) struct Transfer {
    backing: Box<UnsafeCell<[MaybeUninit<u64>; WORDS]>>,
    root: Release,
}

impl Transfer {
    #[expect(
        clippy::result_large_err,
        reason = "bounded fixture returns the original arena on refusal without another allocation"
    )]
    pub(super) fn take(source: Arena) -> Result<Self, (Error, Arena)> {
        let Some(root) = source.release.get().filter(|_| source.suspended.get()) else {
            return Err((Error::State, source));
        };
        let token = Parked {
            arena: &source,
            descriptor: root.descriptor,
        };
        if let Err(error) = suspension::complete_root(&token, &source.ledger.get()) {
            return Err((Error::Source(error), source));
        }
        // The source controller ends here. No Vec or token borrowing it can
        // survive the move. Its Box allocation stays live inside this owner.
        let Arena { backing, .. } = source;
        Ok(Self { backing, root })
    }

    pub(super) fn destination(self, limit: usize) -> Result<Destination, (Error, Self)> {
        if limit == 0 || limit > WORDS {
            return Err((Error::Domain, self));
        }
        if self.root.descriptor.is_some_and(|descriptor| {
            descriptor
                .start
                .checked_add(descriptor.capacity)
                .is_none_or(|end| end > limit)
        }) {
            // Geometry admission precedes controller creation and payload access.
            // Refusal returns the sole original owner; it never selects a fallback.
            return Err((Error::IncompatibleExtent, self));
        }
        Ok(Destination {
            arena: Arena {
                backing: self.backing,
                limit,
                ledger: Cell::new(Ledger {
                    occupied: [false; WORDS],
                    blocks: [None; WORDS],
                    next_generation: 1,
                }),
                suspended: Cell::new(true),
                release: Cell::new(None),
                reservation: Cell::new(self.root.descriptor.map(Span::from)),
            },
            root: Some(self.root),
        })
    }
}

impl From<Descriptor> for Span {
    fn from(descriptor: Descriptor) -> Self {
        Self {
            start: descriptor.start,
            words: descriptor.capacity,
        }
    }
}

pub(super) struct Destination {
    arena: Arena,
    root: Option<Release>,
}

pub(super) struct Record<'arena> {
    pub(super) counter: u64,
    pub(super) values: Vec<u64, &'arena Arena>,
}

impl Destination {
    pub(super) fn restore(&mut self) -> Result<Record<'_>, Error> {
        let root = self.root.ok_or(Error::State)?;
        let ledger = self.arena.ledger.get();
        if !self.arena.suspended.get()
            || self.arena.reservation.get() != root.descriptor.map(Span::from)
            || ledger.validate().is_err()
            || ledger.blocks.iter().any(Option::is_some)
        {
            return Err(Error::State);
        }
        let layout = root
            .descriptor
            .map(|descriptor| Layout::array::<u64>(descriptor.capacity))
            .transpose()
            .map_err(|_| Error::Allocation)?;
        self.arena.suspended.set(false);
        let values = if let (Some(descriptor), Some(layout)) = (root.descriptor, layout) {
            let pointer = match self.arena.allocate(layout) {
                Ok(pointer) => pointer.cast::<u64>().as_ptr(),
                Err(_) => {
                    self.arena.suspended.set(true);
                    return Err(Error::Allocation);
                }
            };
            // SAFETY: this pointer was just returned by this exact allocator,
            // from the still-live Box moved through exclusive Transfer custody.
            // The reserved span has the source's exact capacity/alignment and
            // initialized length; neither transfer nor allocation writes payload.
            // Source owners have ended. This new Vec is the sole owner, and its
            // borrow prevents moving/dropping the destination while it is live.
            // After allocation there is no callback, fallible step or payload
            // access before ownership is handed to Vec. See transfer.md.
            #[expect(
                unsafe_code,
                reason = "fresh allocation over explicitly transferred live backing"
            )]
            unsafe {
                Vec::from_raw_parts_in(pointer, descriptor.length, descriptor.capacity, &self.arena)
            }
        } else {
            Vec::new_in(&self.arena)
        };
        self.root = None;
        Ok(Record {
            counter: root.counter,
            values,
        })
    }
}

#[cfg(test)]
mod tests;
