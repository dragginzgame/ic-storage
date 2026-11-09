use super::*;
use crate::VectorMemory;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[derive(Clone, Default)]
struct Observed {
    bytes: VectorMemory,
    refuse_growth: Rc<Cell<bool>>,
    fail_write: Rc<Cell<Option<(usize, usize)>>>,
    writes: Rc<Cell<usize>>,
    grows: Rc<Cell<usize>>,
}
impl Memory for Observed {
    fn size(&self) -> u64 {
        self.bytes.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        self.grows.set(self.grows.get() + 1);
        if self.refuse_growth.get() {
            -1
        } else {
            self.bytes.grow(pages)
        }
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.bytes.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        self.writes.set(self.writes.get() + 1);
        if let Some((remaining, prefix)) = self.fail_write.get() {
            if remaining == 0 {
                self.bytes.write(offset, &bytes[..prefix.min(bytes.len())]);
                panic!("injected partial write");
            }
            self.fail_write.set(Some((remaining - 1, prefix)));
        }
        self.bytes.write(offset, bytes);
    }
}
fn key(index: usize) -> StableKey {
    StableKey::parse(format!("test.store{index}.v1")).unwrap()
}
fn store() -> (Observed, Store<Observed>) {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store.bind(&[key(0), key(1)].into()).unwrap();
    (memory, store)
}

#[test]
fn names_above_256_reopen_and_keep_ids_across_omission_and_append() {
    let (memory, mut store) = store();
    store.bind(&(0..1024).map(key).collect()).unwrap();
    let original = store.by_key.clone();
    let id = store.by_key[&key(900)];
    store.grow(usize::try_from(id.0 - 1).unwrap(), 9).unwrap();
    store.write_data(id, 8 * PAGE - 2, b"cross bucket").unwrap();
    store.bind(&[key(2000)].into()).unwrap();
    let before = memory.bytes.borrow().clone();
    let writes = memory.writes.get();
    drop(store);
    let store = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    for (key, id) in original {
        assert_eq!(store.by_key[&key], id);
    }
    assert_eq!(store.by_key[&key(2000)], AllocationId(1025));
    let mut data = [0; 12];
    store.read_data(id, 8 * PAGE - 2, &mut data).unwrap();
    assert_eq!(&data, b"cross bucket");
    assert_eq!(*memory.bytes.borrow(), before);
    assert_eq!(memory.writes.get(), writes);
}

#[test]
fn bucket_slack_is_inaccessible_until_logical_growth_and_zero_on_admission() {
    let (memory, mut store) = store();
    store.grow(0, 1).unwrap();
    let physical = memory.size();
    store.write_data(AllocationId(1), PAGE - 1, &[7]).unwrap();
    for (offset, bytes) in [(PAGE, &[1][..]), (PAGE + 1, &[][..]), (u64::MAX, &[1][..])] {
        assert!(matches!(
            store.write_data(AllocationId(1), offset, bytes),
            Err(Error::Bounds)
        ));
    }
    store.grow(1, 8).unwrap();
    store.write_data(AllocationId(2), 0, b"neighbour").unwrap();
    let calls = memory.grows.get();
    assert_eq!(store.grow(0, 7).unwrap(), 1);
    assert_eq!(memory.grows.get(), calls);
    let mut data = vec![1; 7 * usize::try_from(PAGE).unwrap()];
    store.read_data(AllocationId(1), PAGE, &mut data).unwrap();
    assert!(data.iter().all(|&byte| byte == 0));
    store.grow(0, 1).unwrap();
    store
        .write_data(AllocationId(1), 8 * PAGE - 2, &[1, 2, 3, 4])
        .unwrap();
    let mut got = [0; 4];
    store
        .read_data(AllocationId(1), 8 * PAGE - 2, &mut got)
        .unwrap();
    assert_eq!(got, [1, 2, 3, 4]);
    assert_eq!(memory.size(), physical + 16);
    let mut neighbour = [0; 9];
    store.read_data(AllocationId(2), 0, &mut neighbour).unwrap();
    assert_eq!(&neighbour, b"neighbour");
}

#[test]
fn refusal_and_budget_errors_preserve_bytes_sizes_and_allow_retry() {
    let (memory, mut store) = store();
    let before = memory.bytes.borrow().clone();
    memory.refuse_growth.set(true);
    assert!(matches!(store.grow(0, 1), Err(Error::Growth)));
    assert_eq!(store.pages(0).unwrap(), 0);
    assert_eq!(*memory.bytes.borrow(), before);
    memory.refuse_growth.set(false);
    assert!(matches!(store.grow(0, u64::MAX), Err(Error::Budget)));
    assert_eq!(*memory.bytes.borrow(), before);
    store.grow(0, 1).unwrap();
    assert!(matches!(
        store.grow(0, u64::MAX),
        Err(Error::ArithmeticOverflow)
    ));
    store.verify().unwrap();
    let budget = StorageConfig {
        allocations: 1,
        extents: 1,
        pages: 10,
    };
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), budget).unwrap();
    store.bind(&[key(0)].into()).unwrap();
    let before = memory.bytes.borrow().clone();
    assert!(matches!(store.bind(&[key(1)].into()), Err(Error::Budget)));
    assert!(matches!(store.grow(0, 9), Err(Error::Budget)));
    assert_eq!(*memory.bytes.borrow(), before);
    store.grow(0, 8).unwrap();
    assert_eq!(store.grow(0, 0).unwrap(), 8);
}

