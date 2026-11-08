//! Sparse exclusive leases retain the backing claim through detached handles.
use super::{AllocationId, Error, StableKey, StorageConfig, persistence::Store};
use crate::{Memory, MemoryBacking, runtime::custody::BackingClaim};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

struct State<M: Memory> {
    // Drop the identity claim before freeing the allocation that identifies it.
    _claim: BackingClaim,
    store: RefCell<Store<Rc<M>>>,
    live: RefCell<BTreeSet<AllocationId>>,
}

pub struct Owner<M: Memory>(Rc<State<M>>);

impl<M: MemoryBacking> Owner<M> {
    pub fn init(memory: M, budget: StorageConfig) -> Result<Self, Error> {
        // Acquire before backing IO; on failure/unwind, drop the claim before
        // this retained clone releases the allocation identifying the backing.
        let memory = Rc::new(memory);
        let claim = BackingClaim::acquire(&memory).map_err(|_| Error::AlreadyClaimed)?;
        let store = if memory.size() != 0 {
            Store::open(Rc::clone(&memory), budget)
        } else {
            Store::create(Rc::clone(&memory), budget)
        }?;
        Ok(Self(Rc::new(State {
            _claim: claim,
            store: RefCell::new(store),
            live: RefCell::default(),
        })))
    }
}

impl<M: Memory> Owner<M> {
    pub fn allocation_id(&self, key: &StableKey) -> Option<AllocationId> {
        self.0.store.borrow().directory.by_key.get(key).copied()
    }
    pub fn bindings(&self) -> Vec<(StableKey, AllocationId, u64)> {
        let store = self.0.store.borrow();
        store
            .directory
            .image
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.key.clone(),
                    entry.id,
                    entry.extents.iter().map(|extent| extent.pages).sum(),
                )
            })
            .collect()
    }
    pub fn generation(&self) -> u64 {
        self.0.store.borrow().directory.image.generation
    }
    pub fn verify(&self) -> Result<(), Error> {
        self.0.store.borrow().verify()
    }
    pub fn diagnostics(&self) -> Result<crate::StorageDiagnostics, Error> {
        let mut report = self.0.store.borrow().diagnostics()?;
        report.live_leases = self.0.live.borrow().len();
        Ok(report)
    }
    pub fn bind(&self, keys: &BTreeSet<StableKey>) -> Result<(), Error> {
        let mut store = self.0.store.borrow_mut();
        store.healthy()?;
        if keys
            .iter()
            .all(|key| store.directory.by_key.contains_key(key))
        {
            return Ok(());
        }
        store.mutate(keys, &[])
    }

    pub fn lease(&self, key: &StableKey) -> Result<Lease<M>, Error> {
        let store = self.0.store.borrow();
        store.healthy()?;
        let id = *store.directory.by_key.get(key).ok_or(Error::Missing)?;
        if self.0.live.borrow().contains(&id) {
            return Err(Error::AlreadyOpen);
        }
        store.verify()?;
        self.0.live.borrow_mut().insert(id);
        Ok(Lease {
            state: Rc::clone(&self.0),
            id,
            index: store.directory.entry_index(id)?,
        })
    }
}

pub struct Lease<M: Memory> {
    state: Rc<State<M>>,
    id: AllocationId,
    // Admission resolves this once. Committed directory mutations append entries
    // and extend their extents; they never reorder or remove live bindings.
    index: usize,
}

#[cfg(not(target_arch = "wasm32"))]
pub struct PanicGuard<M: Memory> {
    state: Rc<State<M>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<M: Memory> Drop for PanicGuard<M> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.state.store.borrow().poison();
        }
    }
}

impl<M: Memory> Lease<M> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn poison_on_unwind(&self) -> PanicGuard<M> {
        PanicGuard {
            state: Rc::clone(&self.state),
        }
    }
    pub fn try_grow(&self, pages: u64) -> Result<u64, Error> {
        let mut store = self.state.store.borrow_mut();
        let previous = store.pages(self.index)?;
        previous
            .checked_add(pages)
            .ok_or(Error::ArithmeticOverflow)?;
        if pages != 0 {
            store.mutate(&BTreeSet::new(), &[(self.id, pages)])?;
        }
        Ok(previous)
    }
    #[inline]
    pub fn size(&self) -> u64 {
        self.state
            .store
            .borrow()
            .pages(self.index)
            .expect("invalid allocation lease")
    }

    #[inline]
    pub fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.state
            .store
            .borrow()
            .read_data(self.id, offset, bytes)
            .expect("allocation read");
    }

    /// # Safety
    /// The destination must satisfy `Memory::read_unsafe`'s exclusive range contract.
    #[inline]
    #[expect(
        unsafe_code,
        reason = "Forward an admitted destination through complete governed range checks"
    )]
    pub unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        // SAFETY: the same destination retains the caller's guarantees.
        unsafe {
            self.state
                .store
                .borrow()
                .read_data_unsafe(self.id, offset, dst, count)
                .expect("allocation read");
        }
    }

    #[inline]
    pub fn write(&self, offset: u64, bytes: &[u8]) {
        self.state
            .store
            .borrow()
            .write_data(self.id, offset, bytes)
            .expect("allocation write");
    }
}

impl<M: Memory> Drop for Lease<M> {
    fn drop(&mut self) {
        self.state.live.borrow_mut().remove(&self.id);
    }
}

#[cfg(test)]
mod tests;
