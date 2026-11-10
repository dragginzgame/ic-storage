//! Isolated infrastructure backings exercise Memory without allocation authority.

use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use super::{Memory, VectorMemory};
use crate::WASM_PAGE_SIZE_BYTES;

fn page_bytes() -> usize {
    usize::try_from(WASM_PAGE_SIZE_BYTES).unwrap()
}

#[test]
fn growth_preserves_bytes_and_zeroes_new_pages() {
    let memory = VectorMemory::default();
    assert_eq!(memory.size(), 0);
    assert_eq!(memory.grow(0), 0);
    assert_eq!(memory.grow(1), 0);
    memory.write(WASM_PAGE_SIZE_BYTES - 2, &[31, 32]);
    assert_eq!(memory.grow(2), 1);
    assert_eq!(memory.size(), 3);
    let mut boundary = [255; 4];
    memory.read(WASM_PAGE_SIZE_BYTES - 2, &mut boundary);
    assert_eq!(boundary, [31, 32, 0, 0]);
    assert!(
        memory.borrow()[page_bytes()..]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(memory.grow(0), 3);
}

#[test]
fn cloned_and_borrowed_handles_share_bytes() {
    let memory = VectorMemory::default();
    let clone = Rc::clone(&memory);
    assert_eq!(clone.grow(1), 0);
    memory.write(17, &[41]);
    let borrowed = &clone;
    let mut output = [0];
    Memory::read(&borrowed, 17, &mut output);
    assert_eq!(output, [41]);
    assert_eq!(memory.size(), 1);
    assert!(Rc::ptr_eq(&memory, &clone));
}

#[test]
fn refused_growth_keeps_extent_and_bytes() {
    let memory = RefCell::new(vec![7; page_bytes()]);
    let before = memory.borrow().clone();
    assert_eq!(memory.grow(u64::MAX), -1);
    assert_eq!(memory.grow(i64::MAX as u64 / WASM_PAGE_SIZE_BYTES), -1);
    assert_eq!(memory.size(), 1);
    assert_eq!(*memory.borrow(), before);
}

#[test]
fn partial_pages_are_not_accessible_or_completed_by_growth() {
    let memory = RefCell::new(vec![19; page_bytes() + 3]);
    assert_eq!(memory.size(), 1);
    assert_eq!(memory.grow(0), 1);
    assert_eq!(memory.grow(1), -1);
    memory.read(WASM_PAGE_SIZE_BYTES, &mut []);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            memory.read(WASM_PAGE_SIZE_BYTES, &mut [0]);
        }))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            memory.write(WASM_PAGE_SIZE_BYTES, &[1]);
        }))
        .is_err()
    );
    assert_eq!(*memory.borrow(), vec![19; page_bytes() + 3]);
}

#[test]
fn complete_ranges_are_checked_before_copying() {
    let memory = RefCell::new(vec![23; page_bytes()]);
    let mut destination = [91; 4];
    for offset in [WASM_PAGE_SIZE_BYTES - 2, u64::MAX - 1] {
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                memory.read(offset, &mut destination);
            }))
            .is_err()
        );
        assert_eq!(destination, [91; 4]);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                memory.write(offset, &[1, 2, 3, 4]);
            }))
            .is_err()
        );
        assert!(memory.borrow().iter().all(|byte| *byte == 23));
    }
    memory.read(WASM_PAGE_SIZE_BYTES, &mut []);
    memory.write(WASM_PAGE_SIZE_BYTES, &[]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            memory.read(WASM_PAGE_SIZE_BYTES + 1, &mut []);
        }))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            memory.write(WASM_PAGE_SIZE_BYTES + 1, &[]);
        }))
        .is_err()
    );
}

#[expect(
    unsafe_code,
    reason = "Tests raw initialization into allocated but uninitialized Vec capacity."
)]
fn raw_output<M: Memory + ?Sized>(memory: &M, offset: u64, count: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(count);
    // SAFETY: Vec owns count writable, exclusive bytes disjoint from memory.
    // Its length is published only after the raw-read contract initializes all.
    unsafe {
        memory.read_unsafe(offset, output.as_mut_ptr(), count);
        output.set_len(count);
    }
    output
}

