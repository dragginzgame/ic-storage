//! Small valid Wasm counterexamples to the transient-stack proof obligations.

use ic_host_artifacts::{wasm, wasm_text};
use wasmparser::{Parser, Payload};

use super::{Error, Report};

fn source(helper: &str, body: &str, reserve: u32, restore: u32) -> String {
    format!(
        r#"(module
        (type (func))
        (import "provider" "claim" (func (type 0)))
        (memory 3 3)
        (table 1 funcref)
        (global (mut i32) (i32.const 32768))
        (func {helper})
        (func (result i64) (local i32)
            global.get 0 i32.const {reserve} i32.sub local.tee 0 global.set 0
            block
                {body}
                local.get 0 i32.const {restore} i32.add global.set 0
                i64.const 7 return
            end
            unreachable))"#
    )
}

fn inspect(source: &str) -> Result<Report, Error> {
    inspect_at(source, 32768)
}

fn inspect_at(source: &str, stack_top: u32) -> Result<Report, Error> {
    let bytes = wasm_text::compile(
        source,
        wasm_text::CompileLimits {
            source_bytes: 65536,
            module_bytes: 65536,
        },
    )
    .unwrap();
    // These must be valid modules. Otherwise parser rejection would mask the
    // stack/call contract being tested independently of ordinary Wasm validity.
    wasm::validate(&bytes, 65536).unwrap();
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap() {
            bodies.push(body);
        }
    }
    super::inspect(&bodies, 2, 0, stack_top)
}

#[test]
fn balanced_frame_and_acyclic_calls_have_a_bounded_floor() {
    let report = inspect(&source("", "call 0 call 1 call 1", 64, 64)).unwrap();
    assert_eq!(
        report,
        Report {
            frame_bytes: 64,
            lowest_stack_pointer: 32704,
            direct_edges: 2,
        }
    );
    // Local block exits join before the restore/return suffix.
    assert!(inspect(&source("", "block i32.const 1 br_if 0 end", 32, 32)).is_ok());
    assert_eq!(
        inspect_at(&source("", "", 64, 64), 32767),
        Err(Error::Stack)
    );
}

#[test]
fn stack_mutation_mismatch_and_exhaustion_refuse() {
    for (helper, body, reserve, restore) in [
        ("", "", 64, 80),
        ("", "", 0, 0),
        ("", "", 40, 40),
        ("", "", 32768, 32768),
        ("", "i32.const 128 local.set 0", 64, 64),
        ("", "i32.const 128 local.tee 0 local.set 0", 64, 64),
        ("", "i32.const 128 global.set 0", 64, 64),
        ("i32.const 128 global.set 0", "call 1", 64, 64),
        ("", "global.get 0 local.set 0", 64, 64),
    ] {
        assert_eq!(
            inspect(&source(helper, body, reserve, restore)),
            Err(Error::Stack)
        );
    }
}

#[test]
fn early_return_and_function_label_branches_cannot_bypass_restoration() {
    for body in [
        "i64.const 0 return",
        "i64.const 0 br 1",
        "i64.const 0 i32.const 1 br_if 1 local.set 0",
        "i64.const 0 i32.const 0 br_table 1",
    ] {
        // br_if leaves the result behind on its fall-through path. Use a local
        // i64 in that one case so the module remains valid without allowing Drop.
        let candidate = source("", body, 64, 64)
            .replace("(local i32)", "(local i32) (local i64)")
            .replace("br_if 1 local.set 0", "br_if 1 local.set 1");
        assert_eq!(inspect(&candidate), Err(Error::Control));
    }
}

#[test]
fn return_suffix_cannot_gain_an_effect_or_new_control_entry() {
    let original = source("", "", 64, 64);
    for suffix in [
        "block end i64.const 7 return",
        "call 0 i64.const 7 return",
        "i32.const 0 i64.const 7 i64.store i64.const 7 return",
    ] {
        assert_eq!(
            inspect(&original.replace("i64.const 7 return", suffix)),
            Err(Error::Control)
        );
    }
    let fallthrough = original.replace("i64.const 7 return", "");
    assert_eq!(inspect(&fallthrough), Err(Error::Control));
}

#[test]
fn recursion_and_unreviewed_control_operators_refuse() {
    for (helper, body) in [("call 1", "call 1"), ("", "call 2")] {
        assert_eq!(inspect(&source(helper, body, 64, 64)), Err(Error::Cycle));
    }
    for body in [
        "loop end",
        "i32.const 0 call_indirect (type 0)",
        "i32.const 0 memory.grow local.set 0",
    ] {
        assert_eq!(inspect(&source("", body, 64, 64)), Err(Error::Unsupported));
    }
}
