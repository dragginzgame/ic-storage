//! Shared scalar staging contract; callbacks never enter native Rust.
#[expect(
    unsafe_code,
    reason = "scalar imports bound to non-reentrant WAT callbacks"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    pub(super) safe fn map_stage_start(fault: u32);
    pub(super) safe fn map_stage_prepare(a: u32, b: u32, complete: u32, fault: u32);
    pub(super) safe fn map_stage_state();
    pub(super) safe fn map_stage_damage(mode: u32);
    pub(super) safe fn stage_ready();
    pub(super) safe fn map_stage_commit_state() -> u32;
    pub(super) safe fn map_stage_verified(complete: u32);
}
