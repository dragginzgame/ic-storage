use super::*;

const VALID: &str = r#"(module
    (import "ic_storage_retained_v1" "admitted" (func $gate (result i32)))
    (memory 640 2048)
    (global (export "__stack_pointer") (mut i32) (i32.const 1048576))
    (global (export "__data_end") i32 (i32.const 37748737))
    (global (export "__heap_base") i32 (i32.const 37748752))
    (func $init (export "canister_init") call $gate drop)
    (func (export "canister_pre_upgrade"))
    (func (export "canister_post_upgrade"))
    (data (i32.const 37748736) "x")
)"#;

#[test]
fn artifact_rejects_unapproved_memory_startup_and_lifecycle() {
    let raw = wat::parse_str(VALID).unwrap();
    let prepared = prepare(&raw).unwrap();
    admit(&prepared, &raw).unwrap();
    for (from, to) in [
        ("640 2048", "641 2048"),
        ("640 2048", "640 4096"),
        ("640 2048", "i64 640 2048"),
        ("640 2048", "640 2048 shared"),
        ("ic_storage_retained_v1", "unknown_runtime"),
        ("admitted", "wrong_gate"),
        ("(result i32)", "(result i64)"),
        ("(i32.const 1048576)", "(i32.const 2097152)"),
        ("(data (i32.const 37748736)", "(data (i32.const 2097152)"),
        ("(i32.const 37748737)", "(i32.const 37748736)"),
        (
            "(export \"canister_pre_upgrade\")",
            "(export \"wrong_hook\")",
        ),
        (
            "(export \"canister_pre_upgrade\"))",
            "(export \"canister_pre_upgrade\") (param i32))",
        ),
        ("(data", "(start $init) (data"),
    ] {
        let changed = VALID.replace(from, to);
        assert_ne!(changed, VALID);
        let raw = wat::parse_str(changed).unwrap();
        assert!(prepare(&raw).is_err(), "admitted mutation: {from} => {to}");
    }
    let duplicate = VALID.replace(
        "(memory",
        "(import \"ic_storage_retained_v1\" \"admitted\" (func (result i32))) (memory",
    );
    assert!(prepare(&wat::parse_str(duplicate).unwrap()).is_err());
}
