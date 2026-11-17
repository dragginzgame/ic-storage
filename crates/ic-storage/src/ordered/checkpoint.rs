//! Logical ordered bytes only; no native links, heights, padding or allocator state.
use super::{Bytes, Error, Map, Range, View};
use alloc::vec::Vec;
use core::{alloc::Allocator, ops::Bound};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"ORDMAP01";
const HEADER: usize = 24;
const DIGEST: usize = 32;

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> Map<K, V, A, P> {
    /// Encode sorted logical records with an explicit epoch and SHA-256 seal.
    /// This complete-buffer path uses space proportional to logical bytes.
    ///
    /// # Errors
    /// Refuses poisoned state, unrepresentable lengths or allocation failure.
    pub fn checkpoint(&self, epoch: u64) -> Result<Vec<u8>, Error> {
        self.healthy()?;
        let mut size = HEADER + DIGEST;
        for (key, value) in Range::new(self, Bound::Unbounded, Bound::Unbounded) {
            u32::try_from(key.as_slice().len()).map_err(|_| Error::Limit)?;
            u32::try_from(value.as_slice().len()).map_err(|_| Error::Limit)?;
            size = size
                .checked_add(8)
                .and_then(|n| n.checked_add(key.as_slice().len()))
                .and_then(|n| n.checked_add(value.as_slice().len()))
                .ok_or(Error::Limit)?;
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| Error::Allocation)?;
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&epoch.to_le_bytes());
        bytes.extend_from_slice(&(self.image.len as u64).to_le_bytes());
        for (key, value) in Range::new(self, Bound::Unbounded, Bound::Unbounded) {
            bytes.extend_from_slice(&(key.as_slice().len() as u32).to_le_bytes());
            bytes.extend_from_slice(&(value.as_slice().len() as u32).to_le_bytes());
            bytes.extend_from_slice(key.as_slice());
            bytes.extend_from_slice(value.as_slice());
        }
        let seal = Sha256::digest(&bytes);
        bytes.extend_from_slice(&seal);
        Ok(bytes)
    }

    /// Construct a private candidate from complete logical bytes in a fresh allocator.
    /// Caller-owned schema identity, custody and expected epoch supply admission
    /// context; a digest detects corruption, not hostile authorship or freshness.
    /// Smaller K/V are an explicit bounded conversion; oversized records refuse.
    ///
    /// # Errors
    /// Refuses unknown/truncated framing, digest/epoch mismatch, excessive sizes,
    /// unordered/duplicate keys, trailing bytes or allocator refusal. No existing
    /// collection is modified or published on failure.
    pub fn from_checkpoint_in_allocators(
        bytes: &[u8],
        epoch: u64,
        limit: usize,
        allocator: A,
        payload_allocator: P,
    ) -> Result<Self, Error> {
        if bytes.len() < HEADER + DIGEST || &bytes[..8] != MAGIC {
            return Err(Error::Checkpoint);
        }
        let (payload, seal) = bytes.split_at(bytes.len() - DIGEST);
        if Sha256::digest(payload).as_slice() != seal {
            return Err(Error::Checkpoint);
        }
        let mut header = &payload[8..HEADER];
        if u64::from_le_bytes(
            take(&mut header, 8)?
                .try_into()
                .map_err(|_| Error::Checkpoint)?,
        ) != epoch
        {
            return Err(Error::Epoch);
        }
        let count = u64::from_le_bytes(
            take(&mut header, 8)?
                .try_into()
                .map_err(|_| Error::Checkpoint)?,
        );
        let count = usize::try_from(count).map_err(|_| Error::Limit)?;
        if count > limit || count > (payload.len() - HEADER) / 8 {
            return Err(Error::Limit);
        }
        let mut input = &payload[HEADER..];
        let mut candidate = Self::new_in_allocators(limit, allocator, payload_allocator);
        let mut prior: Option<Bytes<K>> = None;
        for _ in 0..count {
            let key_len = u32::from_le_bytes(
                take(&mut input, 4)?
                    .try_into()
                    .map_err(|_| Error::Checkpoint)?,
            ) as usize;
            let value_len = u32::from_le_bytes(
                take(&mut input, 4)?
                    .try_into()
                    .map_err(|_| Error::Checkpoint)?,
            ) as usize;
            if key_len > K || value_len > V {
                return Err(Error::Limit);
            }
            let key = take(&mut input, key_len)?;
            let value = take(&mut input, value_len)?;
            if prior.as_ref().is_some_and(|prior| prior.as_slice() >= key) {
                return Err(Error::Checkpoint);
            }
            View {
                map: &mut candidate,
            }
            .insert(key, value)?;
            prior = Some(Bytes::new(key)?);
        }
        if !input.is_empty() {
            return Err(Error::Checkpoint);
        }
        Ok(candidate)
    }

    /// Explicitly replace even a poisoned owner after complete checkpoint admission.
    /// Failure preserves the existing owner and its poisoned/healthy state.
    ///
    /// # Errors
    /// Returns the same validation/allocation refusals as `from_checkpoint_in`.
    pub fn restore(&mut self, bytes: &[u8], epoch: u64) -> Result<(), Error> {
        let candidate = Self::from_checkpoint_in_allocators(
            bytes,
            epoch,
            self.limit,
            self.image.nodes.allocator().clone(),
            self.image.payload.allocator().clone(),
        )?;
        *self = candidate;
        Ok(())
    }
}

fn take<'a>(input: &mut &'a [u8], length: usize) -> Result<&'a [u8], Error> {
    let (value, remaining) = input.split_at_checked(length).ok_or(Error::Checkpoint)?;
    *input = remaining;
    Ok(value)
}

impl<const K: usize, const V: usize, A: Allocator + Clone> Map<K, V, A> {
    /// Recover using one allocator authority for both nodes and bytes.
    ///
    /// # Errors
    /// Uses the same bounds, framing, epoch and allocation checks as
    /// `from_checkpoint_in_allocators`.
    pub fn from_checkpoint_in(
        bytes: &[u8],
        epoch: u64,
        limit: usize,
        allocator: A,
    ) -> Result<Self, Error> {
        Self::from_checkpoint_in_allocators(bytes, epoch, limit, allocator.clone(), allocator)
    }
}
