//! Host staging through the shared job policy. No installed transaction claim.
use super::*;
use ic_storage_native_feasibility::migration_job::{
    METADATA_BYTES, PagedMapJob, paged_continuation,
};

const METADATA: &str = "native.checkpoint.paged.maps.job.metadata.v1";
const CONTINUATION: &str = "native.checkpoint.paged.maps.job.continuation.v1";
const A: u64 = 131072;
const B: u64 = 163840;
const LIMIT: usize = 4096;

struct Jobs<M: Memory> {
    data: Store<M>,
    metadata: StableCell<[u8; METADATA_BYTES], M>,
    continuation: StableCell<[u8; paged_continuation::BYTES], M>,
}

impl<M: MemoryBacking> Jobs<M> {
    fn open(memory: M, create: bool) -> Self {
        let runtime =
            super::super::runtime(memory, &[MANIFEST, PAGES, METADATA, CONTINUATION], create);
        let metadata = runtime
            .open_memory(&StableKey::parse(METADATA).unwrap())
            .unwrap();
        let continuation = runtime
            .open_memory(&StableKey::parse(CONTINUATION).unwrap())
            .unwrap();
        Self {
            data: Store::from_runtime(&runtime, create),
            metadata: if create {
                StableCell::create(metadata, [0; METADATA_BYTES]).unwrap()
            } else {
                StableCell::load(metadata, METADATA_BYTES).unwrap()
            },
            continuation: if create {
                StableCell::create(continuation, [0; paged_continuation::BYTES]).unwrap()
            } else {
                StableCell::load(continuation, paged_continuation::BYTES).unwrap()
            },
        }
    }
}

impl<M: Memory> Jobs<M> {
    fn publish(&mut self, source: &[Map; 2]) {
        assert!(self.data.pages.is_empty());
        assert_eq!(self.data.manifest.get(), &[0; MANIFEST_BYTES]);
        assert_eq!(self.metadata.get(), &[0; METADATA_BYTES]);
        assert_eq!(self.continuation.get(), &[0; paged_continuation::BYTES]);
        let pair = paged::export(source, 1, |_, _, page| {
            self.data.pages.push(page);
            Ok::<_, ()>(())
        })
        .unwrap();
        let job = PagedMapJob::initial(A)
            .unwrap()
            .seal(A, B, 1, &pair)
            .unwrap();
        self.data.manifest.set(pair.encode());
        self.write(&job); // Final seal; a missing or mixed part refuses reopening.
    }

    fn read(&self) -> PagedMapJob {
        PagedMapJob::decode_split(self.metadata.get(), self.data.manifest.get()).unwrap()
    }

    fn write(&mut self, job: &PagedMapJob) {
        let (metadata, payload) = job.encode_split().unwrap();
        assert_eq!(payload, self.data.manifest.get());
        if self.metadata.get() != &metadata {
            self.metadata.set(metadata);
        }
    }

    fn admit_pages(&self, manifest: &Manifest) {
        assert!(manifest.roots().iter().all(|root| root.len() <= LIMIT));
        assert_eq!(self.data.pages.len(), manifest.page_count() as u64);
    }

    fn begin(&mut self) -> [Map; 2] {
        let mut job = self.read();
        let manifest = job.begin_import(B, A, 1).unwrap();
        self.admit_pages(&manifest);
        let candidates = manifest.roots().each_ref().map(|root| Map::new(root.len()));
        let readers = manifest.roots().each_ref().map(|root| root.reader());
        self.continuation
            .set(paged_continuation::encode(&job, &readers, &candidates).unwrap());
        self.write(&job);
        candidates
    }

    // Explicitly discards a private unfinished target. No completed-root reset.
    fn restart(&mut self) -> [Map; 2] {
        let mut job = self.read();
        let manifest = job.restart_import(B, A, 1).unwrap();
        self.admit_pages(&manifest);
        let candidates = manifest.roots().each_ref().map(|root| Map::new(root.len()));
        let readers = manifest.roots().each_ref().map(|root| root.reader());
        self.continuation
            .set(paged_continuation::encode(&job, &readers, &candidates).unwrap());
        self.write(&job);
        candidates
    }

