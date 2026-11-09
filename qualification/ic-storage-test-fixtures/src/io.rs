// Fixture-only Wire contract compiled by the canister and the native runner.
use ic_wire::Limits;
use serde::{Deserialize, Serialize};

/// Maximum resources admitted for a request frame.
pub const REQUEST: Limits = Limits {
    bytes: 128,
    allocation: 4096,
    nodes: 128,
    depth: 8,
};
/// Maximum resources admitted for a response frame.
pub const RESPONSE: Limits = REQUEST;

/// Installation request identity.
pub const INIT_ID: u64 = 0x4d45_4d51_0000_0001;
/// Upgrade request identity.
pub const UPGRADE_ID: u64 = 0x4d45_4d51_0000_0002;
/// Passive snapshot request identity.
pub const SNAPSHOT_ID: u64 = 0x4d45_4d51_0000_0003;
/// Measured IO request identity.
pub const IO_ID: u64 = 0x4d45_4d51_0000_0004;
/// Composed lease custody request identity.
pub const COMPOSITION_ID: u64 = 0x4d45_4d51_0000_0005;
/// Injected manager metadata fault request identity.
pub const METADATA_FAULT_ID: u64 = 0x4d45_4d51_0000_0006;

///
/// Snapshot
///
/// Passive fixture observation; never allocation authority.
///

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    /// Current committed generation.
    pub generation: u64,
    /// Neighboring retained marker bytes.
    pub markers: [u8; 2],
    /// Physical backing extent in pages.
    pub physical_pages: u64,
    /// Rows allocation extent in pages.
    pub rows_pages: u64,
}

///
/// `IoRequest`
///
/// One bounded request for the fixture's measured IO loop.
///

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IoRequest {
    /// Selected IO delegate.
    pub operation: u8,
    /// Number of bytes in one IO operation.
    pub count: u8,
    /// Virtual byte offset.
    pub offset: u64,
}

///
/// `IoReply`
///
/// Destination bytes and the loop's actual instruction count.
///

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IoReply {
    /// Observed destination bytes.
    pub bytes: [u8; 2],
    /// Measured instructions inside the IO loop.
    pub instructions: u64,
}
