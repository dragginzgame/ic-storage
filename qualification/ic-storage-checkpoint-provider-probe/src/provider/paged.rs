//! Governed paged checkpoint storage shared by host and installed qualification.
//! Callers own authority, native custody and transition/record verification.
use ic_storage_legacy::{
    DefaultMemoryImpl, Memory, MemoryBacking, MemoryRuntime, StableKey,
    adapter::{StableCell, StableVec},
};
use ic_storage_native::map::{
    Map,
    paged::{PAGE_BYTES, Reader},
};
use ic_storage_native_feasibility::{
    checkpoint::maps::paged::{self, MANIFEST_BYTES, Manifest},
    migration_job::{METADATA_BYTES, PagedMapJob, paged_continuation},
};

pub(super) const MANIFEST: &str = "native.checkpoint.paged.maps.manifest.v1";
pub(super) const PAGES: &str = "native.checkpoint.paged.maps.pages.v1";
pub(super) const METADATA: &str = "native.checkpoint.paged.maps.job.metadata.v1";
pub(super) const CONTINUATION: &str = "native.checkpoint.paged.maps.job.continuation.v1";

/// Fault-injection boundaries, after the named governed write has completed.
#[derive(Clone, Copy)]
pub(crate) enum WritePoint {
    Page = 1,
    Manifest = 2,
    Continuation = 3,
    Metadata = 4,
}

pub(super) struct Store<M: Memory> {
    pub(super) manifest: StableCell<[u8; MANIFEST_BYTES], M>,
    pub(super) pages: StableVec<[u8; PAGE_BYTES], M>,
}

impl<M: MemoryBacking> Store<M> {
    pub(super) fn from_runtime(runtime: &MemoryRuntime<M>, create: bool) -> Self {
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
    pub(super) fn page(&self, manifest: &Manifest, root: usize, index: usize) -> [u8; PAGE_BYTES] {
        let selected = &manifest.roots()[root];
        assert!(index < selected.page_count());
        let start = if root == 0 {
            0
        } else {
            manifest.roots()[0].page_count()
        };
        let physical = start + selected.page_count() - index - 1;
        self.pages.get(physical as u64).unwrap()
    }
}

pub(crate) struct Jobs<M: Memory = DefaultMemoryImpl> {
    pub(super) data: Store<M>,
    pub(super) metadata: StableCell<[u8; METADATA_BYTES], M>,
    pub(super) continuation: StableCell<[u8; paged_continuation::BYTES], M>,
}

impl<M: MemoryBacking> Jobs<M> {
    pub(crate) fn open(memory: M, create: bool) -> Self {
        let runtime = super::runtime(memory, &[MANIFEST, PAGES, METADATA, CONTINUATION], create);
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
    /// Fresh namespace only. A failed host call can leave unsealed pages; only
    /// an enclosing IC message supplies rollback, never this write ordering.
    #[cfg(any(test, all(target_arch = "wasm32", feature = "paged-store")))]
    pub(crate) fn publish_checkpoint(
        &mut self,
        maps: &[Map; 2],
        source: u64,
        target: u64,
        epoch: u64,
        mut after: impl FnMut(WritePoint),
    ) {
        assert!(self.data.pages.is_empty());
        assert_eq!(self.data.manifest.get(), &[0; MANIFEST_BYTES]);
        assert_eq!(self.metadata.get(), &[0; METADATA_BYTES]);
        assert_eq!(self.continuation.get(), &[0; paged_continuation::BYTES]);
        let ready = PagedMapJob::initial(source).unwrap();
        let pair = paged::export(maps, epoch, |_, _, page| {
            self.data.pages.push(page);
            after(WritePoint::Page);
            Ok::<_, ()>(())
        })
        .unwrap();
        let job = ready.seal(source, target, epoch, &pair).unwrap();
        self.data.manifest.set(pair.encode());
        after(WritePoint::Manifest);
        self.write(&job);
        after(WritePoint::Metadata);
    }

    pub(crate) fn read(&self) -> PagedMapJob {
        if self.data.manifest.get() == &[0; MANIFEST_BYTES] {
            // Only a valid Ready envelope can decode with an empty payload.
            // Zeroed or corrupt metadata never becomes a fresh job here.
            assert!(self.data.pages.is_empty());
            assert_eq!(self.continuation.get(), &[0; paged_continuation::BYTES]);
            return PagedMapJob::decode_split(self.metadata.get(), &[]).unwrap();
        }
        PagedMapJob::decode_split(self.metadata.get(), self.data.manifest.get()).unwrap()
    }

    fn write(&mut self, job: &PagedMapJob) {
        let (metadata, payload) = job.encode_split().unwrap();
        assert_eq!(payload, self.data.manifest.get());
        if self.metadata.get() != &metadata {
            self.metadata.set(metadata);
        }
    }

    pub(crate) fn admit_pages(&self, manifest: &Manifest, limit: usize) {
        assert!(manifest.roots().iter().all(|root| root.len() <= limit));
        assert_eq!(self.data.pages.len(), manifest.page_count() as u64);
    }

    /// Capture only after authorized native writes and their verification. The
    /// codec checks job/cursor/count binding, not the semantics of applied writes.
    /// The caller must skip exact retries and atomically commit native state too.
    pub(crate) fn commit_progress(
        &mut self,
        job: &PagedMapJob,
        readers: &[Reader; 2],
        prefixes: &[paged_continuation::NativePrefix; 2],
        mut after: impl FnMut(WritePoint),
    ) {
        let continuation = paged_continuation::encode_observed(job, readers, prefixes).unwrap();
        assert_eq!(job.encode_split().unwrap().1, self.data.manifest.get());
        self.continuation.set(continuation);
        after(WritePoint::Continuation);
        self.write(job);
        after(WritePoint::Metadata);
    }

    pub(crate) fn resume(
        &self,
        job: &PagedMapJob,
        observe: impl FnOnce() -> Result<
            [paged_continuation::NativePrefix; 2],
            ic_storage_native_feasibility::migration_job::Error,
        >,
    ) -> [Reader; 2] {
        paged_continuation::decode_observed(self.continuation.get(), job, observe).unwrap()
    }

    pub(crate) fn page(&self, manifest: &Manifest, root: usize, index: usize) -> [u8; PAGE_BYTES] {
        self.data.page(manifest, root, index)
    }

    #[cfg(all(
        target_arch = "wasm32",
        any(feature = "paged-store", feature = "native-map-paged")
    ))]
    pub(crate) fn snapshot(&self) -> Vec<u8> {
        let mut bytes = self.metadata.get().to_vec();
        bytes.extend_from_slice(self.data.manifest.get());
        bytes.extend_from_slice(self.continuation.get());
        bytes.extend_from_slice(&self.data.pages.len().to_le_bytes());
        bytes
    }

    #[cfg(all(
        target_arch = "wasm32",
        any(feature = "paged-store", feature = "native-map-paged")
    ))]
    pub(crate) fn damage(&mut self, part: u64) {
        match part {
            0 => {
                let mut bytes = *self.metadata.get();
                bytes[24] ^= 1;
                self.metadata.set(bytes);
            }
            1 => {
                let mut bytes = *self.data.manifest.get();
                bytes[32] ^= 1;
                self.data.manifest.set(bytes);
            }
            2 => {
                let mut bytes = *self.continuation.get();
                bytes[120] ^= 1;
                self.continuation.set(bytes);
            }
            3 => {
                self.damage_page(0);
            }
            _ => panic!("unknown paged store part"),
        }
    }

    #[cfg(all(
        target_arch = "wasm32",
        any(feature = "paged-store", feature = "native-map-paged")
    ))]
    pub(crate) fn damage_page(&mut self, physical: u64) {
        let mut bytes = self.data.pages.get(physical).unwrap();
        bytes[80] ^= 1;
        self.data.pages.set(physical, &bytes);
    }
}

