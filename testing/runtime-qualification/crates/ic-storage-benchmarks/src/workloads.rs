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
                Case::VectorU64,
                Case::VectorBytes32,
                Case::VectorBounded255,
                Case::VectorBounded256,
                Case::VectorBytes,
                Case::VectorRecord,
            ],
            Self::VectorReads => &[
                Case::VectorU64,
                Case::VectorBytes32,
                Case::VectorBytes,
                Case::VectorRecord,
            ],
            Self::Depth => &[
                Case::U64U64,
                Case::U64Bytes,
                Case::SetU64,
                Case::VectorU64,
                Case::CellFixed32,
                Case::Typed {
                    collection: Collection::Region,
                    key: DataType::U64,
                    value: DataType::U64,
                },
            ],
            Self::Types => crate::MATRIX_CASES,
            Self::Control => &[Case::CellBytes, Case::U64Bytes],
            Self::AuthLookups => &[Case::VectorSession],
            Self::Ordered => &[
                Case::U64U64,
                Case::Bytes32Bytes32,
                Case::StringString,
                Case::U64Bytes,
                Case::SetU64,
                Case::SetBytes64,
                Case::SetString,
            ],
            Self::Iterators => &[
                Case::VectorU64,
                Case::VectorI64,
                Case::VectorU128,
                Case::VectorF64,
                Case::VectorBytes32,
                Case::VectorRecord,
                Case::VectorBytes,
            ],
            Self::Records => &[
                Case::U64Bytes,
                Case::Bytes32Bytes32,
                Case::SetBytes64,
                Case::VectorRecord,
            ],
            Self::Bulk | Self::Boundaries => &[
                Case::U64Bytes,
                Case::VectorBytes,
                Case::CellBytes,
                Case::RegionBytes,
            ],
        }
    }
    pub fn payloads(self, case: Case) -> &'static [u32] {
        match (self, case) {
            (Self::VectorReads | Self::VectorOpen, Case::VectorBytes) => &[31, 4096],
            (Self::VectorReads | Self::VectorOpen, _) => case.payloads(),
            (Self::Depth, Case::U64Bytes) => &[32],
            (Self::Depth, _) => case.payloads(),
            (Self::Types, _) => case.payloads(),
            (Self::AuthLookups, _) => case.payloads(),
            (Self::Ordered, Case::U64Bytes) => &[31, 32, 33, 256],
            (Self::Ordered, Case::StringString) => &[32],
            (Self::Ordered, _) => case.payloads(),
            (Self::Iterators, Case::VectorBytes) => &[0, 32, 256],
            (Self::Iterators, _) => case.payloads(),
            (Self::Control, _) => &[256, 4096],
            (Self::Records, Case::U64Bytes) => &[31, 32, 33, 256, 1024, 4096],
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
