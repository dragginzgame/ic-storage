use super::{Error, Session, Shared, State};
use crate::ordered::{AllocationUsage, ByteSlice, Bytes, Mutation, Range, View};
use alloc::{alloc::Global, vec::Vec};
use core::{alloc::Allocator, ops::Bound};

/// Non-cloneable authority for a declared subset of this session's roots.
pub struct Participant<
    const K: usize,
    const V: usize,
    A: Allocator + Clone = Global,
    P: Allocator + Clone = A,
> {
    pub(super) state: Shared<K, V, A, P>,
    pub(super) mask: u64,
}
/// Borrowed read access. No allocation owner or mutable reference is exposed.
pub struct Read<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> {
    state: &'a State<K, V, A, P>,
    mask: u64,
}
/// Borrowed cross-root writes. Normal callback errors preserve preceding writes.
pub struct Write<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> {
    state: &'a mut State<K, V, A, P>,
    mask: u64,
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Participant<K, V, A, P>
{
    /// Read only granted roots without revalidating or modifying their native maps.
    ///
    /// # Errors
    /// Refuses mutable reentry or a poisoned family.
    pub fn read<R>(
        &self,
        operation: impl for<'a> FnOnce(Read<'a, K, V, A, P>) -> R,
    ) -> Result<R, Error> {
        read(&self.state, self.mask, operation)
    }
    /// Write granted roots synchronously. Unwind poisons every participant;
    /// ordinary returned errors retain earlier writes. IC rollback is separate.
    /// Views/references cannot escape, even through a future.
    ///
    /// ```compile_fail
    /// use ic_storage::ordered::session::Participant;
    /// fn escape(p: &mut Participant<29, 512>) {
    ///     let view = p.update(|view| view).unwrap();
    /// }
    /// ```
    /// ```compile_fail
    /// use ic_storage::ordered::session::Participant;
    /// fn escape(p: &Participant<29, 512>) {
    ///     let future = p.read(|view| async move { view.get(1, b"key") }).unwrap();
    /// }
    /// ```
    /// ```compile_fail
    /// use ic_storage::ordered::session::Participant;
    /// fn escape(p: &Participant<29, 512>) -> &[u8] {
    ///     p.read(|view| view.get(1, b"key").unwrap().unwrap().as_slice()).unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// Refuses active borrows or poison before invoking the callback.
    pub fn update<R>(
        &mut self,
        operation: impl for<'a> FnOnce(Write<'a, K, V, A, P>) -> R,
    ) -> Result<R, Error> {
        update(&self.state, self.mask, operation)
    }
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Session<K, V, A, P>
{
    fn grants(&self, participants: &[&Participant<K, V, A, P>]) -> Result<u64, Error> {
        let mut mask = 0;
        for participant in participants {
            if !alloc::rc::Rc::ptr_eq(&self.state, &participant.state)
                || mask & participant.mask != 0
            {
                return Err(Error::Authority);
            }
            mask |= participant.mask;
        }
        if mask == 0 {
            return Err(Error::Authority);
        }
        Ok(mask)
    }
    /// Compose reads using independently granted participants from this owner.
    ///
    /// # Errors
    /// Refuses foreign/duplicate/empty grants, mutable reentry or poison.
    pub fn read<R>(
        &self,
        participants: &[&Participant<K, V, A, P>],
        operation: impl for<'a> FnOnce(Read<'a, K, V, A, P>) -> R,
    ) -> Result<R, Error> {
        read(&self.state, self.grants(participants)?, operation)
    }
    /// Compose writes under one family poison boundary. This is not Err rollback.
    ///
    /// # Errors
    /// Refuses invalid grants, active borrows or poison before invoking user code.
    pub fn update<R>(
        &mut self,
        participants: &[&Participant<K, V, A, P>],
        operation: impl for<'a> FnOnce(Write<'a, K, V, A, P>) -> R,
    ) -> Result<R, Error> {
        update(&self.state, self.grants(participants)?, operation)
    }
}

fn read<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone, R>(
    shared: &Shared<K, V, A, P>,
    mask: u64,
    operation: impl for<'a> FnOnce(Read<'a, K, V, A, P>) -> R,
) -> Result<R, Error> {
    let state = shared.try_borrow().map_err(|_| Error::Busy)?;
    state.healthy()?;
    Ok(operation(Read {
        state: &state,
        mask,
    }))
}
fn update<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone, R>(
    shared: &Shared<K, V, A, P>,
    mask: u64,
    operation: impl for<'a> FnOnce(Write<'a, K, V, A, P>) -> R,
) -> Result<R, Error> {
    let mut state = shared.try_borrow_mut().map_err(|_| Error::Busy)?;
    state.healthy()?;
    state.poisoned = true;
    let result = operation(Write {
        state: &mut state,
        mask,
    });
    state.poisoned = false;
    Ok(result)
}