#[test]
fn direct_raw_reads_initialize_output() {
    let memory = RefCell::new(vec![37; page_bytes()]);
    memory.write(2047, &[3, 4, 5]);
    assert_eq!(raw_output(&memory, 2047, 3), vec![3, 4, 5]);
    assert_eq!(raw_output(&memory, WASM_PAGE_SIZE_BYTES, 0), [] as [u8; 0]);
}

struct SafeOnly {
    backing: VectorMemory,
    reads: Cell<usize>,
}

impl Memory for SafeOnly {
    fn size(&self) -> u64 {
        self.backing.size()
    }

    fn grow(&self, pages: u64) -> i64 {
        self.backing.grow(pages)
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        // Reading preexisting destination values is valid for the safe API.
        let _ = std::hint::black_box(bytes.iter().fold(0_u8, |sum, byte| sum ^ byte));
        self.reads.set(self.reads.get() + 1);
        self.backing.read(offset, bytes);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        self.backing.write(offset, bytes);
    }
}

#[test]
fn default_raw_read_supports_safe_only_custom_backings() {
    let memory = SafeOnly {
        backing: Rc::new(RefCell::new(vec![67; page_bytes()])),
        reads: Cell::new(0),
    };
    assert_eq!(raw_output(&memory, 2000, 5000), vec![67; 5000]);
    assert!(memory.reads.get() > 0);
}

#[test]
#[expect(
    unsafe_code,
    reason = "Exercises rejected raw ranges with a valid initialized destination."
)]
fn default_raw_read_admits_the_whole_source_before_reading() {
    let memory = SafeOnly {
        backing: Rc::new(RefCell::new(vec![67; page_bytes()])),
        reads: Cell::new(0),
    };
    let mut destination = vec![91; 5000];
    for offset in [WASM_PAGE_SIZE_BYTES - 4000, u64::MAX - 3] {
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                // SAFETY: The destination is valid and disjoint; only the source
                // range is intentionally invalid, which must be rejected safely.
                unsafe { memory.read_unsafe(offset, destination.as_mut_ptr(), destination.len()) };
            }))
            .is_err()
        );
        assert_eq!(memory.reads.get(), 0);
        assert!(destination.iter().all(|byte| *byte == 91));
    }
}

struct RawOverride;

impl Memory for RawOverride {
    fn size(&self) -> u64 {
        1
    }

    fn grow(&self, pages: u64) -> i64 {
        if pages == 0 { 1 } else { -1 }
    }

    fn read(&self, _offset: u64, _bytes: &mut [u8]) {
        panic!("raw forwarding must preserve the override");
    }

    #[expect(
        unsafe_code,
        reason = "The override initializes the caller's valid raw destination."
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        super::admit_range(offset, count, self.size());
        // SAFETY: The caller guarantees exclusive writable destination bytes.
        unsafe { std::ptr::write_bytes(dst, 81, count) };
    }

    fn write(&self, _offset: u64, _bytes: &[u8]) {
        panic!("read-only test backing");
    }
}

#[test]
fn unsized_reference_and_rc_forward_raw_overrides() {
    let memory: Rc<dyn Memory> = Rc::new(RawOverride);
    assert_eq!(raw_output(&memory, 4, 3), vec![81; 3]);
    let borrowed: &dyn Memory = memory.as_ref();
    assert_eq!(raw_output(&borrowed, 4, 3), vec![81; 3]);
}

#[test]
#[expect(
    unsafe_code,
    reason = "Exercises raw range rejection with a valid initialized destination."
)]
fn native_raw_rejection_preserves_destination() {
    let memory = RefCell::new(vec![37; page_bytes()]);
    let mut destination = [91; 4];
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: Destination validity holds; the source range must be rejected.
            unsafe { memory.read_unsafe(WASM_PAGE_SIZE_BYTES - 2, destination.as_mut_ptr(), 4) };
        }))
        .is_err()
    );
    assert_eq!(destination, [91; 4]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            raw_output(&memory, WASM_PAGE_SIZE_BYTES + 1, 0);
        }))
        .is_err()
    );
}
