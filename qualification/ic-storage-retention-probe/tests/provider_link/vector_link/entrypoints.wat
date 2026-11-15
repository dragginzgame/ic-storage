;; Trusted diagnostic glue, outside LLVM. No caller supplies a native descriptor.
(func $vector_run (param $operation i32) (param $value i64) (local $root i64)
  (if (i32.ne (global.get $access_operation) (i32.const -1)) (then unreachable))
  (call $admit)
  (global.set $access_operation (local.get $operation))
  ;; Admission checks the full u64 words before either narrowing. The raw Rust
  ;; entry claims the same header synchronously; no host callback intervenes.
  (local.set $root (call $native_step
    (i32.wrap_i64 (i64.load (i32.const 65568)))
    (i32.wrap_i64 (i64.load (i32.const 65576)))
    (local.get $operation) (local.get $value)))
  ;; Only scalar descriptor bits survive the Rust return. No cleanup callback
  ;; or guard destructor is relied on to return provider custody.
  (call $settle
    (i64.shr_u (local.get $root) (i64.const 32))
    (i64.and (local.get $root) (i64.const 0xffffffff)))
  (global.set $access_operation (i32.const -1)))

(func (export "canister_pre_upgrade")
  (if (i32.ne (global.get $access_operation) (i32.const -1)) (then unreachable))
  (call $provider_pre))
(func (export "canister_post_upgrade")
  (call $provider_post)
  (call $vector_run (i32.const 0) (i64.const 0))
  __POST_FAILURE__)

(func (export "canister_update vector_mutate")
  (call $vector_run (i32.const 0) (global.get $version)) (call $reply))
(func (export "canister_update vector_append")
  (call $vector_run (i32.const 1) (global.get $version)) (call $reply))
(func (export "canister_update vector_remove")
  (call $vector_run (i32.const 2) (i64.const 0)) (call $reply))
(func (export "canister_update vector_shrink")
  (call $vector_run (i32.const 3) (i64.const 0)) (call $reply))
(func (export "canister_update vector_clear")
  (call $vector_run (i32.const 4) (i64.const 0)) (call $reply))
(func (export "canister_update vector_reuse")
  (call $vector_run (i32.const 5) (global.get $version)) (call $reply))

;; A failed second operation must roll back the preceding settled mutation in
;; the same IC transaction. Both paths use the admitted native entrypoint.
(func (export "canister_update vector_trap")
  (call $vector_run (i32.const 1) (i64.const 999)) unreachable)
(func (export "canister_update vector_mutate_append")
  (call $vector_run (i32.const 0) (i64.const 999))
  (call $vector_run (i32.const 1) (i64.const 999)) (call $reply))
