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
fn translation_cache_checks_bounds_and_is_cleared_by_directory_publication() {
    let memory = Observed::default();
    let mut store = Store::create(memory, StorageConfig::default()).unwrap();
    store.mutate(vec![key(0), key(1)], &[]).unwrap();
    let a = store.directory.by_key[&key(0)];
    let b = store.directory.by_key[&key(1)];
    store.mutate(vec![], &[(a, 1), (b, 1), (a, 1)]).unwrap();
    store.write_data(a, PAGE - 2, &[1, 2, 3, 4]).unwrap();
    let mut bytes = [0; 4];
    store.read_data(a, PAGE - 2, &mut bytes).unwrap();
    assert_eq!(bytes, [1, 2, 3, 4]);
    assert!(store.last_span.get().is_some());
    assert!(matches!(
        store.read_data(a, 2 * PAGE, &mut bytes),
        Err(Error::Bounds)
    ));
    store.read_data(b, 0, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 4]);
    store.mutate(vec![], &[(a, 1)]).unwrap();
    assert!(store.last_span.get().is_none());
    store.read_data(a, 2 * PAGE, &mut bytes).unwrap();
    assert_eq!(bytes, [0; 4]);
    store.poisoned.set(true);
    assert!(matches!(
        store.read_data(a, 2 * PAGE, &mut bytes),
        Err(Error::Poisoned)
    ));
}

#[test]
fn diagnostics_never_write_or_grow_the_backing() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store.mutate(vec![key(0)], &[]).unwrap();
    let id = store.directory.by_key[&key(0)];
    store.mutate(vec![], &[(id, 3)]).unwrap();
    let before = (memory.writes.get(), memory.grows.get());
    let report = store.diagnostics().unwrap();
    assert_eq!(report.allocations, 1);
    assert_eq!(report.data_pages, 3);
    assert_eq!(report.physical_pages, memory.size());
    assert_eq!((memory.writes.get(), memory.grows.get()), before);
}

#[test]
fn multi_page_directory_reserves_its_complete_record_before_application_growth() {
    let memory = Observed::default();
    let budget = StorageConfig::default();
    let mut store = Store::create(memory.clone(), budget).unwrap();
    store
        .mutate((0..budget.allocations).map(key).collect(), &[])
        .unwrap();
    let root = Root::decode(&store.root).unwrap();
    assert!(root.length > PAGE);
    assert!(root.end >= root.start + root.length.div_ceil(PAGE));
    let id = store.directory.by_key[&key(0)];
    store.mutate(vec![], &[(id, 1)]).unwrap();
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
    store.mutate((0..1_024).map(key).collect(), &[]).unwrap();
    let a = store.directory.by_key[&key(500)];
    let b = store.directory.by_key[&key(900)];
    assert!(
        store
            .directory
            .image
            .entries
            .iter()
            .any(|entry| entry.id.0 > 255)
    );
    store.mutate(vec![], &[(a, 2), (b, 1), (a, 1)]).unwrap();
    let first = store.directory.page(a, 0).unwrap() * PAGE;
    memory.write(first, b"application payload");
    // A further metadata commit must not overwrite previously allocated data.
    store.mutate(vec![key(2_000)], &[]).unwrap();
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
    assert!(matches!(
        store.mutate(vec![key(0)], &[]),
        Err(Error::Growth)
    ));
    assert_eq!(store.directory.image, image);
    assert_eq!(*memory.bytes.borrow(), before);
    assert_eq!(memory.writes.get(), writes);
    memory.refuse_growth.set(false);
    store.mutate(vec![key(0)], &[]).unwrap();
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
        assert!(catch_unwind(AssertUnwindSafe(|| store.mutate(vec![key(0)], &[]))).is_err());
        assert!(store.poisoned.get());
        assert_eq!(store.directory.image, base);
        memory.fail_write.set(None);
        assert!(matches!(
            store.mutate(vec![key(1)], &[]),
            Err(Error::Poisoned)
        ));
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
fn mutation_rejects_a_changed_backing_before_growth_or_publication() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    let mut other = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    // Only this private fixture bypasses public backing custody, to exercise
    // the boundary check against a structurally valid foreign publication.
    other.mutate(vec![key(0)], &[]).unwrap();
    let before = memory.bytes.borrow().clone();
    let effects = (memory.grows.get(), memory.writes.get());
    assert!(matches!(
        store.mutate(vec![key(1)], &[]),
        Err(Error::Invalid(_))
    ));
    assert!(store.poisoned.get());
    assert_eq!(*memory.bytes.borrow(), before);
    assert_eq!((memory.grows.get(), memory.writes.get()), effects);
    assert!(matches!(
        store.mutate(vec![key(1)], &[]),
        Err(Error::Poisoned)
    ));
}

