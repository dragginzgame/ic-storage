//! New-format corruption and arithmetic tests with isolated in-memory backing.

use std::{cell::RefCell, panic::AssertUnwindSafe};

use crate::storable::CodecError;

use super::*;

#[derive(Default)]
struct TestMemory {
    bytes: RefCell<Vec<u8>>,
    reads: Cell<usize>,
    initializing_reads: Cell<usize>,
    initialized_bytes: Cell<usize>,
    fail_initializing_read: Cell<bool>,
    writes: Cell<usize>,
    refuse_growth: Cell<bool>,
}

impl Memory for TestMemory {
    fn size(&self) -> u64 {
        u64::try_from(self.bytes.borrow().len()).unwrap() / PAGE_BYTES
    }

    fn grow(&self, pages: u64) -> i64 {
        if self.refuse_growth.get() {
            return -1;
        }
        let previous = self.size();
        let bytes = usize::try_from((previous + pages) * PAGE_BYTES).unwrap();
        self.bytes.borrow_mut().resize(bytes, 0);
        i64::try_from(previous).unwrap()
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.reads.set(self.reads.get() + 1);
        let offset = usize::try_from(offset).unwrap();
        bytes.copy_from_slice(&self.bytes.borrow()[offset..offset + bytes.len()]);
    }

