//! Durable two-map jobs; native construction runs after this Rust domain returns.
use super::{freeze, fresh, read_source, stage};
use crate::{
    job_control::{args, controller},
    provider::map_job,
};
use ic_platform_runtime::raw as ic0;
use ic_storage_native_feasibility::migration_job::MapJob;

mod staged;

/// The native wrapper admits the roots and restores frozen/private custody
/// before entering this Rust domain. Scalar callbacks never reenter native Rust.
#[expect(unsafe_code, reason = "private admitted native-prefix evidence query")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_native_prefix() {
    use ic_storage_native_feasibility::migration_job::paged_continuation::NativePrefix;
    for id in 0..2 {
        let count = super::map_count(id);
        assert!(count <= super::maps::LIMIT as u32);
        let prefix = NativePrefix::from_records(
            count as usize,
            (0..count).map(|index| {
                let key = super::map_word(id, index, 0);
                let value = super::map_word(id, index, 1);
                (key, value)
            }),
        )
        .unwrap();
        ic0::msg_reply_data_append(&(prefix.len() as u64).to_le_bytes());
        ic0::msg_reply_data_append(&prefix.fingerprint());
    }
    ic0::msg_reply();
}

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
    map_job::open(true).write(&MapJob::initial(domain()).unwrap());
}
#[expect(unsafe_code, reason = "private controller-authorized export")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    controller();
    let [epoch, target, fault] = args();
    assert!(fault <= 2);
    freeze();
    let mut cell = map_job::open(false);
    let job = cell.read();
    let next = job
        .export(domain(), target, epoch, &read_source(super::maps::LIMIT))
        .unwrap();
    cell.write_with_fault(&next, fault == 2);
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
    let mut cell = map_job::open(false);
    let mut job = cell.read();
    let decoded = job.consume(domain(), source, epoch).unwrap();
    // This write and subsequent WAT/native construction share one IC upgrade.
    // Traps after Rust returns must roll this consumption back as well.
    cell.write(&job);
    if fault == 1 {
        ic0::trap(b"map job recovery after consumed write");
    }
    stage(&decoded, fault);
}
#[expect(unsafe_code, reason = "private validated frozen-source resumption")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_resume() {
    let [epoch, target, fault] = args();
    assert!(fault <= 1);
    freeze();
    let job = map_job::open(false).read();
    job.frozen(domain(), target, epoch).unwrap();
    // Export's exact retry compares both payloads, including their root binding.
    // Read in the same stable native iteration order used by the original export.
    assert_eq!(
        job.export(domain(), target, epoch, &read_source(super::maps::LIMIT))
            .unwrap(),
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
    let mut cell = map_job::open(false);
    let mut job = cell.read();
    let changed = job.cancel(domain(), target, epoch).unwrap();
    if changed {
        cell.write(&job);
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
    ic0::msg_reply_data_append(&map_job::open(false).read().encode().unwrap());
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "private controller-authorized fault injection")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_damage() {
    controller();
    let [mode, epoch, target] = args();
    freeze();
    let mut cell = map_job::open(false);
    if mode == 0 || mode == 2 {
        assert_eq!([epoch, target], [0, 0]);
        cell.damage(u64::from(mode == 2));
    } else {
        assert_eq!(mode, 1);
        let mut maps = read_source(super::maps::LIMIT);
        maps[1]
            .with(|mut view| view.insert(u64::MAX - 1, 123).unwrap())
            .unwrap();
        cell.write(
            &MapJob::initial(domain())
                .unwrap()
                .export(domain(), target, epoch, &maps)
                .unwrap(),
        );
    }
    ic0::msg_reply();
}
