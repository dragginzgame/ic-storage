//! Small valid Wasm counterexamples to the transient-stack proof obligations.

use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};
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
    let bytes = compile_wat(source, 65536, 65536).unwrap();
    // These must be valid modules. Otherwise parser rejection would mask the
    // stack/call contract being tested independently of ordinary Wasm validity.
    validate_wasm(&bytes, 65536).unwrap();
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap() {
            bodies.push(body);
        }
    }
    super::inspect(&bodies, 2, 0, stack_top)
}

fn inspect_map(source: &str) -> Result<Report, Error> {
    let bytes = compile_wat(source, 65536, 65536).unwrap();
    validate_wasm(&bytes, 65536).unwrap();
    let bodies = Parser::new(0)
        .parse_all(&bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::CodeSectionEntry(body) => Some(body),
            _ => None,
        })
        .collect::<Vec<_>>();
    super::inspect_map(&bodies, 2, 0, 32768)
}

#[test]
fn staged_map_imports_are_leaves_but_defined_cycles_still_refuse() {
    for (helper, expected) in [("", Ok(64)), ("call 2", Err(Error::Cycle))] {
        let input = source(
            &helper_frame(16, helper, ""),
            "call 0 call 1 call 2",
            48,
            48,
        )
        .replace(
            "(memory 3 3)",
            "(import \"provider\" \"stage\" (func (type 0))) (memory 3 3)",
        );
        let bytes = compile_wat(&input, 65536, 65536).unwrap();
        validate_wasm(&bytes, 65536).unwrap();
        let bodies = Parser::new(0)
            .parse_all(&bytes)
            .filter_map(|payload| match payload.unwrap() {
                Payload::CodeSectionEntry(body) => Some(body),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            super::inspect_staged_map(&bodies, 3, 0, 32768).map(|report| report.frame_bytes),
            expected
        );
        assert_eq!(
            super::inspect_staged_map(&bodies, 1, 0, 32768),
            Err(Error::Shape)
        );
    }
}

#[test]
fn map_loops_preserve_frames_and_do_not_expand_vector_admission() {
    let looped = source(
        &helper_frame(16, "", ""),
        "loop call 1 i32.const 0 br_if 0 end",
        48,
        48,
    );
    assert_eq!(inspect(&looped), Err(Error::Unsupported));
    assert_eq!(inspect_map(&looped).unwrap().frame_bytes, 64);
    for (body, error) in [
        ("loop i32.const 1 local.set 0 end", Error::Stack),
        ("loop i64.const 1 br 2 end", Error::Control),
        ("loop call 2 i32.wrap_i64 local.set 1 end", Error::Cycle),
        (
            "loop i32.const 1 memory.grow local.set 1 end",
            Error::Unsupported,
        ),
    ] {
        let input = source("", body, 32, 32).replace("(local i32)", "(local i32 i32)");
        assert_eq!(inspect_map(&input), Err(error));
    }
    let memory = source("", "i32.const 32688 i32.const 0 i32.const 4 memory.fill i32.const 32688 i64.load32_u local.set 1", 80, 80)
        .replace("(local i32)", "(local i32 i64)");
    assert_eq!(inspect(&memory), Err(Error::Unsupported));
    assert!(inspect_map(&memory).is_ok());
    for expression in [
        "i32.const 8 i32.ctz",
        "i32.const 8 i32.const 2 i32.div_u",
        "i32.const 8 i32.const 2 i32.or",
        "i64.const 8 i64.const 2 i64.le_u",
    ] {
        let arithmetic = source("", &format!("{expression} local.set 1"), 32, 32)
            .replace("(local i32)", "(local i32 i32)");
        assert_eq!(inspect(&arithmetic), Err(Error::Unsupported));
        assert!(inspect_map(&arithmetic).is_ok());
    }
    let packed = source("", "", 32, 32).replace(
        "i64.const 7 return",
        "i32.const 8 i32.ctz i32.const 12 i32.shl i32.const 6 i32.or i64.extend_i32_u return",
    );
    assert!(inspect_map(&packed).is_ok());
    assert!(inspect(&packed).is_err());
    for suffix in ["i32.const 32736 i64.load", "call 1 i64.const 7"] {
        let after_restore =
            source("", "", 32, 32).replace("i64.const 7 return", &format!("{suffix} return"));
        assert!(inspect_map(&after_restore).is_err());
    }
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

fn helper_frame(bytes: u32, body: &str, suffix: &str) -> String {
    format!(
        "(local i32) global.get 0 i32.const {bytes} i32.sub local.tee 0 global.set 0 {body} local.get 0 i32.const {bytes} i32.add global.set 0 {suffix}"
    )
}

#[test]
fn nested_frames_use_peak_live_depth_not_individual_or_sequential_sizes() {
    let helper = helper_frame(16, "call 3", "");
    let mut nested = source(&helper, "call 1 call 1", 48, 48);
    let end = nested.rfind(')').unwrap();
    nested.insert_str(end, &format!("(func {})", helper_frame(16, "", "")));
    assert_eq!(
        inspect(&nested).unwrap(),
        Report {
            frame_bytes: 80,
            lowest_stack_pointer: 32688,
            direct_edges: 3
        }
    );
    let exhausted = source(&helper_frame(32720, "", ""), "call 1", 48, 48);
    assert_eq!(inspect(&exhausted), Err(Error::Stack));
    // A helper's size alone must also be rejected when the total would wrap.
    let huge = source(&helper_frame(2147483632, "", ""), "call 1", 64, 64);
    assert_eq!(inspect(&huge), Err(Error::Stack));
}

#[test]
fn helper_frames_cannot_escape_or_execute_effects_after_restoration() {
    for (body, suffix) in [
        ("return", ""),
        ("br 0", ""),
        ("block br 1 end", ""),
        ("", "call 0"),
        ("", "block end"),
        ("", "i32.const 0 i32.const 1 i32.store"),
    ] {
        assert_eq!(
            inspect(&source(&helper_frame(16, body, suffix), "call 1", 48, 48)),
            Err(Error::Control)
        );
    }
    let helper = helper_frame(16, "", "").replace(
        "local.get 0 i32.const 16 i32.add",
        "local.get 0 i32.const 32 i32.add",
    );
    assert_eq!(
        inspect(&source(&helper, "call 1", 48, 48)),
        Err(Error::Stack)
    );
}
