//! Workload wrapper over the unchanged closed native-map executable and guards.
use super::{
    super::{super::vector_link, link},
    LIMIT,
};
use ic_storage_retention_probe::artifact::compile_wat;

pub(super) fn prepare(raw: &[u8], rows: u64, batch: u64, seed: u64) -> Vec<u8> {
    vector_link::compose_map(
        raw,
        &compile_wat(&source(rows, batch, seed), LIMIT, LIMIT).unwrap(),
    )
    .unwrap()
}

pub(super) fn source(rows: u64, batch: u64, seed: u64) -> String {
    let mut source = link::source(link::Profile::A).unwrap();
    source = source.replace(
        "(func $staged_word (param i32 i32) (result i64) unreachable)",
        &format!(r#"
        (global $bench_mode (mut i32) (i32.const -1))
        (global $bench_index (mut i32) (i32.const 0))
        (global $bench_field (mut i32) (i32.const 0))
        (func $staged_word (param $index i32) (param $field i32) (result i64)
          (local $key i64)
          (if (i32.ne (global.get $phase) (i32.const 2)) (then unreachable))
          (if (i64.ne (i64.load (i32.const 65584)) (i64.const 0)) (then unreachable))
          (if (i32.ne (global.get $operation) (i32.const 13)) (then unreachable))
          (if (i32.gt_u (global.get $bench_mode) (i32.const 3)) (then unreachable))
          (if (i32.or (i32.ne (local.get $index) (global.get $bench_index)) (i32.ge_u (local.get $index) (i32.const {batch}))) (then unreachable))
          (if (i32.ne (local.get $field) (global.get $bench_field)) (then unreachable))
          (if (i32.eq (local.get $field) (i32.const 2)) (then
            (global.set $bench_index (i32.add (global.get $bench_index) (i32.const 1)))
            (global.set $bench_field (i32.const 0)) (return (i64.const 0))))
          (global.set $bench_field (i32.add (local.get $field) (i32.const 1)))
          (local.set $key (i64.rem_u (i64.add (i64.mul (i64.extend_i32_u (local.get $index)) (i64.const 17)) (i64.const {})) (i64.const {rows})))
          (if (i32.eqz (local.get $field)) (then (return (local.get $key))))
          (i64.add (local.get $key) (select (i64.const 2000) (i64.const 1000)
            (i32.ge_u (global.get $bench_mode) (i32.const 2)))))
        "#, seed * 13),
    );
    let end = source.rfind(')').unwrap();
    source.truncate(end);
    source.push_str("(func (export \"canister_update bench_admit\") (call $run_pair (i32.const 0) (i64.const 0) (i64.const 0) (i32.const 0)) (call $reply))\n");
    source.push_str(&format!(
        "(func (export \"canister_update bench_setup\")
          (drop (call $expand_pool (i32.const 0)))
          (drop (call $expand_pool (i32.const 0)))
          (drop (call $expand_pool (i32.const 0)))
          (call $run_pair (i32.const 11) (i64.const 0) (i64.const {rows}) (i32.const 0))
          (call $reply))\n"
    ));
    for operation in 0..6 {
        source.push_str(&format!(
            "(func (export \"canister_update bench_{operation}\") (local $i i64) (local $key i64)\n"
        ));
        if operation == 5 {
            // Existing command 11 inserts the complete range in one native scope.
            source.push_str(&format!(
                "(call $run_pair (i32.const 11) (i64.const {rows}) (i64.const {}) (i32.const 0))",
                rows + batch
            ));
        } else if operation == 0 {
            source.push_str(&format!("(loop $next
              (local.set $key (i64.rem_u (i64.add (i64.mul (local.get $i) (i64.const 17)) (i64.const {})) (i64.const {rows})))", seed * 13));
            source.push_str("(drop (local.get $key))");
            source.push_str(&format!("(local.set $i (i64.add (local.get $i) (i64.const 1))) (br_if $next (i64.lt_u (local.get $i) (i64.const {batch}))))"));
        } else {
            let mode = operation % 4;
            source.push_str(&format!(r#"
              (global.set $bench_mode (i32.const {mode}))
              (global.set $bench_index (i32.const 0)) (global.set $bench_field (i32.const 0))
              (call $run_pair (i32.const 13) (i64.const {batch}) (i64.const {mode}) (i32.const 0))
              (if (i32.or (i32.ne (global.get $bench_index) (i32.const {batch})) (global.get $bench_field)) (then unreachable))
              (global.set $bench_mode (i32.const -1))
            "#));
        }
        source.push_str("(call $reply))\n");
    }
    source.push(')');
    source
}
