//! Application byte records with one backing owner and independently granted roots.
//! This uses the existing checkpoint allocation/record implementation. Native
//! collection/runtime admission and legacy-format migration remain separate work.
//! Writes reach backing synchronously; no end-of-message or upgrade flush exists.
//! Ordinary errors preserve preceding writes. IC trap rollback is runtime-owned;
//! a caught host unwind poisons every participant instead of pretending to roll back.

use crate::checkpoint::{self, PAGE_SIZE, Record, Specification};
use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;
use sha2::{Digest, Sha256};

mod backing;
use backing::Memory;
#[cfg(all(feature = "platform", target_arch = "wasm32"))]
pub use backing::StableMemory;
pub use backing::{Backing, HostMemory};
mod access;
pub use access::{Participant, Read, Write};
#[cfg(test)]
mod tests;

/// The current backing directory's root bound, shared with its allocation owner.
pub const MAX_ROOTS: usize = checkpoint::MAX_REGIONS;
const ENVELOPE: usize = 64;

/// Stable application identity and reservation. IDs, participants and schemas
/// are explicit nonzero integers, never Rust TypeIds or implicit allocation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Root {
    /// Stable root identity, strictly increasing in a plan.
    pub id: u64,
    /// Participant authorized by the application bootstrap to receive this root.
    pub participant: u64,
    /// Exact application byte-schema identity. Conversions must be explicit.
    pub schema: u64,
    /// Maximum logical payload bytes, excluding Storage framing.
    pub capacity: usize,
}

/// Checked declarations and physical page ceiling for one application backing.
pub struct Plan {
    roots: Vec<Root>,
    specs: Vec<Specification>,
    pages: u64,
}
impl Plan {
    /// Validate all reservations before touching backing. The current layout is
    /// fixed at creation; changing root identities or capacities refuses reopening.
    ///
    /// # Errors
    /// Refuses invalid/duplicate/unsorted identities, overflow, excessive roots,
    /// physical budget or transient allocation failure.
    pub fn new(roots: &[Root], max_pages: u64) -> Result<Self, Error> {
        if roots.is_empty() || roots.len() > MAX_ROOTS {
            return Err(Error::Layout);
        }
        let mut declarations = Vec::new();
        let mut specs = Vec::new();
        declarations
            .try_reserve_exact(roots.len())
            .map_err(|_| Error::Allocation)?;
        specs
            .try_reserve_exact(roots.len())
            .map_err(|_| Error::Allocation)?;
        let mut prior = 0;
        for root in roots {
            if root.id <= prior || root.participant == 0 || root.schema == 0 {
                return Err(Error::Layout);
            }
            let size = root
                .capacity
                .checked_add(ENVELOPE + 16)
                .ok_or(Error::Capacity)? as u64;
            prior = root.id;
            declarations.push(*root);
            specs.push(Specification {
                id: root.id,
                bytes: size,
            });
        }
        let (_, bytes) = checkpoint::layout(&specs)?;
        let pages = bytes.div_ceil(PAGE_SIZE);
        if pages > max_pages {
            return Err(Error::Capacity);
        }
        Ok(Self {
            roots: declarations,
            specs,
            pages,
        })
    }
    /// Exact fixed physical reservation including all Storage framing.
    #[must_use]
    pub const fn physical_pages(&self) -> u64 {
        self.pages
    }
}

/// Refusal at ownership, declaration, record or operation admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Missing, corrupt, foreign or incompatible data/declarations.
    Layout,
    /// Record or physical reservation bound exceeded.
    Capacity,
    /// Fallible transient allocation refused.
    Allocation,
    /// Participant or backing authority has already been issued.
    Claimed,
    /// A root is undeclared or not granted to this operation.
    Authority,
    /// Another mutable scoped operation is active.
    Busy,
    /// A host unwind left the shared owner quarantined.
    Poisoned,
    /// Underlying allocation/record admission refused.
    Backing(checkpoint::Error),
}
impl From<checkpoint::Error> for Error {
    fn from(error: checkpoint::Error) -> Self {
        Self::Backing(error)
    }
}

