(module
  (@custom "icp:private enhanced-orthogonal-persistence" "")
  (memory (export "memory") 2)
  (func (export "canister_post_upgrade")
    ;; A different ABI must reject the existing record without resetting it.
    (if (i64.ne (i64.load (i32.const 65536)) (i64.const 0x4f50524f42453032))
      (then unreachable))))
