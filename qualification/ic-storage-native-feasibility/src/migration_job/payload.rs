//! Two closed logical payloads; phase and progress policy stays in MigrationJob.
use super::{Error, checkpoint};
use crate::record::narrow::V3;

mod sealed {
    pub trait Sealed {}
}

/// Closed payload contract for the two qualified job profiles.
pub trait Payload: sealed::Sealed {
    /// Distinct outer identity, including for an empty Ready job.
    const MAGIC: &'static [u8; 8];
    /// Maximum nested logical checkpoint bytes.
    const MAX_BYTES: usize;
    /// Validated transient logical values; never retained owners.
    type Decoded;
    /// Validate the complete checkpoint and epoch.
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self::Decoded, Error>;
    /// Number of elements advanced by staged import.
    fn len(decoded: &Self::Decoded) -> usize;
}

/// Original scalar counter and u64-vector payload.
#[derive(Debug, PartialEq, Eq)]
pub struct ScalarVector;
/// Sixteen bounded V3 records, with no implicit schema conversion.
#[derive(Debug, PartialEq, Eq)]
pub struct RecordVector;
impl sealed::Sealed for ScalarVector {}
impl sealed::Sealed for RecordVector {}

impl Payload for ScalarVector {
    const MAGIC: &'static [u8; 8] = b"ICJOB001";
    const MAX_BYTES: usize = checkpoint::MAX_CHECKPOINT_BYTES;
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
    type Decoded = ([Option<V3>; 16], usize);
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self::Decoded, Error> {
        checkpoint::record_vector::decode(bytes, epoch).map_err(Error::RecordCheckpoint)
    }
    fn len(decoded: &Self::Decoded) -> usize {
        decoded.1
    }
}
