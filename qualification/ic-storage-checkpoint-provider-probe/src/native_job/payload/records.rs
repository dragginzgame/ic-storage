//! Concrete V3 logical fields; no native record owner crosses these callbacks.
use super::super::{Job, ic0};
use ic_storage_native_feasibility::record::narrow::V3;
pub(in crate::native_job) type State = ([Option<V3>; 16], usize);
pub(in crate::native_job) const LIMIT: u32 = 16;
#[expect(
    unsafe_code,
    reason = "record field imports bound to reviewed WAT callbacks"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn record_value(index: u32, field: u32) -> u64;
    safe fn begin();
    safe fn record_put(index: u32, counter: u32, ceiling: u64);
    safe fn stage_begin();
    safe fn stage_count() -> u32;
    safe fn stage_value(index: u32, field: u32) -> u64;
    safe fn stage_put(index: u32, counter: u32, ceiling: u64);
}
pub(in crate::native_job) fn read(count: u32, _epoch: u64) -> State {
    assert!(count <= LIMIT);
    let mut state = ([None; 16], count as usize);
    for i in 0..count {
        state.0[i as usize] =
            Some(V3::new(record_value(i, 0).try_into().unwrap(), record_value(i, 1)).unwrap());
    }
    state
}
pub(in crate::native_job) fn seal(
    job: &Job,
    source: u64,
    target: u64,
    epoch: u64,
    state: &State,
) -> Job {
    let values: Vec<_> = state.0[..state.1].iter().map(|r| r.unwrap()).collect();
    job.export(source, target, epoch, &values).unwrap()
}
pub(in crate::native_job) fn len(state: &State) -> u32 {
    state.1 as u32
}
pub(in crate::native_job) fn restore_begin(_state: &State) {
    begin();
}
pub(in crate::native_job) fn restore_put(state: &State, i: u32) {
    let r = state.0[i as usize].unwrap();
    record_put(i, r.counter(), r.ceiling());
}
pub(in crate::native_job) fn frozen(state: &State, count: u32) {
    if len(state) != count {
        ic0::trap(b"frozen job length mismatch");
    }
    for i in 0..count {
        let r = state.0[i as usize].unwrap();
        if record_value(i, 0) != u64::from(r.counter()) || record_value(i, 1) != r.ceiling() {
            ic0::trap(b"frozen job values mismatch");
        }
    }
}
pub(in crate::native_job) fn damage(state: &mut State, mode: u64) {
    assert!(state.1 > 0);
    let r = state.0[0].unwrap();
    match mode {
        1 => state.0[0] = Some(V3::new(r.counter() ^ 1, r.ceiling()).unwrap()),
        2 => state.0[0] = Some(V3::new(r.counter(), r.ceiling() ^ 1).unwrap()),
        3 => {
            state.1 -= 1;
            state.0[state.1] = None;
        }
        _ => unreachable!(),
    }
}
pub(in crate::native_job) fn start(_state: &State) {
    stage_begin();
}
pub(in crate::native_job) fn validate(state: &State, prefix: u32) {
    if stage_count() != prefix || prefix > len(state) {
        ic0::trap(b"import prefix metadata mismatch");
    }
    for i in 0..prefix {
        let r = state.0[i as usize].unwrap();
        if stage_value(i, 0) != u64::from(r.counter()) || stage_value(i, 1) != r.ceiling() {
            ic0::trap(b"import prefix values mismatch");
        }
    }
}
pub(in crate::native_job) fn append(state: &State, i: u32) {
    let r = state.0[i as usize].unwrap();
    stage_put(i, r.counter(), r.ceiling());
}