    fn advance(&mut self, candidates: &mut [Map; 2], start: u32, limit: u32) -> bool {
        let (job, batch) = PagedMapJob::decode_split_batch(
            self.metadata.get(),
            self.data.manifest.get(),
            B,
            A,
            1,
            start,
            limit,
        )
        .unwrap();
        self.admit_pages(&batch.checkpoint);
        let mut readers =
            paged_continuation::decode(self.continuation.get(), &self.read(), candidates).unwrap();
        // A consumed retry acknowledges prior publication. Later normal writes
        // are authoritative and must neither be compared with nor overwritten by
        // the old checkpoint.
        if batch.retry {
            return batch.complete;
        }
        let mut pending = Vec::new();
        for global in batch.start as usize..batch.end as usize {
            let root = usize::from(global >= batch.checkpoint.roots()[0].page_count());
            let index = readers[root].next_page().unwrap();
            let records = readers[root]
                .read_page(&self.data.page(&batch.checkpoint, root, index))
                .unwrap();
            pending.push((root, records));
        }
        // Prefix and all requested page bytes validate before any target write.
        // An installed adapter must roll back allocation failure/native writes
        // together with metadata. Host unwinding does not establish that contract.
        for (root, records) in pending {
            candidates[root]
                .with(|mut view| {
                    for (key, value) in records {
                        assert!(view.insert(key, value).unwrap().is_none());
                    }
                })
                .unwrap();
        }
        self.continuation
            .set(paged_continuation::encode(&job, &readers, candidates).unwrap());
        self.write(&job);
        batch.complete
    }
}

fn observations(maps: &[Map; 2]) -> [Vec<(u64, u64)>; 2] {
    maps.each_ref()
        .map(|map| Map::records_from_checkpoint(&map.checkpoint(1).unwrap(), 1, LIMIT).unwrap())
}

#[test]
fn durable_page_batches_reopen_retry_and_publish_both_roots() {
    for counts in [[0, 0], [0, 65], [65, 0], [129, 65], [2304, 2304]] {
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Jobs::open(backing.clone(), true);
        let source = create_sources(counts);
        let expected = observations(&source);
        store.publish(&source);
        drop(source);
        let manifest = *store.data.manifest.get();
        let mut candidates = store.begin();
        drop(store);
        let mut start = 0;
        loop {
            let mut store = Jobs::open(backing.clone(), false);
            backing.reset();
            let complete = store.advance(&mut candidates, start, 2);
            assert_eq!(
                backing.counts().write_bytes,
                104 + paged_continuation::BYTES as u64
            );
            let pages = counts.iter().map(|count| count.div_ceil(64)).sum::<usize>();
            let read_pages = pages.saturating_sub(start as usize).min(2);
            assert_eq!(
                backing.counts().read_bytes,
                (read_pages * PAGE_BYTES) as u64
            );
            assert_eq!(backing.counts().grows, 0);
            assert_eq!(store.data.manifest.get(), &manifest);
            let before = observations(&candidates);
            let metadata = *store.metadata.get();
            drop(store);
            let mut store = Jobs::open(backing.clone(), false);
            backing.reset();
            assert_eq!(store.advance(&mut candidates, start, 2), complete);
            assert_eq!(backing.counts().writes, 0);
            assert_eq!(backing.counts().read_bytes, 0);
            assert_eq!(store.metadata.get(), &metadata);
            assert_eq!(observations(&candidates), before);
            if complete {
                assert_eq!(before, expected);
                // Application mutation after publication survives the final retry.
                if counts[1] != 0 {
                    candidates[1]
                        .with(|mut view| view.insert(u64::MAX, 777).unwrap())
                        .unwrap();
                }
                let changed = observations(&candidates);
                assert!(store.advance(&mut candidates, start, 2));
                assert_eq!(observations(&candidates), changed);
                assert!(catch_unwind(AssertUnwindSafe(|| store.restart())).is_err());
                break;
            }
            drop(store);
            start += 2;
        }
    }
}

#[test]
fn damaged_prefix_or_later_page_refuses_before_progress_and_target_writes() {
    for fault in 0..6 {
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Jobs::open(backing.clone(), true);
        store.publish(&create_sources([129, 129]));
        let mut candidates = store.begin();
        assert!(!store.advance(&mut candidates, 0, 2));
        match fault {
            0 => {
                // Wrong native value in the committed prefix.
                candidates[0]
                    .with(|mut view| view.insert(u64::MAX - 128, 999).unwrap())
                    .unwrap();
            }
            1 => {
                // An unexpected root-2 value before root 2 begins.
                candidates[1]
                    .with(|mut view| view.insert(1, 2).unwrap())
                    .unwrap();
            }
            2 => {
                let mut bytes = *store.continuation.get();
                bytes[128] ^= 1;
                store.continuation.set(bytes);
            }
            3 => {
                // Root-2 first page after a valid newly staged root-1 last page.
                let index = 5;
                let mut page = store.data.pages.get(index).unwrap();
                page[80] ^= 1;
                store.data.pages.set(index, &page);
            }
            4 => {
                store.data.pages.pop().unwrap();
            }
            _ => {
                let other = paged::export(&create_sources([129, 65]), 1, |_, _, _| Ok::<_, ()>(()))
                    .unwrap();
                store.data.manifest.set(other.encode());
            }
        }
        let before = observations(&candidates);
        let metadata = *store.metadata.get();
        let continuation = *store.continuation.get();
        drop(store);
        let mut store = Jobs::open(backing.clone(), false);
        backing.reset();
        assert!(catch_unwind(AssertUnwindSafe(|| store.advance(&mut candidates, 2, 2))).is_err());
        assert_eq!(backing.counts().writes, 0);
        assert_eq!(store.metadata.get(), &metadata);
        assert_eq!(store.continuation.get(), &continuation);
        assert_eq!(observations(&candidates), before);
        if fault < 3 {
            // Explicit fresh-target restart needs no valid old native prefix.
            candidates = store.restart();
            assert!(!store.advance(&mut candidates, 0, 2));
            assert!(!store.advance(&mut candidates, 2, 2));
            assert!(store.advance(&mut candidates, 4, 2));
            assert_eq!(
                observations(&candidates),
                observations(&create_sources([129, 129]))
            );
        }
    }
}

