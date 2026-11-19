use super::super::{AllocationId, Extent, StableKey};
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

    fn read(&self, offset: u64, target: &mut [u8]) {
        self.bytes.read(offset, target);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        self.writes.set(self.writes.get() + 1);
        if let Some((remaining, prefix)) = self.fail_write.get() {
            if remaining == 0 {
                self.bytes.write(offset, &bytes[..prefix.min(bytes.len())]);
                panic!("injected partial backing write");
            }
            self.fail_write.set(Some((remaining - 1, prefix)));
        }
        self.bytes.write(offset, bytes);
    }
}

fn key(index: usize) -> StableKey {
    StableKey::parse(format!("test.store{index}.v1")).unwrap()
}

#[test]
fn multi_page_directory_reserves_its_complete_record_before_application_growth() {
    let memory = Observed::default();
    let budget = StorageConfig::default();
    let mut store = Store::create(memory.clone(), budget).unwrap();
    store
        .commit(
            store
                .prepare((0..budget.allocations).map(key).collect(), &[])
                .unwrap(),
        )
        .unwrap();
    let root = Root::decode(&store.root).unwrap();
    assert!(root.length > PAGE);
    assert!(root.end >= root.start + root.length.div_ceil(PAGE));
    let id = store.directory.by_key[&key(0)];
    store
        .commit(store.prepare(vec![], &[(id, 1)]).unwrap())
        .unwrap();
    assert_eq!(store.directory.page(id, 0), Some(root.end));
    let expected = store.directory.image.clone();
    drop(store);
    let reopened = Store::open(memory, budget).unwrap();
    assert_eq!(reopened.directory.image, expected);
}

#[test]
fn reopen_more_than_256_allocations_preserves_data_and_metadata_separation() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store
        .commit(store.prepare((0..1_024).map(key).collect(), &[]).unwrap())
        .unwrap();
    let a = store.directory.by_key[&key(500)];
    let b = store.directory.by_key[&key(900)];
    assert!(store.directory.by_id.keys().any(|id| id.0 > 255));
    store
        .commit(store.prepare(vec![], &[(a, 2), (b, 1), (a, 1)]).unwrap())
        .unwrap();
    let first = store.directory.page(a, 0).unwrap() * PAGE;
    memory.write(first, b"application payload");
    // A further metadata commit must not overwrite previously allocated data.
    store
        .commit(store.prepare(vec![key(2_000)], &[]).unwrap())
        .unwrap();
    let expected = store.directory.image.clone();
    let before = memory.bytes.borrow().clone();
    let writes = memory.writes.get();
    let grows = memory.grows.get();
    drop(store);
    let reopened = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    assert_eq!(expected, reopened.directory.image);
    assert_eq!(
        &memory.bytes.borrow()[usize::try_from(first).unwrap()..][..19],
        b"application payload"
    );
    assert_eq!(*memory.bytes.borrow(), before);
    assert_eq!((memory.writes.get(), memory.grows.get()), (writes, grows));
}

#[test]
fn growth_refusal_preserves_root_cache_and_bytes_and_allows_retry() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    let before = memory.bytes.borrow().clone();
    let image = store.directory.image.clone();
    let writes = memory.writes.get();
    memory.refuse_growth.set(true);
    let candidate = store.prepare(vec![key(0)], &[]).unwrap();
    assert!(matches!(store.commit(candidate), Err(Error::Growth)));
    assert_eq!(store.directory.image, image);
    assert_eq!(*memory.bytes.borrow(), before);
    assert_eq!(memory.writes.get(), writes);
    memory.refuse_growth.set(false);
    store
        .commit(store.prepare(vec![key(0)], &[]).unwrap())
        .unwrap();
    assert!(
        Store::open(memory, StorageConfig::default())
            .unwrap()
            .directory
            .by_key
            .contains_key(&key(0))
    );
}

