//! Installed paged/native transaction qualification within the bounded map ABI.
//! Sequential checkpoint epochs, source cancellation and one page per call.
use super::{
    freeze, fresh, map_count, map_finish, map_put, map_word, read_source,
    staging::{
        map_stage_commit_state, map_stage_damage, map_stage_prepare, map_stage_start,
        map_stage_state, map_stage_verified, stage_ready,
    },
};
use crate::{
    job_control::{args, controller},
    provider::paged::{Jobs, WritePoint},
};
use ic_platform_runtime::raw as ic0;
use ic_storage_legacy::DefaultMemoryImpl;
use ic_storage_native::map::paged::Reader;
use ic_storage_native_feasibility::{
    checkpoint::maps::{Records, paged, verify_records},
    migration_job::{PagedMapJob, paged_continuation::NativePrefix},
};
use std::cell::RefCell;

// Closed installed shape: a 4096-bucket first root and 2048-bucket second
// root, including their sequential rehash candidates in the 192 KiB pool.
// One staged page remains bounded to 64 records.
const LIMIT: usize = 3072;
const SECOND_LIMIT: usize = 1536;

#[expect(
    unsafe_code,
    reason = "scalar domain identity supplied by the native wrapper"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn domain() -> u64;
    safe fn thaw(retry: u32);
}

struct Pending {
    job: PagedMapJob,
    readers: [Reader; 2],
    expected: Records,
    complete: bool,
    fault: u64,
}
thread_local! {
    // Only between two checkpoint frames in ONE IC message. Native Rust runs
    // between them, with no live checkpoint borrow. Heap allocations are outside
    // its retained pool. Success takes/drops the plan; an IC trap rolls it back.
    // Neither this Rust value nor a borrow is used to resume another message.
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}
fn idle() {
    PENDING.with(|slot| assert!(slot.borrow().is_none()));
}
fn pending(plan: Pending) {
    PENDING.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(plan);
    });
}
fn open() -> Jobs {
    Jobs::open(DefaultMemoryImpl::default(), false)
}
fn admit_pages(store: &Jobs, manifest: &paged::Manifest) {
    store.admit_pages(manifest, LIMIT);
    assert!(manifest.roots()[1].len() <= SECOND_LIMIT);
}
fn fault(point: WritePoint, selected: u64) {
    if matches!(
        (point, selected),
        (WritePoint::Continuation, 1) | (WritePoint::Metadata, 9)
    ) {
        ic0::trap(b"paged native progress after governed write");
    }
}
fn observe(mut record: impl FnMut(usize, (u64, u64))) -> [NativePrefix; 2] {
    std::array::from_fn(|id| {
        let count = map_count(id as u32);
        assert!(count as usize <= if id == 0 { LIMIT } else { SECOND_LIMIT });
        NativePrefix::from_records(
            count as usize,
            (0..count).map(|index| {
                let key = map_word(id as u32, index, 0);
                let value = map_word(id as u32, index, 1);
                record(id, (key, value));
                (key, value)
            }),
        )
        .unwrap()
    })
}

