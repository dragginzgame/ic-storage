//! Exclusive, allocation-free mutation of caller-owned M0 images.
//!
//! The caller supplies only retained bytes, never the stack or transient region.
//! The slice contains a physical prefix plus optional uncommitted capacity. Growth
//! zeroes newly committed bytes and never reallocates the backing slice.
//! Session handles are generative and cannot cross sessions or escape in futures.

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use super::layout::{CAPACITY, COUNTER, DESCRIPTORS, INDEX_BASE, INDEX_STRIDE, NODE, PAGE};
use super::{
    AdmissionError, ObjectReference, Summary, bytes, inspect_retained as inspect, metadata_digest,
    reference, u64_at,
};

// Slice-relative addresses; persisted layout fields remain absolute.
const META_BASE: usize = 0;
const ARENA_BASE: usize = PAGE;
const MEMORY_LIMIT: usize = super::layout::MEMORY_LIMIT - super::layout::META_BASE;

/// Refusal from the image model. Ordinary validation/resource errors precede writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Existing bytes fail M0 admission, or a requested physical size is invalid.
    Admission(AdmissionError),
    /// Fresh initialization requires the entire retained prefix to be zero.
    NotFresh,
    /// Backing capacity or the physical-page request is invalid.
    Capacity,
    /// The current committed arena cannot fit another complete object.
    ArenaFull,
    /// All 1024 allocation identities are already in use.
    IndexFull,
    /// A handle does not match the admitted index/schema in this session.
    Handle,
    /// A prior panic or unexpected mutation fault left this owner poisoned.
    Poisoned,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "M0 image operation refused: {self:?}")
    }
}
impl core::error::Error for Error {}
impl From<AdmissionError> for Error {
    fn from(value: AdmissionError) -> Self {
        Self::Admission(value)
    }
}

// Invariance prevents shortening an outer session's handle into an inner one.
type Brand<'id> = PhantomData<fn(&'id mut ()) -> &'id mut ()>;

/// A Node identity bound to exactly one update session. Contains no Rust pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Node<'id> {
    reference: ObjectReference,
    brand: Brand<'id>,
}

impl Node<'_> {
    /// Export the durable identity as data, not as a reusable access capability.
    #[must_use]
    pub const fn reference(self) -> ObjectReference {
        self.reference
    }
}

/// A CounterState identity bound to exactly one update session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Counter<'id> {
    reference: ObjectReference,
    brand: Brand<'id>,
}

impl Counter<'_> {
    /// Export the durable identity as data, not as a reusable access capability.
    #[must_use]
    pub const fn reference(self) -> ObjectReference {
        self.reference
    }
}

/// Exclusive custody of a caller-owned backing buffer and its physical prefix.
///
/// This owner performs no allocation and exposes no mutable byte access. A panic
/// during `with_update` leaves it poisoned. Dropping it returns custody to the
/// caller; reopening bytes is explicit readmission, never automatic rollback.
pub struct Image<'memory> {
    backing: &'memory mut [u8],
    length: usize,
    poisoned: bool,
}

fn physical_length(backing: &[u8], pages: u32) -> Result<usize, Error> {
    if !(ARENA_BASE..=MEMORY_LIMIT).contains(&backing.len())
        || !backing.len().is_multiple_of(PAGE)
        || !(129..=256).contains(&pages)
    {
        return Err(Error::Capacity);
    }
    let length = (pages as usize - 128) * PAGE;
    if length > backing.len() {
        return Err(Error::Capacity);
    }
    Ok(length)
}

