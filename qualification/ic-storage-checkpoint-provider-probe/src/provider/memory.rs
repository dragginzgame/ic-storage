//! Fixture-owned stable-memory binding. The entire stable address space belongs
//! to this checkpoint store; no application allocator or legacy namespace shares it.
#[cfg(target_arch = "wasm32")]
use ic_platform_runtime::raw as ic0;
use ic_storage::checkpoint::Memory;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Default)]
pub(crate) struct StableMemory {
    _private: (),
}

#[cfg(target_arch = "wasm32")]
impl Memory for StableMemory {
    fn size(&self) -> u64 {
        ic0::stable64_size()
    }
    fn grow(&self, pages: u64) -> Option<u64> {
        ic0::stable64_grow(pages)
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        ic0::stable64_read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        ic0::stable64_write(offset, bytes);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use host::TestMemory as StableMemory;
#[cfg(test)]
pub(super) use host::TestMemory;

#[cfg(test)]
mod host {
    use super::Memory;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    #[derive(Clone, Copy, Default)]
    pub(crate) struct Counts {
        pub(crate) reads: u64,
        pub(crate) writes: u64,
        pub(crate) grows: u64,
        pub(crate) read_bytes: u64,
        pub(crate) write_bytes: u64,
        pub(crate) overflowed: bool,
    }

    #[derive(Clone, Default)]
    pub(crate) struct TestMemory {
        bytes: Rc<RefCell<Vec<u8>>>,
        counts: Rc<Cell<Counts>>,
    }

    impl TestMemory {
        pub(crate) fn reset(&self) {
            self.counts.set(Counts::default());
        }
        pub(crate) fn counts(&self) -> Counts {
            self.counts.get()
        }
    }

    impl Memory for TestMemory {
        fn size(&self) -> u64 {
            self.bytes.borrow().len() as u64 / 65_536
        }
        fn grow(&self, pages: u64) -> Option<u64> {
            let old = self.size();
            let bytes = old.checked_add(pages)?.checked_mul(65_536)?;
            if bytes > 8 * 1024 * 1024 {
                return None;
            }
            self.bytes.borrow_mut().resize(bytes as usize, 0);
            let mut counts = self.counts.get();
            counts.grows = counts.grows.checked_add(1).unwrap();
            self.counts.set(counts);
            Some(old)
        }
        fn read(&self, offset: u64, bytes: &mut [u8]) {
            bytes.copy_from_slice(
                &self.bytes.borrow()[offset as usize..offset as usize + bytes.len()],
            );
            let mut counts = self.counts.get();
            counts.reads = counts.reads.checked_add(1).unwrap();
            counts.read_bytes = counts.read_bytes.checked_add(bytes.len() as u64).unwrap();
            self.counts.set(counts);
        }
        fn write(&self, offset: u64, bytes: &[u8]) {
            self.bytes.borrow_mut()[offset as usize..offset as usize + bytes.len()]
                .copy_from_slice(bytes);
            let mut counts = self.counts.get();
            counts.writes = counts.writes.checked_add(1).unwrap();
            counts.write_bytes = counts.write_bytes.checked_add(bytes.len() as u64).unwrap();
            self.counts.set(counts);
        }
    }
}
