use super::{Error, PAGE_SIZE};
use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;

pub(super) mod sealed {
    pub trait Sealed {}
}

/// Storage-owned backing types. Consumers receive participant grants, not raw IO.
/// The sealed implementations prevent constructing cloned aliases to one owner.
pub trait Backing: sealed::Sealed {
    #[doc(hidden)]
    fn size(&self) -> u64;
    #[doc(hidden)]
    fn grow(&mut self, pages: u64) -> Option<u64>;
    #[doc(hidden)]
    fn read(&self, offset: u64, destination: &mut [u8]);
    #[doc(hidden)]
    fn write(&mut self, offset: u64, source: &[u8]);
}

/// Owned native test backing. It is not cloneable and cannot alias an active store.
#[derive(Default)]
pub struct HostMemory {
    bytes: Vec<u8>,
}
impl HostMemory {
    /// Admit an owned, page-aligned snapshot for explicit reopen/corruption tests.
    /// No bytes are initialized or repaired.
    ///
    /// # Errors
    /// Refuses a length that is not a whole number of backing pages.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        if !(bytes.len() as u64).is_multiple_of(PAGE_SIZE) {
            return Err(Error::Layout);
        }
        Ok(Self { bytes })
    }
    /// Inspect inactive backing. Active stores own this value exclusively.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Consume inactive backing, returning its exact bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
impl sealed::Sealed for HostMemory {}
impl Backing for HostMemory {
    fn size(&self) -> u64 {
        self.bytes.len() as u64 / PAGE_SIZE
    }
    fn grow(&mut self, pages: u64) -> Option<u64> {
        let old = self.size();
        let length = usize::try_from(old.checked_add(pages)?.checked_mul(PAGE_SIZE)?).ok()?;
        self.bytes
            .try_reserve_exact(length.checked_sub(self.bytes.len())?)
            .ok()?;
        self.bytes.resize(length, 0);
        Some(old)
    }
    fn read(&self, offset: u64, destination: &mut [u8]) {
        destination
            .copy_from_slice(&self.bytes[offset as usize..offset as usize + destination.len()]);
    }
    fn write(&mut self, offset: u64, source: &[u8]) {
        self.bytes[offset as usize..offset as usize + source.len()].copy_from_slice(source);
    }
}

pub(super) struct Memory<B>(pub(super) Rc<RefCell<B>>);
impl<B> Clone for Memory<B> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<B: Backing> crate::checkpoint::Memory for Memory<B> {
    fn size(&self) -> u64 {
        self.0.borrow().size()
    }
    fn grow(&self, pages: u64) -> Option<u64> {
        self.0.borrow_mut().grow(pages)
    }
    fn read(&self, offset: u64, destination: &mut [u8]) {
        self.0.borrow().read(offset, destination);
    }
    fn write(&self, offset: u64, source: &[u8]) {
        self.0.borrow_mut().write(offset, source);
    }
}

#[cfg(all(feature = "platform", target_arch = "wasm32"))]
mod platform;
#[cfg(all(feature = "platform", target_arch = "wasm32"))]
pub use platform::StableMemory;
