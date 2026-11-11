//! Framing tests use a sparse, read-only memory substitute. Large reported
//! capacities exercise arithmetic admission without allocating those capacities.

use super::{
    STABLE_CELL_HEADER_SIZE, StableCellPayloadError, header, read_payload,
    stable_cell_payload_length, write_payload,
};
use crate::{Memory, VectorMemory, WASM_PAGE_SIZE_BYTES};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

struct HeaderMemory {
    pages: u64,
    header: [u8; STABLE_CELL_HEADER_SIZE],
    reads: Cell<usize>,
}

impl HeaderMemory {
    fn new(pages: u64, value_len: u64) -> Self {
        Self {
            pages,
            header: header(value_len),
            reads: Cell::new(0),
        }
    }
}

impl Memory for HeaderMemory {
    fn size(&self) -> u64 {
        self.pages
    }

    fn grow(&self, _: u64) -> i64 {
        panic!("read-only admission must not grow memory");
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        assert_eq!(offset, 0);
        assert_eq!(bytes.len(), STABLE_CELL_HEADER_SIZE);
        assert!(self.pages > 0);
        self.reads.set(self.reads.get() + 1);
        bytes.copy_from_slice(&self.header);
    }

    fn write(&self, _: u64, _: &[u8]) {
        panic!("read-only admission must not write memory");
    }
}

#[test]
fn supported_header_admits_zero_and_exact_ceiling_without_payload_io() {
    for length in [0, 1, 41, 65_512] {
        let memory = HeaderMemory::new(1, length);
        let length = usize::try_from(length).unwrap();
        assert_eq!(stable_cell_payload_length(&memory, length), Ok(length));
        assert_eq!(memory.reads.get(), 1);
    }
}

#[test]
fn corrupted_discriminator_version_and_reserved_bytes_are_rejected() {
    for index in 0..16 {
        let mut memory = HeaderMemory::new(1, 3);
        memory.header[index] ^= 0x80;
        let original = memory.header;
        assert_eq!(
            stable_cell_payload_length(&memory, 3),
            Err(StableCellPayloadError::NotStableCell)
        );
        assert_eq!(memory.header, original);
        assert_eq!(memory.reads.get(), 1);
    }
}

#[test]
fn empty_backing_is_rejected_before_any_read() {
    let memory = HeaderMemory::new(0, 0);
    assert_eq!(
        stable_cell_payload_length(&memory, usize::MAX),
        Err(StableCellPayloadError::NotStableCell)
    );
    assert_eq!(memory.reads.get(), 0);
}

#[test]
fn payload_beyond_capacity_is_rejected_without_mutation() {
    let available_bytes = WASM_PAGE_SIZE_BYTES - STABLE_CELL_HEADER_SIZE as u64;
    for value_len in [available_bytes + 1, u64::MAX] {
        let memory = HeaderMemory::new(1, value_len);
        let original = memory.header;
        assert_eq!(
            stable_cell_payload_length(&memory, usize::MAX),
            Err(StableCellPayloadError::InvalidLength {
                value_len,
                available_bytes,
            })
        );
        assert_eq!(memory.header, original);
        assert_eq!(memory.reads.get(), 1);
    }
}

#[test]
fn caller_ceiling_is_checked_without_payload_read_or_mutation() {
    let memory = HeaderMemory::new(2, 70_000);
    let original = memory.header;
    assert_eq!(
        stable_cell_payload_length(&memory, 69_999),
        Err(StableCellPayloadError::TooLarge { value_len: 70_000 })
    );
    assert_eq!(memory.header, original);
    assert_eq!(memory.reads.get(), 1);
}

#[test]
fn overflowing_backing_capacity_is_rejected_before_any_read() {
    for pages in [u64::MAX / WASM_PAGE_SIZE_BYTES + 1, u64::MAX] {
        let memory = HeaderMemory::new(pages, 0);
        assert_eq!(
            stable_cell_payload_length(&memory, usize::MAX),
            Err(StableCellPayloadError::NotStableCell)
        );
        assert_eq!(memory.reads.get(), 0);
    }
}

