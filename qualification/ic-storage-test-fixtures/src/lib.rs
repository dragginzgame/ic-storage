//! Passive Wire protocols shared by installed fixture binaries and their runner.

/// Raw backing IO requests and readback replies.
pub mod io;
/// Checkpoint, restoration and bounded retention scenarios for the Logging adapter.
pub mod logging;
/// Governed allocation and collection operation requests.
pub mod storage;
