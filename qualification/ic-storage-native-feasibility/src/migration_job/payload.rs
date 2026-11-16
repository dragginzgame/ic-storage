//! Closed logical payloads; phase and progress policy stays in MigrationJob.
use super::{Error, checkpoint};
use crate::record::narrow::V3;

mod sealed {
    pub trait Sealed {}
}

/// Closed payload contract for the qualified job profiles.
pub trait Payload: sealed::Sealed {
    /// Distinct outer identity, including for an empty Ready job.
    const MAGIC: &'static [u8; 8];
    /// Maximum nested logical checkpoint bytes.
    const MAX_BYTES: usize;
    /// Qualified allocation domains for this payload's installed representation.
    const DOMAINS: [u64; 2];
    /// Validated transient logical values; never retained owners.
    type Decoded;
    /// Validate the complete checkpoint and epoch.
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self::Decoded, Error>;
    /// Number of elements advanced by staged import.
    ///
    /// # Panics
    /// The map payload requires the unchanged, unpoisoned result of `decode`.
    fn len(decoded: &Self::Decoded) -> usize;
}

/// Original scalar counter and u64-vector payload.
#[derive(Debug, PartialEq, Eq)]
pub struct ScalarVector;
/// Sixteen bounded V3 records, with no implicit schema conversion.
#[derive(Debug, PartialEq, Eq)]
pub struct RecordVector;
/// Two independently identified, bounded native maps.
#[derive(Debug, PartialEq, Eq)]
pub struct MapPair;
impl sealed::Sealed for ScalarVector {}
impl sealed::Sealed for RecordVector {}
impl sealed::Sealed for MapPair {}

impl Payload for ScalarVector {
    const MAGIC: &'static [u8; 8] = b"ICJOB001";
    const MAX_BYTES: usize = checkpoint::MAX_CHECKPOINT_BYTES;
    const DOMAINS: [u64; 2] = [131072, 147456];
    type Decoded = checkpoint::Checkpoint;
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self::Decoded, Error> {
        if bytes.get(8..10) != Some(2u16.to_le_bytes().as_slice()) {
            return Err(Error::Framing);
        }
        checkpoint::decode(bytes, epoch).map_err(Error::Checkpoint)
    }
    fn len(decoded: &Self::Decoded) -> usize {
        decoded.values.len()
    }
}

impl Payload for RecordVector {
    const MAGIC: &'static [u8; 8] = b"ICJRV001";
    const MAX_BYTES: usize = 64 + 16 * V3::STORE_BYTES;
    const DOMAINS: [u64; 2] = [131072, 147456];
    type Decoded = ([Option<V3>; 16], usize);
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self::Decoded, Error> {
        checkpoint::record_vector::decode(bytes, epoch).map_err(Error::RecordCheckpoint)
    }
    fn len(decoded: &Self::Decoded) -> usize {
        decoded.1
    }
}

impl Payload for MapPair {
    const MAGIC: &'static [u8; 8] = b"ICJMP001";
    const MAX_BYTES: usize = checkpoint::maps::MAX_BYTES;
    const DOMAINS: [u64; 2] = [131072, 163840];
    type Decoded = [ic_storage_native::map::Map; 2];
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self::Decoded, Error> {
        checkpoint::maps::decode(bytes, epoch).map_err(Error::MapCheckpoint)
    }
    fn len(decoded: &Self::Decoded) -> usize {
        // Only freshly decoded, unpoisoned maps enter the transition owner.
        decoded
            .iter()
            .map(|map| map.len().expect("decoded map"))
            .sum()
    }
}
