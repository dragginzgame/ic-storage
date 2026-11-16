;; V3 logical fields only: u32 counter at 0, u64 ceiling at 8, stride 16.
;; Bytes 4..8 are padding: never interpreted or required to be canonical.
;; This WAT protocol does not establish Rust allocation or pointer validity.
(func $element_header
  __SCHEMA_CHECKS__)

(func $element_init (local $index i32)
  __SCHEMA_WRITES__
  ;; Invalid spare records make accidental scans beyond length observable.
  (memory.fill (i32.const 131072) (i32.const 165) (i32.const 256))
  (loop $slots
    (i64.store (i32.add (i32.const 131080) (i32.mul (local.get $index) (i32.const 16))) (i64.const 0))
    (local.set $index (i32.add (local.get $index) (i32.const 1)))
    (br_if $slots (i32.lt_u (local.get $index) (i32.const 16))))
  (local.set $index (i32.const 0))
  (block $done
    (loop $values
      (br_if $done (i32.ge_u (local.get $index) (i32.const __LENGTH__)))
      (call $write_element
        (i32.add (i32.const 131072) (i32.mul (local.get $index) (i32.const 16)))
        (i64.add (i64.const 17) (i64.extend_i32_u (local.get $index))))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br $values))))

(func $write_element (param $address i32) (param $counter i64)
  (if (i64.gt_u (local.get $counter) (i64.const 1000)) (then unreachable))
  (i32.store (local.get $address) (i32.wrap_i64 (local.get $counter)))
  (i64.store offset=8 (local.get $address) (i64.const 1000)))

;; The caller has checked full-width length/capacity and exclusive custody.
;; Before claim this runs for the provider; settlement runs after clients end.
(func $elements_admit (param $length i64) (local $index i32) (local $address i32)
  (block $done
    (loop $values
      (br_if $done (i32.ge_u (local.get $index) (i32.wrap_i64 (local.get $length))))
      (local.set $address
        (i32.add (i32.const 131072) (i32.mul (local.get $index) (i32.const 16))))
      (if (i64.gt_u (i64.extend_i32_u (i32.load (local.get $address)))
                    (i64.load offset=8 (local.get $address))) (then unreachable))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br $values))))

(func (export "canister_update invalid_element_settlement") (local $base i32)
  (local.set $base (call $claim))
  (i32.store (local.get $base) (i32.const 1))
  (i64.store offset=8 (local.get $base) (i64.const 0))
  (call $settle (i64.const 1) (i64.const 1))
  (call $reply))
