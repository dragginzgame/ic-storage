// Shared fixture-only Wire policy and passive DTOs, compiled by both endpoints.
use ic_wire::Limits;
use serde::{Deserialize, Serialize};

pub const INIT_ID: u64 = 0x4c4f_4751_0000_0001;
pub const UPGRADE_ID: u64 = 0x4c4f_4751_0000_0002;
pub const QUERY_ID: u64 = 0x4c4f_4751_0000_0003;
pub const UPDATE_ID: u64 = 0x4c4f_4751_0000_0004;
pub const REQUEST: Limits = Limits {
    bytes: 1024,
    allocation: 16 * 1024,
    nodes: 128,
    depth: 12,
};
pub const RESPONSE: Limits = Limits {
    bytes: 8 * 1024,
    allocation: 32 * 1024,
    nodes: 256,
    depth: 16,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Append {
    pub created_at_secs: u64,
    pub message: String,
    pub trap_after_save: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Update {
    Append(Append),
    TextBudgetProbe,
    ReadTextBudget,
    Prune {
        now_secs: u64,
        max_age_secs: u64,
        max_remove: u8,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum Upgrade {
    Restore,
    TrapAfterCheckpoint,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Outcome {
    Read,
    Appended(u64),
    Pruned(u64),
    TimestampRefused,
    LabelRefused,
    Exhausted,
    TextRefused,
    BudgetChecked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Row {
    pub sequence: u64,
    pub created_at_secs: u64,
    pub message: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub rows: Vec<Row>,
    pub next_sequence: Option<u64>,
    pub last_created_at_secs: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reply {
    pub outcome: Outcome,
    pub snapshot: Snapshot,
}
