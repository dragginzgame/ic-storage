// Adapted from ic-stable-structures 0.7.2, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See the package LICENSE-APACHE and docs/storage/icss-source.json.
use super::{Address, Bytes, Memory, PageSize, write};
use crate::engine::btreemap::allocator::Allocator;
use crate::engine::btreemap::node::layout::{
    OVERFLOW_ADDRESS_OFFSET, OVERFLOW_MAGIC, PAGE_OVERFLOW_DATA_OFFSET, PAGE_OVERFLOW_NEXT_OFFSET,
};
use crate::engine::{types::NULL, write_u64};
use std::cmp::min;
pub struct NodeReader<'a, M: Memory> {
    pub address: Address,
    pub overflows: &'a [Address],
    pub page_size: PageSize,
    pub memory: &'a M,
}

// Note: The `Memory` interface is implemented so that helper methods such `read_u32`,
// `read_struct`, etc. can be used with a `NodeReader` directly.
impl<M: Memory> Memory for NodeReader<'_, M> {
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        // SAFETY: The caller supplies a writable count-byte destination; each
        // segment is checked against count before forwarding that same contract.
        unsafe {
            // If the read is only in the initial page, then read it directly in one go.
            // This is a performance enhancement to avoid the cost of creating a `NodeIterator`.
            if (offset + count as u64) < u64::from(self.page_size.get()) {
                self.memory
                    .read_unsafe(self.address.get() + offset, dst, count);
                return;
            }

            // The read is split across several pages. Create a `NodeIterator` to read from
            // each of the individual pages.
            let iter = NodeIterator::new(
                VirtualSegment {
                    address: Address::from(offset),
                    length: Bytes::from(count as u64),
                },
                Bytes::from(self.page_size.get()),
            );

            let mut position = 0;
            for RealSegment {
                page_idx,
                offset,
                length,
            } in iter
            {
                // SAFETY: read_unsafe() is safe to call iff `position + length <= count` since the
                // caller guarantees that we can write `count` number of bytes to `dst`.
                let len = usize::try_from(length.get()).expect("node segment length");
                assert!(position + len <= count);
                match page_idx {
                    // Initial page.
                    0 => self.memory.read_unsafe(
                        (self.address + offset).get(),
                        dst.add(position),
                        len,
                    ),
                    // Overflow page.
                    _ => self.memory.read_unsafe(
                        (self.overflows[page_idx - 1] + offset).get(),
                        dst.add(position),
                        len,
                    ),
                }

                position += len;
            }
        }
    }

    #[inline]
    fn read(&self, offset: u64, dst: &mut [u8]) {
        // SAFETY: since dst is dst.len() long, it fulfills the safety requirements of read_unsafe.
        unsafe { self.read_unsafe(offset, dst.as_mut_ptr(), dst.len()) }
    }

    fn write(&self, _: u64, _: &[u8]) {
        unreachable!("NodeReader does not support write")
    }

    fn size(&self) -> u64 {
        unreachable!("NodeReader does not support size")
    }

    fn grow(&self, _: u64) -> i64 {
        unreachable!("NodeReader does not support grow")
    }
}

#[derive(Debug)]
struct VirtualSegment {
    address: Address,
    length: Bytes,
}

struct RealSegment {
    page_idx: usize,
    offset: Bytes,
    length: Bytes,
}

// An iterator that maps a segment of a node into segments of its underlying memory.
//
// A segment in a node can map to multiple segments of memory. Here's an example:
//
// Node
// --------------------------------------------------------
//          (A) ---------- SEGMENT ---------- (B)
// --------------------------------------------------------
// ↑               ↑               ↑               ↑
// Page 0        Page 1          Page 2          Page 3
//
// The [`Node`] is internally divided into fixed-size pages. In the node's virtual address space,
// all these pages are consecutive, but in the underlying memory this may not be the case.
//
// A virtual segment would be split at the page boundaries. The example virtual segment
// above would be split into the following "real" segments:
//
//    (A, end of page 0)
//    (start of page 1, end of page 1)
//    (start of page 2, B)
//
// Each of the segments above can then be translated into the real address space by looking up
// the pages' addresses in the underlying memory.
struct NodeIterator {
    virtual_segment: VirtualSegment,
    page_size: Bytes,

