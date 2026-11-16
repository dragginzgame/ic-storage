;; Private access helpers for the reviewed 80-byte peak vector stack use.
;; The trusted entrypoint selects the operation before entering Rust, then resets
;; it after return. These helpers never call Rust or expose provider metadata.
(global $access_operation (mut i32) (i32.const -1))

(func $access_address (param $address i32) (param $offset i64)
    (param $bytes i64) (param $write i32) (result i32)
  (local $start i64) (local $end i64) (local $words i64)
  (local.set $start (i64.add (i64.extend_i32_u (local.get $address)) (local.get $offset)))
  (if (i64.lt_u (local.get $start) (local.get $offset)) (then unreachable))
  (local.set $end (i64.add (local.get $start) (local.get $bytes)))
  (if (i64.lt_u (local.get $end) (local.get $start)) (then unreachable))
  ;; Stack scratch is independent of the provider claim; globals/locals live
  ;; outside linear memory. Returning an address never touches that memory.
  (if (i32.and (i64.ge_u (local.get $start) (i64.const 32688))
               (i64.le_u (local.get $end) (i64.const 32768)))
    (then (return (i32.wrap_i64 (local.get $start)))))
  (if (i32.ne (global.get $phase) (i32.const 2)) (then unreachable))
  (if (i64.ne (i64.load (i32.const 65584)) (i64.const 0)) (then unreachable))
  (if (i32.gt_u (global.get $access_operation) (i32.const 7)) (then unreachable))
  (local.set $words (i64.load (i32.const 65568)))
  ;; Defensive shape check precedes length arithmetic, even under a claim.
  (call $shape (local.get $words) (i64.load (i32.const 65576)))
  (if (i32.and (i32.eqz (local.get $write))
               (i32.and (i32.ge_u (global.get $access_operation) (i32.const 3))
                        (i32.le_u (global.get $access_operation) (i32.const 5))))
    (then (local.set $words (i64.const 0))))
  (if (local.get $write)
    (then
      ;; The business-error operation may append once or refuse at full capacity
      ;; after mutating an existing scalar. It never reads the appended word.
      (if (i32.and (i32.eq (global.get $access_operation) (i32.const 6))
                   (i64.lt_u (local.get $words) (i64.const 512)))
        (then (local.set $words (i64.add (local.get $words) (i64.const 1)))))
      (if (i32.eq (global.get $access_operation) (i32.const 1))
        (then
          (if (i64.eq (local.get $words) (i64.const 512)) (then unreachable))
          (local.set $words (i64.add (local.get $words) (i64.const 1)))))
      (if (i32.or (i32.eq (global.get $access_operation) (i32.const 3))
                  (i32.eq (global.get $access_operation) (i32.const 4)))
        (then (local.set $words (i64.const 0))))
      (if (i32.eq (global.get $access_operation) (i32.const 5))
        (then (local.set $words (i64.const 1))))))
  (if (i64.lt_u (local.get $start) (i64.const 131072)) (then unreachable))
  (if (i64.gt_u (local.get $end)
        (i64.add (i64.const 131072) (i64.mul (local.get $words) (i64.const 8))))
    (then unreachable))
  (i32.wrap_i64 (local.get $start)))

(func $access_load_i32 (param $address i32) (param $offset i64) (result i32)
  (i32.load (call $access_address (local.get $address) (local.get $offset) (i64.const 4) (i32.const 0))))
(func $access_load_u8 (param $address i32) (param $offset i64) (result i32)
  (i32.load8_u (call $access_address (local.get $address) (local.get $offset) (i64.const 1) (i32.const 0))))
(func $access_load_i64 (param $address i32) (param $offset i64) (result i64)
  (i64.load (call $access_address (local.get $address) (local.get $offset) (i64.const 8) (i32.const 0))))
(func $access_store_i32 (param $address i32) (param $value i32) (param $offset i64)
  (i32.store (call $access_address (local.get $address) (local.get $offset) (i64.const 4) (i32.const 1)) (local.get $value)))
(func $access_store_u8 (param $address i32) (param $value i32) (param $offset i64)
  (i32.store8 (call $access_address (local.get $address) (local.get $offset) (i64.const 1) (i32.const 1)) (local.get $value)))
(func $access_store_i64 (param $address i32) (param $value i64) (param $offset i64)
  (i64.store (call $access_address (local.get $address) (local.get $offset) (i64.const 8) (i32.const 1)) (local.get $value)))

(func $access_copy (param $destination i32) (param $source i32) (param $bytes i32)
  (if (i32.ne (global.get $access_operation) (i32.const 2)) (then unreachable))
  ;; The only compiled copy is removal within the payload. Do not permit copying
  ;; stack bytes into the retained vector, even when each separate range is valid.
  (if (i32.or (i32.lt_u (local.get $destination) (i32.const 131072))
               (i32.lt_u (local.get $source) (i32.const 131072))) (then unreachable))
  (drop (call $access_address (local.get $source) (i64.const 0)
    (i64.extend_i32_u (local.get $bytes)) (i32.const 0)))
  (drop (call $access_address (local.get $destination) (i64.const 0)
    (i64.extend_i32_u (local.get $bytes)) (i32.const 1)))
  (memory.copy (local.get $destination) (local.get $source) (local.get $bytes)))
