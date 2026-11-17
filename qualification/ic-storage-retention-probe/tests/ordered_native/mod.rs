//! Closed ordered-node artifact admission and installed lifecycle qualification.
#![forbid(unsafe_code)]
use ic_storage_retention_probe::artifact::{sha256, sha256_hex, validate_wasm};
use ic_storage_retention_probe::startup::{admit_ordered, prepare_ordered};
use std::{fs, path::PathBuf};
use wasmparser::{DataKind, Operator, Parser, Payload};

mod installed_tests;

fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("ORDERED_NATIVE_INPUTS").expect("make ordered-native-test"))
}

fn candidate(name: &str) -> Vec<u8> {
    let path = directory();
    let raw = ic_host_artifacts::read(
        fs::File::open(path.join(format!("{name}.wasm"))).unwrap(),
        ic_host_artifacts::Length::AtMost(1024 * 1024),
    )
    .unwrap()
    .0;
    let artifact = prepare_ordered(&raw).unwrap();
    let bytes = artifact.bytes;
    admit_ordered(&bytes, &raw).unwrap();
    fs::write(path.join(format!("{name}-startup.txt")), artifact.report).unwrap();
    validate_wasm(&bytes, 1024 * 1024).unwrap();
    fs::write(path.join(format!("{name}-installed.wasm")), &bytes).unwrap();
    println!(
        "ordered {name} bytes={} sha256={}",
        bytes.len(),
        sha256_hex(&bytes)
    );
    bytes
}

#[test]
fn artifact_admission() {
    for name in ["a", "b", "recovery", "no-close", "pre-fail"] {
        let bytes = candidate(name);
        check_startup(&bytes);
        assert!(prepare_ordered(&bytes[..bytes.len() / 2]).is_err());
        assert!(prepare_ordered(&bytes).is_err()); // Startup cannot be stacked twice.
        let raw = fs::read(directory().join(format!("{name}.wasm"))).unwrap();
        let mut tampered = bytes;
        *tampered.last_mut().unwrap() ^= 1; // Still valid custom-section data.
        validate_wasm(&tampered, 1024 * 1024).unwrap();
        assert_eq!(admit_ordered(&tampered, &raw), Err("final mismatch"));
    }
    // A valid foreign module cannot acquire the fixture's retained-window authority.
    let wrong =
        wat::parse_str("(module (memory 128 128) (func (export \"canister_init\")))").unwrap();
    assert!(prepare_ordered(&wrong).is_err());
    fs::write(
        directory().join("ADMITTED"),
        "closed ordered fixture geometry and imports\n",
    )
    .unwrap();
}

fn check_startup(bytes: &[u8]) {
    let mut start = None;
    let mut function = 0;
    let mut checked = false;
    let mut segments = 0;
    let mut initialized = 0;
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::ImportSection(reader) => function = reader.into_imports().count() as u32,
            Payload::StartSection { func, .. } => start = Some(func),
            Payload::CodeSectionEntry(body) => {
                if start == Some(function) {
                    let mut ops = body.get_operators_reader().unwrap();
                    for expected in [0, 0, 1_572_864] {
                        assert!(
                            matches!(ops.read().unwrap(), Operator::I32Const { value } if value == expected)
                        );
                    }
                    assert!(matches!(
                        ops.read().unwrap(),
                        Operator::MemoryFill { mem: 0 }
                    ));
                    loop {
                        let offset = match ops.read().unwrap() {
                            Operator::End => break,
                            Operator::I32Const { value } => value,
                            _ => panic!("unexpected startup operation"),
                        };
                        assert!((1_048_576..1_572_864).contains(&offset));
                        assert!(matches!(
                            ops.read().unwrap(),
                            Operator::I32Const { value: 0 }
                        ));
                        let Operator::I32Const { value: length } = ops.read().unwrap() else {
                            panic!("segment length");
                        };
                        assert!(length >= 0 && offset.checked_add(length).unwrap() <= 1_572_864);
                        assert!(
                            matches!(ops.read().unwrap(), Operator::MemoryInit { mem: 0, data_index } if data_index == initialized)
                        );
                        initialized += 1;
                    }
                    assert!(ops.eof());
                    checked = true;
                }
                function += 1;
            }
            Payload::DataSection(reader) => {
                for segment in reader {
                    assert!(matches!(segment.unwrap().kind, DataKind::Passive));
                    segments += 1;
                }
            }
            _ => {}
        }
    }
    assert!(checked && segments > 0);
    assert_eq!(segments, initialized);
}

#[test]
fn artifact_geometry_refusals() {
    for (source, expected) in [
        ("(module (memory 128 129))", "memory geometry"),
        (
            "(module (memory 128 128) (data (i32.const 1572864) \"overlap\"))",
            "retained initialization overlap",
        ),
        (
            "(module (global (export \"__heap_base\") i32 (i32.const 2097152)))",
            "transient compiler geometry",
        ),
        (
            "(module (import \"ic0\" \"call_new\" (func)))",
            "unapproved import",
        ),
        (
            "(module (memory 128 128) (func (drop (memory.grow (i32.const 1)))))",
            "unexpected memory growth",
        ),
        ("(module (func $init) (start $init))", "unexpected start"),
        (
            "(module (memory 128 128) (data (i32.add (i32.const 1048576) (i32.const 524288)) \"overlap\"))",
            "data expression",
        ),
        (
            "(module (memory 128 128) (data (i32.const 1048576) \"value\") (func (data.drop 0)))",
            "compiler-owned data lifecycle",
        ),
    ] {
        let bytes = wat::parse_str(source).unwrap();
        assert_eq!(prepare_ordered(&bytes).err(), Some(expected));
    }
}
