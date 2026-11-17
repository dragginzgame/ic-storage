//! Supplier-owned allocation and lifecycle for bounded native ordered roots.
//! Host backing is process-local; Wasm requires the matching prepared artifact.
//! Private Rust headers stay transient. Ordinary scopes do not re-admit tables.
mod backing;
#[cfg(target_arch = "wasm32")]
mod canister;
mod directory;
mod pool;
#[cfg(target_arch = "wasm32")]
pub use canister::Canister;
#[cfg(test)]
mod tests;
pub use crate::ordered::session::Root;
use crate::ordered::{
    self, Map,
    session::{self, Participant, Session},
};
use alloc::{
    rc::{Rc, Weak},
    vec::Vec,
};
use backing::Backing;
use core::cell::RefCell;
use directory::Image;
#[doc(hidden)]
pub use pool::{Element, NativeAllocator};
type Node = ordered::Node<512, 16384>;
const _: () = assert!(size_of::<Node>() == 56 && align_of::<Node>() == 8);
const DIRECTORY_BYTES: usize = 4096;
const MAX_ROOTS: usize = 16;
type Nodes = NativeAllocator<Node>;
type Bytes = NativeAllocator<u8>;
type Native = Map<512, 16384, Nodes, Bytes>;
type Family = Session<512, 16384, Nodes, Bytes>;
/// Read-only scoped access to granted native roots; references cannot escape.
pub type Read<'a> = session::Read<'a, 512, 16384, Nodes, Bytes>;
/// Synchronous scoped mutation. Ordinary Err retains earlier writes; unwind
/// poisons the host family. The IC supplies transaction rollback on traps.
pub type Write<'a> = session::Write<'a, 512, 16384, Nodes, Bytes>;

