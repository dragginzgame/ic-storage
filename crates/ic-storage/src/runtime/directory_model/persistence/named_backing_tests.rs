//! Infrastructure-only backing fixtures; production callers use runtime leases.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use crate::{
    Memory,
    hash::{FNV_OFFSET, fnv64},
};

use super::{
    AllocationId, Error, StableKey, StorageConfig, Store,
    framing::{self, ENTRY_BYTES, PAGE_BYTES, ROOT_BYTES, Root, UNIT_BYTES},
};

#[derive(Clone)]
struct TestMemory(Rc<State>);

#[derive(Default)]
struct State {
    data: RefCell<Vec<u8>>,
    writes: Cell<usize>,
    reads: Cell<usize>,
    refuse_growth: Cell<bool>,
    panic_read: Cell<bool>,
    interrupt: Cell<Option<(usize, usize)>>,
}

impl TestMemory {
    fn empty() -> Self {
        Self(Rc::new(State::default()))
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        let memory = Self::empty();
        *memory.0.data.borrow_mut() = bytes;
        memory
    }

    fn snapshot(&self) -> Vec<u8> {
        self.0.data.borrow().clone()
    }
}

impl Memory for TestMemory {
    fn size(&self) -> u64 {
        self.0.data.borrow().len() as u64 / PAGE_BYTES
    }

    fn grow(&self, pages: u64) -> i64 {
        if self.0.refuse_growth.get() {
            return -1;
        }
        let previous = self.size();
        let length = (previous + pages) * PAGE_BYTES;
        self.0
            .data
            .borrow_mut()
            .resize(usize::try_from(length).unwrap(), 0);
        i64::try_from(previous).unwrap()
    }

    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.0.reads.set(self.0.reads.get() + 1);
        assert!(!self.0.panic_read.get(), "injected read fault");
        let start = usize::try_from(offset).unwrap();
        bytes.copy_from_slice(&self.0.data.borrow()[start..start + bytes.len()]);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        let write = self.0.writes.get() + 1;
        self.0.writes.set(write);
        let start = usize::try_from(offset).unwrap();
        if let Some((target, prefix)) = self.0.interrupt.get()
            && write == target
        {
            let count = prefix.min(bytes.len());
            self.0.data.borrow_mut()[start..start + count].copy_from_slice(&bytes[..count]);
            panic!("injected publication fault");
        }
        self.0.data.borrow_mut()[start..start + bytes.len()].copy_from_slice(bytes);
    }

    #[expect(
        unsafe_code,
        reason = "Test backing implements initializing reads into the caller-owned destination"
    )]
    unsafe fn read_unsafe(&self, offset: u64, destination: *mut u8, count: usize) {
        self.0.reads.set(self.0.reads.get() + 1);
        assert!(!self.0.panic_read.get(), "injected raw read fault");
        let start = usize::try_from(offset).unwrap();
        let data = self.0.data.borrow();
        let source = &data[start..start + count];
        // SAFETY: The Memory caller provides the writable, nonoverlapping range.
        unsafe {
            std::ptr::copy_nonoverlapping(source.as_ptr(), destination, count);
        }
    }
}

fn config() -> StorageConfig {
    StorageConfig {
        allocations: 8,
        extents: 32,
        pages: 512,
    }
}

fn keys(names: &[&str]) -> BTreeSet<StableKey> {
    names
        .iter()
        .map(|name| StableKey::parse(name).unwrap())
        .collect()
}

fn key(name: &str) -> StableKey {
    StableKey::parse(name).unwrap()
}

fn two_allocations(memory: TestMemory) -> Store<TestMemory> {
    let mut store = Store::create(memory, config()).unwrap();
    store.bind(&keys(&["app.a.v1", "app.b.v1"])).unwrap();
    store.grow(0, 8).unwrap();
    store.grow(1, 8).unwrap();
    store
}

fn rewrite_root(memory: &TestMemory, root: Root, layout: framing::Layout) {
    memory.write(0, &root.encode(layout));
}

