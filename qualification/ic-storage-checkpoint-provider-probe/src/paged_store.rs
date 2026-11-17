//! Installed storage transaction probe, not a retained native import adapter.
//! Each request reconstructs transient maps from pages solely to exercise the
//! shared continuation codec. This deliberately does not claim bounded reads.
use crate::provider::memory::StableMemory;
use crate::{
    job_control::{args, controller},
    provider::paged::{Jobs, WritePoint},
};
use ic_platform_runtime::raw as ic0;
use ic_storage::map::{Map, paged::Reader};
use ic_storage_native_feasibility::migration_job::paged_continuation::NativePrefix;
use ic_storage_native_feasibility::{
    checkpoint::maps::paged::Manifest, migration_job::PagedMapJob,
};

const A: u64 = 131072;
const B: u64 = 393216;
const LIMIT: usize = 4096;
const BUILD: u64 = if cfg!(feature = "version-b") {
    202
} else {
    101
};

fn open() -> Jobs {
    Jobs::open(StableMemory::default(), false)
}
fn fault(point: WritePoint, selected: u64) {
    if point as u64 == selected {
        ic0::trap(b"paged store after governed write");
    }
}

fn apply(
    store: &Jobs,
    manifest: &Manifest,
    readers: &mut [Reader; 2],
    maps: &mut [Map; 2],
    start: u32,
    end: u32,
) {
    for global in start as usize..end as usize {
        let root = usize::from(global >= manifest.roots()[0].page_count());
        let index = readers[root].next_page().unwrap();
        let records = readers[root]
            .read_page(&store.page(manifest, root, index))
            .unwrap();
        maps[root]
            .with(|mut view| {
                for (key, value) in records {
                    assert!(view.insert(key, value).unwrap().is_none());
                }
            })
            .unwrap();
    }
}

fn materialize(store: &Jobs, job: &PagedMapJob) -> ([Reader; 2], [Map; 2]) {
    let (manifest, next, _) = job.staged_prefix(B, A, 1).unwrap();
    store.admit_pages(&manifest, LIMIT);
    let mut readers = manifest.roots().each_ref().map(|root| root.reader());
    let mut maps = manifest.roots().each_ref().map(|root| Map::new(root.len()));
    apply(store, &manifest, &mut readers, &mut maps, 0, next);
    (store.resume(job, || NativePrefix::from_maps(&maps)), maps)
}

#[expect(unsafe_code, reason = "fixture-owned governed page initialization")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    assert_eq!(ic0::msg_arg_data_size(), 0);
    let _ = Jobs::open(StableMemory::default(), true);
}

#[expect(unsafe_code, reason = "fixture-owned ordinary reopening")]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn post_upgrade() {
    if ic0::msg_arg_data_size() != 0 {
        advance_stored(args());
        return;
    }
    let store = open();
    let job = store.read();
    if let Ok(manifest) = job.frozen(A, B, 1) {
        store.admit_pages(&manifest, LIMIT);
    } else {
        drop(materialize(&store, &job));
    }
}

#[expect(
    unsafe_code,
    reason = "fixture-owned controller-authorized publication"
)]
#[unsafe(export_name = "canister_update publish")]
extern "C" fn publish() {
    controller();
    let [a, b, fail] = args();
    assert!(a <= LIMIT as u64 && b <= LIMIT as u64 && matches!(fail, 0 | 1 | 2 | 4 | 5));
    let maps = [a, b].map(|count| {
        let mut map = Map::new(count as usize);
        map.with(|mut view| {
            for key in 0..count {
                view.insert(u64::MAX - key, key ^ count).unwrap();
            }
        })
        .unwrap();
        map
    });
    let mut written_pages = 0;
    open().publish_checkpoint(&maps, A, B, 1, |point| {
        if matches!(point, WritePoint::Page) {
            written_pages += 1;
            if fail == 5 && written_pages == a.div_ceil(64) + 1 {
                fault(point, 1); // First page of the second root after root 1.
            }
        }
        fault(point, fail);
    });
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "fixture-owned controller-authorized progress start"
)]
#[unsafe(export_name = "canister_update begin")]
extern "C" fn begin() {
    controller();
    let [fail, reserved, other] = args();
    assert!(matches!(fail, 0 | 3 | 4));
    assert_eq!([reserved, other], [0, 0]);
    let mut store = open();
    let mut job = store.read();
    let manifest = job.begin_import(B, A, 1).unwrap();
    store.admit_pages(&manifest, LIMIT);
    let maps = manifest.roots().each_ref().map(|root| Map::new(root.len()));
    let readers = manifest.roots().each_ref().map(|root| root.reader());
    store.commit_progress(
        &job,
        &readers,
        &NativePrefix::from_maps(&maps).unwrap(),
        |point| fault(point, fail),
    );
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "fixture-owned controller-authorized progress publication"
)]
#[unsafe(export_name = "canister_update advance")]
extern "C" fn advance() {
    controller();
    advance_stored(args());
    ic0::msg_reply();
}

fn advance_stored([start, limit, fail]: [u64; 3]) {
    assert!(matches!(fail, 0 | 3 | 4));
    let mut store = open();
    let mut job = store.read();
    let (mut readers, mut maps) = materialize(&store, &job);
    let batch = job
        .batch(
            B,
            A,
            1,
            start.try_into().unwrap(),
            limit.try_into().unwrap(),
        )
        .unwrap();
    if !batch.retry {
        apply(
            &store,
            &batch.checkpoint,
            &mut readers,
            &mut maps,
            batch.start,
            batch.end,
        );
        store.commit_progress(
            &job,
            &readers,
            &NativePrefix::from_maps(&maps).unwrap(),
            |point| fault(point, fail),
        );
    }
}

#[expect(unsafe_code, reason = "fixture-owned bounded storage evidence query")]
#[unsafe(export_name = "canister_query state")]
extern "C" fn state() {
    ic0::msg_reply_data_append(&BUILD.to_le_bytes());
    ic0::msg_reply_data_append(&open().snapshot());
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "fixture-owned independent page evidence query")]
#[unsafe(export_name = "canister_query page")]
extern "C" fn page() {
    let [root, index, reserved] = args();
    assert_eq!(reserved, 0);
    let store = open();
    let job = store.read();
    let manifest = job
        .frozen(A, B, 1)
        .unwrap_or_else(|_| job.staged_prefix(B, A, 1).unwrap().0);
    store.admit_pages(&manifest, LIMIT);
    ic0::msg_reply_data_append(&store.page(
        &manifest,
        root.try_into().unwrap(),
        index.try_into().unwrap(),
    ));
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "fixture-owned governed corruption injection")]
#[unsafe(export_name = "canister_update damage")]
extern "C" fn damage() {
    controller();
    let [part, reserved, other] = args();
    assert_eq!([reserved, other], [0, 0]);
    open().damage(part);
    ic0::msg_reply();
}
