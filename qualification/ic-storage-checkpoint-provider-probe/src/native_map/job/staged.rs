//! Durable cursor owner is MapJob; this adapter validates native prefixes and
//! stages only the next bounded batch. Native construction happens after return.
use super::{args, controller, domain, fresh, ic0, map_job};
use crate::native_map::{map_finish, map_put, read_source};
use ic_storage_native::map::Map;

#[expect(
    unsafe_code,
    reason = "scalar imports bound to non-reentrant WAT callbacks"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn map_stage_start(fault: u32);
    safe fn map_stage_prepare(a: u32, b: u32, complete: u32, fault: u32);
    safe fn map_stage_state();
    safe fn map_stage_damage(mode: u32);
    safe fn stage_ready();
}

// Canonical cursor order: root 1 then root 2, ascending u64 key in each root.
// No native probe position or decoder's allocation history enters progress.
fn ordered(mut maps: [Map; 2]) -> [Vec<(u64, u64)>; 2] {
    std::array::from_fn(|id| {
        let mut values = Vec::new();
        values.try_reserve_exact(maps[id].len().unwrap()).unwrap();
        maps[id]
            .with(|view| view.for_each(|key, value| values.push((key, value))))
            .unwrap();
        values.sort_unstable_by_key(|entry| entry.0);
        values
    })
}
fn counts(values: &[Vec<(u64, u64)>; 2], prefix: u32) -> [usize; 2] {
    let prefix = prefix as usize;
    assert!(prefix <= values[0].len() + values[1].len());
    [
        prefix.min(values[0].len()),
        prefix.saturating_sub(values[0].len()),
    ]
}
fn validate(values: &[Vec<(u64, u64)>; 2], prefix: u32) {
    map_stage_state();
    let counts = counts(values, prefix);
    let mut actual = read_source();
    for id in 0..2 {
        assert_eq!(actual[id].len().unwrap(), counts[id]);
        actual[id]
            .with(|view| {
                for &(key, value) in &values[id][..counts[id]] {
                    assert_eq!(view.get(key), Some(value));
                }
            })
            .unwrap();
    }
}
#[expect(unsafe_code, reason = "private staged-start upgrade entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage() {
    start(false);
}
#[expect(unsafe_code, reason = "private explicit restart upgrade entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_restart() {
    start(true);
}
fn start(restart: bool) {
    let [epoch, source, fault] = args();
    assert!(fault <= 3);
    fresh();
    let mut cell = map_job::open(false);
    let mut job = cell.read();
    let decoded = if restart {
        job.restart_import(domain(), source, epoch)
    } else {
        job.begin_import(domain(), source, epoch)
    }
    .unwrap();
    drop(decoded);
    map_stage_start(fault as u32);
    cell.write(&job);
    if fault == 1 {
        ic0::trap(b"map import start after stable write");
    }
}
#[expect(unsafe_code, reason = "private interrupted-import resumption")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_resume() {
    let [epoch, source, fault] = args();
    assert!(fault <= 1);
    let job = map_job::open(false).read();
    let (maps, prefix) = job.importing(domain(), source, epoch).unwrap();
    validate(&ordered(maps), prefix);
    if fault == 1 {
        ic0::trap(b"map import resume after validation");
    }
}
#[expect(unsafe_code, reason = "private controller-authorized batch entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_batch() {
    controller();
    assert_eq!(ic0::msg_arg_data_size(), 40);
    let mut bytes = [0; 40];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    let [epoch, source, start, limit, fault]: [u64; 5] =
        std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()));
    assert!(fault <= 4);
    let mut cell = map_job::open(false);
    let mut job = cell.read();
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
        // The completed retry acknowledges progress without overwriting newer writes.
        stage_ready();
    } else {
        let values = ordered(batch.checkpoint);
        validate(&values, if batch.retry { batch.end } else { batch.start });
        if !batch.retry {
            let begin = counts(&values, batch.start);
            let end = counts(&values, batch.end);
            cell.write(&job);
            if fault == 1 {
                ic0::trap(b"map batch after stable progress write");
            }
            map_stage_prepare(
                (end[0] - begin[0]) as u32,
                (end[1] - begin[1]) as u32,
                u32::from(batch.complete),
                fault as u32,
            );
            for id in 0..2 {
                for (index, &(key, value)) in values[id][begin[id]..end[id]].iter().enumerate() {
                    map_put(id as u32, index as u32, key, value);
                    if fault == 2 {
                        ic0::trap(b"map batch during scalar staging");
                    }
                }
            }
            map_finish();
        }
    }
    // The wrapper applies the native batch and replies only after Rust returns.
}
#[expect(
    unsafe_code,
    reason = "private controller-authorized import fault fixture"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_stage_damage() {
    controller();
    let [mode, reserved, other] = args();
    assert_eq!([reserved, other], [0, 0]);
    assert!(mode <= 4);
    map_stage_state();
    if mode >= 3 {
        map_job::open(false).damage(mode - 3);
    } else {
        map_stage_damage(mode as u32);
    }
    ic0::msg_reply();
}
