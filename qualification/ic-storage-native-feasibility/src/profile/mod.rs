//! Canic-shaped workload over public native session custody, not Canic's engine.
//! Logical codecs and application invariants stay here. Storage owns grants,
//! scoped access, the live maps and whole-family poisoning.

mod codec;
mod recovery;
#[cfg(test)]
mod tests;
use alloc::{alloc::Global, vec};
pub use codec::{Profile, Schema};
use core::{alloc::Allocator, ops::Bound::Unbounded};
use ic_storage::ordered::{
    AllocationUsage, Error as StorageError, Map,
    session::{self, Participant, Root, Session, Write},
};

/// Consumer-shaped cardinality bound, not a general Storage limit.
pub const LIMIT: usize = 16;

/// Application refusals and native storage faults have different failure policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Anonymous owner.
    Unauthenticated,
    /// Invalid identity, UTF-8, value, framing or application schema.
    Invalid,
    /// Existing identity belongs to someone else.
    Forbidden,
    /// One owner cannot register another identity.
    Registered,
    /// Requested record does not exist.
    Missing,
    /// Revision does not describe this transition or its exact retry.
    Revision,
    /// Consumer live-record bound reached.
    Capacity,
    /// Explicit narrowing cannot represent a source revision.
    Conversion,
    /// Both roots are unavailable after an unwind or storage fault.
    Poisoned,
    /// Native collection failure; a composed write may already have changed a root.
    Storage(StorageError),
    /// The public session refused access or declaration custody.
    Custody(session::Error),
}
impl From<StorageError> for Error {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}
impl From<session::Error> for Error {
    fn from(value: session::Error) -> Self {
        match value {
            session::Error::Poisoned => Self::Poisoned,
            session::Error::Collection(error) => Self::Storage(error),
            other => Self::Custody(other),
        }
    }
}

type Table<A> = Map<29, 512, A>;
const ROWS: u64 = 1;
const OWNERS: u64 = 2;
fn roots(schema: Schema) -> [Root; 2] {
    [
        Root {
            id: ROWS,
            participant: 10,
            schema: schema as u64,
            entries: LIMIT,
            key_bytes: 8,
            value_bytes: 512,
        },
        Root {
            id: OWNERS,
            participant: 10,
            schema: 1,
            entries: LIMIT,
            key_bytes: 29,
            value_bytes: 8,
        },
    ]
}

/// Qualification application using Storage's public native generation owner.
/// The allocator's actual retained-memory admission remains the provider's job.
pub struct State<A: Allocator + Clone = Global> {
    session: Session<29, 512, A>,
    grant: Participant<29, 512, A>,
    schema: Schema,
}

impl<A: Allocator + Clone> State<A> {
    /// Explicit fresh logical roots in one live allocator authority.
    ///
    /// # Panics
    /// Propagates allocator clone failure or an internal declaration mismatch.
    #[must_use]
    pub fn new_in(schema: Schema, allocator: A) -> Self {
        Self::admit(
            Map::new_in(LIMIT, allocator.clone()),
            Map::new_in(LIMIT, allocator),
            schema,
        )
        .expect("empty maps match the fixed profile declarations")
    }

    /// Admit application invariants after the provider has admitted both maps.
    /// Failed candidates are discarded; no published owner is mutated or repaired.
    pub fn admit(rows: Table<A>, owners: Table<A>, schema: Schema) -> Result<Self, Error> {
        let declarations = roots(schema);
        let mut session = Session::from_maps(
            vec![(declarations[0], rows), (declarations[1], owners)],
            &declarations,
        )
        .map_err(|failure| Error::from(failure.error))?;
        let grant = session.take(10)?;
        let state = Self {
            session,
            grant,
            schema,
        };
        state.validate()?;
        Ok(state)
    }

    /// End the grant before returning either map for provider settlement.
    pub fn release(self) -> Result<(Table<A>, Table<A>), Error> {
        let Self { session, grant, .. } = self;
        drop(grant);
        let mut maps = session.into_maps().map_err(|_| Error::Poisoned)?;
        let owners = maps.pop().ok_or(Error::Invalid)?.1;
        let rows = maps.pop().ok_or(Error::Invalid)?.1;
        Ok((rows, owners))
    }