#[cfg(any(test, all(target_arch = "wasm32", feature = "native-map-paged")))]
impl<M: Memory> Jobs<M> {
    /// Called only while creating the native profile's fresh governed namespace.
    pub(crate) fn initialize(&mut self, source: u64) {
        assert!(self.data.pages.is_empty());
        assert_eq!(self.data.manifest.get(), &[0; MANIFEST_BYTES]);
        assert_eq!(self.metadata.get(), &[0; METADATA_BYTES]);
        assert_eq!(self.continuation.get(), &[0; paged_continuation::BYTES]);
        let (metadata, _) = PagedMapJob::initial(source)
            .unwrap()
            .encode_split()
            .unwrap();
        self.metadata.set(metadata);
    }

    /// Replace a retired checkpoint only after next-epoch and continuation
    /// admission. The caller freezes the authoritative roots first. These writes
    /// require an enclosing IC transaction; host unwinding supplies no rollback.
    pub(crate) fn publish_epoch(
        &mut self,
        maps: &[Map; 2],
        source: u64,
        target: u64,
        epoch: u64,
        mut after: impl FnMut(WritePoint),
    ) {
        let previous = self.read();
        let manifest = paged::export(maps, epoch, |_, _, _| Ok::<_, ()>(())).unwrap();
        let job = previous.seal(source, target, epoch, &manifest).unwrap();
        // Preserve this adapter's refusal of repeated exports. Retrying a batch
        // or a cancellation is distinct from replacing a published checkpoint.
        assert_ne!(previous, job);
        paged_continuation::validate_retirement(self.continuation.get(), &previous).unwrap();
        let (_, payload) = previous.encode_split().unwrap();
        let old_pages = if payload.is_empty() {
            0
        } else {
            Manifest::decode(payload, epoch - 1, usize::MAX)
                .unwrap()
                .page_count()
        };
        assert_eq!(self.data.pages.len(), old_pages as u64);
        let mut index = 0;
        let written = paged::export(maps, epoch, |_, _, page| {
            if index < self.data.pages.len() {
                self.data.pages.set(index, page);
            } else {
                self.data.pages.push(page);
            }
            index += 1;
            after(WritePoint::Page);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(written, manifest);
        while self.data.pages.len() > index {
            assert!(self.data.pages.pop().is_some());
            after(WritePoint::Page);
        }
        self.data.manifest.set(manifest.encode());
        after(WritePoint::Manifest);
        self.continuation.set([0; paged_continuation::BYTES]);
        after(WritePoint::Continuation);
        self.write(&job);
        after(WritePoint::Metadata);
    }

    /// Commit cancellation metadata; the caller must atomically thaw the source.
    pub(crate) fn cancel(&mut self, source: u64, target: u64, epoch: u64) -> bool {
        let mut job = self.read();
        let changed = job.cancel(source, target, epoch).unwrap();
        paged_continuation::validate_retirement(self.continuation.get(), &job).unwrap();
        let manifest = Manifest::decode(job.encode_split().unwrap().1, epoch, usize::MAX).unwrap();
        assert_eq!(self.data.pages.len(), manifest.page_count() as u64);
        self.write(&job);
        changed
    }
}
