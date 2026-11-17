//! Shared custody of valid live native maps, never raw retained-pointer admission.
//! Root declarations must come from the bootstrap's authenticated schema/custody
//! context. Private session, Rc, Vec and allocator headers are not a retained ABI.

mod access;
#[cfg(test)]
mod tests;
use super::{AllocationUsage, Error as MapError, Map, Range};
pub use access::{Participant, Read, Write};
use alloc::{alloc::Global, rc::Rc, vec::Vec};
use core::{alloc::Allocator, cell::RefCell, ops::Bound::Unbounded};

/// Explicit per-root identity and logical bounds within this native generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Root {
    /// Nonzero stable root ID, in increasing order.
    pub id: u64,
    /// Nonzero participant identity assigned by bootstrap.
    pub participant: u64,
    /// Exact nonzero application schema identity.
    pub schema: u64,
    /// Maximum live records; must match the map's configured limit.
    pub entries: usize,
    /// Maximum logical key length, no greater than the map's K.
    pub key_bytes: usize,
    /// Maximum logical value length, no greater than the map's V.
    pub value_bytes: usize,
}

/// Custody/scoping refusal, distinct from an application's returned error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Missing, mismatched, unsorted or excessive root declarations.
    Declaration,
    /// Unknown, foreign, duplicate or empty participant access.
    Authority,
    /// Participant authority was already issued, even if its grant was dropped.
    Claimed,
    /// Another mutable scope or an outstanding read prevents this operation.
    Busy,
    /// A prior unwind or explicit quarantine invalidated every participant.
    Poisoned,
    /// A native collection operation or declared byte bound refused.
    Collection(MapError),
}
impl From<MapError> for Error {
    fn from(error: MapError) -> Self {
        Self::Collection(error)
    }
}

/// Paged export refusal, preserving the distinction between authority and I/O.
#[derive(Debug, PartialEq, Eq)]
pub enum ExportError<E> {
    /// The requested root is outside this scope's authority.
    Session(Error),
    /// Collection encoding, allocation or caller-owned page placement failed.
    Pages(super::paged::ExportError<E>),
}

/// Owned, healthy map plus its bootstrap declaration. No raw pointer authority.
pub type RootMap<const K: usize, const V: usize, A = Global, P = A> = (Root, Map<K, V, A, P>);

/// Admission returns all original live maps on expected refusal, without mutation.
pub struct AdmissionError<
    const K: usize,
    const V: usize,
    A: Allocator + Clone,
    P: Allocator + Clone,
> {
    /// Why the declared family refused.
    pub error: Error,
    /// Original maps in their original order, including any poisoned map.
    pub maps: Vec<RootMap<K, V, A, P>>,
}
impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> core::fmt::Debug
    for AdmissionError<K, V, A, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AdmissionError")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

struct State<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> {
    maps: Vec<RootMap<K, V, A, P>>,
    claimed: u64,
    poisoned: bool,
}
type Shared<const K: usize, const V: usize, A, P> = Rc<RefCell<State<K, V, A, P>>>;

/// Generation-local owner for at most 64 admitted ordered roots. Participants
/// retain its lifetime; allocation headers remain owned by their original maps.
/// This supplies no allocator, Wasm startup or physical memory budget contract.
pub struct Session<
    const K: usize,
    const V: usize,
    A: Allocator + Clone = Global,
    P: Allocator + Clone = A,
> {
    state: Shared<K, V, A, P>,
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Session<K, V, A, P>
{
    /// Accept maps already created/recovered/admitted through their native owner.
    /// Exact expected declarations and each map's logical bounds are validated
    /// before custody is transferred. A narrower per-root byte bound requires one
    /// record scan; the map's existing K/V bounds need no redundant scan. Ordinary
    /// lookup never revalidates the table. This does not reopen raw retained bytes.
    ///
    /// # Errors
    /// Returns every original map on declaration, bound or poison refusal.
    pub fn from_maps(
        maps: Vec<RootMap<K, V, A, P>>,
        expected: &[Root],
    ) -> Result<Self, AdmissionError<K, V, A, P>> {
        let check = || {
            if maps.is_empty() || maps.len() > 64 || maps.len() != expected.len() {
                return Err(Error::Declaration);
            }
            let mut previous = 0;
            for ((root, map), expected) in maps.iter().zip(expected) {
                if root != expected
                    || root.id <= previous
                    || root.participant == 0
                    || root.schema == 0
                    || root.key_bytes > K
                    || root.value_bytes > V
                    || root.entries != map.limit
                {
                    return Err(Error::Declaration);
                }
                map.healthy()?;
                if root.key_bytes < K || root.value_bytes < V {
                    for (key, value) in Range::new(map, Unbounded, Unbounded) {
                        if key.as_slice().len() > root.key_bytes
                            || value.as_slice().len() > root.value_bytes
                        {
                            return Err(Error::Collection(MapError::Limit));
                        }
                    }
                }
                previous = root.id;
            }
            Ok(())
        };
        if let Err(error) = check() {
            return Err(AdmissionError { error, maps });
        }
        Ok(Self {
            state: Rc::new(RefCell::new(State {
                maps,
                claimed: 0,
                poisoned: false,
            })),
        })
    }

    /// Issue all roots for one participant once per generation. Dropping or
    /// forgetting the grant never reissues authority in this session.
    ///
    /// # Errors
    /// Refuses unknown/claimed identities, active borrows or a poisoned family.
    pub fn take(&mut self, participant: u64) -> Result<Participant<K, V, A, P>, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::Busy)?;
        state.healthy()?;
        let mask = state
            .maps
            .iter()
            .enumerate()
            .filter(|(_, (root, _))| root.participant == participant)
            .fold(0, |mask, (index, _)| mask | (1u64 << index));
        if mask == 0 {
            return Err(Error::Authority);
        }
        if mask & state.claimed != 0 {
            return Err(Error::Claimed);
        }
        state.claimed |= mask;
        Ok(Participant {
            state: self.state.clone(),
            mask,
        })
    }

    /// Irreversibly quarantine this entire family after an application detects a
    /// partial logical failure. No repair or rollback is implied.
    ///
    /// # Errors
    /// Refuses during another borrowed scope. Repeated quarantine is harmless.
    pub fn quarantine(&mut self) -> Result<(), Error> {
        self.state
            .try_borrow_mut()
            .map_err(|_| Error::Busy)?
            .poisoned = true;
        Ok(())
    }

    /// Return all original map owners only after every participant has ended.
    /// Outstanding/forgotten grants or poison return this session unchanged.
    /// The caller must still release/settle every returned map before moving its
    /// backing or discarding allocator headers. This performs no durable flush.
    pub fn into_maps(self) -> Result<Vec<RootMap<K, V, A, P>>, Self> {
        if Rc::strong_count(&self.state) != 1 || self.state.borrow().poisoned {
            return Err(self);
        }
        match Rc::try_unwrap(self.state) {
            Ok(state) => Ok(state.into_inner().maps),
            Err(state) => Err(Self { state }),
        }
    }
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> State<K, V, A, P> {
    fn healthy(&self) -> Result<(), Error> {
        if self.poisoned {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
    }
    fn index(&self, mask: u64, root: u64) -> Result<usize, Error> {
        self.maps
            .iter()
            .position(|(declaration, _)| declaration.id == root)
            .filter(|&index| mask & (1u64 << index) != 0)
            .ok_or(Error::Authority)
    }
    fn usage(&self, mask: u64, root: u64) -> Result<AllocationUsage, Error> {
        Ok(self.maps[self.index(mask, root)?].1.allocation_usage()?)
    }
}
