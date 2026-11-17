//! Recompute the exact pooled provider composition before installed admission.
use super::super::{artifact::LIMIT, vector_access, vector_link};
use ic_storage_retention_probe::artifact::compile_wat;

#[derive(Clone, Copy, Debug)]
pub(super) struct Profile {
    pub(super) version: u64,
    pub(super) buckets: u64,
    pub(super) post_trap: bool,
    pub(super) post_pool_trap: bool,
    pub(super) max_pages: u64,
    pub(super) source_guard: bool,
    pub(super) fault: Option<(u32, u64)>,
}
impl Profile {
    pub(super) const A: Self = Self {
        version: 1,
        buckets: 8,
        post_trap: false,
        post_pool_trap: false,
        max_pages: 5,
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
        || !matches!(profile.max_pages, 3 | 5)
        || profile.fault.is_some_and(|(offset, _)| {
            !matches!(
                offset,
                65536
                    | 65544
                    | 65552
                    | 65560
                    | 65568
                    | 65576
                    | 65584
                    | 65592
                    | 65600
                    | 65608
                    | 131072
                    | 131080
                    | 131264
            )
        })
    {
        return Err("map profile");
    }
    let mut helpers = include_str!("access.wat").to_owned();
    helpers.push_str("(export \"__link_claim\" (func $claim))\n");
    helpers.push_str("(export \"__link_staged_word\" (func $staged_word))\n");
    for helper in vector_access::MAP_HELPERS {
        helpers.push_str(&format!(
            "(export \"__link_{helper}\" (func $access_{helper}))\n"
        ));
    }
    for fault in 1..=4 {
        helpers.push_str(&format!("(func (export \"canister_update pool_failure_{fault}\") (drop (call $expand_pool (i32.const {fault}))) (call $reply))\n"));
    }
    for (id, suffix) in [(0, "a"), (1, "b")] {
        for (name, command, key, value, fail, trap) in [
            ("insert", 1, 7u64, 70u64, 0, false),
            ("replace", 1, 7, 71, 0, false),
            ("lookup", 3, 7, 71, 0, false),
            ("delete", 2, 7, 71, 0, false),
            ("business_error", 5, 99, 990, 0, false),
            ("forget", 6, 100, 1000, 0, false),
            ("reset", 4, 0, 0, 0, false),
            ("reset_finalize_failure", 4, 0, 0, 1, false),
            ("reset_first_root_failure", 4, 0, 0, 2, false),
            ("grow", 1, 65535, 999, 0, false),
            ("grow_business_error", 5, 65535, 999, 0, false),
            ("grow_forget", 6, 65535, 999, 0, false),
            ("remove_growth", 2, 65535, 999, 0, false),
            ("refuse_growth", 7, 65535, 999, 0, false),
            ("delete_full", 2, 2, 1002, 0, false),
            ("lookup_full", 3, 2, 1002, 0, false),
            ("both", 9, 65535, 999, 0, false),
            ("partial_release", 10, 65535, 999, 0, false),
            ("stage_empty", 13, 0, 0, 0, false),
            ("stage_overflow", 13, 65, 0, 0, false),
            ("stage_reserved", 13, 1, 4, 0, false),
            ("stage_unbound", 13, 1, 0, 0, false),
            ("native_release", 8, 65535, 999, 0, false),
            ("write_failure", 1, 65535, 999, 5, false),
            ("finalize_failure", 9, 65535, 999, 1, false),
            ("first_root_failure", 9, 65535, 999, 2, false),
            ("second_root_failure", 9, 65535, 999, 3, false),
            ("settled_failure", 9, 65535, 999, 4, false),
            ("trap", 1, 65535, 999, 0, true),
        ] {
            helpers.push_str(&format!("(func (export \"canister_update {name}_{suffix}\") (call $run (i32.const {}) (i64.const {}) (i32.const {fail})) {} (call $reply))\n", command | id << 8, key | value << 32, if trap { "unreachable" } else { "" }));
        }
    }
    Ok(include_str!("provider.wat")
        .replace("__VERSION__", &profile.version.to_string())
        .replace("__MAX_PAGES__", &profile.max_pages.to_string())
        .replace(
            "__ROOT_A__",
            &(u64::from(profile.buckets.trailing_zeros()) << 12).to_string(),
        )
        .replace(
            "__ROOT_B__",
            &((u64::from(profile.buckets.trailing_zeros()) << 12) | (profile.buckets << 16))
                .to_string(),
        )
        .replace(
            "__POST_POOL_FAILURE__",
            if profile.post_pool_trap {
                "(drop (call $expand_pool (i32.const 4))) unreachable"
            } else {
                ""
            },
        )
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
                "(call $run (i32.const 9) (i64.const 42945378058239) (i32.const 0)) unreachable"
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
    let provider = compile_wat(&source(profile)?, LIMIT, LIMIT).map_err(|error| {
        eprintln!("map provider WAT: {error:?}");
        "map provider WAT"
    })?;
    vector_link::compose_map(raw, &provider)
}
pub(super) fn admit_final(bytes: &[u8], raw: &[u8], profile: Profile) -> Result<(), &'static str> {
    if bytes != prepare(raw, profile)? {
        return Err("final map mismatch");
    }
    Ok(())
}