#[test]
fn interrupted_metadata_publication_is_fail_closed_or_fully_committed() {
    // dirty root, owner, row, final root. Native partial updates cannot be
    // reopened except when the final root was completely persisted.
    for write in 0..4 {
        for prefix in [0, 1, 25, 64, 160] {
            let (memory, mut store) = store();
            memory.fail_write.set(Some((write, prefix)));
            assert!(catch_unwind(AssertUnwindSafe(|| store.grow(0, 1))).is_err());
            assert!(matches!(store.pages(0), Err(Error::Poisoned)));
            memory.fail_write.set(None);
            drop(store);
            let reopened = Store::open(memory.clone(), StorageConfig::default());
            if write == 3 && prefix >= ROOT_BYTES {
                assert_eq!(reopened.unwrap().pages(0).unwrap(), 1);
            } else {
                assert!(reopened.is_err());
            }
        }
    }
    // Binding touches no data pages: a zero-byte failure at its first write
    // leaves the prior image intact; any torn/dirty publication is rejected.
    for write in 0..3 {
        let (memory, mut store) = store();
        memory.fail_write.set(Some((write, 1)));
        assert!(catch_unwind(AssertUnwindSafe(|| store.bind(&[key(2)].into()))).is_err());
        assert!(store.poisoned.get());
        memory.fail_write.set(None);
        let reopened = Store::open(memory, StorageConfig::default());
        if write == 0 {
            assert_eq!(reopened.unwrap().entries.len(), 2);
        } else {
            assert!(reopened.is_err());
        }
    }
}

#[test]
fn every_control_byte_corruption_and_foreign_image_refuse_without_writes() {
    let (memory, mut store) = store();
    store.grow(0, 1).unwrap();
    let owner = usize::try_from(store.root.owners().unwrap()).unwrap();
    let original = memory.bytes.borrow().clone();
    for offset in (0..ROOT_BYTES + ENTRY_SIZE).chain(owner..owner + OWNER_SIZE) {
        *memory.bytes.borrow_mut() = original.clone();
        memory.bytes.borrow_mut()[offset] ^= 1;
        let before = memory.bytes.borrow().clone();
        let writes = memory.writes.get();
        assert!(
            Store::open(memory.clone(), StorageConfig::default()).is_err(),
            "offset {offset}"
        );
        assert_eq!(*memory.bytes.borrow(), before);
        assert_eq!(memory.writes.get(), writes);
    }
    let foreign = VectorMemory::default();
    foreign.grow(1);
    foreign.write(0, b"foreign!");
    let before = foreign.borrow().clone();
    assert!(Store::open(foreign.clone(), StorageConfig::default()).is_err());
    assert!(Store::create(foreign.clone(), StorageConfig::default()).is_err());
    assert_eq!(*foreign.borrow(), before);
}

#[test]
fn diagnostics_account_for_reserved_metadata_bucket_slack_and_logical_data() {
    let (memory, mut store) = store();
    store.grow(0, 1).unwrap();
    store.grow(1, 9).unwrap();
    let before = (memory.grows.get(), memory.writes.get());
    let report = store.diagnostics().unwrap();
    assert_eq!(report.data_pages, 10);
    assert_eq!(report.extents, 3);
    assert_eq!(report.unassigned_pages, 14);
    assert_eq!(
        1 + report.metadata_pages + report.data_pages + report.unassigned_pages,
        report.physical_pages
    );
    assert_eq!((memory.grows.get(), memory.writes.get()), before);
}

#[test]
fn checked_boundary_detects_foreign_publication_and_poisons_owner() {
    let (memory, mut store) = store();
    let mut other = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    other.bind(&[key(9)].into()).unwrap();
    let before = memory.bytes.borrow().clone();
    assert!(store.bind(&[key(10)].into()).is_err());
    assert!(matches!(store.grow(0, 1), Err(Error::Poisoned)));
    assert_eq!(*memory.bytes.borrow(), before);
}

#[test]
fn growth_preserves_name_bytes_and_table_capacity_cannot_silently_change() {
    let (memory, mut store) = store();
    let name_start = ROOT_BYTES + 16;
    let name_end = ROOT_BYTES + ENTRY_SIZE;
    let name = memory.bytes.borrow()[name_start..name_end].to_vec();
    store.grow(0, 17).unwrap();
    assert_eq!(
        &memory.bytes.borrow()[name_start..name_end],
        name.as_slice()
    );
    drop(store);
    let small = Observed::default();
    let budget = StorageConfig {
        allocations: 1,
        extents: 1,
        pages: 64,
    };
    let mut store = Store::create(small.clone(), budget).unwrap();
    store.bind(&[key(0)].into()).unwrap();
    drop(store);
    let mut store = Store::open(small.clone(), StorageConfig::default()).unwrap();
    let before = small.bytes.borrow().clone();
    assert!(matches!(store.bind(&[key(1)].into()), Err(Error::Budget)));
    assert!(matches!(store.grow(0, 9), Err(Error::Budget)));
    assert_eq!(*small.bytes.borrow(), before);
}

#[test]
fn contiguous_requested_buckets_share_one_io_without_exposing_slack() {
    let memory = crate::testing::MeteredMemory::<VectorMemory>::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store.bind(&[key(0)].into()).unwrap();
    store.grow(0, 17).unwrap();
    let payload = vec![7; 17 * usize::try_from(PAGE).unwrap()];
    memory.reset();
    store.write_data(AllocationId(1), 0, &payload).unwrap();
    assert_eq!(memory.counts().writes, 1);
    let mut bytes = vec![0; payload.len()];
    store.read_data(AllocationId(1), 0, &mut bytes).unwrap();
    assert_eq!(memory.counts().reads, 1);
    assert_eq!(bytes, payload);
    assert!(matches!(
        store.write_data(AllocationId(1), 17 * PAGE, &[1]),
        Err(Error::Bounds)
    ));
    assert_eq!(memory.counts().writes, 1);
}
