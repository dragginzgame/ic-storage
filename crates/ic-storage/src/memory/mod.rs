//! Page-addressed backing IO for Storage infrastructure.
//!
//! A backing implements byte access, not allocation authority. Applications use
//! admitted Storage handles rather than treating this trait as a grant of access.

use std::{cell::RefCell, rc::Rc};

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

    /// Initializes the destination from the selected backing bytes.
    ///
    /// The default implementation admits the entire source range, then reads
    /// through a bounded, initialized scratch buffer. Custom backings pay an
    /// additional copy per chunk; direct implementations can override this.
    /// Failure may leave an initialized prefix in the destination.
    ///
    /// # Safety
    ///
    /// `dst` must satisfy the validity, alignment, and exclusive writable access
    /// requirements of a Rust slice of `count` bytes, including for zero length.
    /// That destination must not overlap the source or the backing's own state.
    ///
    /// # Panics
    ///
    /// Panics or traps if the complete source range is outside the backing or
    /// if the backing's safe read fails.
    #[expect(
        unsafe_code,
        reason = "The raw destination is an explicit caller-owned boundary."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        admit_range(offset, count, self.size());
        let mut scratch = [0_u8; 1024];
        let mut copied = 0;
        while copied < count {
            let chunk = (count - copied).min(scratch.len());
            let source_offset = offset + u64::try_from(copied).expect("admitted offset");
            self.read(source_offset, &mut scratch[..chunk]);
            // SAFETY: The caller supplies the complete exclusive destination;
            // admission and the loop bounds keep this copy inside it. Scratch
            // contains initialized bytes and cannot overlap that destination.
            unsafe {
                std::ptr::copy_nonoverlapping(scratch.as_ptr(), dst.add(copied), chunk);
            }
            copied += chunk;
        }
    }

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

    #[expect(
        unsafe_code,
        reason = "Forwarding preserves the caller's raw-read obligations."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        // SAFETY: The destination and source obligations are unchanged.
        unsafe { (**self).read_unsafe(offset, dst, count) };
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

    #[expect(
        unsafe_code,
        reason = "Forwarding preserves the caller's raw-read obligations."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        // SAFETY: The destination and source obligations are unchanged.
        unsafe { (**self).read_unsafe(offset, dst, count) };
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

///
/// `DefaultMemoryImpl`
///
/// Native default backing, shared through reference-counted ownership.
///
#[cfg(not(target_arch = "wasm32"))]
pub type DefaultMemoryImpl = VectorMemory;

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

    #[expect(
        unsafe_code,
        reason = "Direct copying initializes caller-owned raw output without a temporary."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        let backing = self.borrow();
        let range = native_range(offset, count, backing.len());
        // SAFETY: The admitted source slice contains count initialized bytes;
        // the caller guarantees a valid, exclusive, nonoverlapping destination.
        unsafe {
            std::ptr::copy_nonoverlapping(backing[range].as_ptr(), dst, count);
        }
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

///
/// `DefaultMemoryImpl`
///
/// The IC stable-memory backing accessed through Platform Runtime's safe IC0 API.
///
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Default)]
pub struct DefaultMemoryImpl {
    _private: (),
}

#[cfg(target_arch = "wasm32")]
impl Memory for DefaultMemoryImpl {
    fn size(&self) -> u64 {
        ic_platform_runtime::ic0::stable64_size()
    }

    fn grow(&self, pages: u64) -> i64 {
        let old_pages = self.size();
        let Ok(previous) = i64::try_from(old_pages) else {
            return -1;
        };
        if pages == 0 {
            return previous;
        }
        if old_pages
            .checked_add(pages)
            .and_then(|total| total.checked_mul(WASM_PAGE_SIZE_BYTES))
            .is_none()
        {
            return -1;
        }
        i64::try_from(ic_platform_runtime::ic0::stable64_grow(pages)).unwrap_or(-1)
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        // Empty syscalls need not check their offset. Admit that case here;
        // nonempty syscalls own complete range and overflow admission.
        if bytes.is_empty() {
            admit_range(offset, 0, self.size());
            return;
        }
        ic_platform_runtime::ic0::stable64_read(bytes, offset);
    }

    #[expect(
        unsafe_code,
        reason = "MaybeUninit exposes raw writable bytes to the safe Runtime boundary."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        if count == 0 {
            admit_range(offset, 0, self.size());
            return;
        }
        // SAFETY: The caller guarantees slice validity and exclusive access.
        // MaybeUninit permits the bytes to be uninitialized on entry; Runtime
        // admits the complete nonempty stable range and initializes the entire
        // slice on success without a temporary buffer.
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(dst.cast::<std::mem::MaybeUninit<u8>>(), count)
        };
        ic_platform_runtime::ic0::stable64_read_uninit(bytes, offset);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        // Empty writes require explicit admission; nonempty syscalls check the
        // complete stable range before writing.
        if bytes.is_empty() {
            admit_range(offset, 0, self.size());
            return;
        }
        ic_platform_runtime::ic0::stable64_write(bytes, offset);
    }
}

#[cfg(test)]
mod contract_tests;
