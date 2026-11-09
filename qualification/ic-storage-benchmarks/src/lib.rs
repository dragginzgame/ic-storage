//! Matched map workload and bounded benchmark protocol; not a production API.
use serde::{Deserialize, Serialize};
pub mod catalogue;
mod types;
pub use catalogue::{Collection, DataType, MATRIX_CASES, ReferenceValue};
pub use ic_principal::Principal as BenchmarkPrincipal;
pub use ic_stable_structures as codec_reference;
#[cfg(not(target_arch = "wasm32"))]
pub mod workloads;
#[cfg(all(
    feature = "dfinity",
    any(not(target_arch = "wasm32"), not(feature = "dfinity-head"))
))]
pub use ic_stable_structures as upstream;
#[cfg(all(target_arch = "wasm32", feature = "dfinity-head"))]
pub use ic_stable_structures_head as upstream;
pub use types::*;

pub const MAX_ROWS: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(clap::ValueEnum))]
pub enum Pattern {
    #[default]
    Uniform,
    Hot16,
    Zipf,
    Sequential,
    First,
    Middle,
    Last,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Initialization {
    pub case: Case,
    pub upstream_cache_slots: usize,
    pub upstream_bucket_pages: std::num::NonZeroU16,
    pub cold_cache: bool,
    /// Keep the allocation owner, but reopen the Vector for each request.
    pub vector_open: bool,
}

pub const SETUP_ID: u64 = 1;
pub const RUN_ID: u64 = 2;
pub const INIT_COST_ID: u64 = 4;
pub const LIMITS: ic_wire::Limits = ic_wire::Limits {
    bytes: 8 * 1024 * 1024,
    // Wire conservatively charges visited byte references as well as buffers.
    allocation: 256 * 1024 * 1024,
    // Vec<u8> visits each byte; admit one 1.5 MiB grown bulk result plus framing.
    nodes: 8_000_000,
    depth: 16,
};
///
/// Setup
///
/// Bounded setup or final verification request.
///

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Setup {
    pub start: u64,
    pub count: u32,
    pub payload: u32,
    pub ids: Vec<u64>,
}

pub fn population_ids(setup: &Setup) -> Vec<u64> {
    if setup.ids.is_empty() {
        (setup.start..setup.start + u64::from(setup.count)).collect()
    } else {
        assert_eq!(setup.ids.len(), setup.count as usize);
        setup.ids.clone()
    }
}
///
/// Operation
///
/// One measured map operation, or the loop control.
///

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Control,
    Get,
    MissAbove,
    Replace,
    Scan,
    Insert,
    Remove,
    Contains,
    ReverseScan,
    KeysScan,
    Grow,
    Shrink,
    RemoveMiss,
    RemoveInserted,
    Reinsert,
    Set,
    Push,
    Pop,
    First,
    Last,
    BoundedScan,
    Nth,
    NthBack,
    AlternatingScan,
    ScanSet,
    ValuesScan,
    ReverseKeysScan,
    BoundedKeysScan,
    Decode,
    WarmGet,
    IterLast,
    KeysLast,
    ValuesLast,
}
pub const OPERATIONS: [Operation; 27] = [
    Operation::Control,
    Operation::Get,
    Operation::MissAbove,
    Operation::Replace,
    Operation::Scan,
    Operation::Insert,
    Operation::Contains,
    Operation::ReverseScan,
    Operation::KeysScan,
    Operation::Grow,
    Operation::Shrink,
    Operation::Remove,
    Operation::RemoveMiss,
    Operation::Reinsert,
    Operation::RemoveInserted,
    Operation::First,
    Operation::Last,
    Operation::BoundedScan,
    Operation::Nth,
    Operation::NthBack,
    Operation::AlternatingScan,
    Operation::ValuesScan,
    Operation::ReverseKeysScan,
    Operation::BoundedKeysScan,
    Operation::IterLast,
    Operation::KeysLast,
    Operation::ValuesLast,
];
///
/// Request
///
/// A deterministic operation batch shared by both backends.
///

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Request {
    pub case: Case,
    pub operation: Operation,
    pub rows: u32,
    pub payload: u32,
    pub count: u32,
    pub seed: u64,
    pub pattern: Pattern,
}
///
/// Response
///
/// Measured instructions and complete results for independent validation.
///

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub instructions: u64,
    pub entries: Vec<(u64, Option<Vec<u8>>)>,
    pub length: u64,
    pub stable_pages: u64,
    pub decode: Option<DecodeMetrics>,
}
///
/// DecodeMetrics
///
/// Instrumented outer-record decoder work, not Auth's inner session decoder.
///

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct DecodeMetrics {
    pub calls: u64,
    pub bytes: u64,
    pub instructions: u64,
}
/// Identical allocation/codec bytes for both engines; no Candid in the workload.
pub fn value(key: u64, length: u32, replacement: bool) -> Vec<u8> {
    (0..length)
        .map(|i| (key as u8).wrapping_add(i as u8) ^ if replacement { 128 } else { 0 })
        .collect()
}
/// Bound result construction independently of population and selected payload.
pub fn max_batch(case: Case) -> u32 {
    match case.structure() {
        "Vector" => 256,
        "Map" | "Set" => 128,
        _ => 32,
    }
}
/// Generate the complete trace outside the measured interval.
pub fn keys(request: Request) -> Vec<u64> {
    assert!((1..=MAX_ROWS).contains(&request.rows));
    assert!((1..=max_batch(request.case)).contains(&request.count));
    assert!(request.case.accepts_payload(request.payload));
    let mut state = request.seed;
    let cdf = if request.pattern == Pattern::Zipf {
        let mut sum = 0.0;
        (1..=request.rows)
            .map(|rank| {
                sum += 1.0 / f64::from(rank);
                sum
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    (0..request.count)
        .map(|index| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            match request.operation {
                Operation::RemoveMiss => {
                    u64::from(request.rows) + u64::from(request.count) + u64::from(index)
                }
                Operation::MissAbove
                | Operation::Insert
                | Operation::RemoveInserted
                | Operation::Push => u64::from(request.rows) + u64::from(index),
                _ => {
                    match request.pattern {
                        Pattern::First => return 0,
                        Pattern::Middle => return u64::from(request.rows / 2),
                        Pattern::Last => return u64::from(request.rows - 1),
                        _ => {}
                    }
                    if request.pattern == Pattern::Sequential {
                        return (request.seed % u64::from(request.rows) + u64::from(index))
                            % u64::from(request.rows);
                    }
                    if request.pattern == Pattern::Zipf {
                        let fraction =
                            f64::from((state >> 32) as u32) / (f64::from(u32::MAX) + 1.0);
                        return cdf.partition_point(|&sum| sum <= fraction * cdf.last().unwrap())
                            as u64;
                    }
                    (state >> 32)
                        % u64::from(if request.pattern == Pattern::Hot16 {
                            request.rows.min(16)
                        } else {
                            request.rows
                        })
                }
            }
        })
        .collect()
}
/// Replacement-size variation is shared by canisters and the independent model.
pub fn payload_for(r: Request) -> u32 {
    match r.operation {
        // Leave room for response framing below the IC's 2 MiB reply ceiling.
        Operation::Grow => r.payload.max(32).saturating_mul(4).min(1536 * 1024),
        Operation::Shrink => 0,
        _ => r.payload,
    }
}
pub fn replacement(op: Operation) -> bool {
    matches!(
        op,
        Operation::Replace | Operation::Grow | Operation::Shrink | Operation::Set
    )
}

#[cfg(test)]
mod tests;
