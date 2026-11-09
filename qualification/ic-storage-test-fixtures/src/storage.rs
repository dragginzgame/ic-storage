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
pub const INIT_ID: u64 = 0x5354_4f51_0000_0001;
/// Upgrade request identity.
pub const UPGRADE_ID: u64 = 0x5354_4f51_0000_0002;
/// Passive snapshot request identity.
pub const SNAPSHOT_ID: u64 = 0x5354_4f51_0000_0003;
/// Controlled collection mutation request identity.
pub const MUTATE_ID: u64 = 0x5354_4f51_0000_0004;
/// Cell admission failure request identity.
pub const CELL_FAILURE_ID: u64 = 0x5354_4f51_0000_0005;

///
/// Snapshot
///
/// Current generation and the three retained collection values.
///

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    /// Current committed generation.
    pub generation: u64,
    /// Retained collection values.
    pub values: [u64; 3],
}