#[expect(unsafe_code, reason = "private fresh governed paged namespace")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_create() {
    idle();
    Jobs::open(DefaultMemoryImpl::default(), true).initialize(domain());
}
#[expect(unsafe_code, reason = "private controller-authorized paged export")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    idle();
    controller();
    let [epoch, target, fail] = args();
    assert!(fail <= 4);
    freeze();
    let maps = read_source(LIMIT);
    assert!(maps[1].len().unwrap() <= SECOND_LIMIT);
    open().publish_epoch(&maps, domain(), target, epoch, |point| {
        if point as u64 == fail {
            ic0::trap(b"paged native export after governed write");
        }
    });
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "private controller-authorized source cancellation"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_cancel() {
    idle();
    controller();
    let [epoch, target, fail] = args();
    assert!(fail <= 2);
    let changed = open().cancel(domain(), target, epoch);
    if fail == 1 {
        ic0::trap(b"paged cancellation after governed write");
    }
    thaw(u32::from(!changed));
    if fail == 2 {
        ic0::trap(b"paged cancellation after thaw");
    }
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "private admitted frozen-source resumption")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_resume() {
    idle();
    let [epoch, target, fail] = args();
    assert!(fail <= 1);
    freeze();
    let store = open();
    let manifest = store.read().frozen(domain(), target, epoch).unwrap();
    admit_pages(&store, &manifest);
    let actual = paged::export(&read_source(LIMIT), epoch, |_, _, _| Ok::<_, ()>(())).unwrap();
    assert_eq!(actual, manifest);
    if fail == 1 {
        ic0::trap(b"paged frozen resume after validation");
    }
}
#[expect(unsafe_code, reason = "private fresh-target staged upgrade")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage() {
    start(false);
}
#[expect(unsafe_code, reason = "private explicit unfinished-target restart")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_restart() {
    start(true);
}
fn start(restart: bool) {
    idle();
    let [epoch, source, fail] = args();
    assert!(matches!(fail, 0..=3 | 5 | 9 | 10));
    fresh();
    let store = open();
    let mut job = store.read();
    let manifest = if restart {
        job.restart_import(domain(), source, epoch)
    } else {
        job.begin_import(domain(), source, epoch)
    }
    .unwrap();
    admit_pages(&store, &manifest);
    let readers = manifest.roots().each_ref().map(|root| root.reader());
    map_stage_start(fail as u32);
    pending(Pending {
        job,
        readers,
        expected: [Vec::new(), Vec::new()],
        complete: false,
        fault: fail,
    });
}
#[expect(
    unsafe_code,
    reason = "private retained-prefix and continuation resumption"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_resume() {
    idle();
    let [epoch, source, fail] = args();
    assert!(fail <= 1);
    map_stage_state();
    let store = open();
    let job = store.read();
    let (manifest, _) = job.importing(domain(), source, epoch).unwrap();
    admit_pages(&store, &manifest);
    store.resume(&job, || Ok(observe(|_, _| {})));
    if fail == 1 {
        ic0::trap(b"paged import resume after validation");
    }
}
#[expect(
    unsafe_code,
    reason = "private controller-authorized paged native batch"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_batch() {
    idle();
    controller();
    assert_eq!(ic0::msg_arg_data_size(), 40);
    let mut bytes = [0; 40];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    let [epoch, source, start, limit, fail] =
        std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()));
    assert_eq!(limit, 1); // The native staging ABI admits at most 64 new records.
    assert!(fail <= 14);
    let store = open();
    let mut job = store.read();
    job.staged_prefix(domain(), source, epoch).unwrap();
    let mut expected: Records = [Vec::new(), Vec::new()];
    let mut readers = store.resume(&job, || {
        map_stage_state();
        Ok(observe(|id, pair| expected[id].push(pair)))
    });
    let batch = job
        .batch(domain(), source, epoch, start.try_into().unwrap(), 1)
        .unwrap();
    admit_pages(&store, &batch.checkpoint);
    if batch.retry {
        if batch.complete {
            stage_ready();
        }
        return; // No pending plan, native writes, continuation or metadata writes.
    }
    let mut staged: Records = [Vec::new(), Vec::new()];
    for global in batch.start as usize..batch.end as usize {
        let id = usize::from(global >= batch.checkpoint.roots()[0].page_count());
        let index = readers[id].next_page().unwrap();
        staged[id] = readers[id]
            .read_page(&store.page(&batch.checkpoint, id, index))
            .unwrap();
        expected[id].extend_from_slice(&staged[id]);
    }
    for records in &mut expected {
        records.sort_unstable_by_key(|pair| pair.0);
    }
    map_stage_prepare(
        staged[0].len() as u32,
        staged[1].len() as u32,
        u32::from(batch.complete),
        fail as u32,
    );
    for (id, records) in staged.iter().enumerate() {
        for (index, &(key, value)) in records.iter().enumerate() {
            map_put(id as u32, index as u32, key, value);
            if fail == 2 {
                ic0::trap(b"paged native batch during staging");
            }
        }
    }
    map_finish();
    pending(Pending {
        job,
        readers,
        expected,
        complete: batch.complete,
        fault: fail,
    });
}
#[expect(
    unsafe_code,
    reason = "private native post-apply verification and governed commit"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_commit() {
    if map_stage_commit_state() == 0 {
        idle();
        return;
    }
    let plan = PENDING.with(|slot| slot.borrow_mut().take().unwrap());
    for (id, expected) in plan.expected.iter().enumerate() {
        let count = map_count(id as u32) as usize;
        verify_records::<{ LIMIT.div_ceil(64) }>(expected, count, |index| {
            let key = map_word(id as u32, index as u32, 0);
            let value = map_word(id as u32, index as u32, 1);
            (key, value)
        })
        .unwrap_or_else(|_| ic0::trap(b"paged native records differ after apply"));
    }
    if plan.fault == 5 {
        ic0::trap(b"paged native after verification");
    }
    let prefixes = observe(|_, _| {});
    open().commit_progress(&plan.job, &plan.readers, &prefixes, |point| {
        fault(point, plan.fault)
    });
    map_stage_verified(u32::from(plan.complete));
    // The wrapper publishes both native roots only after this frame has ended.
}
#[expect(unsafe_code, reason = "private governed paged evidence query")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_bytes() {
    idle();
    ic0::msg_reply_data_append(&open().snapshot());
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "private governed page evidence query")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_page() {
    idle();
    let [epoch, id, index] = args();
    let store = open();
    let job = store.read();
    let manifest = paged::Manifest::decode(job.encode_split().unwrap().1, epoch, LIMIT).unwrap();
    admit_pages(&store, &manifest);
    assert!(id < 2);
    ic0::msg_reply_data_append(&store.page(&manifest, id as usize, index.try_into().unwrap()));
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "private admitted native-prefix evidence query")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_native_prefix() {
    idle();
    for prefix in observe(|_, _| {}) {
        ic0::msg_reply_data_append(&(prefix.len() as u64).to_le_bytes());
        ic0::msg_reply_data_append(&prefix.fingerprint());
    }
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "private controller-authorized storage corruption fixture"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_damage() {
    idle();
    controller();
    let [part, reserved, other] = args();
    if part == 4 {
        assert_eq!(other, 0);
        open().damage_page(reserved); // Physical page index, in exporter order.
    } else {
        assert_eq!([reserved, other], [0, 0]);
        open().damage(part);
    }
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "private controller-authorized native corruption fixture"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_damage() {
    idle();
    controller();
    let [mode, reserved, other] = args();
    assert_eq!([reserved, other], [0, 0]);
    assert!(mode <= 2);
    map_stage_damage(mode as u32);
    ic0::msg_reply();
}
