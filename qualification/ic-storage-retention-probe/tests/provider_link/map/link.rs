//! Recompute the exact closed provider composition before installed admission.
use super::super::{artifact::LIMIT, vector_access, vector_link};
use ic_storage_retention_probe::artifact::compile_wat;

#[derive(Clone, Copy, Debug)]
pub(super) struct Profile {
    pub(super) version: u64,
    pub(super) buckets: u64,
    pub(super) post_trap: bool,
    pub(super) source_guard: bool,
    pub(super) fault: Option<(u32, u64)>,
}

impl Profile {
    pub(super) const A: Self = Self {
        version: 1,
        buckets: 512,
        post_trap: false,
        source_guard: true,
        fault: None,
    };
    pub(super) const B: Self = Self {
        version: 2,
        ..Self::A
    };
}

pub(super) fn source(profile: Profile) -> Result<String, &'static str> {
    if !matches!(profile.version, 1 | 2)
        || !matches!(profile.buckets, 8 | 512)
        || profile.fault.is_some_and(|(offset, _)| {
            !matches!(
                offset,
                65536
                    | 65544
                    | 65568
                    | 65576
                    | 65584
                    | 65592
                    | 65600
                    | 65608
                    | 65616
                    | 131072
                    | 131080
            )
        })
    {
        return Err("map profile");
    }
    let mut helpers = include_str!("access.wat").to_owned();
    helpers.push_str("(export \"__link_claim\" (func $claim))\n");
    for helper in vector_access::MAP_HELPERS {
        helpers.push_str(&format!(
            "(export \"__link_{helper}\" (func $access_{helper}))\n"
        ));
    }
    for (name, operation, key, value, fail, trap) in [
        ("insert", 1, 7u64, 70u64, 0, false),
        ("insert_second", 1, 42, 420, 0, false),
        ("replace", 1, 7, 71, 0, false),
        ("lookup", 3, 7, 71, 0, false),
        ("delete", 2, 42, 420, 0, false),
        ("absent", 4, 42, 0, 0, false),
        ("business_error", 5, 99, 990, 0, false),
        ("forget", 6, 100, 1000, 0, false),
        ("refuse_growth", 7, 999, 999, 0, false),
        ("delete_full", 2, 2, 1002, 0, false),
        ("reuse", 1, 999, 999, 0, false),
        ("clear", 8, 0, 0, 0, false),
        ("trap", 1, 0, 9999, 0, true),
        ("finalize_trap", 5, 0, 9999, 1, false),
        ("grow", 1, 999, 999, 0, false),
        ("grow_business_error", 5, 999, 999, 0, false),
        ("grow_forget", 6, 999, 999, 0, false),
        ("grow_write_trap", 9, 999, 999, 0, false),
        ("grow_candidate_trap", 10, 999, 999, 0, false),
        ("grow_finalize_trap", 1, 999, 999, 1, false),
        ("grow_published_trap", 1, 999, 999, 2, false),
    ] {
        helpers.push_str(&format!("(func (export \"canister_update {name}\") (call $run (i32.const {operation}) (i64.const {}) (i32.const {fail})) {} (call $reply))\n", key | value << 32, if trap { "unreachable" } else { "" }));
    }
    Ok(include_str!("provider.wat")
        .replace("__VERSION__", &profile.version.to_string())
        .replace("__BUCKETS__", &profile.buckets.to_string())
        .replace(
            "__PRE_EXPORT__",
            if profile.source_guard {
                "(export \"canister_pre_upgrade\")"
            } else {
                ""
            },
        )
        .replace(
            "__POST_FAILURE__",
            if profile.post_trap {
                "(call $run (i32.const 1) (i64.const 42945377993703) (i32.const 0)) unreachable"
            } else {
                ""
            },
        )
        .replace(
            "__FAULT__",
            &profile.fault.map_or(String::new(), |(offset, value)| {
                format!("(i64.store (i32.const {offset}) (i64.const {value}))")
            }),
        )
        .replace("__HELPERS_AND_METHODS__", &helpers))
}

pub(super) fn prepare(raw: &[u8], profile: Profile) -> Result<Vec<u8>, &'static str> {
    let provider = compile_wat(&source(profile)?, LIMIT, LIMIT).map_err(|_| "map provider WAT")?;
    vector_link::compose_map(raw, &provider)
}

pub(super) fn admit_final(bytes: &[u8], raw: &[u8], profile: Profile) -> Result<(), &'static str> {
    if bytes != prepare(raw, profile)? {
        return Err("final map mismatch");
    }
    Ok(())
}
