;; Format-2 settlement 3 exclusively represents an unpublished construction
;; prefix. Length and capacity both record its durable cursor. Older admission
;; rejects this seal; neither Vec ownership nor the transient restore_count is used.
(func $staged_header
  (call $static_header)
  (call $shape (i64.load (i32.const 65568)) (i64.load (i32.const 65576)))
  (if (i64.ne (i64.load (i32.const 65568)) (i64.load (i32.const 65576))) (then unreachable))
  (if (i64.ne (i64.load (i32.const 65584)) (i64.const 3)) (then unreachable))
  (if (i32.ne (global.get $access_operation) (i32.const -1)) (then unreachable))
  __STAGE_SCHEMA__
)
(func $staged
  (if (i32.ne (global.get $phase) (i32.const 5)) (then unreachable))
  (call $staged_header))
(func (export "__bridge_importing")
  (if (i32.ne (global.get $phase) (i32.const 0)) (then unreachable))
  (call $staged_header)
  (global.set $phase (i32.const 5)))

(func (export "ncp_stage_count") (result i32)
  (call $staged) (i32.wrap_i64 (i64.load (i32.const 65568))))

(func (export "ncp_stage_finish") (param $length i32)
  (call $staged)
  (if (i64.ne (i64.extend_i32_u (local.get $length)) (i64.load (i32.const 65568))) (then unreachable))
  (i64.store (i32.const 65584) (i64.const 0))
  (global.set $phase (i32.const 2))
  (call $settle (i64.extend_i32_u (local.get $length)) (i64.extend_i32_u (local.get $length))))
(func (export "ncp_stage_ready")
  (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
  (if (i32.ne (global.get $access_operation) (i32.const -1)) (then unreachable))
  (call $admit))
