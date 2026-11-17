//! Controller-authorized, single-use native migration job fixture.
use crate::provider::job_cell;
use ic_platform_runtime::raw as ic0;
#[cfg(not(feature = "native-record-vector-job"))]
use ic_storage_native_feasibility::migration_job::Job;
#[cfg(feature = "native-record-vector-job")]
use ic_storage_native_feasibility::migration_job::RecordJob as Job;
mod payload;

mod staged;

#[expect(unsafe_code, reason = "scalar imports bound to reviewed WAT callbacks")]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn domain() -> u64;
    safe fn freeze() -> u32;
    safe fn fresh();
    safe fn finish(length: u32);
    safe fn thaw(retry: u32);
}

use crate::job_control::{args, controller};

#[expect(unsafe_code, reason = "private entry bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_create() {
    job_cell(true).set(Job::initial(domain()).unwrap().encode().unwrap());
}

#[expect(unsafe_code, reason = "private entry bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    controller();
    let [epoch, target, fault] = args();
    assert!(fault <= 1);
    let mut cell = job_cell(false);
    let job = Job::decode(cell.get()).unwrap();
    let count = freeze();
    let values = payload::read(count, epoch);
    let next = payload::seal(&job, domain(), target, epoch, &values);
    cell.set(next.encode().unwrap());
    if fault == 1 {
        ic0::trap(b"job export after stable write");
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private recovery entry bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore() {
    let [epoch, source, fault] = args();
    assert!(fault <= 3);
    fresh();
    let mut cell = job_cell(false);
    let mut job = Job::decode(cell.get()).unwrap();
    let decoded = job.consume(domain(), source, epoch).unwrap();
    if fault == 1 {
        cell.set(job.encode().unwrap());
        ic0::trap(b"job restore after consumed write");
    }
    payload::restore_begin(&decoded);
    for i in 0..payload::len(&decoded) {
        payload::restore_put(&decoded, i);
        if fault == 2 {
            ic0::trap(b"job restore during construction");
        }
    }
    finish(payload::len(&decoded));
    cell.set(job.encode().unwrap());
    if fault == 3 {
        ic0::trap(b"job restore after native publication");
    }
}

#[expect(
    unsafe_code,
    reason = "private frozen-resume entry bound by final composer"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_resume() {
    let [epoch, target, fault] = args();
    assert!(fault <= 1);
    let cell = job_cell(false);
    let job = Job::decode(cell.get()).unwrap();
    let decoded = job.frozen(domain(), target, epoch).unwrap();
    // The WAT bootstrap has already restored phase 3 from an admitted frozen
    // header. This call checks it again and cannot thaw or create a native owner.
    let count = freeze();
    payload::frozen(&decoded, count);
    if fault == 1 {
        ic0::trap(b"frozen resume after validation");
    }
}

#[expect(
    unsafe_code,
    reason = "private cancellation entry bound by final composer"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_cancel() {
    controller();
    let [epoch, target, fault] = args();
    assert!(fault <= 2);
    let mut cell = job_cell(false);
    let mut job = Job::decode(cell.get()).unwrap();
    let changed = job.cancel(domain(), target, epoch).unwrap();
    if changed {
        cell.set(job.encode().unwrap());
    }
    if fault == 1 {
        ic0::trap(b"job cancellation after stable write");
    }
    // No freeze on retry: the source may have legitimate newer native writes.
    thaw(u32::from(!changed));
    if fault == 2 {
        ic0::trap(b"job cancellation after native thaw");
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private evidence query bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_bytes() {
    ic0::msg_reply_data_append(job_cell(false).get());
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private governed fault injection entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_damage() {
    controller();
    let [mode, epoch, target] = args();
    assert!(mode <= 3);
    let count = freeze();
    assert!(count <= payload::LIMIT);
    let mut cell = job_cell(false);
    if mode == 0 {
        assert_eq!([epoch, target], [0, 0]);
        let mut bytes = cell.get().clone();
        bytes[24] ^= 1;
        cell.set(bytes);
    } else {
        // Deliberate, correctly encoded disagreement with the frozen native
        // source, using the real codec/provider rather than host stable writes.
        let mut state = payload::read(count, epoch);
        payload::damage(&mut state, mode);
        let job = payload::seal(
            &Job::initial(domain()).unwrap(),
            domain(),
            target,
            epoch,
            &state,
        );
        cell.set(job.encode().unwrap());
    }
    ic0::msg_reply();
}