#[test]
fn foreign_backing_and_metadata_aliases_refuse() {
    let memory = Observed::default();
    memory.grow(1);
    assert!(Store::create(memory.clone(), StorageConfig::default()).is_err());
    assert!(Store::open(memory, StorageConfig::default()).is_err());
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store.mutate(vec![key(1)], &[]).unwrap();

    // A checksum-valid record still has to pass structural metadata separation.
    let mut root = Root::decode(&store.root).unwrap();
    let mut image = store.directory.image.clone();
    image.entries[0].extents.push(Extent {
        start: root.start,
        pages: 1,
    });
    let mut bytes = super::super::FRAME.to_vec();
    bytes.extend(ic_wire::native::encode(&image, StorageConfig::default().wire).unwrap());
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
    assert!(matches!(
        store.mutate(vec![key(1)], &[]),
        Err(Error::Budget)
    ));
    assert_eq!(*memory.bytes.borrow(), before);
    assert_eq!(memory.grows.get(), grows);
    assert!(store.directory.page(AllocationId(1), 0).is_none());
}

#[test]
fn mutation_checks_owner_allocation_and_wire_limits_before_backing_mutation() {
    let memory = Observed::default();
    let source = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    let root = Root::decode(&source.root).unwrap();
    for budget in [
        StorageConfig {
            allocations: 1,
            ..StorageConfig::default()
        },
        StorageConfig {
            wire: ic_wire::Limits {
                bytes: usize::try_from(root.length).unwrap(),
                ..StorageConfig::default().wire
            },
            ..StorageConfig::default()
        },
    ] {
        let mut receiver = Store::open(memory.clone(), budget).unwrap();
        let before = memory.bytes.borrow().clone();
        let effects = (memory.grows.get(), memory.writes.get());
        assert!(matches!(
            receiver.mutate(vec![key(0), key(1)], &[]),
            Err(Error::Budget | Error::Wire(_))
        ));
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
    store.mutate(vec![key(0)], &[]).unwrap();
    memory.fail_write.set(Some((0, 16)));
    assert!(catch_unwind(AssertUnwindSafe(|| store.mutate(vec![key(1)], &[]))).is_err());
    let abandoned_end = memory.size();
    memory.fail_write.set(None);
    drop(store);
    let mut reopened = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    let id = reopened.directory.by_key[&key(0)];
    reopened.mutate(vec![], &[(id, 1)]).unwrap();
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
    store.mutate(vec![key(0)], &[]).unwrap();
    let id = store.directory.by_key[&key(0)];
    let base_pages = memory.size();
    for index in 0..100 {
        store.mutate(vec![key(index + 1)], &[(id, 1)]).unwrap();
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
        store.mutate(vec![key(0)], &[]).unwrap();
        let base = store.directory.image.clone();
        let pages = memory.size();
        memory.fail_write.set(Some((0, prefix)));
        assert!(catch_unwind(AssertUnwindSafe(|| store.mutate(vec![key(1)], &[]))).is_err());
        assert!(store.poisoned.get());
        memory.fail_write.set(None);
        drop(store);
        let mut reopened = Store::open(memory.clone(), StorageConfig::default()).unwrap();
        assert_eq!(reopened.directory.image, base);
        reopened.mutate(vec![key(2)], &[]).unwrap();
        assert_eq!(memory.size(), pages);
        assert!(!reopened.directory.by_key.contains_key(&key(1)));
        reopened.verify().unwrap();
    }
}

#[test]
fn larger_snapshot_arenas_are_reused_after_growth_and_reopen() {
    let memory = Observed::default();
    let mut store = Store::create(memory.clone(), StorageConfig::default()).unwrap();
    store.mutate((0..3000).map(key).collect(), &[]).unwrap();
    store.mutate(vec![key(3001)], &[]).unwrap();
    let pages = memory.size();
    drop(store);
    let mut store = Store::open(memory.clone(), StorageConfig::default()).unwrap();
    for index in 3002..3020 {
        store.mutate(vec![key(index)], &[]).unwrap();
        store.verify().unwrap();
    }
    assert_eq!(memory.size(), pages);
}