    #[expect(
        unsafe_code,
        reason = "Test the initializing-read contract and partial initialization failure"
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        self.reads.set(self.reads.get() + 1);
        self.initializing_reads
            .set(self.initializing_reads.get() + 1);
        let offset = usize::try_from(offset).unwrap();
        let backing = self.bytes.borrow();
        let source = &backing[offset..offset + count];
        let fail = self.fail_initializing_read.get();
        let initialized = if fail { count / 2 } else { count };
        // SAFETY: the caller supplies a writable, nonoverlapping destination of
        // `count` bytes. The checked source slice covers every byte copied.
        unsafe {
            std::ptr::copy_nonoverlapping(source.as_ptr(), dst, initialized);
        }
        self.initialized_bytes.set(initialized);
        assert!(!fail, "injected partial initializing-read failure");
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        self.writes.set(self.writes.get() + 1);
        let offset = usize::try_from(offset).unwrap();
        self.bytes.borrow_mut()[offset..offset + bytes.len()].copy_from_slice(bytes);
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Record(Vec<u8>);

thread_local! {
    static DECODES: Cell<usize> = const { Cell::new(0) };
}

impl Storable for Record {
    const BOUND: Bound = Bound::Bounded {
        max_size: 70_000,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> std::result::Result<Cow<'_, [u8]>, CodecError> {
        Ok(Cow::Borrowed(&self.0))
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> std::result::Result<Self, CodecError> {
        DECODES.with(|count| count.set(count.get() + 1));
        Ok(Self(bytes.into_owned()))
    }
}

fn traps(action: impl FnOnce()) {
    assert!(std::panic::catch_unwind(AssertUnwindSafe(action)).is_err());
}

#[test]
fn header_admission_preserves_length_and_rejects_unsupported_framing() {
    let layout = Layout {
        maximum: 9,
        fixed: false,
    };
    let original = layout.header(123_456);
    assert_eq!(layout.admit_header(&original).unwrap(), 123_456);
    for (offset, value) in [(0, b'!'), (8, 2), (16, 2), (17, 1), (23, 1)] {
        let mut header = original;
        header[offset] = value;
        assert!(matches!(layout.admit_header(&header), Err(Error::Format)));
    }
}

#[test]
fn header_bound_admission_traps_for_width_or_kind_mismatch() {
    let layout = Layout {
        maximum: 9,
        fixed: false,
    };
    for other in [
        Layout {
            maximum: 10,
            fixed: false,
        },
        Layout {
            maximum: 9,
            fixed: true,
        },
    ] {
        traps(|| {
            let _ = layout.admit_header(&other.header(0));
        });
    }
    traps(|| {
        let _ = Layout::for_bound(Bound::Unbounded);
    });
}

#[test]
fn slot_extents_admit_the_last_representable_record_only() {
    for layout in [
        Layout {
            maximum: 1,
            fixed: true,
        },
        Layout {
            maximum: u32::MAX,
            fixed: true,
        },
        Layout {
            maximum: u32::MAX,
            fixed: false,
        },
    ] {
        let maximum_length = (u64::MAX - DATA_OFFSET) / layout.stride();
        assert!(layout.extent(maximum_length).is_some());
        assert_eq!(layout.extent(maximum_length + 1), None);
    }
    let empty = Layout {
        maximum: 0,
        fixed: true,
    };
    assert_eq!(empty.extent(u64::MAX), Some(DATA_OFFSET));
    assert_eq!(
        empty.admit_header(&empty.header(u64::MAX)).unwrap(),
        u64::MAX
    );
}

#[test]
fn maximum_extent_page_rounding_does_not_overflow() {
    let memory = TestMemory::default();
    assert!(!contains(&memory, u64::MAX));
    assert!(reserve(&memory, PAGE_BYTES));
    assert!(contains(&memory, PAGE_BYTES));
    assert!(!contains(&memory, PAGE_BYTES + 1));
    memory.refuse_growth.set(true);
    assert!(reserve(&memory, PAGE_BYTES));
    assert!(!reserve(&memory, PAGE_BYTES + 1));
    assert_eq!(memory.size(), 1);
    assert_eq!(memory.writes.get(), 0);
}

#[test]
fn selected_variable_record_is_bounded_before_buffer_allocation() {
    let memory = TestMemory::default();
    assert!(reserve(&memory, PAGE_BYTES));
    let layout = Layout {
        maximum: 3,
        fixed: false,
    };
    memory.write(DATA_OFFSET, &u32::MAX.to_le_bytes());
    traps(|| {
        let _: Record = read_value(&memory, layout, 0);
    });
    assert_eq!(memory.reads.get(), 1);
    assert_eq!(memory.writes.get(), 1);
}

#[test]
fn missing_or_overflowing_slot_extent_refuses_before_reading() {
    let memory = TestMemory::default();
    let layout = Layout {
        maximum: 8,
        fixed: true,
    };
    for index in [0, u64::MAX / 8, u64::MAX] {
        traps(|| {
            let _: Record = read_value(&memory, layout, index);
        });
    }
    assert_eq!(memory.reads.get(), 0);
}

#[test]
fn selected_records_cross_pages_without_reading_their_neighbors() {
    let memory = TestMemory::default();
    let layout = Layout::for_bound(Record::BOUND);
    assert!(reserve(&memory, layout.extent(3).unwrap()));
    // Leave both neighbors malformed. Only the selected slot may be decoded.
    memory.write(DATA_OFFSET, &u32::MAX.to_le_bytes());
    memory.write(layout.extent(2).unwrap(), &u32::MAX.to_le_bytes());
    let record = Record(vec![0xa7; 70_000]);
    write_value(&memory, layout, 1, &encode(&record, layout));
    assert_eq!(read_value::<Record>(&memory, layout, 1), record);
    assert_eq!(memory.reads.get(), 2);
}

#[test]
fn small_large_and_empty_records_round_trip_with_fixed_or_variable_slots() {
    for width in [0, 1, 64, 65, 70_000] {
        for fixed in [false, true] {
            let memory = TestMemory::default();
            let layout = Layout {
                maximum: width,
                fixed,
            };
            assert!(reserve(&memory, layout.extent(1).unwrap()));
            let record = Record(vec![0x59; usize::try_from(width).unwrap()]);
            write_value(&memory, layout, 0, &encode(&record, layout));
            assert_eq!(read_value::<Record>(&memory, layout, 0), record);
            assert_eq!(memory.initializing_reads.get(), usize::from(width > 64));
            if width == 0 && fixed {
                assert_eq!(memory.reads.get(), 0);
                assert_eq!(memory.writes.get(), 0);
            }
        }
    }
}

#[test]
fn large_selected_record_initializes_every_byte_before_decoding() {
    let memory = TestMemory::default();
    let layout = Layout::for_bound(Record::BOUND);
    assert!(reserve(&memory, layout.extent(2).unwrap()));
    let record = Record(
        (0..70_000)
            .map(|index| u8::try_from(index % 251).unwrap())
            .collect(),
    );
    memory.write(DATA_OFFSET, &u32::MAX.to_le_bytes());
    write_value(&memory, layout, 1, &encode(&record, layout));
    DECODES.with(|count| count.set(0));
    assert_eq!(read_value::<Record>(&memory, layout, 1), record);
    assert_eq!(memory.initializing_reads.get(), 1);
    assert_eq!(memory.initialized_bytes.get(), 70_000);
    assert_eq!(memory.reads.get(), 2);
    DECODES.with(|count| assert_eq!(count.get(), 1));
}

#[test]
fn partial_initializing_read_never_publishes_bytes_to_the_decoder() {
    let memory = TestMemory::default();
    let layout = Layout {
        maximum: 65,
        fixed: true,
    };
    assert!(reserve(&memory, layout.extent(1).unwrap()));
    let record = Record(vec![0xa9; 65]);
    write_value(&memory, layout, 0, &encode(&record, layout));
    DECODES.with(|count| count.set(0));
    memory.fail_initializing_read.set(true);
    traps(|| {
        let _: Record = read_value(&memory, layout, 0);
    });
    assert_eq!(memory.initializing_reads.get(), 1);
    assert_eq!(memory.initialized_bytes.get(), 32);
    DECODES.with(|count| assert_eq!(count.get(), 0));
    memory.fail_initializing_read.set(false);
    assert_eq!(read_value::<Record>(&memory, layout, 0), record);
    assert_eq!(memory.initialized_bytes.get(), 65);
    DECODES.with(|count| assert_eq!(count.get(), 1));
}

#[test]
fn encoding_enforces_the_declared_bound_without_convenience_helpers() {
    let record = Record(vec![1, 2]);
    for layout in [
        Layout {
            maximum: 1,
            fixed: false,
        },
        Layout {
            maximum: 3,
            fixed: true,
        },
    ] {
        traps(|| {
            let _ = encode(&record, layout);
        });
    }
}
