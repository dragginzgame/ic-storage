//! Storage memory contract and Platform-backed canister storage.
cfg_select! {
    target_arch = "wasm32" => {
        mod wasm;
        pub use wasm::DefaultMemoryImpl;
    }
    _ => {
        /// Native default backing for fixture and host integration.
        pub type DefaultMemoryImpl = VectorMemory;
    }
}

use crate::WASM_PAGE_SIZE_BYTES;

///
/// Memory
///
/// Page-addressed storage with initialized reads and nonshrinking capacity.
/// Implementations validate complete access ranges before touching backing bytes.
///
pub trait Memory {
    /// Returns the number of complete accessible Wasm pages.
    fn size(&self) -> u64;

    /// Adds zero-filled pages, returning the previous size or `-1` on refusal.
    /// Refusal leaves the existing bytes and extent unchanged.
    fn grow(&self, pages: u64) -> i64;

    /// Copies the selected backing bytes into an initialized destination.
    ///
    /// # Panics
    ///
    /// Panics or traps if the complete range is outside the backing.
    fn read(&self, offset: u64, bytes: &mut [u8]);

    /// Replaces the selected backing bytes.
    ///
    /// # Panics
    ///
    /// Panics or traps if the complete range is outside the backing.
    fn write(&self, offset: u64, bytes: &[u8]);
}

impl<M: Memory + ?Sized> Memory for Rc<M> {
    fn size(&self) -> u64 {
        (**self).size()
    }

    fn grow(&self, pages: u64) -> i64 {
        (**self).grow(pages)
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        (**self).read(offset, bytes);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        (**self).write(offset, bytes);
    }
}

impl<M: Memory + ?Sized> Memory for &M {
    fn size(&self) -> u64 {
        (**self).size()
    }

    fn grow(&self, pages: u64) -> i64 {
        (**self).grow(pages)
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        (**self).read(offset, bytes);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        (**self).write(offset, bytes);
    }
}

///
/// `VectorMemory`
///
/// Shared native byte backing. Clones refer to the same physical bytes.
///
pub type VectorMemory = Rc<RefCell<Vec<u8>>>;

impl Memory for RefCell<Vec<u8>> {
    fn size(&self) -> u64 {
        u64::try_from(self.borrow().len()).expect("native byte length fits u64")
            / WASM_PAGE_SIZE_BYTES
    }

    fn grow(&self, pages: u64) -> i64 {
        let mut bytes = self.borrow_mut();
        let length = u64::try_from(bytes.len()).expect("native byte length fits u64");
        let old_pages = length / WASM_PAGE_SIZE_BYTES;
        let Ok(previous) = i64::try_from(old_pages) else {
            return -1;
        };
        if pages == 0 {
            return previous;
        }
        // An externally supplied partial page is not advertised or completed
        // implicitly: its tail remains the caller's data, outside this extent.
        if !length.is_multiple_of(WASM_PAGE_SIZE_BYTES) {
            return -1;
        }
        let Some(new_length) = old_pages
            .checked_add(pages)
            .and_then(|total| total.checked_mul(WASM_PAGE_SIZE_BYTES))
            .and_then(|total| usize::try_from(total).ok())
        else {
            return -1;
        };
        let additional = new_length - bytes.len();
        if bytes.try_reserve_exact(additional).is_err() {
            return -1;
        }
        bytes.resize(new_length, 0);
        previous
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        let backing = self.borrow();
        let range = native_range(offset, bytes.len(), backing.len());
        bytes.copy_from_slice(&backing[range]);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        let mut backing = self.borrow_mut();
        let range = native_range(offset, bytes.len(), backing.len());
        backing[range].copy_from_slice(bytes);
    }
}

// Admission precedes every copy, so overflow and out-of-range failures cannot
// leave a partially modified native destination or backing.
fn admit_range(offset: u64, count: usize, pages: u64) {
    let extent = pages
        .checked_mul(WASM_PAGE_SIZE_BYTES)
        .expect("memory extent overflow");
    let count = u64::try_from(count).expect("memory access length overflow");
    let end = offset.checked_add(count).expect("memory access overflow");
    assert!(end <= extent, "memory access outside complete pages");
}

fn native_range(offset: u64, count: usize, length: usize) -> std::ops::Range<usize> {
    let pages = u64::try_from(length).expect("native byte length fits u64") / WASM_PAGE_SIZE_BYTES;
    admit_range(offset, count, pages);
    let start = usize::try_from(offset).expect("admitted native offset");
    start..start + count
}

use std::{cell::RefCell, rc::Rc, sync::Arc};

///
/// BackingIdentity
/// Identity of a physical backing allocation, shared by all of its wrappers.
/// This is host custody metadata, not a virtual memory ID or durable record.
///

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BackingIdentity(usize);

impl BackingIdentity {
    /// Identify an Rc-owned backing. Its allocation must remain alive through
    /// every runtime/handle using this identity. Identify the underlying backing,
    /// not a newly allocated wrapper or an address inside a growable byte buffer.
    #[must_use]
    pub fn of_rc<T: ?Sized>(backing: &Rc<T>) -> Self {
        Self(Rc::as_ptr(backing).cast::<()>() as usize)
    }

    /// Identify an Arc-owned backing under the same contract as [`Self::of_rc`].
    #[must_use]
    pub fn of_arc<T: ?Sized>(backing: &Arc<T>) -> Self {
        Self(Arc::as_ptr(backing).cast::<()>() as usize)
    }
}
