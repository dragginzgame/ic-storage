//! Concrete scalar/u64 field callbacks for the shared job adapter.
use super::super::{Job, ic0};
use ic_storage_native_feasibility::checkpoint::Checkpoint;
pub(in crate::native_job) type State = Checkpoint;
pub(in crate::native_job) const LIMIT: u32 = 512;
#[expect(unsafe_code, reason = "scalar imports bound to reviewed WAT callbacks")]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn counter() -> u64;
    safe fn value(index: u32) -> u64;
    safe fn begin(counter: u64);
    safe fn put(index: u32, value: u64);
    safe fn stage_begin(counter: u64);
    safe fn stage_count() -> u32;
    safe fn stage_counter() -> u64;
    safe fn stage_value(index: u32) -> u64;
    safe fn stage_put(index: u32, value: u64);
}
pub(in crate::native_job) fn read(count: u32, epoch: u64) -> State {
    assert!(count <= LIMIT);
    State {
        source_epoch: epoch,
        counter: counter(),
        values: (0..count).map(|i| value(i)).collect(),
    }
}
pub(in crate::native_job) fn seal(
    job: &Job,
    source: u64,
    target: u64,
    epoch: u64,
    state: &State,
) -> Job {
    job.export(source, target, epoch, state.counter, &state.values)
        .unwrap()
}
pub(in crate::native_job) fn len(state: &State) -> u32 {
    state.values.len() as u32
}
pub(in crate::native_job) fn restore_begin(state: &State) {
    begin(state.counter);
}
pub(in crate::native_job) fn restore_put(state: &State, i: u32) {
    put(i, state.values[i as usize]);
}
pub(in crate::native_job) fn frozen(state: &State, count: u32) {
    if len(state) != count {
        ic0::trap(b"frozen job length mismatch");
    }
    if state.counter != counter() {
        ic0::trap(b"frozen job counter mismatch");
    }
    for (i, expected) in state.values.iter().enumerate() {
        if value(i as u32) != *expected {
            ic0::trap(b"frozen job values mismatch");
        }
    }
}
pub(in crate::native_job) fn damage(state: &mut State, mode: u64) {
    match mode {
        1 => state.counter ^= 1,
        2 => state.values[0] ^= 1,
        3 => {
            state.values.pop().unwrap();
        }
        _ => unreachable!(),
    }
}
pub(in crate::native_job) fn start(state: &State) {
    stage_begin(state.counter);
}
pub(in crate::native_job) fn validate(state: &State, prefix: u32) {
    if stage_count() != prefix || state.counter != stage_counter() {
        ic0::trap(b"import prefix metadata mismatch");
    }
    for (i, expected) in state.values[..prefix as usize].iter().enumerate() {
        if stage_value(i as u32) != *expected {
            ic0::trap(b"import prefix values mismatch");
        }
    }
}
pub(in crate::native_job) fn append(state: &State, i: u32) {
    stage_put(i, state.values[i as usize]);
}