/// Failed admission returns the original backing for inspection or explicit recovery.
/// It never falls back to fresh initialization. Partial IO requires the enclosing
/// transaction's rollback; creation is not a crash-atomic protocol on arbitrary hosts.
pub struct AdmissionError<B> {
    /// Admission refusal.
    pub error: Error,
    /// Exact backing after the failed operation.
    pub backing: B,
}
impl<B> core::fmt::Debug for AdmissionError<B> {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        output
            .debug_struct("AdmissionError")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

struct Slot<B: Backing> {
    root: Root,
    record: Record<Vec<u8>, Memory<B>>,
}
struct State<B: Backing> {
    memory: Memory<B>,
    slots: Vec<Slot<B>>,
    claimed: u8,
    poisoned: bool,
}

/// Application bootstrap owner. Participants hold its shared lifetime without
/// receiving raw backing or cloneable grants. Reads use admitted cached bytes.
/// Drop all grants before reclaiming backing or creating another owner.
pub struct Store<B: Backing> {
    state: Rc<RefCell<State<B>>>,
}
impl<B: Backing> Store<B> {
    /// Explicitly create every declared root in a fresh, zero-page backing.
    /// Initial values follow declaration order. Their bounds and transient buffers
    /// are prepared before any backing writes. Applications own logical codecs.
    ///
    /// # Errors
    /// Returns the backing on malformed initials, allocation or creation refusal.
    pub fn create(backing: B, plan: &Plan, initial: &[&[u8]]) -> Result<Self, AdmissionError<B>> {
        Self::admit(backing, plan, Some(initial))
    }
    /// Reopen only the exact declared layout, identities, schemas and valid seals.
    /// No missing or corrupt state is initialized, repaired or written.
    /// Application codecs must additionally validate the logical bytes before use.
    ///
    /// # Errors
    /// Returns the unchanged backing on any admission or allocation refusal.
    pub fn open(backing: B, plan: &Plan) -> Result<Self, AdmissionError<B>> {
        Self::admit(backing, plan, None)
    }
    fn admit(
        backing: B,
        plan: &Plan,
        initial: Option<&[&[u8]]>,
    ) -> Result<Self, AdmissionError<B>> {
        let memory = Memory(Rc::new(RefCell::new(backing)));
        let result = Self::build(memory.clone(), plan, initial);
        result.map_err(|error| AdmissionError {
            error,
            backing: unwrap_memory(memory),
        })
    }
    fn build(memory: Memory<B>, plan: &Plan, initial: Option<&[&[u8]]>) -> Result<Self, Error> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(plan.roots.len())
            .map_err(|_| Error::Allocation)?;
        let mut prepared = Vec::new();
        if let Some(values) = initial {
            if values.len() != plan.roots.len() {
                return Err(Error::Layout);
            }
            prepared
                .try_reserve_exact(values.len())
                .map_err(|_| Error::Allocation)?;
            for (root, value) in plan.roots.iter().zip(values) {
                prepared.push(encode(*root, value)?);
            }
        }
        let mut backing = if initial.is_some() {
            checkpoint::Store::create(memory.clone(), &plan.specs)?
        } else {
            checkpoint::Store::open(memory.clone(), &plan.specs)?
        };
        let mut prepared = prepared.into_iter();
        for root in &plan.roots {
            let region = backing.take(root.id)?;
            let record = if let Some(bytes) = prepared.next() {
                Record::create(region, bytes)?
            } else {
                Record::<Vec<u8>, _>::load(region, root.capacity + ENVELOPE)?
            };
            validate(*root, record.get())?;
            slots.push(Slot {
                root: *root,
                record,
            });
        }
        Ok(Self {
            state: Rc::new(RefCell::new(State {
                memory,
                slots,
                claimed: 0,
                poisoned: false,
            })),
        })
    }
    /// Issue all roots declared for a participant exactly once. Dropping or
    /// forgetting a grant does not permit reissuance in this owner generation.
    ///
    /// # Errors
    /// Refuses unknown/already-issued participants, active mutation or poison.
    pub fn take(&mut self, participant: u64) -> Result<Participant<B>, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::Busy)?;
        if state.poisoned {
            return Err(Error::Poisoned);
        }
        let mask = state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.root.participant == participant)
            .fold(0, |mask, (index, _)| mask | (1 << index));
        if mask == 0 {
            return Err(Error::Authority);
        }
        if state.claimed & mask != 0 {
            return Err(Error::Claimed);
        }
        state.claimed |= mask;
        Ok(Participant {
            state: self.state.clone(),
            mask,
        })
    }
    /// End a healthy owner and reclaim its backing only after every participant
    /// is dropped. An outstanding/forgotten grant or poison returns this owner.
    /// No flush is needed: all successful writes already reached backing.
    pub fn into_backing(self) -> Result<B, Self> {
        if Rc::strong_count(&self.state) != 1 || self.state.borrow().poisoned {
            return Err(self);
        }
        let state = match Rc::try_unwrap(self.state) {
            Ok(state) => state.into_inner(),
            Err(state) => return Err(Self { state }),
        };
        let State { memory, slots, .. } = state;
        drop(slots);
        Ok(unwrap_memory(memory))
    }
}

fn unwrap_memory<B>(memory: Memory<B>) -> B {
    Rc::try_unwrap(memory.0)
        .ok()
        .expect("all record handles have ended")
        .into_inner()
}
fn encode(root: Root, value: &[u8]) -> Result<Vec<u8>, Error> {
    if value.len() > root.capacity {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(ENVELOPE + value.len())
        .map_err(|_| Error::Allocation)?;
    bytes.extend_from_slice(b"APREC001");
    for word in [root.id, root.participant, root.schema] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(value);
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}
fn validate(root: Root, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() < ENVELOPE
        || bytes.len() - ENVELOPE > root.capacity
        || &bytes[..8] != b"APREC001"
        || bytes[8..16] != root.id.to_le_bytes()
        || bytes[16..24] != root.participant.to_le_bytes()
        || bytes[24..32] != root.schema.to_le_bytes()
        || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
    {
        return Err(Error::Layout);
    }
    Ok(())
}