fn seal_row(row: &mut [u8]) {
    let end = row.len() - 8;
    let checksum = fnv64(FNV_OFFSET, &row[..end]);
    row[end..].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
#[expect(
    unsafe_code,
    reason = "Verify initializing raw reads across discontiguous allocation units"
)]
fn interleaved_growth_preserves_ranges_and_zeroes_new_capacity() {
    let memory = TestMemory::empty();
    let mut store = two_allocations(memory.clone());
    store
        .write_data(AllocationId(1), UNIT_BYTES - 3, &[1, 2, 3])
        .unwrap();
    store.write_data(AllocationId(2), 0, &[77; 5]).unwrap();
    store.grow(0, 9).unwrap();
    let payload: Vec<_> = (0..usize::try_from(UNIT_BYTES).unwrap() + 9)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect();
    let writes = memory.0.writes.get();
    store
        .write_data(AllocationId(1), UNIT_BYTES - 3, &payload)
        .unwrap();
    assert_eq!(memory.0.writes.get(), writes + 2);
    let mut found = vec![0; payload.len()];
    let reads = memory.0.reads.get();
    store
        .read_data(AllocationId(1), UNIT_BYTES - 3, &mut found)
        .unwrap();
    assert_eq!(memory.0.reads.get(), reads + 2);
    assert_eq!(found, payload);
    let mut raw = [std::mem::MaybeUninit::<u8>::uninit(); 8];
    // SAFETY: The eight-byte destination is writable and independent of backing.
    unsafe {
        store
            .read_data_unsafe(
                AllocationId(1),
                UNIT_BYTES - 3,
                raw.as_mut_ptr().cast(),
                raw.len(),
            )
            .unwrap();
    }
    // SAFETY: The successful fragmented read initialized all destination bytes.
    let initialized = raw.map(|byte| unsafe { byte.assume_init() });
    assert_eq!(initialized, payload[..8]);
    let mut neighbor = [0; 5];
    store.read_data(AllocationId(2), 0, &mut neighbor).unwrap();
    assert_eq!(neighbor, [77; 5]);
    let mut zero = [255; 32];
    store
        .read_data(AllocationId(1), 17 * PAGE_BYTES - 32, &mut zero)
        .unwrap();
    assert_eq!(zero, [0; 32]);
    let reopened = Store::open(memory, config()).unwrap();
    reopened
        .read_data(AllocationId(1), UNIT_BYTES - 3, &mut found)
        .unwrap();
    assert_eq!(found, payload);
    let stats = reopened.diagnostics().unwrap();
    assert_eq!(
        stats.physical_pages,
        1 + stats.metadata_pages + stats.data_pages + stats.unassigned_pages
    );
    assert_eq!(stats.data_pages, 25);
    assert_eq!(stats.unassigned_pages, 7);
}

#[test]
#[expect(
    unsafe_code,
    reason = "Validate zero-count raw reads without accessing destination memory"
)]
fn empty_ranges_are_checked_before_translation_without_backing_io() {
    let memory = TestMemory::empty();
    let mut store = Store::create(memory.clone(), config()).unwrap();
    store.bind(&keys(&["app.a.v1"])).unwrap();
    for pages in [0, 1] {
        if pages != 0 {
            store.grow(0, pages).unwrap();
        }
        let end = pages * PAGE_BYTES;
        let reads = memory.0.reads.get();
        let writes = memory.0.writes.get();
        let mut empty: [u8; 0] = [];
        store.read_data(AllocationId(1), end, &mut empty).unwrap();
        store.write_data(AllocationId(1), end, &empty).unwrap();
        // SAFETY: No destination bytes are required for the admitted empty range.
        unsafe {
            store
                .read_data_unsafe(AllocationId(1), end, empty.as_mut_ptr(), 0)
                .unwrap();
        }
        assert!(matches!(
            store.read_data(AllocationId(1), end + 1, &mut empty),
            Err(Error::Bounds)
        ));
        assert!(matches!(
            store.write_data(AllocationId(1), end + 1, &empty),
            Err(Error::Bounds)
        ));
        // SAFETY: Invalid logical bounds reject before accessing the destination.
        assert!(matches!(
            unsafe { store.read_data_unsafe(AllocationId(1), end + 1, empty.as_mut_ptr(), 0) },
            Err(Error::Bounds)
        ));
        assert_eq!(memory.0.reads.get(), reads);
        assert_eq!(memory.0.writes.get(), writes);
    }
}