    // Storage owns scope poisoning. Application policy additionally quarantines
    // a normal storage-error return because the first root may already be written.
    fn access<R>(
        &mut self,
        operation: impl for<'a> FnOnce(Write<'a, 29, 512, A, A>, Schema) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let schema = self.schema;
        let result = self.grant.update(|view| operation(view, schema))?;
        if matches!(result, Err(Error::Storage(_) | Error::Custody(_))) {
            self.session.quarantine()?;
        }
        result
    }

    /// Validate bounded iteration and exact bidirectional ownership, not every read.
    pub fn validate(&self) -> Result<(), Error> {
        self.grant.read(|view| {
            if view.len(ROWS)? > LIMIT || view.len(ROWS)? != view.len(OWNERS)? {
                return Err(Error::Invalid);
            }
            for (key, value) in view.range(ROWS, Unbounded, Unbounded)? {
                let profile = Profile::decode(value.as_slice(), self.schema)?;
                if key.as_slice() != profile.id.to_be_bytes()
                    || view
                        .get(OWNERS, profile.owner.as_slice())?
                        .map(|v| v.as_slice())
                        != Some(profile.id.to_le_bytes().as_slice())
                {
                    return Err(Error::Invalid);
                }
            }
            Ok(())
        })?
    }

    /// Authenticated lookup; authentication belongs to the embedding caller.
    pub fn get(&self, caller: &[u8], id: u64) -> Result<Profile, Error> {
        codec::identity(caller)?;
        self.grant.read(|view| {
            let bytes = view.get(ROWS, &id.to_be_bytes())?.ok_or(Error::Missing)?;
            let profile = Profile::decode(bytes.as_slice(), self.schema)?;
            if profile.owner.as_slice() != caller {
                return Err(Error::Forbidden);
            }
            Ok(profile)
        })?
    }

    /// Create/update or acknowledge an exact retry. Business refusals precede writes.
    /// Storage faults quarantine both roots; installed callers must trap on them.
    pub fn put(
        &mut self,
        caller: &[u8],
        id: u64,
        expected: u64,
        name: &str,
    ) -> Result<Profile, Error> {
        self.put_after_row(caller, id, expected, name, || {})
    }

    fn put_after_row(
        &mut self,
        caller: &[u8],
        id: u64,
        expected: u64,
        name: &str,
        after_row: impl FnOnce(),
    ) -> Result<Profile, Error> {
        let revision = expected.checked_add(1).ok_or(Error::Revision)?;
        let next = Profile::new(id, revision, caller, name)?;
        let encoded = next.encode(self.schema)?;
        self.access(|mut view, schema| {
            let read = view.read();
            if read
                .get(OWNERS, caller)?
                .is_some_and(|v| v.as_slice() != id.to_le_bytes())
            {
                return Err(Error::Registered);
            }
            if let Some(bytes) = read.get(ROWS, &id.to_be_bytes())? {
                let current = Profile::decode(bytes.as_slice(), schema)?;
                if current.owner != next.owner {
                    return Err(Error::Forbidden);
                }
                if current == next {
                    return Ok(current);
                }
                if current.revision != expected {
                    return Err(Error::Revision);
                }
            } else {
                if expected != 0 {
                    return Err(Error::Revision);
                }
                if read.len(ROWS)? == LIMIT {
                    return Err(Error::Capacity);
                }
            }
            view.insert(ROWS, &id.to_be_bytes(), encoded.as_slice())?;
            after_row();
            view.insert(OWNERS, caller, &id.to_le_bytes())?;
            Ok(next)
        })
    }

    /// Storage qualification only: delete both roots, freeing identity and payload.
    /// The current Canic API does not expose this operation.
    pub fn remove(&mut self, caller: &[u8], id: u64, expected: u64) -> Result<Profile, Error> {
        codec::identity(caller)?;
        self.access(|mut view, schema| {
            let profile = Profile::decode(
                view.read()
                    .get(ROWS, &id.to_be_bytes())?
                    .ok_or(Error::Missing)?
                    .as_slice(),
                schema,
            )?;
            if profile.owner.as_slice() != caller {
                return Err(Error::Forbidden);
            }
            if profile.revision != expected {
                return Err(Error::Revision);
            }
            view.remove(ROWS, &id.to_be_bytes())?;
            view.remove(OWNERS, caller)?;
            Ok(profile)
        })
    }

    /// Observe both actual native allocations; excludes provider reserves and scratch.
    pub fn usage(&self) -> Result<[AllocationUsage; 2], Error> {
        self.grant
            .read(|view| Ok([view.usage(ROWS)?, view.usage(OWNERS)?]))?
    }
}