/// Disjoint retained domains under the supported Wasm runtime contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bank {
    /// First native allocation domain.
    A,
    /// Separate domain for explicit portable reconstruction.
    B,
}
impl Bank {
    fn base(self) -> usize {
        match self {
            Self::A => backing::A,
            Self::B => backing::B,
        }
    }
}
/// Physical native capacity, separate from per-root logical limits. Capacities
/// include growth coexistence and fragmentation; per-root maxima need not all fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    /// Native allocation domain; cannot change on compatible reopen.
    pub bank: Bank,
    /// Total 56-byte node slots across every map, from 1 through 16,384.
    pub nodes: usize,
    /// Shared byte-payload capacity, from 1 through 15 MiB.
    pub bytes: usize,
}
impl Geometry {
    fn check(self) -> Result<(), Error> {
        if !(1..=16384).contains(&self.nodes) || !(1..=15 * 1024 * 1024).contains(&self.bytes) {
            Err(Error::Geometry)
        } else {
            Ok(())
        }
    }
}
/// Allocation, admission or lifecycle refusal. No failed reopen initializes data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Unsupported physical capacity or invalid/overlapping retained spans.
    Geometry,
    /// Invalid or changed root/schema/bounds declarations.
    Declaration,
    /// Missing, corrupt, open or incompatible retained directory.
    Directory,
    /// Explicit creation found existing retained bytes.
    NotFresh,
    /// Allocation refused before ownership publication.
    Allocation,
    /// An active callback prevents lifecycle/access reentry.
    Busy,
    /// Wrong lifecycle phase.
    State,
    /// Foreign, stale or already issued participant identity.
    Authority,
    /// Public collection/session refusal, including host unwind poisoning.
    Session(session::Error),
    /// Wasm was not prepared for this runtime contract.
    Startup,
}
impl From<session::Error> for Error {
    fn from(error: session::Error) -> Self {
        Self::Session(error)
    }
}
impl From<ordered::Error> for Error {
    fn from(error: ordered::Error) -> Self {
        Self::Session(session::Error::Collection(error))
    }
}
/// Non-cloneable participant capability. Close revokes it; reopening never
/// revives stale grants, even if a caller kept or forgot their token.
pub struct Grant {
    identity: Weak<()>,
    index: usize,
}
struct Holder {
    id: u64,
    issued: bool,
    grant: Participant<512, 16384, Nodes, Bytes>,
}
struct Open {
    identity: Rc<()>,
    family: Family,
    holders: Vec<Holder>,
    nodes: Nodes,
    bytes: Bytes,
}
/// Native host owner and internal canister engine. No pointer, allocator or map
/// ownership is exposed. Host construction owns backing until all native owners end.
///
/// ```
/// use ic_storage::retained::{Bank, Geometry, Root, Runtime};
/// let roots = [Root { id: 1, participant: 1, schema: 1, entries: 32,
///     key_bytes: 512, value_bytes: 4096 }];
/// let runtime = Runtime::host(Geometry { bank: Bank::A, nodes: 128,
///     bytes: 65536 }, &roots).unwrap();
/// let grant = runtime.grant(1).unwrap();
/// runtime.update(&[&grant], |mut v| v.insert(1, b"key", b"value").unwrap()).unwrap();
/// runtime.close().unwrap();
/// runtime.reopen(&roots).unwrap();
/// assert!(runtime.read(&[&grant], |_| ()).is_err());
/// let next = runtime.grant(1).unwrap();
/// runtime.read(&[&next], |v| assert!(v.contains_key(1, b"key").unwrap())).unwrap();
/// ```
///
/// Borrowed data cannot outlive its runtime scope:
/// ```compile_fail
/// use ic_storage::retained::{Grant, Runtime};
/// fn escape(runtime: &Runtime, grant: &Grant) {
///     let value = runtime.read(&[grant], |v| v.get(1, b"key").unwrap());
/// }
/// ```
pub struct Runtime {
    backing: Rc<Backing>,
    roots: Vec<Root>,
    state: RefCell<Option<Open>>,
}
fn declarations(roots: &[Root]) -> Result<(), Error> {
    if roots.is_empty() || roots.len() > MAX_ROOTS {
        return Err(Error::Declaration);
    }
    let mut prior = 0;
    for root in roots {
        if root.id <= prior
            || root.participant == 0
            || root.schema == 0
            || root.key_bytes > 512
            || root.value_bytes > 16384
        {
            return Err(Error::Declaration);
        }
        prior = root.id;
    }
    Ok(())
}
impl Runtime {
    /// Create fresh process-local host backing with the same allocator and public
    /// scopes as the canister binding. Does not model process restart or IC rollback.
    ///
    /// # Errors
    /// Refuses invalid declarations/capacity or allocation failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn host(geometry: Geometry, roots: &[Root]) -> Result<Self, Error> {
        declarations(roots)?;
        Self::create(Backing::host(geometry)?, roots)
    }
    fn create(backing: Rc<Backing>, roots: &[Root]) -> Result<Self, Error> {
        declarations(roots)?;
        if !backing.fresh() {
            return Err(Error::NotFresh);
        }
        let runtime = Self {
            backing,
            roots: roots.to_vec(),
            state: RefCell::new(None),
        };
        runtime.backing.publish(directory::encode(
            runtime.backing.geometry,
            roots,
            &[Image::default(); MAX_ROOTS][..roots.len()],
            true,
        ));
        runtime.reopen(roots)?;
        Ok(runtime)
    }
    /// Reopen closed roots under exactly the expected declarations. Native
    /// admission occurs here, once per reopening, never on ordinary access.
    ///
    /// # Errors
    /// Refuses active/poisoned owners, changed declarations or invalid retained data.
    pub fn reopen(&self, expected: &[Root]) -> Result<(), Error> {
        if expected != self.roots {
            return Err(Error::Declaration);
        }
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::Busy)?;
        if state.is_some() {
            return Err(Error::State);
        }
        let images = directory::decode(&self.backing.header(), self.backing.geometry, &self.roots)?;
        let node_spans: Vec<_> = images[..self.roots.len()]
            .iter()
            .filter(|i| i.nodes.cap != 0)
            .map(|i| i.nodes)
            .collect();
        let byte_spans: Vec<_> = images[..self.roots.len()]
            .iter()
            .filter(|i| i.bytes.cap != 0)
            .map(|i| i.bytes)
            .collect();
        let nodes = Nodes::claim(self.backing.clone(), &node_spans)?;
        let bytes = Bytes::claim(self.backing.clone(), &byte_spans)?;
        let mut maps = Vec::new();
        maps.try_reserve_exact(self.roots.len())
            .map_err(|_| Error::Allocation)?;
        let (mut n, mut b) = (0, 0);
        for (root, image) in self.roots.iter().copied().zip(images) {
            let map = if image.nodes.cap == 0 {
                Native::new_in_allocators(root.entries, nodes.clone(), bytes.clone())
            } else {
                let node_vec = nodes.restore(n)?;
                n += 1;
                let byte_vec = if image.bytes.cap == 0 {
                    Vec::new_in(bytes.clone())
                } else {
                    let value = bytes.restore(b)?;
                    b += 1;
                    value
                };
                Native::admit(
                    ordered::Image {
                        nodes: node_vec,
                        payload: byte_vec,
                        root: image.root,
                        free: image.free,
                        len: image.len,
                    },
                    root.entries,
                )
                .map_err(|e| Error::from(e.error))?
            };
            maps.push((root, map));
        }
        let mut family = Session::from_maps(maps, &self.roots).map_err(|e| Error::from(e.error))?;
        let mut holders: Vec<Holder> = Vec::new();
        holders
            .try_reserve_exact(self.roots.len())
            .map_err(|_| Error::Allocation)?;
        for root in &self.roots {
            if !holders.iter().any(|h| h.id == root.participant) {
                holders.push(Holder {
                    id: root.participant,
                    issued: false,
                    grant: family.take(root.participant)?,
                });
            }
        }
        *state = Some(Open {
            identity: Rc::new(()),
            family,
            holders,
            nodes,
            bytes,
        });
        self.backing.publish(directory::encode(
            self.backing.geometry,
            &self.roots,
            &images[..self.roots.len()],
            false,
        ));
        Ok(())
    }
    /// Issue a participant once in this generation. Dropping a token does not
    /// reissue it. Close/reopen creates a new generation and revokes old tokens.
    ///
    /// # Errors
    /// Refuses a closed/busy runtime, unknown identity or duplicate issuance.
    pub fn grant(&self, participant: u64) -> Result<Grant, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::Busy)?;
        let open = state.as_mut().ok_or(Error::State)?;
        let index = open
            .holders
            .iter()
            .position(|h| h.id == participant)
            .ok_or(Error::Authority)?;
        if open.holders[index].issued {
            return Err(Error::Authority);
        }
        open.holders[index].issued = true;
        Ok(Grant {
            identity: Rc::downgrade(&open.identity),
            index,
        })
    }
    fn indices(open: &Open, grants: &[&Grant]) -> Result<Vec<usize>, Error> {
        if grants.is_empty() || grants.len() > MAX_ROOTS {
            return Err(Error::Authority);
        }
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(grants.len())
            .map_err(|_| Error::Allocation)?;
        for grant in grants {
            if !grant.identity.ptr_eq(&Rc::downgrade(&open.identity))
                || indices.contains(&grant.index)
                || grant.index >= open.holders.len()
            {
                return Err(Error::Authority);
            }
            indices.push(grant.index);
        }
        Ok(indices)
    }
    /// Read composed grants without scanning or readmitting native tables.
    ///
    /// # Errors
    /// Refuses closed, busy, foreign/stale/duplicate grants or host poisoning.
    pub fn read<R>(
        &self,
        grants: &[&Grant],
        operation: impl for<'a> FnOnce(Read<'a>) -> R,
    ) -> Result<R, Error> {
        let state = self.state.try_borrow().map_err(|_| Error::Busy)?;
        let open = state.as_ref().ok_or(Error::State)?;
        let indices = Self::indices(open, grants)?;
        let grants: Vec<_> = indices.iter().map(|i| &open.holders[*i].grant).collect();
        Ok(open.family.read(&grants, operation)?)
    }
    /// Mutate composed grants synchronously. Err preserves preceding writes;
    /// escaping unwind poisons the host family. IC traps roll back the message.
    ///
    /// # Errors
    /// Refuses closed/busy state, invalid grants or poisoned native owners.
    pub fn update<R>(
        &self,
        grants: &[&Grant],
        operation: impl for<'a> FnOnce(Write<'a>) -> R,
    ) -> Result<R, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::Busy)?;
        let open = state.as_mut().ok_or(Error::State)?;
        let indices = Self::indices(open, grants)?;
        let grants: Vec<_> = indices.iter().map(|i| &open.holders[*i].grant).collect();
        Ok(open.family.update(&grants, operation)?)
    }
    /// Revoke grants, end every owning map and settle both pools before publishing
    /// the closed directory. Live callbacks and poisoned hosts cannot close.
    ///
    /// # Errors
    /// Refuses closed/busy/poisoned state or an inconsistent custody ledger.
    pub fn close(&self) -> Result<(), Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::Busy)?;
        let open = state.as_ref().ok_or(Error::State)?;
        let grants: Vec<_> = open.holders.iter().map(|h| &h.grant).collect();
        open.family.read(&grants, |_| ())?;
        drop(grants);
        let Open {
            identity: _,
            family,
            holders,
            nodes,
            bytes,
        } = state.take().ok_or(Error::State)?;
        drop(holders);
        let maps = family.into_maps().map_err(|_| Error::State)?;
        let mut images = [Image::default(); MAX_ROOTS];
        for (index, (_, map)) in maps.into_iter().enumerate() {
            let image = map.release()?;
            images[index] = Image {
                nodes: nodes.release(image.nodes)?.unwrap_or_default(),
                bytes: bytes.release(image.payload)?.unwrap_or_default(),
                root: image.root,
                free: image.free,
                len: image.len,
            };
        }
        if !nodes.settled() || !bytes.settled() {
            return Err(Error::State);
        }
        drop(nodes);
        drop(bytes);
        self.backing.publish(directory::encode(
            self.backing.geometry,
            &self.roots,
            &images[..self.roots.len()],
            true,
        ));
        Ok(())
    }
}
