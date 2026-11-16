;; Job-specific native format. The stable locator never inherits a bare checkpoint.
(func (export "ncp_domain") (result i64) (i64.const 131072))
(func (export "__bridge_frozen") (call $boundary) (call $frozen))
(func (export "__bridge_cancel") (call $boundary))
(func (export "ncp_thaw") (param $retry i32)
  (call $static_header)
  (if (i32.eqz (local.get $retry))
    (then
      (call $frozen)
      (i64.store (i32.const 65584) (i64.const 1))
      (global.set $phase (i32.const 1)))
    (else
      (if (i32.ne (local.get $retry) (i32.const 1)) (then unreachable))
      (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
      (if (i64.ne (i64.load (i32.const 65584)) (i64.const 1)) (then unreachable)))))
