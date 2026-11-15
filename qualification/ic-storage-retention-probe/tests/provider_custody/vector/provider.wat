;; Independently authored vector-metadata custody protocol. No Rust executes.
;; Header: magic, format, base, extent, initialized length, capacity, settlement.
(module
  (@custom "icp:private enhanced-orthogonal-persistence" "")
  (import "ic0" "msg_reply_data_append" (func $reply_data (param i32 i32)))
  (import "ic0" "msg_reply" (func $reply))
  (memory (export "memory") 3 3)
  (global $version i64 (i64.const __VERSION__))
  (global $phase (mut i32) (i32.const 0))

  (func $shape (param $length i64) (param $capacity i64)
    ;; Check full-width unsigned words before any i32 narrowing or multiplication.
    (if (i64.gt_u (local.get $length) (local.get $capacity)) (then unreachable))
    (if (i64.gt_u (local.get $capacity) (i64.const 512)) (then unreachable)))

  (func $static_header
    (if (i32.ne (memory.size) (i32.const 3)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65536)) (i64.const 0x4e5650524f563031))
      (then unreachable))
    (if (i64.ne (i64.load (i32.const 65544)) (i64.const 1)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65552)) (i64.const 131072)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65560)) (i64.const __EXTENT__)) (then unreachable)))

  (func $admit
    (call $static_header)
    (if (i64.ne (i64.load (i32.const 65584)) (i64.const 1)) (then unreachable))
    (call $shape (i64.load (i32.const 65568)) (i64.load (i32.const 65576))))

  (func $claim (result i32)
    (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
    (call $admit)
    (i64.store (i32.const 65584) (i64.const 0))
    (global.set $phase (i32.const 2))
    (i32.const 131072))

  ;; Trusted caller has ended all payload access and supplies the new initialized
  ;; prefix. Numeric shape alone cannot prove initialization or Rust owner release.
  (func $settle (param $length i64) (param $capacity i64)
    (if (i32.ne (global.get $phase) (i32.const 2)) (then unreachable))
    (call $static_header)
    (if (i64.ne (i64.load (i32.const 65584)) (i64.const 0)) (then unreachable))
    (call $shape (local.get $length) (local.get $capacity))
    (i64.store (i32.const 65568) (local.get $length))
    (i64.store (i32.const 65576) (local.get $capacity))
    (i64.store (i32.const 65584) (i64.const 1))
    (global.set $phase (i32.const 1)))

  (func $append (local $base i32) (local $length i64) (local $capacity i64)
    (local.set $base (call $claim))
    (local.set $length (i64.load (i32.const 65568)))
    (local.set $capacity (i64.load (i32.const 65576)))
    (if (i64.eq (local.get $length) (i64.const 512)) (then unreachable))
    (i64.store
      (i32.add (local.get $base) (i32.mul (i32.wrap_i64 (local.get $length)) (i32.const 8)))
      (i64.add (i64.const 100) (global.get $version)))
    (local.set $length (i64.add (local.get $length) (i64.const 1)))
    (if (i64.gt_u (local.get $length) (local.get $capacity))
      (then (local.set $capacity (local.get $length))))
    (call $settle (local.get $length) (local.get $capacity)))

  (func (export "canister_init")
    (memory.fill (i32.const 131072) (i32.const 0) (i32.const 4096))
    (i64.store (i32.const 131072) (i64.const 17))
    (i64.store (i32.const 131080) (i64.const 23))
    (i64.store (i32.const 131088) (i64.const 41))
    (i64.store (i32.const 65536) (i64.const 0x4e5650524f563031))
    (i64.store (i32.const 65544) (i64.const 1))
    (i64.store (i32.const 65552) (i64.const 131072))
    (i64.store (i32.const 65560) (i64.const 4096))
    (i64.store (i32.const 65568) (i64.const __LENGTH__))
    (i64.store (i32.const 65576) (i64.const __CAPACITY__))
    (i64.store (i32.const 65584) (i64.const 1))
    (call $admit)
    (global.set $phase (i32.const 1)))

  (func (export "canister_pre_upgrade")
    (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
    (call $admit))

  (func (export "canister_post_upgrade")
    (call $admit)
    (global.set $phase (i32.const 1))
    (if (i32.const __TRAP__) (then (call $append) unreachable)))

  (func (export "canister_update append") (call $append) (call $reply))

  (func (export "canister_update trim") (local $length i64)
    (drop (call $claim))
    (local.set $length (i64.load (i32.const 65568)))
    (if (i64.gt_u (local.get $length) (i64.const 1))
      (then (local.set $length (i64.const 1))))
    (call $settle (local.get $length) (local.get $length))
    (call $reply))

  (func (export "canister_update clear")
    (drop (call $claim)) (call $settle (i64.const 0) (i64.const 0)) (call $reply))

  (func (export "canister_update remove_first") (local $base i32) (local $length i64)
    (local.set $base (call $claim))
    (local.set $length (i64.load (i32.const 65568)))
    (if (i64.ne (local.get $length) (i64.const 0))
      (then
        (local.set $length (i64.sub (local.get $length) (i64.const 1)))
        (memory.copy (local.get $base) (i32.add (local.get $base) (i32.const 8))
          (i32.mul (i32.wrap_i64 (local.get $length)) (i32.const 8)))))
    (call $settle (local.get $length) (i64.load (i32.const 65576)))
    (call $reply))

  (func (export "canister_update duplicate_claim")
    (drop (call $claim)) (drop (call $claim)) (call $reply))

  (func (export "canister_update settle_without_claim")
    (call $settle (i64.const 0) (i64.const 0)) (call $reply))

  (func (export "canister_update invalid_settlement")
    (i64.store (call $claim) (i64.const 999))
    (call $settle (i64.const 3) (i64.const 2)) (call $reply))

  (func (export "canister_update trap_after_settlement") (call $append) unreachable)

  (func (export "canister_update abandon_claim") (drop (call $claim)) (call $reply))

  (func (export "canister_update corrupt")
    (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
    __FAULT__
    (call $reply))

  (func (export "canister_query observe")
    ;; Privileged byte diagnostics, including spare capacity; never a Rust lease.
    (i64.store (i32.const 0) (global.get $version))
    (i64.store (i32.const 8) (i64.extend_i32_u (global.get $phase)))
    (call $reply_data (i32.const 0) (i32.const 16))
    (call $reply_data (i32.const 65536) (i32.const 56))
    (call $reply_data (i32.const 131072) (i32.const 4096))
    (call $reply)))
