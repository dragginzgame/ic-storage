;; Immutable geometry from this same native provider, not caller input.
(func (export "ncp_domain") (result i64) (i64.const 131072))

;; Cold-instance bootstrap for an explicitly selected frozen-resume target.
;; Validate bytes without entering either Rust runtime or issuing the allocation.
(func (export "__bridge_frozen")
  (if (i32.ne (global.get $phase) (i32.const 0)) (then unreachable))
  (if (i32.ne (global.get $access_operation) (i32.const -1)) (then unreachable))
  (call $static_header)
  (call $shape (i64.load (i32.const 65568)) (i64.load (i32.const 65576)))
  (if (i64.ne (i64.load (i32.const 65584)) (i64.const 2)) (then unreachable))
  (global.set $phase (i32.const 3))
  (call $frozen))

;; Cancellation releases the original frozen custody; it never issues a new
;; allocation, reconstructs a Vec or writes payload/length/capacity. Checkpoint
;; Rust remains live, so this callback and its closure must stay WAT-only.
(func (export "ncp_thaw") (param $retry i32)
  (if (i32.ne (global.get $access_operation) (i32.const -1)) (then unreachable))
  (if (i32.gt_u (local.get $retry) (i32.const 1)) (then unreachable))
  (if (local.get $retry)
    (then
      (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
      (call $admit))
    (else
      (call $frozen)
      (i64.store (i32.const 65584) (i64.const 1))
      (global.set $phase (i32.const 1)))))
