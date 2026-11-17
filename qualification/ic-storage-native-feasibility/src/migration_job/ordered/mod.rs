//! Ordered payload projection of the existing migration state machine.
use super::{Error, OrderedJob, OrderedRoot, Payload, Phase};
use alloc::vec::Vec;
use ic_storage::ordered::paged::{Manifest, Reader};

impl OrderedJob {
    /// Bind the already stored ordered page chain to the next migration epoch.
    /// The adapter owns controller authorization, frozen source custody and
    /// atomic manifest publication after all pages. This admits only the manifest.
    pub fn seal(
        &self,
        source: u64,
        target: u64,
        epoch: u64,
        manifest: &Manifest,
    ) -> Result<Self, Error> {
        self.export_payload(source, target, epoch, || {
            let bytes = manifest.encode();
            OrderedRoot::decode(&bytes, epoch)?;
            let mut payload = Vec::new();
            payload
                .try_reserve_exact(bytes.len())
                .map_err(|_| Error::Allocation)?;
            payload.extend_from_slice(&bytes);
            Ok(payload)
        })
    }

    /// Reopen a trusted verifier continuation at exactly this job's committed
    /// page offset. The adapter must keep exclusive custody of the private native
    /// candidate and commit its writes, this cursor and job metadata atomically.
    /// This verifies neither native contents nor caller authorization. A completed
    /// retry must acknowledge the batch without replaying writes or resetting data.
    pub fn reader(&self, cursor: &[u8]) -> Result<Reader<512, 4096>, Error> {
        if !matches!(self.phase, Phase::Importing | Phase::Consumed) {
            return Err(Error::State);
        }
        let manifest = OrderedRoot::decode(&self.checkpoint, self.epoch)?;
        let reader = Reader::resume(&manifest, cursor).map_err(Error::OrderedCheckpoint)?;
        if reader.next_page().unwrap_or(manifest.page_count())
            != self.progress.ok_or(Error::State)?.next as usize
        {
            return Err(Error::State);
        }
        Ok(reader)
    }
}

#[cfg(test)]
mod tests;