    // The amount of data that can be stored in an overflow page.
    overflow_page_capacity: Bytes,
}

impl NodeIterator {
    #[inline]
    fn new(virtual_segment: VirtualSegment, page_size: Bytes) -> Self {
        Self {
            virtual_segment,
            page_size,
            overflow_page_capacity: page_size - PAGE_OVERFLOW_DATA_OFFSET,
        }
    }
}

impl Iterator for NodeIterator {
    type Item = RealSegment;

    fn next(&mut self) -> Option<Self::Item> {
        // The segment is empty. End iteration.
        if self.virtual_segment.length == Bytes::from(0u64) {
            return None;
        }

        let offset = Bytes::from(self.virtual_segment.address.get());

        // Compute the page where the segment begins.
        let page_idx = if offset < self.page_size {
            0 // The segment begins in the initial page.
        } else {
            // The segment begins in an overflow page.
            ((offset - self.page_size) / self.overflow_page_capacity).get() + 1
        };

        // Compute the length of the next real segment.
        // The length of the real segment is either up to the end of the page, or the end of the
        // virtual segment, whichever is smaller.
        let length = {
            let page_end = self.page_size + self.overflow_page_capacity * page_idx;
            min(page_end - offset, self.virtual_segment.length)
        };

        // Compute the offset within the page.
        let offset = if offset < self.page_size {
            offset
        } else {
            // The offset is in the overflow pages.
            PAGE_OVERFLOW_DATA_OFFSET + (offset - self.page_size) % self.overflow_page_capacity
        };

        // Update the virtual segment to exclude the portion we're about to return.
        self.virtual_segment.length -= length;
        self.virtual_segment.address += length;

        Some(RealSegment {
            page_idx: usize::try_from(page_idx).expect("node page index"),
            offset,
            length,
        })
    }
}
pub struct NodeWriter<'a, M: Memory> {
    address: Address,
    page_size: PageSize,
    overflows: Vec<Address>,
    allocator: &'a mut Allocator<M>,

    // The maximum offset accessed while writing to the node.
    max_offset: u64,
}

impl<'a, M: Memory> NodeWriter<'a, M> {
    pub const fn new(
        address: Address,
        overflows: Vec<Address>,
        page_size: PageSize,
        allocator: &'a mut Allocator<M>,
    ) -> Self {
        Self {
            address,
            overflows,
            page_size,
            allocator,
            max_offset: 0,
        }
    }
    pub fn finish(mut self) -> Vec<Address> {
        self.deallocate_unused_pages();
        self.overflows
    }

    pub const fn memory(&self) -> &M {
        self.allocator.memory()
    }
    pub fn write(&mut self, offset: Address, src: &[u8]) {
        // Update the max offset.
        let offset = offset.get();
        let end_offset = offset + src.len() as u64;
        self.max_offset = std::cmp::max(self.max_offset, end_offset);

        // If the write is only in the initial page, then write it directly in one go.
        // This is a performance enhancement to avoid the cost of creating a `NodeIterator`.
        if end_offset < u64::from(self.page_size.get()) {
            // Leaf child lists and empty values occupy no bytes. The node's
            // initial page is already allocated; retain max_offset accounting
            // above without entering the backing's write path for these fields.
            if !src.is_empty() {
                write(self.allocator.memory(), self.address.get() + offset, src);
            }
            return;
        }

        // Batch overflow page allocation.
        let needed_pages = usize::try_from(compute_num_overflow_pages_needed(
            end_offset,
            u64::from(self.page_size.get()),
        ))
        .expect("overflow page count");
        self.overflows
            .reserve(needed_pages.saturating_sub(self.overflows.len()));
        while self.overflows.len() < needed_pages {
            self.allocate_new_page();
        }

        let iter = NodeIterator::new(
            VirtualSegment {
                address: Address::from(offset),
                length: Bytes::from(src.len() as u64),
            },
            Bytes::from(self.page_size.get()),
        );

        let mut position = 0;
        for RealSegment {
            page_idx,
            offset,
            length,
        } in iter
        {
            let len = usize::try_from(length.get()).expect("node segment length");
            match page_idx {
                // Initial page.
                0 => write(
                    self.allocator.memory(),
                    (self.address + offset).get(),
                    &src[position..position + len],
                ),
                // Overflow page.
                _ => write(
                    self.allocator.memory(),
                    (self.overflows[page_idx - 1] + offset).get(),
                    &src[position..position + len],
                ),
            }
            position += len;
        }
    }