#[test]
fn already_consumed_page_damage_is_checked_on_explicit_restart() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let mut store = Jobs::open(backing.clone(), true);
    store.publish(&create_sources([129, 129]));
    let mut candidates = store.begin();
    assert!(!store.advance(&mut candidates, 0, 2));
    let mut page = store.data.pages.get(2).unwrap(); // already imported page 0
    page[80] ^= 1;
    store.data.pages.set(2, &page);
    drop(store);
    let mut store = Jobs::open(backing.clone(), false);
    backing.reset();
    // Those records and their fingerprint are already committed. The next
    // request reads only its own pages; it is not a backup-integrity audit.
    assert!(!store.advance(&mut candidates, 2, 2));
    assert_eq!(backing.counts().read_bytes, 2 * PAGE_BYTES as u64);
    candidates = store.restart();
    let metadata = *store.metadata.get();
    let continuation = *store.continuation.get();
    backing.reset();
    assert!(catch_unwind(AssertUnwindSafe(|| store.advance(&mut candidates, 0, 2))).is_err());
    assert_eq!(backing.counts().writes, 0);
    assert_eq!(store.metadata.get(), &metadata);
    assert_eq!(store.continuation.get(), &continuation);
    assert_eq!(candidates.each_ref().map(|map| map.len().unwrap()), [0, 0]);
}

#[test]
fn mixed_job_and_continuation_publication_never_advances() {
    for restore_metadata in [false, true] {
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Jobs::open(backing.clone(), true);
        store.publish(&create_sources([129, 129]));
        let mut candidates = store.begin();
        let metadata = *store.metadata.get();
        let continuation = *store.continuation.get();
        assert!(!store.advance(&mut candidates, 0, 2));
        if restore_metadata {
            store.metadata.set(metadata);
        } else {
            store.continuation.set(continuation);
        }
        let before = observations(&candidates);
        drop(store);
        let mut store = Jobs::open(backing.clone(), false);
        backing.reset();
        let start = if restore_metadata { 0 } else { 2 };
        assert!(
            catch_unwind(AssertUnwindSafe(|| store.advance(
                &mut candidates,
                start,
                2
            )))
            .is_err()
        );
        assert_eq!((backing.counts().reads, backing.counts().writes), (0, 0));
        assert_eq!(observations(&candidates), before);
    }
}

#[test]
fn absent_job_metadata_and_unsealed_publication_refuse_without_writes() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let mut plain = Store::open(backing.clone(), true);
    plain.publish(&create_sources([1, 1]));
    drop(plain);
    backing.reset();
    assert!(catch_unwind(AssertUnwindSafe(|| Jobs::open(backing.clone(), false))).is_err());
    assert_eq!(backing.counts().writes, 0);
    // An older host job namespace cannot silently acquire a continuation cell.
    let backing = MeteredMemory::<VectorMemory>::default();
    let runtime = super::super::runtime(backing.clone(), &[MANIFEST, PAGES, METADATA], true);
    let store = Store::from_runtime(&runtime, true);
    let memory = runtime
        .open_memory(&StableKey::parse(METADATA).unwrap())
        .unwrap();
    drop(StableCell::create(memory, [0; METADATA_BYTES]).unwrap());
    drop(store);
    drop(runtime);
    backing.reset();
    assert!(catch_unwind(AssertUnwindSafe(|| Jobs::open(backing.clone(), false))).is_err());
    assert_eq!(backing.counts().writes, 0);
    for part in 0..3 {
        let backing = MeteredMemory::<VectorMemory>::default();
        let mut store = Jobs::open(backing.clone(), true);
        store.publish(&create_sources([65, 65]));
        match part {
            0 => {
                store.metadata.set([0; METADATA_BYTES]);
            }
            1 => {
                store.data.manifest.set([0; MANIFEST_BYTES]);
            }
            _ => {
                store.data.pages.pop().unwrap();
            }
        }
        drop(store);
        backing.reset();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || Jobs::open(backing.clone(), false).begin()
            ))
            .is_err()
        );
        assert_eq!(backing.counts().writes, 0);
    }
}
