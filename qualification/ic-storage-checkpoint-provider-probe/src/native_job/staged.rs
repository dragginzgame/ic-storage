//! Bounded native construction; no Rust collection owner exists until publication.
use super::payload;
use super::{Job, args, controller, domain, ic0, job_cell};

#[expect(unsafe_code, reason = "scalar imports bound to reviewed WAT callbacks")]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn stage_finish(length: u32);
    safe fn stage_ready();
    safe fn stage_damage(mode: u32);
}

#[expect(
    unsafe_code,
    reason = "private staged-start entry bound by final composer"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage() {
    start(false);
}

#[expect(
    unsafe_code,
    reason = "private restart entry bound only to fresh-heap upgrade composition"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_restart() {
    start(true);
}

fn start(restart: bool) {
    let [epoch, source, fault] = args();
    assert!(fault <= 1);
    super::fresh();
    let mut cell = job_cell(false);
    let mut job = Job::decode(cell.get()).unwrap();
    let decoded = if restart {
        job.restart_import(domain(), source, epoch)
    } else {
        job.begin_import(domain(), source, epoch)
    }
    .unwrap();
    payload::start(&decoded);
    cell.set(job.encode().unwrap());
    if fault == 1 {
        ic0::trap(b"import start after publication");
    }
}

#[expect(
    unsafe_code,
    reason = "private staged-resume entry bound by final composer"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_resume() {
    let [epoch, source, fault] = args();
    assert!(fault <= 1);
    let cell = job_cell(false);
    let job = Job::decode(cell.get()).unwrap();
    let (decoded, prefix) = job.importing(domain(), source, epoch).unwrap();
    payload::validate(&decoded, prefix);
    if fault == 1 {
        ic0::trap(b"import resume after validation");
    }
}

#[expect(
    unsafe_code,
    reason = "private staged-batch entry bound by final composer"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_batch() {
    controller();
    assert_eq!(ic0::msg_arg_data_size(), 40);
    let mut bytes = [0; 40];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    let [epoch, source, start, limit, fault]: [u64; 5] =
        std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()));
    assert!(fault <= 3);
    let mut cell = job_cell(false);
    let mut job = Job::decode(cell.get()).unwrap();
    let batch = job
        .batch(
            domain(),
            source,
            epoch,
            start.try_into().unwrap(),
            limit.try_into().unwrap(),
        )
        .unwrap();
    if batch.retry && batch.complete {
        // A successful final retry must preserve subsequent ordinary native writes.
        stage_ready();
    } else {
        payload::validate(
            &batch.checkpoint,
            if batch.retry { batch.end } else { batch.start },
        );
    }
    if !batch.retry {
        cell.set(job.encode().unwrap());
        if fault == 1 {
            ic0::trap(b"import batch after stable write");
        }
        for i in batch.start..batch.end {
            payload::append(&batch.checkpoint, i);
            if fault == 2 {
                ic0::trap(b"import batch during native copy");
            }
        }
        if batch.complete {
            stage_finish(batch.end);
        }
    }
    if fault == 3 {
        ic0::trap(b"import batch after native publication");
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private staged-custody fault injection entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_damage() {
    controller();
    let [mode, reserved, other] = args();
    assert_eq!([reserved, other], [0, 0]);
    assert!(mode <= 4);
    if mode == 4 {
        let mut cell = job_cell(false);
        let mut bytes = cell.get().clone();
        bytes[24] ^= 1;
        cell.set(bytes);
    } else {
        stage_damage(mode as u32);
    }
    ic0::msg_reply();
}
