//! Closed record-provider composition; every final artifact is recomputed exactly.

use ic_storage_retention_probe::{artifact::compile_wat, vector_provider::records};

use super::super::{artifact::LIMIT, vector_access, vector_link};

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Profile {
    pub(crate) version: u64,
    pub(crate) root: (u64, u64),
    pub(crate) post_trap: bool,
    pub(crate) fault: Option<(u32, u64)>,
    pub(crate) source_guard: bool,
}

impl Profile {
    pub(crate) const A: Self = Self {
        version: 1,
        root: (3, 4),
        post_trap: false,
        fault: None,
        source_guard: true,
    };
    pub(crate) const B: Self = Self {
        version: 2,
        ..Self::A
    };

    fn check(self) -> Result<()> {
        if !matches!(self.version, 1 | 2)
            || self.root.0 > self.root.1
            || self.root.1 > 16
            || self.fault.is_some_and(|(offset, _)| {
                !((65536..=65672).contains(&offset) && offset % 8 == 0
                    || matches!(offset, 131080 | 131112))
            })
        {
            return Err("record vector fixture profile");
        }
        Ok(())
    }
}

fn replace(source: &mut String, from: &str, to: &str, count: usize) -> Result<()> {
    if source.matches(from).count() != count {
        return Err("record provider anchor");
    }
    *source = source.replace(from, to);
    Ok(())
}

// Reuse the guarded u64 operations with the admitted record geometry. Raw Rust
// uses a 64-byte stack, sixteen slots and a sixteen-byte stride. No element
// reference or old Vec layout crosses the foreign claim boundary.
pub(super) fn guards() -> Result<String> {
    let mut source = include_str!("../../provider_custody/vector/access.wat").to_owned();
    replace(&mut source, "(i64.const 32688)", "(i64.const 32704)", 1)?;
    replace(&mut source, "(i64.const 512)", "(i64.const 16)", 2)?;
    replace(
        &mut source,
        "(i64.mul (local.get $words) (i64.const 8))",
        "(i64.mul (local.get $words) (i64.const 16))",
        1,
    )?;
    Ok(source)
}

pub(crate) fn source(profile: Profile) -> Result<String> {
    profile.check()?;
    let fault = profile.fault.map_or(String::new(), |(offset, value)| {
        format!("(i64.store (i32.const {offset}) (i64.const {value}))")
    });
    let mut source = records::source(profile.version, profile.root, false, &fault, true);
    replace(
        &mut source,
        "(func $reply))",
        "(func $reply))\n(import \"vector_link\" \"step\" (func $native_step (param i32 i32 i32 i64) (result i64)))",
        1,
    )?;
    replace(
        &mut source,
        "(func (export \"canister_pre_upgrade\")",
        "(func $provider_pre",
        1,
    )?;
    replace(
        &mut source,
        "(func (export \"canister_post_upgrade\")",
        "(func $provider_post",
        1,
    )?;
    let end = source.rfind(')').ok_or("provider closing delimiter")?;
    source.truncate(end);
    source.push_str(&guards()?);
    let mut glue = include_str!("../vector_link/entrypoints.wat").replace(
        "__POST_FAILURE__",
        if profile.post_trap {
            "(call $vector_run (i32.const 1) (i64.const 999)) unreachable"
        } else {
            ""
        },
    );
    if !profile.source_guard {
        replace(&mut glue, "(export \"canister_pre_upgrade\")", "", 1)?;
    }
    source.push_str(&glue);
    source.push_str("(func (export \"canister_update record_mutate_then_fail\")\n(call $vector_run (i32.const 0) (i64.const 1))\n(call $vector_run (i32.const 1) (i64.const 1001)) (call $reply))\n");
    source.push_str("(export \"__link_claim\" (func $claim))\n");
    for helper in vector_access::HELPERS {
        source.push_str(&format!(
            "(export \"__link_{helper}\" (func $access_{helper}))\n"
        ));
    }
    source.push(')');
    Ok(source)
}

pub(super) fn prepare(raw: &[u8], profile: Profile) -> Result<Vec<u8>> {
    let provider =
        compile_wat(&source(profile)?, LIMIT, LIMIT).map_err(|_| "record provider WAT")?;
    vector_link::compose_record(raw, &provider)
}

pub(super) fn admit_final(bytes: &[u8], raw: &[u8], profile: Profile) -> Result<()> {
    if bytes != prepare(raw, profile)? {
        return Err("final record vector mismatch");
    }
    Ok(())
}