impl<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Read<'a, K, V, A, P>
{
    /// Look up one key without scanning unrelated records.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn get(&self, root: u64, key: &[u8]) -> Result<Option<ByteSlice<'a>>, Error> {
        let map = &self.state.maps[self.state.index(self.mask, root)?].1;
        let id = map.find(key);
        Ok((id != 0).then(|| ByteSlice(map.value(id))))
    }
    /// Check membership without visiting the value bytes.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn contains_key(&self, root: u64, key: &[u8]) -> Result<bool, Error> {
        Ok(self.state.maps[self.state.index(self.mask, root)?]
            .1
            .find(key)
            != 0)
    }
    /// Number of live entries in one granted root.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn len(&self, root: u64) -> Result<usize, Error> {
        Ok(self.state.maps[self.state.index(self.mask, root)?]
            .1
            .image
            .len)
    }
    /// Whether a granted root has no live records.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn is_empty(&self, root: u64) -> Result<bool, Error> {
        self.len(root).map(|len| len == 0)
    }
    /// Borrow a bounded lexical range; no buffer or full-table scan is created.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn range(
        &self,
        root: u64,
        lower: Bound<&[u8]>,
        upper: Bound<&[u8]>,
    ) -> Result<Range<'a, K, V, A, P>, Error> {
        Ok(Range::new(
            &self.state.maps[self.state.index(self.mask, root)?].1,
            lower,
            upper,
        ))
    }
    /// Traverse an exact byte prefix in either direction, with optional lazy
    /// payload access through `Range::entries`.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn prefix(&self, root: u64, prefix: &[u8]) -> Result<Range<'a, K, V, A, P>, Error> {
        Ok(Range::prefix(
            &self.state.maps[self.state.index(self.mask, root)?].1,
            prefix,
        ))
    }
    /// Export one granted root's portable logical checkpoint. The application
    /// must bind all required roots/schema/custody in its recovery envelope.
    ///
    /// # Errors
    /// Refuses an ungranted root or checkpoint allocation/size failure.
    pub fn checkpoint(&self, root: u64, epoch: u64) -> Result<Vec<u8>, Error> {
        Ok(self.state.maps[self.state.index(self.mask, root)?]
            .1
            .checkpoint(epoch)?)
    }
    /// Export a granted root as bounded pages, binding its declared root ID.
    /// Uses the map's two-pass synchronous export and one reusable page buffer.
    /// The sink receives descending page indices and must publish the returned
    /// manifest last. This does not freeze the family after this scope ends.
    ///
    /// # Errors
    /// Refuses authority before calling the sink, then reports page-encoding,
    /// allocation or sink failure without changing native records. A failed sink
    /// can leave incomplete external pages; their transaction is caller-owned.
    pub fn export_pages<E>(
        &self,
        root: u64,
        epoch: u64,
        sink: impl FnMut(usize, &[u8]) -> Result<(), E>,
    ) -> Result<crate::ordered::paged::Manifest, super::ExportError<E>> {
        let index = self
            .state
            .index(self.mask, root)
            .map_err(super::ExportError::Session)?;
        self.state.maps[index]
            .1
            .export_pages(epoch, root, sink)
            .map_err(super::ExportError::Pages)
    }
    /// Observe native allocation capacity without traversing records.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn usage(&self, root: u64) -> Result<AllocationUsage, Error> {
        self.state.usage(self.mask, root)
    }
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Write<'_, K, V, A, P>
{
    /// Read current values, including earlier writes, while borrowing this scope.
    #[must_use]
    pub fn read(&self) -> Read<'_, K, V, A, P> {
        Read {
            state: self.state,
            mask: self.mask,
        }
    }
    /// Insert/replace after enforcing this root's key/value/cardinality bounds.
    ///
    /// # Errors
    /// Refusal precedes this operation's logical writes. Earlier writes remain.
    pub fn insert(
        &mut self,
        root: u64,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<Bytes<V>>, Error> {
        let index = self.state.index(self.mask, root)?;
        let (declaration, map) = &mut self.state.maps[index];
        if key.len() > declaration.key_bytes || value.len() > declaration.value_bytes {
            return Err(Error::Collection(crate::ordered::Error::Limit));
        }
        Ok(View { map }.insert(key, value)?)
    }
    /// Delete and reclaim one record without allocating.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn remove(&mut self, root: u64, key: &[u8]) -> Result<Option<Bytes<V>>, Error> {
        let index = self.state.index(self.mask, root)?;
        Ok(View {
            map: &mut self.state.maps[index].1,
        }
        .remove(key))
    }
    /// Reclaim/scrub a record without copying or returning its old value.
    ///
    /// # Errors
    /// Refuses an ungranted root.
    pub fn remove_discard(&mut self, root: u64, key: &[u8]) -> Result<bool, Error> {
        let index = self.state.index(self.mask, root)?;
        Ok(View {
            map: &mut self.state.maps[index].1,
        }
        .remove_discard(key))
    }
    /// Preflight and apply a batch to one granted root, preserving that root's
    /// records on expected refusal. Earlier writes to other roots remain applied;
    /// this does not turn a composed callback error into a transaction rollback.
    ///
    /// # Errors
    /// Refuses an ungranted root, declared bounds, batch/entry limits or allocation.
    pub fn apply(&mut self, root: u64, mutations: &[Mutation<'_>]) -> Result<(), Error> {
        let index = self.state.index(self.mask, root)?;
        let (declaration, map) = &mut self.state.maps[index];
        if mutations.len() > crate::ordered::batch::MAX_MUTATIONS {
            return Err(Error::Collection(crate::ordered::Error::Limit));
        }
        for mutation in mutations {
            let (key, value_len) = match mutation {
                Mutation::Put(key, value) => (*key, value.len()),
                Mutation::Remove(key) => (*key, 0),
            };
            if key.len() > declaration.key_bytes || value_len > declaration.value_bytes {
                return Err(Error::Collection(crate::ordered::Error::Limit));
            }
        }
        Ok(View { map }.apply(mutations)?)
    }
}