fn put32(memory: &mut [u8], at: usize, value: u32) {
    memory[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(memory: &mut [u8], at: usize, value: u64) {
    memory[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn put_reference(memory: &mut [u8], at: usize, value: Option<ObjectReference>) {
    let value = value.unwrap_or(ObjectReference {
        offset: 0,
        identity: 0,
    });
    put64(memory, at, value.offset);
    put64(memory, at + 8, value.identity);
}
fn seal(memory: &mut [u8]) {
    let hash = metadata_digest(&memory[META_BASE..ARENA_BASE]);
    memory[META_BASE + 160..META_BASE + 192].copy_from_slice(&hash);
}

impl<'memory> Image<'memory> {
    /// Admit an existing physical image without changing it, even on refusal.
    /// The slice starts at linear address 8 MiB; `physical_pages` counts the
    /// whole linear memory including its 128 transient pages. Bytes beyond that
    /// retained prefix are uncommitted capacity, not state.
    ///
    /// # Errors
    /// Refuses invalid capacity/extent or any failed M0 admission. Zero metadata
    /// is invalid here; this function never initializes or repairs an image.
    pub fn open(backing: &'memory mut [u8], physical_pages: u32) -> Result<Self, Error> {
        let length = physical_length(backing, physical_pages)?;
        inspect(&backing[..length])?;
        Ok(Self {
            backing,
            length,
            poisoned: false,
        })
    }

    /// Initialize explicitly fresh zeroed retained storage. The slice starts at
    /// linear address 8 MiB and must exclude all transient bytes.
    /// The caller must know this is a fresh installation, not an upgrade. The zero
    /// check is an additional overwrite guard, not proof of freshness.
    ///
    /// # Errors
    /// Refuses invalid capacity or any nonzero byte in the physical retained
    /// region before writing. Existing or unknown data is never reset.
    pub fn initialize_fresh(
        backing: &'memory mut [u8],
        physical_pages: u32,
    ) -> Result<Self, Error> {
        let length = physical_length(backing, physical_pages)?;
        if backing[META_BASE..length].iter().any(|&byte| byte != 0) {
            return Err(Error::NotFresh);
        }
        let meta = &mut backing[META_BASE..ARENA_BASE];
        meta[..8].copy_from_slice(b"ICOPM0\0\0");
        meta[16..32].copy_from_slice(b"op.m0.layout.v01");
        for (at, value) in [
            (8, 1u32),
            (12, 256),
            (92, 1024),
            (96, 256),
            (100, 2),
            (104, 128),
            (108, 512),
            (112, 1),
            (116, 64),
            (120, 1024),
            (124, 32),
            (128, 1),
        ] {
            put32(meta, at, value);
        }
        for (at, value) in [
            (32, super::layout::META_BASE as u64),
            (40, PAGE as u64),
            (48, super::layout::ARENA_BASE as u64),
            (56, (MEMORY_LIMIT - ARENA_BASE) as u64),
            (64, (length - ARENA_BASE) as u64),
            (80, 1),
        ] {
            put64(meta, at, value);
        }
        meta[256..512].copy_from_slice(&DESCRIPTORS);
        meta[512..528].copy_from_slice(b"op.m0.root.v1\0\0\0");
        meta[528..544].copy_from_slice(COUNTER);
        seal(backing);
        Self::open(backing, physical_pages)
    }

    /// Borrow the physical image for evidence or inspection, including after a
    /// poison event. This does not return handles or authorize further mutation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.backing[..self.length]
    }

    /// Whether update access has been withheld after an unexpected failure.
    #[must_use]
    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Run one synchronous, generatively branded update session.
    ///
    /// This is a borrow scope, not a transaction. If `f` returns an ordinary
    /// `Err`, earlier successful operations remain applied and that result is
    /// returned inside `Ok`. Panic unwinding poisons this owner without restoring
    /// any bytes. There is no automatic reset or unpoison operation.
    ///
    /// # Errors
    /// Refuses an already poisoned owner or failed admission; unexpected faults
    /// after mutation begins poison the owner even if `f` catches their error.
    ///
    /// Build a shared, cyclic graph, then resolve fresh handles in a later session:
    /// ```
    /// use ic_storage_m0::m0::image::{Error, Image};
    /// let mut memory = vec![0; 2 * 65536];
    /// let mut image = Image::initialize_fresh(&mut memory, 129)?;
    /// image.with_update(|s| -> Result<(), Error> {
    ///     s.grow(1)?;
    ///     let node = s.allocate_node(7, None)?;
    ///     s.set_node_next(node, Some(node))?;
    ///     let root = s.allocate_counter(41, Some(node), Some(node))?;
    ///     s.publish_root(root)
    /// })??;
    /// image.with_update(|s| -> Result<(), Error> {
    ///     let root = s.root()?.unwrap();
    ///     let left = s.left(root)?.unwrap();
    ///     s.set_node_value(left, 42)?;
    ///     assert_eq!(s.node_value(s.right(root)?.unwrap())?, 42);
    ///     assert_eq!(s.node_next(left)?, Some(left));
    ///     Ok(())
    /// })??;
    /// # Ok::<(), Error>(())
    /// ```
    ///
    /// Handles cannot escape their session:
    /// ```compile_fail
    /// use ic_storage_m0::m0::image::Image;
    /// let mut memory = vec![0; 2 * 65536];
    /// let mut image = Image::initialize_fresh(&mut memory, 130).unwrap();
    /// let escaped = image.with_update(|s| s.allocate_node(7, None).unwrap()).unwrap();
    /// ```
    /// A returned future cannot capture session access:
    /// ```compile_fail
    /// use ic_storage_m0::m0::image::Image;
    /// let mut memory = vec![0; 2 * 65536];
    /// let mut image = Image::initialize_fresh(&mut memory, 130).unwrap();
    /// let future = image.with_update(|s| async move { s.allocate_node(7, None) });
    /// ```
    /// Handles from another image/session cannot be passed into this one:
    /// ```compile_fail
    /// use ic_storage_m0::m0::image::Image;
    /// let mut a = vec![0; 2 * 65536];
    /// let mut b = vec![0; 2 * 65536];
    /// let mut a = Image::initialize_fresh(&mut a, 130).unwrap();
    /// let mut b = Image::initialize_fresh(&mut b, 130).unwrap();
    /// a.with_update(|outer| {
    ///     let node = outer.allocate_node(7, None).unwrap();
    ///     b.with_update(|inner| inner.set_node_value(node, 9)).unwrap();
    /// }).unwrap();
    /// ```
    pub fn with_update<R>(
        &mut self,
        f: impl for<'id> FnOnce(&mut Session<'id>) -> R,
    ) -> Result<R, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let admitted = inspect(self.bytes())?;
        // Set before invoking user code. Unwinding cannot skip into the success
        // path; no catch_unwind/std dependency is required to preserve poison.
        self.poisoned = true;
        let mut faulted = false;
        let result = f(&mut Session {
            backing: self.backing,
            length: &mut self.length,
            faulted: &mut faulted,
            admitted,
            brand: PhantomData,
        });
        if faulted {
            return Err(Error::Poisoned);
        }
        inspect(self.bytes())?;
        self.poisoned = false;
        Ok(result)
    }
}

/// Exclusive access during one image update. Constructors and fields are private.
/// Raw `ObjectReference` values cannot be rebound as handles through this API.
pub struct Session<'id> {
    backing: &'id mut [u8],
    length: &'id mut usize,
    faulted: &'id mut bool,
    // Valid only under this exclusive borrow. Every write goes through commit,
    // which refreshes this after complete admission and before clearing poison.
    // Never persisted or reused by another session or reopened image.
    admitted: Summary,
    brand: Brand<'id>,
}