#[test]
fn interrupted_writes_poison_owner_and_never_guess_a_fallback_root() {
    // Payload interruption leaves the previous root valid. A torn root is
    // rejected. A fully written root followed by panic is durably visible on
    // reopen, while the original owner's cache remains unpublished/poisoned.
    for (remaining, prefix, outcome) in [(0, 0, 0), (0, 10, 0), (1, 25, 1), (1, ROOT_BYTES, 2)] {
        let memory = Observed::default();
        let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
        let base = store.directory.image.clone();
        memory.fail_write.set(Some((remaining, prefix)));
        let candidate = store.prepare(vec![key(0)], &[]).unwrap();
        let retry = store.prepare(vec![key(1)], &[]).unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| store.commit(candidate))).is_err());
        assert!(store.poisoned.get());
        assert_eq!(store.directory.image, base);
        memory.fail_write.set(None);
        assert!(matches!(store.commit(retry), Err(Error::Poisoned)));
        drop(store);
        let recovered = Store::open(memory, StorageConfig::default());

        match outcome {
            0 => assert_eq!(recovered.unwrap().directory.image, base),
            1 => assert!(recovered.is_err()),
            _ => assert!(recovered.unwrap().directory.by_key.contains_key(&key(0))),
        }
    }
}

#[test]
fn every_root_byte_and_directory_corruption_is_rejected_without_writes() {
    let memory = Observed::default();
    let store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    let root = Root::decode(&store.root).unwrap();
    let original = memory.bytes.borrow().clone();
    drop(store);

    for offset in (0..ROOT_BYTES).chain([usize::try_from(root.start * PAGE).unwrap()]) {
        *memory.bytes.borrow_mut() = original.clone();
        memory.bytes.borrow_mut()[offset] ^= 1;
        let before = memory.bytes.borrow().clone();
        let writes = memory.writes.get();
        assert!(Store::open(memory.clone(), StorageConfig::default()).is_err());
        assert_eq!(*memory.bytes.borrow(), before);
        assert_eq!(memory.writes.get(), writes);
    }
}

#[test]
fn foreign_backing_stale_candidates_and_metadata_aliases_refuse() {
    let memory = Observed::default();
    memory.grow(1);
    assert!(Store::create(memory.clone(), StorageConfig::default()).is_err());
    assert!(Store::open(memory, StorageConfig::default()).is_err());
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    let stale = store.prepare(vec![key(0)], &[]).unwrap();
    store
        .commit(store.prepare(vec![key(1)], &[]).unwrap())
        .unwrap();
    let before = memory.bytes.borrow().clone();
    assert!(matches!(store.commit(stale), Err(Error::Stale)));
    assert_eq!(*memory.bytes.borrow(), before);

    // A checksum-valid record still has to pass structural metadata separation.
    let mut root = Root::decode(&store.root).unwrap();
    let mut image = store.directory.image.clone();
    image.entries[0].extents.push(Extent {
        start: root.start,
        pages: 1,
    });
    let bytes =
        super::super::format::encode(&image, StorageConfig::default().directory_bytes).unwrap();
    root.length = bytes.len() as u64;
    root.digest = digest(&bytes);
    memory.write(root.start * PAGE, &bytes);
    memory.write(0, &root.encode());
    assert!(Store::open(memory, StorageConfig::default()).is_err());
}

#[test]
fn empty_growth_refusal_and_commit_budgets_have_no_writes() {
    let memory = Observed::default();
    memory.refuse_growth.set(true);
    assert!(matches!(
        Store::create(memory.clone(), StorageConfig::default()),
        Err(Error::Growth)
    ));
    assert_eq!((memory.size(), memory.writes.get()), (0, 0));
    memory.refuse_growth.set(false);
    let mut store = Store::create(
        memory.clone(),
        StorageConfig {
            pages: 2,
            ..StorageConfig::default()
        },
    )
    .unwrap();
    let before = memory.bytes.borrow().clone();
    let grows = memory.grows.get();
    let candidate = store.prepare(vec![key(0)], &[]).unwrap();
    assert!(matches!(store.commit(candidate), Err(Error::Budget)));
    assert_eq!(*memory.bytes.borrow(), before);
    assert_eq!(memory.grows.get(), grows);
    assert!(store.directory.page(AllocationId(1), 0).is_none());
}

