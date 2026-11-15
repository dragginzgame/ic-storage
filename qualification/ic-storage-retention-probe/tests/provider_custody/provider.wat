;; Independently authored custody protocol, not a Rust allocation provider.
;; Page 0: diagnostic replies. Page 1: controlled header. Page 2: payload.
;; No Rust, compiler allocations, start function, data segments or memory growth.
(module
  (@custom "icp:private enhanced-orthogonal-persistence" "")
  (import "ic0" "msg_reply_data_append" (func $append (param i32 i32)))
  (import "ic0" "msg_reply" (func $reply))
  (memory (export "memory") 3 3)
  (global $version i64 (i64.const __VERSION__))
  ;; 0=cold, 1=ready, 2=claimed. This transient global resets on upgrade.
  (global $phase (mut i32) (i32.const 0))

  (func $admit
    (if (i32.ne (memory.size) (i32.const 3)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65536)) (i64.const 0x4e50524f56303031))
      (then unreachable))
    (if (i64.ne (i64.load (i32.const 65544)) (i64.const 1)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65552)) (i64.const 131072)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65560)) (i64.const __EXTENT__)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65568)) (i64.const 8)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65576)) (i64.const 1)) (then unreachable)))

  ;; The result is only a Wasm integer in this fixture. No Rust dereference is
  ;; authorized by returning it. Its intended foreign allocation contract is open.
  (func $claim (result i32)
    (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
    (call $admit)
    (i64.store (i32.const 65576) (i64.const 0))
    (global.set $phase (i32.const 2))
    (i32.const 131072))

  ;; Private trusted caller must have ended all payload access before settlement.
  ;; There are no Rust owners in this fixture; this does not enforce their lifetimes.
  (func $settle
    (if (i32.ne (global.get $phase) (i32.const 2)) (then unreachable))
    (i64.store (i32.const 65576) (i64.const 1))
    (global.set $phase (i32.const 1)))

  (func (export "canister_init")
    (i64.store (i32.const 65536) (i64.const 0x4e50524f56303031))
    (i64.store (i32.const 65544) (i64.const 1))
    (i64.store (i32.const 65552) (i64.const 131072))
    (i64.store (i32.const 65560) (i64.const 4096))
    (i64.store (i32.const 65568) (i64.const 8))
    (i64.store (i32.const 65576) (i64.const 1))
    (i64.store (i32.const 131072) (i64.const 17))
    (call $admit)
    (global.set $phase (i32.const 1)))

  (func (export "canister_pre_upgrade")
    ;; Reject an outstanding claim before handing bytes to another instance.
    (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
    (if (i64.ne (i64.load (i32.const 65576)) (i64.const 1)) (then unreachable)))

  (func (export "canister_post_upgrade")
    ;; Admission precedes readiness, claim, or payload access. Never reset unknown bytes.
    (call $admit)
    (global.set $phase (i32.const 1))
    (if (i32.const __TRAP__)
      (then
        (i64.store (call $claim) (i64.const 999))
        unreachable)))

  (func (export "canister_update advance") (local $base i32)
    (local.set $base (call $claim))
    (i64.store (local.get $base)
      (i64.add (i64.load (local.get $base)) (i64.const 1)))
    (call $settle)
    (call $reply))

  (func (export "canister_update duplicate_claim")
    (drop (call $claim))
    (drop (call $claim))
    (call $reply))

  (func (export "canister_update trap_after_write")
    (i64.store (call $claim) (i64.const 999))
    unreachable)

  (func (export "canister_update abandon_claim")
    (drop (call $claim))
    ;; Deliberately commit an unsettled claim. There is no recovery endpoint.
    (call $reply))

  (func (export "canister_update corrupt_extent")
    ;; Deliberate test-only corruption while no claim is outstanding.
    (if (i32.ne (global.get $phase) (i32.const 1)) (then unreachable))
    (i64.store (i32.const 65560) (i64.const 0))
    (call $reply))

  (func (export "canister_query observe")
    ;; Privileged WAT-only diagnostics; never a proposed Rust lease bypass.
    (i64.store (i32.const 0) (global.get $version))
    (i64.store (i32.const 8) (i64.extend_i32_u (global.get $phase)))
    (i64.store (i32.const 16) (i64.load (i32.const 65576)))
    (i64.store (i32.const 24) (i64.load (i32.const 131072)))
    (i64.store (i32.const 32) (i64.load (i32.const 65560)))
    (call $append (i32.const 0) (i32.const 40))
    (call $reply)))