#[test]
fn host_buffer_limit_is_checked_without_payload_read() {
    let value_len = u64::try_from(isize::MAX).unwrap() + 1;
    let pages = (value_len + STABLE_CELL_HEADER_SIZE as u64).div_ceil(WASM_PAGE_SIZE_BYTES);
    let memory = HeaderMemory::new(pages, value_len);
    assert_eq!(
        stable_cell_payload_length(&memory, usize::MAX),
        Err(StableCellPayloadError::TooLarge { value_len })
    );
    assert_eq!(memory.reads.get(), 1);
}

#[test]
fn largest_representable_backing_capacity_does_not_wrap() {
    let pages = u64::MAX / WASM_PAGE_SIZE_BYTES;
    let memory = HeaderMemory::new(pages, 0);
    assert_eq!(stable_cell_payload_length(&memory, 0), Ok(0));
    assert_eq!(memory.reads.get(), 1);
}

#[derive(Default)]
struct WritableMemory {
    bytes: VectorMemory,
    sizes: Cell<usize>,
    grows: Cell<usize>,
    reads: Cell<usize>,
    writes: Cell<usize>,
    fail_read: Cell<Option<usize>>,
}

impl WritableMemory {
    fn reset_counts(&self) {
        self.sizes.set(0);
        self.grows.set(0);
        self.reads.set(0);
        self.writes.set(0);
    }
}

impl Memory for WritableMemory {
    fn size(&self) -> u64 {
        self.sizes.set(self.sizes.get() + 1);
        self.bytes.size()
    }

    fn grow(&self, pages: u64) -> i64 {
        self.grows.set(self.grows.get() + 1);
        self.bytes.grow(pages)
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.reads.set(self.reads.get() + 1);
        assert_ne!(self.fail_read.get(), Some(self.reads.get()), "read failure");
        self.bytes.read(offset, bytes);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        self.writes.set(self.writes.get() + 1);
        self.bytes.write(offset, bytes);
    }
}

struct RawPayloadMemory {
    payload: Vec<u8>,
    reads: Cell<usize>,
    fail_after: Cell<Option<usize>>,
}

impl Memory for RawPayloadMemory {
    fn size(&self) -> u64 {
        panic!("payload reading must not query this raw backing");
    }

    fn grow(&self, _: u64) -> i64 {
        panic!("payload reading must not grow memory");
    }

    fn read(&self, _: u64, _: &mut [u8]) {
        panic!("payload reading must use the backing's raw read");
    }

    #[expect(
        unsafe_code,
        reason = "The fixture initializes the caller-owned destination."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        assert_eq!(offset, STABLE_CELL_HEADER_SIZE as u64);
        assert_eq!(count, self.payload.len());
        self.reads.set(self.reads.get() + 1);
        let copied = self.fail_after.get().unwrap_or(count);
        assert!(copied <= count);
        // SAFETY: The caller supplies an exclusive destination for `count`
        // bytes, disjoint from this payload. The selected prefix fits both.
        unsafe {
            std::ptr::copy_nonoverlapping(self.payload.as_ptr(), dst, copied);
        }
        assert!(self.fail_after.get().is_none(), "partial raw read failure");
    }

    fn write(&self, _: u64, _: &[u8]) {
        panic!("payload reading must not write memory");
    }
}

fn patterned_payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

#[test]
fn empty_payload_read_performs_no_backing_operations() {
    let memory = RawPayloadMemory {
        payload: Vec::new(),
        reads: Cell::new(0),
        fail_after: Cell::new(Some(0)),
    };
    assert_eq!(read_payload(&memory, 0), [0_u8; 0]);
    assert_eq!(memory.reads.get(), 0);
}