#[test]
fn commit_checks_receiving_allocation_and_byte_limits_before_backing_mutation() {
    let memory = Observed::default();
    let source = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    let root = Root::decode(&source.root).unwrap();

    for budget in [
        StorageConfig {
            allocations: 1,
            ..StorageConfig::default()
        },
        StorageConfig {
            directory_bytes: usize::try_from(root.length).unwrap(),
            ..StorageConfig::default()
        },
    ] {
        let mut receiver = Store::open(memory.clone(), budget).unwrap();
        let before = memory.bytes.borrow().clone();
        let effects = (memory.grows.get(), memory.writes.get());
        let candidate = source.prepare(vec![key(0), key(1)], &[]).unwrap();
        assert!(matches!(receiver.commit(candidate), Err(Error::Budget)));
        assert_eq!(*memory.bytes.borrow(), before);
        assert_eq!((memory.grows.get(), memory.writes.get()), effects);
        assert_eq!(receiver.directory.image, source.directory.image);
        receiver.verify().unwrap();
    }
}

#[test]
fn abandoned_tail_is_never_reused_as_zeroed_application_pages() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store
        .commit(store.prepare(vec![key(0)], &[]).unwrap())
        .unwrap();
    memory.fail_write.set(Some((0, 16)));
    let candidate = store.prepare(vec![key(1)], &[]).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| store.commit(candidate))).is_err());
    let abandoned_end = memory.size();
    memory.fail_write.set(None);
    drop(store);
    let mut reopened = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    let id = reopened.directory.by_key[&key(0)];
    reopened
        .commit(reopened.prepare(vec![], &[(id, 1)]).unwrap())
        .unwrap();
    let page = reopened.directory.page(id, 0).unwrap();
    assert!(page >= abandoned_end);
    let mut data = vec![1; usize::try_from(PAGE).unwrap()];
    memory.read(page * PAGE, &mut data);
    assert!(data.iter().all(|&byte| byte == 0));
}

#[test]
fn snapshot_arenas_bound_metadata_growth_and_coalesce_data_extents() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store
        .commit(store.prepare(vec![key(0)], &[]).unwrap())
        .unwrap();
    let id = store.directory.by_key[&key(0)];
    let base_pages = memory.size();
    for index in 0..100 {
        store
            .commit(store.prepare(vec![key(index + 1)], &[(id, 1)]).unwrap())
            .unwrap();
        store
            .write_data(id, index as u64 * PAGE, &(index as u64).to_le_bytes())
            .unwrap();
    }
    assert_eq!(memory.size(), base_pages + 100);
    assert_eq!(store.directory.image.metadata.len(), 2);
    assert_eq!(store.directory.image.entries[0].extents.len(), 1);
    drop(store);
    let reopened = Store::open(memory, StorageConfig::default()).unwrap();

    for index in 0_u64..100 {
        let mut bytes = [0; 8];
        reopened.read_data(id, index * PAGE, &mut bytes).unwrap();
        assert_eq!(u64::from_le_bytes(bytes), index);
    }
}

#[test]
fn interrupted_reuse_preserves_active_snapshot_and_retry_overwrites_inactive_bytes() {
    for prefix in [0, 16, 1000] {
        let memory = Observed::default();
        let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
        store
            .commit(store.prepare(vec![key(0)], &[]).unwrap())
            .unwrap();
        let base = store.directory.image.clone();
        let pages = memory.size();
        memory.fail_write.set(Some((0, prefix)));
        let candidate = store.prepare(vec![key(1)], &[]).unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| store.commit(candidate))).is_err());
        assert!(store.poisoned.get());
        memory.fail_write.set(None);
        drop(store);
        let mut reopened = Store::open(memory.clone(), StorageConfig::default()).unwrap();
        assert_eq!(reopened.directory.image, base);
        reopened
            .commit(reopened.prepare(vec![key(2)], &[]).unwrap())
            .unwrap();
        assert_eq!(memory.size(), pages);
        assert!(!reopened.directory.by_key.contains_key(&key(1)));
        reopened.verify().unwrap();
    }
}

#[test]
fn larger_snapshot_arenas_are_reused_after_growth_and_reopen() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store
        .commit(store.prepare((0..3000).map(key).collect(), &[]).unwrap())
        .unwrap();
    store
        .commit(store.prepare(vec![key(3001)], &[]).unwrap())
        .unwrap();
    let pages = memory.size();
    drop(store);
    let mut store = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    for index in 3002..3020 {
        store
            .commit(store.prepare(vec![key(index)], &[]).unwrap())
            .unwrap();
        store.verify().unwrap();
    }
    assert_eq!(memory.size(), pages);
}
