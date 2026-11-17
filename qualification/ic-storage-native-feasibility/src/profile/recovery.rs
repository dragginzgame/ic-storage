//! One bounded, sealed family snapshot. Never a per-message persistence flush.
use super::{Error, LIMIT, OWNERS, Profile, ROWS, Schema, State};
use alloc::vec::Vec;
use core::{alloc::Allocator, ops::Bound::Unbounded};
use ic_storage::ordered::{Error as StorageError, Map};
use sha2::{Digest, Sha256};

const HEADER: usize = 56;
const SEAL: usize = 32;
const MAXIMUM: usize = HEADER + SEAL + 2 * 56 + LIMIT * (8 + 8 + 512 + 8 + 29 + 8);

impl<A: Allocator + Clone> State<A> {
    /// Freeze both logical roots into one epoch/schema-bound recovery artifact.
    /// This bounded full-buffer fixture path is not used for compatible reopening.
    pub fn checkpoint(&self, epoch: u64) -> Result<Vec<u8>, Error> {
        let (rows, owners) = self.grant.read(|view| {
            Ok::<_, Error>((
                view.checkpoint(ROWS, epoch)?,
                view.checkpoint(OWNERS, epoch)?,
            ))
        })??;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(HEADER + SEAL + rows.len() + owners.len())
            .map_err(|_| Error::Storage(StorageError::Allocation))?;
        bytes.extend_from_slice(b"PROFCP01");
        for value in [
            self.schema as u64,
            epoch,
            1,
            2,
            rows.len() as u64,
            owners.len() as u64,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&rows);
        bytes.extend_from_slice(&owners);
        let seal = Sha256::digest(&bytes);
        bytes.extend_from_slice(&seal);
        Ok(bytes)
    }

    /// Recover into private fresh native allocations, then validate both roots.
    /// An explicit Wide→Narrow conversion refuses unrepresentable revisions.
    /// Failed admission/conversion publishes nothing; the input remains untouched.
    pub fn recover(
        bytes: &[u8],
        epoch: u64,
        source: Schema,
        target: Schema,
        allocator: A,
    ) -> Result<Self, Error> {
        if !(HEADER + SEAL..=MAXIMUM).contains(&bytes.len())
            || &bytes[..8] != b"PROFCP01"
            || Sha256::digest(&bytes[..bytes.len() - SEAL]).as_slice()
                != &bytes[bytes.len() - SEAL..]
        {
            return Err(Error::Invalid);
        }
        let mut fields = [0u64; 6];
        for (field, bytes) in fields.iter_mut().zip(bytes[8..HEADER].as_chunks::<8>().0) {
            *field = u64::from_le_bytes(*bytes);
        }
        if fields[..4] != [source as u64, epoch, 1, 2]
            || (source != target && (source, target) != (Schema::Wide, Schema::Narrow))
        {
            return Err(Error::Invalid);
        }
        let row_len = usize::try_from(fields[4]).map_err(|_| Error::Invalid)?;
        let owner_len = usize::try_from(fields[5]).map_err(|_| Error::Invalid)?;
        if row_len.checked_add(owner_len) != Some(bytes.len() - HEADER - SEAL) {
            return Err(Error::Invalid);
        }
        let rows = Map::from_checkpoint_in(
            &bytes[HEADER..HEADER + row_len],
            epoch,
            LIMIT,
            allocator.clone(),
        )?;
        let owners = Map::from_checkpoint_in(
            &bytes[HEADER + row_len..bytes.len() - SEAL],
            epoch,
            LIMIT,
            allocator.clone(),
        )?;
        let candidate = Self::admit(rows, owners, source)?;
        if source != target {
            let mut converted = Map::new_in(LIMIT, allocator);
            candidate.grant.read(|rows| {
                converted.with(|mut converted| {
                    for (key, bytes) in rows.range(ROWS, Unbounded, Unbounded)? {
                        let value = Profile::decode(bytes.as_slice(), source)?.encode(target)?;
                        converted.insert(key.as_slice(), value.as_slice())?;
                    }
                    Ok::<_, Error>(())
                })
            })???;
            let (_, owners) = candidate.release()?;
            return Self::admit(converted, owners, target);
        }
        Ok(candidate)
    }
}
