//! Host conformance of the product page codec against governed Storage handles.
//! This is not an installed job adapter or a durable import cursor.
use ic_storage_legacy::{
    Memory, MemoryBacking, StableKey, VectorMemory,
    adapter::{StableCell, StableVec},
    testing::MeteredMemory,
};
use ic_storage_native::map::{
    Error, Map,
    paged::{MANIFEST_BYTES, Manifest, PAGE_BYTES},
};
use std::panic::{AssertUnwindSafe, catch_unwind};

const MANIFEST: &str = "native.checkpoint.paged.map.manifest.v1";
const PAGES: &str = "native.checkpoint.paged.map.pages.v1";

struct Store<M: Memory> {
    manifest: StableCell<[u8; MANIFEST_BYTES], M>,
    pages: StableVec<[u8; PAGE_BYTES], M>,
}

impl<M: MemoryBacking> Store<M> {
    fn open(memory: M, create: bool) -> Self {
        let runtime = super::runtime(memory, &[MANIFEST, PAGES], create);
        let metadata = runtime
            .open_memory(&StableKey::parse(MANIFEST).unwrap())
            .unwrap();
        let data = runtime
            .open_memory(&StableKey::parse(PAGES).unwrap())
            .unwrap();
        if create {
            Self {
                manifest: StableCell::create(metadata, [0; MANIFEST_BYTES]).unwrap(),
                pages: StableVec::create(data).unwrap(),
            }
        } else {
            Self {
                manifest: StableCell::load(metadata, MANIFEST_BYTES).unwrap(),
                pages: StableVec::load(data).unwrap(),
            }
        }
    }
}

impl<M: Memory> Store<M> {
    fn publish(&mut self, source: &Map) {
        // Fresh storage only. Failed exports remain unsealed, never a fallback
        // for an older checkpoint or authorization to erase an existing one.
        assert!(self.pages.is_empty());
        assert_eq!(self.manifest.get(), &[0; MANIFEST_BYTES]);
        let manifest = source
            .export_pages(17, 9, |_, page| {
                self.pages.push(page);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(self.pages.len(), manifest.page_count() as u64);
        self.manifest.set(manifest.encode());
    }

    fn admit(&self, limit: usize) -> Manifest {
        let manifest = Manifest::decode(self.manifest.get(), 17, 9, limit).unwrap();
        assert_eq!(self.pages.len(), manifest.page_count() as u64);
        manifest
    }

    fn page(&self, manifest: &Manifest, index: usize) -> [u8; PAGE_BYTES] {
        // The exporter writes backwards; physical vector order is reversed.
        let physical = manifest.page_count().checked_sub(index + 1).unwrap();
        self.pages.get(physical as u64).unwrap()
    }

    fn restore(&self, limit: usize) -> Result<Map, Error> {
        let manifest = self.admit(limit);
        let mut reader = manifest.reader();
        let mut candidate = Map::new(limit);
        while let Some(index) = reader.next_page() {
            let records = reader.read_page(&self.page(&manifest, index))?;
            candidate.with(|mut view| {
                for (key, value) in records {
                    assert!(view.insert(key, value)?.is_none());
                }
                Ok::<_, Error>(())
            })??;
        }
        Ok(candidate)
    }
}

fn create_source(count: usize) -> Map {
    let mut map = Map::new(count);
    map.with(|mut view| {
        for key in 0..count as u64 {
            view.insert(u64::MAX - key, key).unwrap();
        }
    })
    .unwrap();
    map
}

#[test]
fn selected_page_reads_stay_fixed_after_independent_governed_reopening() {
    let mut open_bytes = None;
    for count in [0, 1, 64, 65, 4608] {
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Store::open(backing.clone(), true);
        let source = create_source(count);
        store.publish(&source);
        drop(source);
        drop(store); // No source owner or provider handle survives the handoff.
        backing.reset();
        let store = Store::open(backing.clone(), false);
        let manifest = store.admit(count);
        let opening = backing.counts();
        assert_eq!(opening.writes, 0);
        assert!(!opening.overflowed);
        if let Some(expected) = open_bytes {
            assert_eq!(opening.read_bytes, expected);
        }
        open_bytes = Some(opening.read_bytes);
        drop(store);
        let mut reader = manifest.reader();
        let mut candidate = Map::new(count);
        while let Some(index) = reader.next_page() {
            let store = Store::open(backing.clone(), false);
            assert_eq!(store.admit(count), manifest);
            backing.reset();
            let page = store.page(&manifest, index);
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
        assert_eq!(candidate.len(), Ok(count));
        candidate
            .with(|view| {
                for key in 0..count as u64 {
                    assert_eq!(view.get(u64::MAX - key), Some(key));
                }
            })
            .unwrap();
        println!(
            "paged checkpoint records={count} open_read_bytes={} selected_page_read_bytes={PAGE_BYTES}",
            opening.read_bytes
        );
    }
}

#[test]
fn failed_publication_missing_extra_mixed_and_corrupt_pages_refuse() {
    for fault in 0..8 {
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Store::open(backing.clone(), true);
        let source = create_source(129);
        if fault == 0 {
            // A sink can retain partial pages, but no successful manifest escapes.
            let result = source.export_pages(17, 9, |_, page| {
                store.pages.push(page);
                Err::<(), _>("interrupted export")
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
                    let last = store.pages.get(0).unwrap();
                    store.pages.set(2, &last);
                    store.pages.set(0, &first);
                }
                5 => {
                    let mut bad = store.pages.get(0).unwrap();
                    bad[88] ^= 1; // late corruption: first two pages still verify
                    store.pages.set(0, &bad);
                    let manifest = store.admit(129);
                    let mut reader = manifest.reader();
                    reader.read_page(&store.page(&manifest, 0)).unwrap();
                    reader.read_page(&store.page(&manifest, 1)).unwrap();
                    assert_eq!(
                        reader.read_page(&store.page(&manifest, 2)),
                        Err(Error::Integrity)
                    );
                    assert_eq!(reader.next_page(), Some(2));
                }
                6 => {
                    let mut other = create_source(129);
                    other
                        .with(|mut view| view.insert(u64::MAX, 777).unwrap())
                        .unwrap();
                    let foreign = other.export_pages(17, 9, |_, _| Ok::<_, ()>(())).unwrap();
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