#[test]
fn positive_growth_advances_once_and_failed_growth_is_retryable() {
    let memory = TestMemory::empty();
    let mut store = Store::create(memory.clone(), config()).unwrap();
    store.bind(&keys(&["app.a.v1"])).unwrap();
    let generation = store.generation();
    store.bind(&keys(&["app.a.v1"])).unwrap();
    assert_eq!(store.grow(0, 0).unwrap(), 0);
    assert_eq!(store.generation(), generation);
    let before = memory.snapshot();
    let writes = memory.0.writes.get();
    memory.0.refuse_growth.set(true);
    assert!(matches!(store.grow(0, 1), Err(Error::Growth)));
    assert_eq!(memory.0.writes.get(), writes);
    assert_eq!(memory.snapshot(), before);
    assert!(store.healthy().is_ok());
    memory.0.refuse_growth.set(false);
    store.grow(0, 1).unwrap();
    assert_eq!(store.generation(), generation + 1);
    store.grow(0, 1).unwrap();
    assert_eq!(store.generation(), generation + 2);
    assert_eq!(store.diagnostics().unwrap().extents, 1);
}

#[test]
#[expect(
    unsafe_code,
    reason = "Exercise checked rejection and full initialization through the raw read API"
)]
fn raw_and_safe_ranges_are_fully_admitted_before_io() {
    let memory = TestMemory::empty();
    let mut store = Store::create(memory.clone(), config()).unwrap();
    store.bind(&keys(&["app.a.v1"])).unwrap();
    store.grow(0, 1).unwrap();
    let reads = memory.0.reads.get();
    let writes = memory.0.writes.get();
    let mut destination = [8; 4];
    assert!(matches!(
        store.read_data(AllocationId(1), PAGE_BYTES - 1, &mut destination),
        Err(Error::Bounds)
    ));
    assert!(matches!(
        store.write_data(AllocationId(1), u64::MAX, &[1]),
        Err(Error::Bounds)
    ));
    // SAFETY: Invalid logical bounds must reject before touching this destination.
    assert!(matches!(
        unsafe {
            store.read_data_unsafe(AllocationId(1), PAGE_BYTES - 1, destination.as_mut_ptr(), 4)
        },
        Err(Error::Bounds)
    ));
    assert_eq!(destination, [8; 4]);
    assert_eq!(memory.0.reads.get(), reads);
    assert_eq!(memory.0.writes.get(), writes);
    store
        .read_data(AllocationId(1), PAGE_BYTES, &mut [])
        .unwrap();
    assert!(matches!(
        store.read_data(AllocationId(1), PAGE_BYTES + 1, &mut []),
        Err(Error::Bounds)
    ));
    store.write_data(AllocationId(1), 0, &[1, 2, 3, 4]).unwrap();
    let mut raw = [std::mem::MaybeUninit::<u8>::uninit(); 4];
    // SAFETY: Four independently owned bytes are writable and nonoverlapping.
    unsafe {
        store
            .read_data_unsafe(AllocationId(1), 0, raw.as_mut_ptr().cast(), raw.len())
            .unwrap();
    }
    // SAFETY: Successful raw read initialized every byte.
    let initialized = raw.map(|byte| unsafe { byte.assume_init() });
    assert_eq!(initialized, [1, 2, 3, 4]);
}