impl<'id> Session<'id> {
    /// Return accounting admitted at entry or after the latest successful write.
    /// Exclusive custody prevents external mutation during the session. Reads
    /// reuse this result; writes still undergo complete image admission.
    /// # Errors
    /// Refuses a poisoned session or invalid representation.
    pub fn summary(&self) -> Result<Summary, Error> {
        if *self.faulted {
            return Err(Error::Poisoned);
        }
        Ok(self.admitted)
    }

    fn handle(&self, value: ObjectReference, schema: &[u8; 16]) -> Result<usize, Error> {
        let summary = self.summary()?;
        if value.identity == 0 || value.identity > u64::from(summary.object_count) {
            return Err(Error::Handle);
        }
        let index = META_BASE + INDEX_BASE + (value.identity as usize - 1) * INDEX_STRIDE;
        if u64_at(self.backing, index)? != value.offset {
            return Err(Error::Handle);
        }
        let at = ARENA_BASE + value.offset as usize; // admitted index; bounded before conversion
        if bytes::<16>(self.backing, at + 8)? != *schema {
            return Err(Error::Handle);
        }
        Ok(at)
    }

    // Only private, preflighted write plans call this. Tests inject faults here;
    // no fallible external callback or system call occurs in actual write plans.
    fn commit(
        &mut self,
        length: usize,
        metadata: bool,
        write: impl FnOnce(&mut [u8]),
    ) -> Result<(), Error> {
        *self.faulted = true;
        write(self.backing);
        *self.length = length;
        if metadata {
            seal(self.backing);
        }
        self.admitted = inspect(&self.backing[..length])?;
        *self.faulted = false;
        Ok(())
    }

