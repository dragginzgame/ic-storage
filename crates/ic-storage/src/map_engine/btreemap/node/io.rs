// Derived from DFINITY ic-stable-structures 0.7.2 (Apache-2.0). See SOURCES.md.
use super::{Address, Bytes, Memory, PageSize, write};
use crate::map_engine::btreemap::allocator::Allocator;
use crate::map_engine::btreemap::node::v2::{
    OVERFLOW_ADDRESS_OFFSET, OVERFLOW_MAGIC, PAGE_OVERFLOW_DATA_OFFSET, PAGE_OVERFLOW_NEXT_OFFSET,
};
use crate::map_engine::{types::NULL, write_u64};
use std::cmp::min;

/// A `NodeReader` abstracts the `Node` as a single contiguous memory, even though internally
/// it can be composed of multiple pages.
pub struct NodeReader<'a, M: Memory> {
    pub address: Address,
    pub overflows: &'a [Address],
    pub page_size: PageSize,
    pub memory: &'a M,
}

// Note: The `Memory` interface is implemented so that helper methods such `read_u32`,
// primitive and header decoders can use a `NodeReader` directly.
impl<M: Memory> Memory for NodeReader<'_, M> {
    fn read(&self, offset: u64, dst: &mut [u8]) {
        // If the read is only in the initial page, then read it directly in one go.
        // This is a performance enhancement to avoid the cost of creating a `NodeIterator`.
        if (offset + dst.len() as u64) < self.page_size.get() as u64 {
            self.memory.read(self.address.get() + offset, dst);
            return;
        }

        // The read is split across several pages. Create a `NodeIterator` to read from
        // each of the individual pages.
        let iter = NodeIterator::new(
            VirtualSegment {
                address: Address::from(offset),
                length: Bytes::from(dst.len() as u64),
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
            let len = length.get() as usize;
            assert!(position + len <= dst.len());
            match page_idx {
                // Initial page.
                0 => self.memory.read(
                    (self.address + offset).get(),
                    &mut dst[position..position + len],
                ),
                // Overflow page.
                _ => self.memory.read(
                    (self.overflows[page_idx - 1] + offset).get(),
                    &mut dst[position..position + len],
                ),
            }

            position += len;
        }
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
            page_idx: page_idx as usize,
            offset,
            length,
        })
    }
}

/// A `NodeWriter` abstracts the `Node` into a single contiguous memory, allocating
/// and deallocating pages as needed to fit the data being written.
pub struct NodeWriter<'a, M: Memory> {
    address: Address,
    page_size: PageSize,
    overflows: Vec<Address>,
    allocator: &'a mut Allocator<M>,

    // The maximum offset accessed while writing to the node.
    max_offset: u64,
}

impl<'a, M: Memory> NodeWriter<'a, M> {
    pub fn new(
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

    /// Finished the writing process by returning the node's overflow pages.
    /// Any unused pages are deallocated to prevent memory leaks.
    pub fn finish(mut self) -> Vec<Address> {
        self.deallocate_unused_pages();
        self.overflows
    }

    pub fn memory(&self) -> &M {
        self.allocator.memory()
    }

    /// Writes `src` to the node at the provided `offset`.
    pub fn write(&mut self, offset: Address, src: &[u8]) {
        // Update the max offset.
        let offset = offset.get();
        let end_offset = offset + src.len() as u64;
        self.max_offset = std::cmp::max(self.max_offset, end_offset);

        // If the write is only in the initial page, then write it directly in one go.
        // This is a performance enhancement to avoid the cost of creating a `NodeIterator`.
        if end_offset < self.page_size.get() as u64 {
            write(self.allocator.memory(), self.address.get() + offset, src);
            return;
        }

        // Batch overflow page allocation.
        let needed_pages =
            compute_num_overflow_pages_needed(end_offset, self.page_size.get() as u64) as usize;
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
            let len = length.get() as usize;
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
            };
            position += len;
        }
    }

    pub fn write_u32(&mut self, offset: Address, val: u32) {
        self.write(offset, &val.to_le_bytes());
    }

    pub fn write_u64(&mut self, offset: Address, val: u64) {
        self.write(offset, &val.to_le_bytes());
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
        let overflow_pages_needed =
            compute_num_overflow_pages_needed(self.max_offset, self.page_size.get() as u64)
                as usize;

        // There cannot be a case where we require more pages than what we currently have.
        assert!(overflow_pages_needed <= self.overflows.len());

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
fn compute_num_overflow_pages_needed(size: u64, page_size: u64) -> u64 {
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
