//! Mid-batch refusals must roll back prior mutations, owners and observations.
use super::*;
use std::path::Path;

fn replace(source: &mut String, anchor: &str, replacement: &str) {
    assert_eq!(source.matches(anchor).count(), 1);
    *source = source.replace(anchor, replacement);
}

pub(super) async fn verify(pic: &PocketIc, raw: &[u8], dir: &Path) {
    let mut source = native::source(64, 8, 1);
    replace(
        &mut source,
        "(global $bench_mode",
        "(global $bench_fault (mut i32) (i32.const 0))\n(global $bench_mode",
    );
    let ack = "(if (i32.eq (local.get $field) (i32.const 2)) (then";
    replace(
        &mut source,
        ack,
        &format!(
            r#"
      (if (i32.and (i32.eq (local.get $index) (i32.const 3)) (i32.eq (local.get $field) (i32.const 2))) (then
        (if (i32.eq (global.get $bench_fault) (i32.const 1)) (then unreachable))
        (if (i32.eq (global.get $bench_fault) (i32.const 3)) (then
          (call $access_store_i64 (i32.const 65568) (i64.const 0) (i64.const 0))))))
      {ack}
    "#
        ),
    );
    let key = "(if (i32.eqz (local.get $field)) (then (return (local.get $key))))";
    replace(
        &mut source,
        key,
        &format!(
            r#"
      (if (i32.and (i32.eq (global.get $bench_fault) (i32.const 2)) (i32.eq (local.get $index) (i32.const 3)))
        (then (local.set $key (select (i64.const 13) (i64.const 999) (i32.eqz (global.get $bench_mode))))))
      {key}
    "#
        ),
    );
    let mut methods = String::new();
    for fault in 0..=3 {
        methods.push_str(&format!("(func (export \"canister_update loan_fault_{fault}\") (global.set $bench_fault (i32.const {fault})) (call $reply))\n"));
    }
    for (name, count, mode) in [("count", 65, 0), ("mode", 1, 4)] {
        methods.push_str(&format!("(func (export \"canister_update loan_invalid_{name}\") (global.set $bench_mode (i32.const 0)) (call $run_pair (i32.const 13) (i64.const {count}) (i64.const {mode}) (i32.const 0)) (call $reply))\n"));
    }
    source.insert_str(source.rfind(')').unwrap(), &methods);
    let provider = compile_wat(&source, LIMIT, LIMIT).unwrap();
    let composed = crate::vector_link::compose_map(raw, &provider).unwrap();
    assert!(
        super::super::link::admit_final(&composed, raw, super::super::link::Profile::A).is_err()
    );
    let bytes = meter::instrument(&composed);
    fs::write(dir.join("batch-custody-counterexamples.wasm"), &bytes).unwrap();
    for operation in 1..=4 {
        let id = pic.create_canister().await;
        pic.install_canister(id, bytes.clone(), vec![], None).await;
        pic.update_call(id, id, "bench_setup", args(64, 1))
            .await
            .unwrap();
        if operation >= 3 {
            pic.update_call(id, id, "bench_2", vec![]).await.unwrap();
        }
        if operation == 4 {
            pic.update_call(id, id, "bench_3", vec![]).await.unwrap();
        }
        let before = observe(pic, id).await;
        let before_cost = measurement(pic, id).await;
        let method = format!("bench_{operation}");
        for fault in 1..=3 {
            pic.update_call(id, id, &format!("loan_fault_{fault}"), vec![])
                .await
                .unwrap();
            let error = pic.update_call(id, id, &method, vec![]).await.unwrap_err();
            assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
            assert_eq!(observe(pic, id).await, before);
            assert_eq!(measurement(pic, id).await, before_cost);
        }
        pic.update_call(id, id, "loan_fault_0", vec![])
            .await
            .unwrap();
        for invalid in ["count", "mode"] {
            let error = pic
                .update_call(id, id, &format!("loan_invalid_{invalid}"), vec![])
                .await
                .unwrap_err();
            assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
            assert_eq!(observe(pic, id).await, before);
            assert_eq!(measurement(pic, id).await, before_cost);
        }
        let mut records = before.records(0);
        expected(&mut records, 64, 1, 8, operation);
        pic.update_call(id, id, &method, vec![]).await.unwrap();
        assert_eq!(state(pic, id, true).await.0, records);
        pic.stop_canister(id, None).await.unwrap();
        pic.delete_canister(id, None).await.unwrap();
    }
    fs::write(dir.join("BATCH_CUSTODY"), "four batch modes: late acknowledgement, wrong key, forbidden metadata write, invalid count/mode and clean retry passed\n").unwrap();
}