    /// Commit additional zeroed physical pages using caller-supplied capacity.
    /// Returns the previous physical page count, including for zero-page growth.
    /// # Errors
    /// Refuses invalid state or insufficient capacity before changing any bytes.
    pub fn grow(&mut self, pages: u32) -> Result<u32, Error> {
        self.summary()?;
        let old = *self.length;
        let old_pages = (old / PAGE) as u32 + 128;
        let new_pages = old_pages.checked_add(pages).ok_or(Error::Capacity)?;
        let length = physical_length(self.backing, new_pages)?;
        if pages != 0 {
            self.commit(length, true, |memory| {
                memory[old..length].fill(0);
                put64(memory, META_BASE + 64, (length - ARENA_BASE) as u64);
            })?;
        }
        Ok(old_pages)
    }

    fn allocate(&mut self, schema: &[u8; 16], payload: &[u8]) -> Result<ObjectReference, Error> {
        let summary = self.summary()?;
        if summary.object_count == CAPACITY as u32 {
            return Err(Error::IndexFull);
        }
        let span = 48 + payload.len() as u64;
        let end = summary
            .used_bytes
            .checked_add(span)
            .ok_or(Error::ArenaFull)?;
        if end > summary.committed_bytes {
            return Err(Error::ArenaFull);
        }
        let value = ObjectReference {
            offset: summary.used_bytes,
            identity: summary.next_identity,
        };
        let at = ARENA_BASE + value.offset as usize;
        let index = META_BASE + INDEX_BASE + summary.object_count as usize * INDEX_STRIDE;
        self.commit(*self.length, true, |memory| {
            memory[at..at + span as usize].fill(0);
            put64(memory, at, value.identity);
            memory[at + 8..at + 24].copy_from_slice(schema);
            put64(memory, at + 24, payload.len() as u64);
            memory[at + 48..at + span as usize].copy_from_slice(payload);
            put64(memory, index, value.offset);
            put64(memory, index + 8, value.identity);
            put64(memory, index + 16, span);
            put64(memory, META_BASE + 72, end);
            put64(memory, META_BASE + 80, value.identity + 1);
            put32(memory, META_BASE + 88, summary.object_count + 1);
        })?;
        Ok(value)
    }

