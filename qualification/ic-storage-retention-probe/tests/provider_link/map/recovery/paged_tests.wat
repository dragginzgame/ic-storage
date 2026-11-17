;; Closed multi-page source fixtures. Values include full-width keys and deletion
;; so recovery must preserve logical records rather than construction history.
(func $page_source (param $a i64) (param $b i64)
  (call $run_pair (i32.const 11) (i64.const 0) (local.get $a) (i32.const 0))
  (call $run_pair (i32.const 267) (i64.const 0) (local.get $b) (i32.const 0))
  (if (i64.ne (local.get $a) (i64.const 0))
    (then
      (call $run_pair (i32.const 2) (i64.const 0) (i64.const 1000) (i32.const 0))
      (call $run_pair (i32.const 1) (i64.const -1) (i64.const -2) (i32.const 0))))
  (if (i64.ne (local.get $b) (i64.const 0))
    (then
      (call $run_pair (i32.const 258) (i64.const 0) (i64.const 1000) (i32.const 0))
      (call $run_pair (i32.const 257) (i64.const -2) (i64.const -1) (i32.const 0))))
  (call $reply))
(func (export "canister_update pages_empty_a") (call $page_source (i64.const 0) (i64.const 129)))
(func (export "canister_update pages_empty_b") (call $page_source (i64.const 129) (i64.const 0)))
(func (export "canister_update pages_partial") (call $page_source (i64.const 129) (i64.const 65)))
(func (export "canister_update pages_full") (call $page_source (i64.const 192) (i64.const 192)))
(func (export "canister_update pages_excess") (call $page_source (i64.const 193) (i64.const 0)))
;; Grow a real root beyond the former checkpoint boundary, then delete back to
;; the current portable-record limit without shrinking its retained allocation.
(func (export "canister_update pages_tail") (local $key i64)
  (call $run_pair (i32.const 11) (i64.const 0) (i64.const 769) (i32.const 0))
  (call $run_pair (i32.const 267) (i64.const 0) (i64.const 192) (i32.const 0))
  (local.set $key (i64.const 192))
  (loop $trim
    (call $run_pair (i32.const 2) (local.get $key) (i64.add (local.get $key) (i64.const 1000)) (i32.const 0))
    (local.set $key (i64.add (local.get $key) (i64.const 1)))
    (br_if $trim (i64.lt_u (local.get $key) (i64.const 769))))
  (call $reply))
(func (export "canister_query pool_window")
  (call $reply_data (i32.const 131072) (i32.const 196608)) (call $reply))
