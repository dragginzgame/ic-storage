(module
  (@custom "icp:private enhanced-orthogonal-persistence" "")
  (memory (export "memory") 2)
  (func (export "canister_post_upgrade")
    ;; Deliberately write before failing to distinguish rollback from rejection
    ;; before execution. The previous module's bytes must be restored.
    (i64.store (i32.const 65536) (i64.const 0))
    (i64.store (i32.const 65544) (i64.const 999))
    unreachable))
