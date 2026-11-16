//! Durable two-map jobs; native construction runs after this Rust domain returns.
use super::{freeze, fresh, read_source, stage};
use crate::{
    job_control::{args, controller},
    provider::map_job_cell,
};
use ic_platform_runtime::raw as ic0;
use ic_storage_native_feasibility::migration_job::MapJob;

#[expect(
    unsafe_code,
    reason = "scalar imports bound to non-reentrant provider callbacks"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn domain() -> u64;
    safe fn thaw(retry: u32);
}

#[expect(unsafe_code, reason = "private governed map job initialization")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_create() {
    map_job_cell(true).set(MapJob::initial(domain()).unwrap().encode().unwrap());
}
#[expect(unsafe_code, reason = "private controller-authorized export")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    controller();
    let [epoch, target, fault] = args();
    assert!(fault <= 1);
    freeze();
    let mut cell = map_job_cell(false);
    let job = MapJob::decode(cell.get()).unwrap();
    let next = job.export(domain(), target, epoch, &read_source()).unwrap();
    cell.set(next.encode().unwrap());
    if fault == 1 {
        ic0::trap(b"map job export after stable write");
    }
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "private explicit fresh-heap recovery")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore() {
    let [epoch, source, fault] = args();
    assert!(fault <= 4);
    fresh();
    let mut cell = map_job_cell(false);
    let mut job = MapJob::decode(cell.get()).unwrap();
    let decoded = job.consume(domain(), source, epoch).unwrap();
    // This write and subsequent WAT/native construction share one IC upgrade.
    // Traps after Rust returns must roll this consumption back as well.
    cell.set(job.encode().unwrap());
    if fault == 1 {
        ic0::trap(b"map job recovery after consumed write");
    }
    stage(decoded, fault);
}
#[expect(unsafe_code, reason = "private validated frozen-source resumption")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_resume() {
    let [epoch, target, fault] = args();
    assert!(fault <= 1);
    freeze();
    let job = MapJob::decode(map_job_cell(false).get()).unwrap();
    job.frozen(domain(), target, epoch).unwrap();
    // Export's exact retry compares both payloads, including their root binding.
    // Read in the same stable native iteration order used by the original export.
    assert_eq!(
        job.export(domain(), target, epoch, &read_source()).unwrap(),
        job
    );
    if fault == 1 {
        ic0::trap(b"map job frozen resume after validation");
    }
}
#[expect(unsafe_code, reason = "private controller-authorized cancellation")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_cancel() {
    controller();
    let [epoch, target, fault] = args();
    assert!(fault <= 2);
    let mut cell = map_job_cell(false);
    let mut job = MapJob::decode(cell.get()).unwrap();
    let changed = job.cancel(domain(), target, epoch).unwrap();
    if changed {
        cell.set(job.encode().unwrap());
    }
    if fault == 1 {
        ic0::trap(b"map job cancellation after stable write");
    }
    thaw(u32::from(!changed));
    if fault == 2 {
        ic0::trap(b"map job cancellation after thaw");
    }
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "private job evidence query")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_bytes() {
    ic0::msg_reply_data_append(map_job_cell(false).get());
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "private controller-authorized fault injection")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_damage() {
    controller();
    let [mode, epoch, target] = args();
    freeze();
    let mut cell = map_job_cell(false);
    if mode == 0 {
        assert_eq!([epoch, target], [0, 0]);
        let mut bytes = cell.get().clone();
        bytes[24] ^= 1;
        cell.set(bytes);
    } else {
        assert_eq!(mode, 1);
        let mut maps = read_source();
        maps[1]
            .with(|mut view| view.insert(u64::MAX - 1, 123).unwrap())
            .unwrap();
        cell.set(
            MapJob::initial(domain())
                .unwrap()
                .export(domain(), target, epoch, &maps)
                .unwrap()
                .encode()
                .unwrap(),
        );
    }
    ic0::msg_reply();
}