#[test]
fn payload_reads_preserve_patterns_with_default_and_direct_readers() {
    for length in [1, 1023, 1024, 1025, 65_511, 65_512, 65_513, 65_536, 70_000] {
        let payload = patterned_payload(length);
        let memory = WritableMemory::default();
        assert!(write_payload(&memory, &payload, None));
        let admitted = stable_cell_payload_length(&memory, length).unwrap();
        let before = memory.bytes.borrow().clone();
        memory.reset_counts();
        assert_eq!(read_payload(&memory, admitted), payload);
        assert_eq!(memory.grows.get(), 0);
        assert_eq!(memory.writes.get(), 0);
        assert_eq!(*memory.bytes.borrow(), before);

        let direct = RawPayloadMemory {
            payload,
            reads: Cell::new(0),
            fail_after: Cell::new(None),
        };
        assert_eq!(read_payload(&direct, admitted), direct.payload);
        assert_eq!(direct.reads.get(), 1);
    }
}

#[test]
fn partial_raw_read_failure_preserves_backing_and_allows_retry() {
    let payload = patterned_payload(4097);
    for prefix in [0, 1, 1024, 4096, 4097] {
        let memory = RawPayloadMemory {
            payload: payload.clone(),
            reads: Cell::new(0),
            fail_after: Cell::new(Some(prefix)),
        };
        assert!(catch_unwind(AssertUnwindSafe(|| read_payload(&memory, payload.len()))).is_err());
        assert_eq!(memory.payload, payload);
        memory.fail_after.set(None);
        assert_eq!(read_payload(&memory, payload.len()), payload);
        assert_eq!(memory.reads.get(), 2);
    }
}

#[test]
fn default_reader_failure_after_a_chunk_preserves_backing_and_allows_retry() {
    let payload = patterned_payload(4097);
    let memory = WritableMemory::default();
    assert!(write_payload(&memory, &payload, None));
    let admitted = stable_cell_payload_length(&memory, payload.len()).unwrap();
    let before = memory.bytes.borrow().clone();
    memory.reset_counts();
    memory.fail_read.set(Some(2));
    assert!(catch_unwind(AssertUnwindSafe(|| read_payload(&memory, admitted))).is_err());
    assert_eq!(memory.reads.get(), 2);
    assert_eq!(memory.grows.get(), 0);
    assert_eq!(memory.writes.get(), 0);
    assert_eq!(*memory.bytes.borrow(), before);
    memory.fail_read.set(None);
    assert_eq!(read_payload(&memory, admitted), payload);
}

#[test]
fn replacing_empty_payload_with_empty_payload_does_no_memory_work() {
    let memory = WritableMemory::default();
    assert!(write_payload(&memory, &[], None));
    memory.reset_counts();
    assert!(write_payload(&memory, &[], Some(0)));
    assert_eq!(memory.sizes.get(), 0);
    assert_eq!(memory.grows.get(), 0);
    assert_eq!(memory.reads.get(), 0);
    assert_eq!(memory.writes.get(), 0);
    assert_eq!(stable_cell_payload_length(&memory, 0), Ok(0));
}

#[test]
fn equal_lengths_write_only_payload_and_keep_valid_framing() {
    let memory = WritableMemory::default();
    assert!(write_payload(&memory, &[1; 17], None));
    memory.reset_counts();
    assert!(write_payload(&memory, &[2; 17], Some(17)));
    assert_eq!(memory.sizes.get(), 0);
    assert_eq!(memory.grows.get(), 0);
    assert_eq!(memory.reads.get(), 0);
    assert_eq!(memory.writes.get(), 1);
    assert_eq!(stable_cell_payload_length(&memory, 17), Ok(17));
    assert_eq!(read_payload(&memory, 17), [2; 17]);
}

#[test]
fn changed_lengths_publish_the_new_extent_across_shrink_and_growth() {
    let memory = WritableMemory::default();
    assert!(write_payload(&memory, &[3; 17], None));
    let mut previous_length = 17;
    for length in [3, 0, 9, 70_000, 0] {
        memory.reset_counts();
        let payload = vec![5; length];
        assert!(write_payload(&memory, &payload, Some(previous_length)));
        assert_eq!(memory.writes.get(), if length == 0 { 1 } else { 2 });
        if length <= previous_length {
            assert_eq!(memory.sizes.get(), 0);
            assert_eq!(memory.grows.get(), 0);
        }
        assert_eq!(stable_cell_payload_length(&memory, length), Ok(length));
        assert_eq!(read_payload(&memory, length), payload);
        previous_length = length;
    }
}
