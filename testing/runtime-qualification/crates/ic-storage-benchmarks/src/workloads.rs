//! Fixed consumer-shaped experiments, not production allocation presets.
use crate::{Case, Collection, DataType, Pattern};

/// Population sweep grounded in the retained Auth/Canic cost report.
pub const LOOKUP_POPULATIONS: [u32; 4] = [1, 512, 1024, 2048];
/// Matched small and populated controls for every supported collection form.
pub const DEPTH_POPULATIONS: [u32; 2] = [256, 4096];

///
/// Workload
///
/// Small repeatable selections grounded in the local consumer memory inventory.
/// Each case still owns one fresh collection; these are not full application traces.
///
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Workload {
    Control,
    Records,
    Bulk,
    Boundaries,
    Iterators,
    Ordered,
    AuthLookups,
    Types,
    Depth,
    VectorReads,
    VectorOpen,
}

impl Workload {
    pub fn cases(self) -> &'static [Case] {
        match self {
            Self::VectorOpen => &[
                crate::case!(Vector, U64, U64),
                crate::case!(Vector, U64, Array32),
                crate::case!(Vector, U64, Bounded255),
                crate::case!(Vector, U64, Bounded256),
                crate::case!(Vector, U64, Chunk),
                crate::case!(Vector, U64, WireRecord),
            ],
            Self::VectorReads => &[
                crate::case!(Vector, U64, U64),
                crate::case!(Vector, U64, Array32),
                crate::case!(Vector, U64, Chunk),
                crate::case!(Vector, U64, WireRecord),
            ],
            Self::Depth => &[
                crate::case!(Map, U64, U64),
                crate::case!(Map, U64, Bytes),
                crate::case!(Set, U64, Unit),
                crate::case!(Vector, U64, U64),
                crate::case!(Cell, U64, Array32),
                Case {
                    collection: Collection::Region,
                    key: DataType::U64,
                    value: DataType::U64,
                },
            ],
            Self::Types => crate::MATRIX_CASES,
            Self::Control => &[
                crate::case!(Cell, U64, Bytes),
                crate::case!(Map, U64, Bytes),
            ],
            Self::AuthLookups => &[crate::case!(Vector, U64, Session)],
            Self::Ordered => &[
                crate::case!(Map, U64, U64),
                crate::case!(Map, Array32, Array32),
                crate::case!(Map, String, String),
                crate::case!(Map, U64, Bytes),
                crate::case!(Set, U64, Unit),
                crate::case!(Set, Array64, Unit),
                crate::case!(Set, String, Unit),
            ],
            Self::Iterators => &[
                crate::case!(Vector, U64, U64),
                crate::case!(Vector, U64, Signed64),
                crate::case!(Vector, U64, U128),
                crate::case!(Vector, U64, F64),
                crate::case!(Vector, U64, Array32),
                crate::case!(Vector, U64, WireRecord),
                crate::case!(Vector, U64, Chunk),
            ],
            Self::Records => &[
                crate::case!(Map, U64, Bytes),
                crate::case!(Map, Array32, Array32),
                crate::case!(Set, Array64, Unit),
                crate::case!(Vector, U64, WireRecord),
            ],
            Self::Bulk | Self::Boundaries => &[
                crate::case!(Map, U64, Bytes),
                crate::case!(Vector, U64, Chunk),
                crate::case!(Cell, U64, Bytes),
                crate::case!(Region, U64, Bytes),
            ],
        }
    }
    pub fn payloads(self, case: Case) -> &'static [u32] {
        match (self, case) {
            (Self::VectorReads | Self::VectorOpen, crate::case!(Vector, U64, Chunk)) => &[31, 4096],
            (Self::VectorReads | Self::VectorOpen, _) => case.payloads(),
            (Self::Depth, crate::case!(Map, U64, Bytes)) => &[32],
            (Self::Depth, _) => case.payloads(),
            (Self::Types, _) => case.payloads(),
            (Self::AuthLookups, _) => case.payloads(),
            (Self::Ordered, crate::case!(Map, U64, Bytes)) => &[31, 32, 33, 256],
            (Self::Ordered, crate::case!(Map, String, String)) => &[32],
            (Self::Ordered, _) => case.payloads(),
            (Self::Iterators, crate::case!(Vector, U64, Chunk)) => &[0, 32, 256],
            (Self::Iterators, _) => case.payloads(),
            (Self::Control, _) => &[256, 4096],
            (Self::Records, crate::case!(Map, U64, Bytes)) => &[31, 32, 33, 256, 1024, 4096],
            (Self::Records, _) => case.payloads(),
            (Self::Bulk, _) => &[65_536, 1_048_576],
            (Self::Boundaries, _) => &[
                31, 32, 33, 235, 236, 237, 4075, 4076, 4077, 65_515, 65_516, 65_517,
            ],
        }
    }
    pub const fn rows(self) -> u32 {
        match self {
            Self::Control => 16,
            Self::AuthLookups => LOOKUP_POPULATIONS[LOOKUP_POPULATIONS.len() - 1],
            Self::Records | Self::Depth | Self::VectorOpen => 4096,
            Self::Bulk => 8,
            Self::Boundaries => 64,
            Self::Types => 32,
            Self::Iterators | Self::Ordered | Self::VectorReads => 256,
        }
    }
    pub const fn counts(self) -> &'static [u32] {
        match self {
            Self::Records | Self::Depth | Self::VectorReads | Self::VectorOpen => &[1, 32],
            Self::Types => &[1, 8],
            Self::AuthLookups => &[1],
            Self::Ordered => &[1, 5, 6, 11, 12, 31, 32, 64, 128],
            Self::Iterators => &[1, 2, 31, 32, 63, 64, 65, 127, 128, 129, 256],
            Self::Control | Self::Bulk | Self::Boundaries => &[1],
        }
    }
    pub const fn patterns(self) -> &'static [Pattern] {
        match self {
            Self::Depth | Self::VectorReads | Self::VectorOpen => {
                &[Pattern::Uniform, Pattern::Hot16]
            }
            Self::Records => &[Pattern::Uniform, Pattern::Hot16, Pattern::Zipf],
            Self::AuthLookups => &[Pattern::First, Pattern::Middle, Pattern::Last],
            Self::Control | Self::Bulk | Self::Iterators | Self::Ordered | Self::Types => {
                &[Pattern::Uniform]
            }
            Self::Boundaries => &[Pattern::Sequential],
        }
    }
    pub const fn rounds(self) -> u32 {
        match self {
            Self::Control
            | Self::Records
            | Self::Boundaries
            | Self::Iterators
            | Self::Ordered
            | Self::Depth
            | Self::VectorOpen
            | Self::VectorReads => 3,
            Self::Bulk | Self::AuthLookups | Self::Types => 1,
        }
    }
}

/// Bound setup and verification work independently of collection population.
/// A single grown bulk value is admitted; batches otherwise target at most 512 KiB.
pub fn transfer_rows(payload: usize) -> usize {
    (512 * 1024 / payload.max(1)).clamp(1, 128)
}

#[cfg(test)]
mod tests;