    /// Append a Node; nullable edges may later be changed to create cycles.
    /// # Errors
    /// Refuses invalid state/handles or exhausted index/arena before mutation.
    pub fn allocate_node(
        &mut self,
        value: u64,
        next: Option<Node<'id>>,
    ) -> Result<Node<'id>, Error> {
        if let Some(next) = next {
            self.handle(next.reference, NODE)?;
        }
        let mut payload = [0; 24];
        put64(&mut payload, 0, value);
        put_reference(&mut payload, 8, next.map(|n| n.reference));
        Ok(Node {
            reference: self.allocate(NODE, &payload)?,
            brand: PhantomData,
        })
    }

    /// Append CounterState with zero, one or two references to existing Nodes.
    /// # Errors
    /// Refuses invalid state/handles or exhausted index/arena before mutation.
    pub fn allocate_counter(
        &mut self,
        value: u64,
        left: Option<Node<'id>>,
        right: Option<Node<'id>>,
    ) -> Result<Counter<'id>, Error> {
        for node in [left, right].into_iter().flatten() {
            self.handle(node.reference, NODE)?;
        }
        let mut payload = [0; 40];
        put64(&mut payload, 0, value);
        put_reference(&mut payload, 8, left.map(|n| n.reference));
        put_reference(&mut payload, 24, right.map(|n| n.reference));
        Ok(Counter {
            reference: self.allocate(COUNTER, &payload)?,
            brand: PhantomData,
        })
    }

    /// Publish a completely validated CounterState as the root and refresh integrity.
    /// # Errors
    /// Refuses invalid state or a mismatched handle before mutation.
    pub fn publish_root(&mut self, counter: Counter<'id>) -> Result<(), Error> {
        self.handle(counter.reference, COUNTER)?;
        self.commit(*self.length, true, |memory| {
            put_reference(memory, META_BASE + 544, Some(counter.reference))
        })
    }

    /// Resolve this image's root into this session; null remains a missing root.
    /// # Errors
    /// Refuses poisoned or incompatible state without initialization.
    pub fn root(&self) -> Result<Option<Counter<'id>>, Error> {
        Ok(self.summary()?.root.map(|reference| Counter {
            reference,
            brand: PhantomData,
        }))
    }

    /// Read a Node's scalar value.
    /// # Errors
    /// Refuses invalid state or a mismatched handle.
    pub fn node_value(&self, node: Node<'id>) -> Result<u64, Error> {
        Ok(u64_at(
            self.backing,
            self.handle(node.reference, NODE)? + 48,
        )?)
    }

    /// Change a Node's scalar without changing metadata or its digest.
    /// # Errors
    /// Refuses invalid state or a mismatched handle before mutation.
    pub fn set_node_value(&mut self, node: Node<'id>, value: u64) -> Result<(), Error> {
        let at = self.handle(node.reference, NODE)? + 48;
        self.commit(*self.length, false, |memory| put64(memory, at, value))
    }

    /// Read a CounterState's scalar value.
    /// # Errors
    /// Refuses invalid state or a mismatched handle.
    pub fn counter_value(&self, counter: Counter<'id>) -> Result<u64, Error> {
        Ok(u64_at(
            self.backing,
            self.handle(counter.reference, COUNTER)? + 48,
        )?)
    }

    /// Change a CounterState scalar without changing metadata or its digest.
    /// # Errors
    /// Refuses invalid state or a mismatched handle before mutation.
    pub fn set_counter_value(&mut self, counter: Counter<'id>, value: u64) -> Result<(), Error> {
        let at = self.handle(counter.reference, COUNTER)? + 48;
        self.commit(*self.length, false, |memory| put64(memory, at, value))
    }

    fn node_field(&self, at: usize) -> Result<Option<Node<'id>>, Error> {
        let summary = self.summary()?;
        Ok(reference(
            self.backing,
            &self.backing[META_BASE..ARENA_BASE],
            summary.object_count,
            at,
            NODE,
        )?
        .map(|reference| Node {
            reference,
            brand: PhantomData,
        }))
    }

    /// Read a Node's nullable next edge, preserving self-cycles and shared identity.
    /// # Errors
    /// Refuses invalid state or a mismatched handle.
    pub fn node_next(&self, node: Node<'id>) -> Result<Option<Node<'id>>, Error> {
        self.node_field(self.handle(node.reference, NODE)? + 56)
    }

    /// Replace a Node's next edge; all targets belong to this session.
    /// # Errors
    /// Refuses invalid state or handles before mutation.
    pub fn set_node_next(&mut self, node: Node<'id>, next: Option<Node<'id>>) -> Result<(), Error> {
        let at = self.handle(node.reference, NODE)? + 56;
        if let Some(next) = next {
            self.handle(next.reference, NODE)?;
        }
        self.commit(*self.length, false, |memory| {
            put_reference(memory, at, next.map(|n| n.reference))
        })
    }

    /// Read CounterState's nullable left edge.
    /// # Errors
    /// Refuses invalid state or a mismatched handle.
    pub fn left(&self, counter: Counter<'id>) -> Result<Option<Node<'id>>, Error> {
        self.node_field(self.handle(counter.reference, COUNTER)? + 56)
    }

    /// Read CounterState's nullable right edge.
    /// # Errors
    /// Refuses invalid state or a mismatched handle.
    pub fn right(&self, counter: Counter<'id>) -> Result<Option<Node<'id>>, Error> {
        self.node_field(self.handle(counter.reference, COUNTER)? + 72)
    }
}