#[test]
fn valid_external_ownership_change_is_rejected_by_live_verification() {
    let memory = TestMemory::empty();
    let store = two_allocations(memory.clone());
    let first = framing::encode_unit(0, 2, 0);
    let second = framing::encode_unit(1, 1, 0);
    let mut root = store.root;
    root.digest ^= framing::checksum(&framing::encode_unit(0, 1, 0));
    root.digest ^= framing::checksum(&framing::encode_unit(1, 2, 0));
    root.digest ^= framing::checksum(&first) ^ framing::checksum(&second);
    memory.write(store.layout.unit_offset(0).unwrap(), &first);
    memory.write(store.layout.unit_offset(1).unwrap(), &second);
    rewrite_root(&memory, root, store.layout);
    assert!(Store::open(memory, config()).is_ok());
    assert!(store.verify().is_err());
    assert!(matches!(store.healthy(), Err(Error::Poisoned)));
    assert!(matches!(store.pages(0), Err(Error::Poisoned)));
    assert!(store.allocation_id(&key("app.a.v1")).is_none());
}

#[test]
fn canonical_padding_names_and_coverage_are_checked_after_valid_checksums() {
    let memory = TestMemory::empty();
    two_allocations(memory.clone());
    let original = memory.snapshot();
    for corruption in 0..5 {
        let altered = TestMemory::from_bytes(original.clone());
        let offset = framing::Layout::entry_offset(1).unwrap();
        let mut row = [0; ENTRY_BYTES];
        altered.read(offset, &mut row);
        match corruption {
            0 => row[159] = 1,
            1 => row[0..8].copy_from_slice(&0_u64.to_le_bytes()),
            2 => row[24..32].copy_from_slice(&129_u64.to_le_bytes()),
            3 => row = framing::encode_entry(1, &key("app.a.v1"), 8, 1),
            4 => row[16..24].copy_from_slice(&2_u64.to_le_bytes()),
            _ => unreachable!(),
        }
        seal_row(&mut row);
        altered.write(offset, &row);
        assert!(Store::open(altered, config()).is_err());
    }
}

#[test]
fn malformed_active_last_unit_and_unexplained_tail_are_rejected_read_only() {
    let memory = TestMemory::empty();
    let store = two_allocations(memory.clone());
    let original = memory.snapshot();
    for row in [
        framing::encode_unit(1, 0, 0),
        framing::encode_unit(1, 2, 1),
        framing::encode_unit(0, 2, 0),
    ] {
        let altered = TestMemory::from_bytes(original.clone());
        altered.write(store.layout.unit_offset(1).unwrap(), &row);
        let before = altered.snapshot();
        let writes = altered.0.writes.get();
        assert!(Store::open(altered.clone(), config()).is_err());
        assert_eq!(altered.snapshot(), before);
        assert_eq!(altered.0.writes.get(), writes);
    }
    memory.grow(1);
    assert!(Store::open(memory, config()).is_err());
}

#[test]
fn enormous_counts_and_overflow_refuse_before_control_allocation_or_writes() {
    let memory = TestMemory::empty();
    let mut store = Store::create(memory.clone(), config()).unwrap();
    store.bind(&keys(&["app.a.v1"])).unwrap();
    let writes = memory.0.writes.get();
    assert!(matches!(
        store.grow(0, u64::MAX),
        Err(Error::ArithmeticOverflow)
    ));
    assert_eq!(memory.0.writes.get(), writes);
    let mut root_bytes = store.root.encode(store.layout);
    root_bytes[48..56].copy_from_slice(&u64::MAX.to_le_bytes());
    seal_row(&mut root_bytes);
    memory.write(0, &root_bytes);
    let reads = memory.0.reads.get();
    assert!(Store::open(memory.clone(), config()).is_err());
    assert_eq!(memory.0.reads.get(), reads + 1);
    root_bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    seal_row(&mut root_bytes);
    memory.write(0, &root_bytes);
    assert!(Store::open(memory, config()).is_err());
    assert!(
        Store::create(
            TestMemory::empty(),
            StorageConfig {
                allocations: 1,
                extents: 1,
                pages: u64::MAX
            }
        )
        .is_err()
    );
}