    pub fn write_u32(&mut self, offset: Address, val: u32) {
        self.write(offset, &val.to_le_bytes());
    }

    pub fn write_u64(&mut self, offset: Address, val: u64) {
        self.write(offset, &val.to_le_bytes());
    }

    pub fn write_struct<T>(&mut self, t: &T, addr: Address) {
        let slice = unsafe {
            core::slice::from_raw_parts(
                std::ptr::from_ref(t).cast::<u8>(),
                core::mem::size_of::<T>(),
            )
        };

        self.write(addr, slice);
    }

    // Allocates a new page and appends it to the node's overflows.
    fn allocate_new_page(&mut self) {
        let new_page = self.allocator.allocate();
        self.allocator
            .memory()
            .write(new_page.get(), &OVERFLOW_MAGIC[..]);
        write_u64(
            self.allocator.memory(),
            new_page + PAGE_OVERFLOW_NEXT_OFFSET,
            NULL.get(),
        );

        // Let the previous overflow page point to this one.
        write_u64(
            self.allocator.memory(),
            match self.overflows.last() {
                Some(prev_overflow) => {
                    // There is a previous overflow. Update its next offset.
                    *prev_overflow + PAGE_OVERFLOW_NEXT_OFFSET
                }
                None => {
                    // There is no previous overflow. Update the overflow address of the initial node.
                    self.address + OVERFLOW_ADDRESS_OFFSET
                }
            },
            new_page.get(),
        );

        self.overflows.push(new_page);
    }

    // Deallocates all the unused pages to avoid memory leaks.
    fn deallocate_unused_pages(&mut self) {
        // Compute how many overflow pages are needed.
        let overflow_pages_needed = usize::try_from(compute_num_overflow_pages_needed(
            self.max_offset,
            u64::from(self.page_size.get()),
        ))
        .expect("overflow page count");

        // There cannot be a case where we require more pages than what we currently have.
        assert!(overflow_pages_needed <= self.overflows.len());

        // Loaded chains already end in NULL; allocation initializes each new
        // tail likewise. The initial node link is written by save(). Only
        // removing pages changes which link must terminate the chain.
        if overflow_pages_needed == self.overflows.len() {
            return;
        }

        // Deallocate all the unused pages.
        while self.overflows.len() > overflow_pages_needed {
            self.allocator.deallocate(self.overflows.pop().unwrap());
        }

        // Let the last overflow page point to NULL.
        write_u64(
            self.allocator.memory(),
            match self.overflows.last() {
                Some(last_overflow) => {
                    // There is a previous overflow. Update its next offset.
                    *last_overflow + PAGE_OVERFLOW_NEXT_OFFSET
                }
                None => {
                    // There is no previous overflow. Update the overflow address of the initial node.
                    self.address + OVERFLOW_ADDRESS_OFFSET
                }
            },
            NULL.get(),
        );
    }
}

// Computes how many overflow pages are needed to store the given bytes.
const fn compute_num_overflow_pages_needed(size: u64, page_size: u64) -> u64 {
    if size > page_size {
        // The amount of data to store in the overflow pages.
        let overflow_data_len = size - page_size;

        // The amount of data that can be stored per overflow page.
        let overflow_page_capacity = page_size - PAGE_OVERFLOW_DATA_OFFSET.get();

        // Ceiling division
        overflow_data_len.div_ceil(overflow_page_capacity)
    } else {
        0
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
