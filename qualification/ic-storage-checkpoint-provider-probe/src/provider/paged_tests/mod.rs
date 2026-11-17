//! Host conformance of the product page codec against governed Storage handles.
//! The child job fixture persists offsets; neither fixture is an installed adapter.
mod job_tests;
mod lifecycle_tests;
use super::paged::{MANIFEST, PAGES, Store};
use ic_storage_legacy::{
    Memory, MemoryBacking, StableKey, VectorMemory,
    adapter::{StableCell, StableVec},
    testing::MeteredMemory,
};
use ic_storage_native::map::{Error, Map, paged::PAGE_BYTES};
use ic_storage_native_feasibility::checkpoint::maps::paged::{self, MANIFEST_BYTES, Manifest};
use std::panic::{AssertUnwindSafe, catch_unwind};

impl<M: MemoryBacking> Store<M> {
    fn open(memory: M, create: bool) -> Self {
        let runtime = super::runtime(memory, &[MANIFEST, PAGES], create);
        Self::from_runtime(&runtime, create)
    }
}

impl<M: Memory> Store<M> {
    fn publish(&mut self, source: &[Map; 2]) {
        // Fresh storage only. Failed exports remain unsealed, never a fallback
        // for an older checkpoint or authorization to erase an existing one.
        assert!(self.pages.is_empty());
        assert_eq!(self.manifest.get(), &[0; MANIFEST_BYTES]);
        let manifest = paged::export(source, 17, |_, _, page| {
            self.pages.push(page);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(self.pages.len(), manifest.page_count() as u64);
        self.manifest.set(manifest.encode());
    }

    fn admit(&self, limit: usize) -> Manifest {
        let manifest = Manifest::decode(self.manifest.get(), 17, limit).unwrap();
        assert_eq!(self.pages.len(), manifest.page_count() as u64);
        manifest
    }

    fn restore(&self, limit: usize) -> Result<[Map; 2], Error> {
        let manifest = self.admit(limit);
        let mut candidates = [Map::new(limit), Map::new(limit)];
        for (root, candidate) in candidates.iter_mut().enumerate() {
            let mut reader = manifest.roots()[root].reader();
            while let Some(index) = reader.next_page() {
                let records = reader.read_page(&self.page(&manifest, root, index))?;
                candidate.with(|mut view| {
                    for (key, value) in records {
                        assert!(view.insert(key, value)?.is_none());
                    }
                    Ok::<_, Error>(())
                })??;
            }
        }
        Ok(candidates) // Neither root escapes if the second root fails.
    }
}

fn create_sources(counts: [usize; 2]) -> [Map; 2] {
    std::array::from_fn(|root| {
        let mut map = Map::new(counts[root]);
        map.with(|mut view| {
            for key in 0..counts[root] as u64 {
                view.insert(u64::MAX - key, key + root as u64).unwrap();
            }
        })
        .unwrap();
        map
    })
}

#[test]
fn selected_page_reads_stay_fixed_after_independent_governed_reopening() {
    let mut open_bytes = None;
    for counts in [[0, 0], [0, 65], [65, 0], [1, 1], [64, 65], [2304, 2304]] {
        let limit = counts[0].max(counts[1]);
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Store::open(backing.clone(), true);
        let source = create_sources(counts);
        store.publish(&source);
        drop(source);
        drop(store); // No source owner or provider handle survives the handoff.
        backing.reset();
        let store = Store::open(backing.clone(), false);
        let manifest = store.admit(limit);
        let opening = backing.counts();
        assert_eq!(opening.writes, 0);
        assert!(!opening.overflowed);
        if let Some(expected) = open_bytes {
            assert_eq!(opening.read_bytes, expected);
        }
        open_bytes = Some(opening.read_bytes);
        drop(store);
        let mut candidates = counts.map(Map::new);
        for (root, candidate) in candidates.iter_mut().enumerate() {
            let mut reader = manifest.roots()[root].reader();
            while let Some(index) = reader.next_page() {
                let store = Store::open(backing.clone(), false);
                assert_eq!(store.admit(limit), manifest);
                backing.reset();
                let page = store.page(&manifest, root, index);
                let records = reader.read_page(&page).unwrap();
                let io = backing.counts();
                assert_eq!(
                    (io.read_bytes, io.writes, io.grows, io.overflowed),
                    (PAGE_BYTES as u64, 0, 0, false)
                );
                candidate
                    .with(|mut view| {
                        for (key, value) in records {
                            assert!(view.insert(key, value).unwrap().is_none());
                        }
                    })
                    .unwrap();
            }
            assert_eq!(candidate.len(), Ok(counts[root]));
            candidate
                .with(|view| {
                    for key in 0..counts[root] as u64 {
                        assert_eq!(view.get(u64::MAX - key), Some(key + root as u64));
                    }
                })
                .unwrap();
        }
        println!(
            "paged pair records={counts:?} open_read_bytes={} selected_page_read_bytes={PAGE_BYTES}",
            opening.read_bytes
        );
    }
}

#[test]
fn failed_publication_missing_extra_mixed_and_corrupt_pages_refuse() {
    for fault in 0..8 {
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Store::open(backing.clone(), true);
        let source = create_sources([129, 129]);
        if fault == 0 {
            // A sink can retain partial pages, but no successful manifest escapes.
            let result = paged::export(&source, 17, |root, _, page| {
                store.pages.push(page);
                if root == 2 {
                    Err("interrupted second root")
                } else {
                    Ok(())
                }
            });
            assert!(result.is_err());
        } else {
            store.publish(&source);
            match fault {
                1 => {
                    store.pages.pop().unwrap();
                }
                2 => {
                    store.pages.push(&[0; PAGE_BYTES]);
                }
                3 => {
                    let mut bad = *store.manifest.get();
                    bad[56] ^= 1;
                    store.manifest.set(bad);
                }
                4 => {
                    let first = store.pages.get(2).unwrap();
                    let second = store.pages.get(5).unwrap();
                    store.pages.set(2, &second);
                    store.pages.set(5, &first);
                }
                5 => {
                    let mut bad = store.pages.get(3).unwrap();
                    bad[88] ^= 1; // last page of root 2: root 1 still verifies
                    store.pages.set(3, &bad);
                    let manifest = store.admit(129);
                    let mut first = manifest.roots()[0].reader();
                    while let Some(index) = first.next_page() {
                        first.read_page(&store.page(&manifest, 0, index)).unwrap();
                    }
                    let mut reader = manifest.roots()[1].reader();
                    reader.read_page(&store.page(&manifest, 1, 0)).unwrap();
                    reader.read_page(&store.page(&manifest, 1, 1)).unwrap();
                    assert_eq!(
                        reader.read_page(&store.page(&manifest, 1, 2)),
                        Err(Error::Integrity)
                    );
                    assert_eq!(reader.next_page(), Some(2));
                }
                6 => {
                    let mut other = create_sources([129, 129]);
                    other[1]
                        .with(|mut view| view.insert(u64::MAX, 777).unwrap())
                        .unwrap();
                    let foreign = paged::export(&other, 17, |_, _, _| Ok::<_, ()>(())).unwrap();
                    store.manifest.set(foreign.encode());
                }
                _ => {
                    store.manifest.set([0; MANIFEST_BYTES]);
                }
            }
        }
        drop(source);
        drop(store);
        backing.reset();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                Store::open(backing.clone(), false).restore(129).unwrap()
            }))
            .is_err()
        );
        assert_eq!(backing.counts().writes, 0);
    }
}

#[test]
fn absent_paged_locators_never_create_replacement_state() {
    for keys in [
        vec![],
        vec![MANIFEST],
        vec![PAGES],
        vec!["native.checkpoint.maps.job.checkpoint.v1"],
        vec![
            "native.checkpoint.paged.map.manifest.v1",
            "native.checkpoint.paged.map.pages.v1",
        ],
    ] {
        let backing = MeteredMemory::<VectorMemory>::default();
        if !keys.is_empty() {
            let runtime = super::runtime(backing.clone(), &keys, true);
            for key in keys {
                let memory = runtime
                    .open_memory(&StableKey::parse(key).unwrap())
                    .unwrap();
                if key == PAGES {
                    drop(StableVec::<[u8; PAGE_BYTES], _>::create(memory).unwrap());
                } else {
                    drop(StableCell::create(memory, [0; MANIFEST_BYTES]).unwrap());
                }
            }
        }
        backing.reset();
        assert!(catch_unwind(AssertUnwindSafe(|| Store::open(backing.clone(), false))).is_err());
        assert_eq!(backing.counts().writes, 0);
    }
}