#[test]
fn publication_interruptions_only_reopen_complete_states() {
    for growing in [false, true] {
        let source = TestMemory::empty();
        let mut initial = Store::create(source.clone(), config()).unwrap();
        initial.bind(&keys(&["app.a.v1"])).unwrap();
        initial.grow(0, 1).unwrap();
        let original = source.snapshot();
        let original_generation = initial.generation();
        for fault in 1..=4 {
            for prefix in [0, 1, 12, usize::MAX] {
                let memory = TestMemory::from_bytes(original.clone());
                let mut store = Store::open(memory.clone(), config()).unwrap();
                memory.0.interrupt.set(Some((fault, prefix)));
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    if growing {
                        store.grow(0, 8).map(|_| ())
                    } else {
                        store.bind(&keys(&["app.a.v1", "app.b.v1"]))
                    }
                }));
                if let Ok(result) = outcome {
                    result.unwrap();
                } else {
                    assert!(matches!(store.healthy(), Err(Error::Poisoned)));
                }
                memory.0.interrupt.set(None);
                if let Ok(reopened) = Store::open(memory, config()) {
                    let complete_new = reopened.generation() == original_generation + 1;
                    let complete_old = reopened.generation() == original_generation;
                    assert!(complete_new || complete_old);
                    if growing {
                        assert_eq!(reopened.pages(0).unwrap(), if complete_new { 9 } else { 1 });
                    } else {
                        assert_eq!(reopened.entries.len(), if complete_new { 2 } else { 1 });
                    }
                    reopened.verify().unwrap();
                }
            }
        }
    }
}

#[test]
fn payload_io_panics_poison_the_current_owner() {
    let memory = TestMemory::empty();
    let store = two_allocations(memory.clone());
    memory.0.panic_read.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| store.read_data(
            AllocationId(1),
            0,
            &mut [0]
        )))
        .is_err()
    );
    memory.0.panic_read.set(false);
    assert!(matches!(
        store.write_data(AllocationId(2), 0, &[7]),
        Err(Error::Poisoned)
    ));
    assert!(Store::open(memory, config()).is_ok());
}

#[test]
fn retained_capacities_and_real_metadata_reservation_are_reported() {
    let memory = TestMemory::empty();
    let store = two_allocations(memory.clone());
    let stats = store.diagnostics().unwrap();
    assert_eq!(stats.metadata_pages, 1);
    assert_eq!(
        stats.directory_bytes,
        ROOT_BYTES as u64 + 2 * ENTRY_BYTES as u64 + 2 * framing::UNIT_ROW_BYTES as u64
    );
    let tighter = StorageConfig {
        allocations: 2,
        extents: 2,
        pages: stats.physical_pages,
    };
    let mut reopened = Store::open(memory.clone(), tighter).unwrap();
    assert!(matches!(
        reopened.bind(&keys(&["app.c.v1"])),
        Err(Error::Budget)
    ));
    assert!(matches!(reopened.grow(0, 1), Err(Error::Budget)));
    let larger = Store::open(
        memory,
        StorageConfig {
            allocations: 9,
            extents: 64,
            pages: 1024,
        },
    )
    .unwrap();
    assert_eq!(larger.limits.allocations, 8);
    assert_eq!(larger.limits.extents, 32);
    assert_eq!(larger.limits.pages, 512);
    let defaults = Store::create(
        TestMemory::empty(),
        StorageConfig {
            allocations: 4096,
            extents: 16384,
            pages: 1_048_576,
        },
    )
    .unwrap();
    let stats = defaults.diagnostics().unwrap();
    assert_eq!(stats.metadata_pages, 17);
    assert_eq!(stats.physical_pages, 18);
    assert_eq!(stats.directory_bytes, ROOT_BYTES as u64);
}
