;; Runs inside staged apply, after checkpoint Rust returns and before any new
;; record is written. The wrapper has temporarily activated private roots; no
;; application endpoint or live Rust owner can observe this intermediate state.
(func $reserve_page (param $id i32) (param $additional i32)
  (local $root i64) (local $capacity i64) (local $fault i32)
  (if (i32.ne (global.get $pending) (i32.const 2)) (then unreachable))
  (if (i32.ne (global.get $committing) (i32.const 0)) (then unreachable))
  (if (i32.or (i32.gt_u (local.get $id) (i32.const 1)) (i32.gt_u (local.get $additional) (i32.const 64))) (then unreachable))
  (if (i32.eqz (local.get $additional)) (then (return)))
  ;; The batch boundary already admitted both roots. Skip reservation when the
  ;; admitted table has room; apply still freshly admits both actual owners.
  (local.set $root (i64.load (i32.add (i32.const 65568) (i32.mul (local.get $id) (i32.const 8)))))
  (local.set $capacity (call $buckets (local.get $root)))
  (if (i64.le_u (i64.add (call $count (local.get $root)) (i64.extend_i32_u (local.get $additional)))
    (i64.sub (local.get $capacity) (i64.shr_u (local.get $capacity) (i64.const 2))))
    (then (return)))
  (if (i32.ge_u (global.get $recovery_fault) (i32.const 11))
    (then (local.set $fault (i32.sub (global.get $recovery_fault) (i32.const 10)))))
  (loop $reserve
    (call $run_pair (i32.or (i32.const 12) (i32.shl (local.get $id) (i32.const 8)))
      (i64.extend_i32_u (local.get $additional)) (i64.const 0) (i32.const 0))
    ;; Both native owners have ended. Capacity tells whether reserve succeeded;
    ;; allocation refusal leaves the roots and all old records unchanged.
    (local.set $root (i64.load (i32.add (i32.const 65568) (i32.mul (local.get $id) (i32.const 8)))))
    (local.set $capacity (call $buckets (local.get $root)))
    (if (i64.le_u (i64.add (call $count (local.get $root)) (i64.extend_i32_u (local.get $additional)))
      (i64.sub (local.get $capacity) (i64.shr_u (local.get $capacity) (i64.const 2))))
      (then (return)))
    ;; Reuse the provider's bounded idle growth and its four rollback points.
    ;; A maximum-capacity or contiguous-space refusal traps this entire batch.
    (if (i32.ne (call $expand_pool (local.get $fault)) (i32.const 1)) (then unreachable))
    (br $reserve)))
