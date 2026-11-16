//! Version-2 progress is meaningful only with the adapter's atomic native writes.
use super::{Error, Job, Phase, checkpoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Progress {
    pub next: u32,
    pub start: u32,
    pub limit: u32,
}

impl Progress {
    pub(super) fn decode(bytes: &[u8], phase: Phase, total: u32) -> Result<Option<Self>, Error> {
        if bytes[8] == 1 {
            return if bytes[44..56] == [0; 12] && phase != Phase::Importing {
                Ok(None)
            } else {
                Err(Error::Framing)
            };
        }
        if !matches!(phase, Phase::Importing | Phase::Consumed) {
            return Err(Error::State);
        }
        let word = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let progress = Self {
            next: word(44),
            start: word(48),
            limit: word(52),
        };
        let initial = progress
            == Self {
                next: 0,
                start: 0,
                limit: 0,
            };
        let advanced = (1..=64).contains(&progress.limit)
            && progress.start <= total
            && progress.next == progress.start.saturating_add(progress.limit).min(total)
            && (progress.start < total || total == 0);
        if (phase == Phase::Importing && (initial || (advanced && progress.next < total)))
            || (phase == Phase::Consumed && advanced && progress.next == total)
        {
            Ok(Some(progress))
        } else {
            Err(Error::State)
        }
    }
}

/// Bounded prefix transition; the adapter validates the old prefix and publishes
/// native writes together with the new envelope in a single IC message.
pub struct Batch {
    /// Logical checkpoint, still bounded to the qualification fixture's 512 values.
    pub checkpoint: checkpoint::Checkpoint,
    /// Prefix length before a new batch (or before the acknowledged retry).
    pub start: u32,
    /// Prefix length after this batch.
    pub end: u32,
    /// Exact latest request was already committed; do not write values again.
    pub retry: bool,
    /// The batch publishes the complete native root.
    pub complete: bool,
}

impl Job {
    /// Begin explicit fresh-target construction; no source cancellation afterward.
    pub fn begin_import(
        &mut self,
        target: u64,
        source: u64,
        epoch: u64,
    ) -> Result<checkpoint::Checkpoint, Error> {
        let decoded = self.frozen(source, target, epoch)?;
        self.phase = Phase::Importing;
        self.progress = Some(Progress {
            next: 0,
            start: 0,
            limit: 0,
        });
        Ok(decoded)
    }

    fn import_binding(&self, target: u64, source: u64, epoch: u64) -> Result<(), Error> {
        if self.target != target || self.source != source {
            return Err(Error::Domain);
        }
        if self.epoch != epoch {
            return Err(Error::Epoch);
        }
        Ok(())
    }

    /// Restart only an unfinished import from its independent checkpoint.
    /// The adapter must discard the previous native heap explicitly and publish
    /// a fresh empty prefix atomically with this reset. No old prefix is read.
    /// Completed, cancelled and sealed jobs cannot use this recovery path.
    pub fn restart_import(
        &mut self,
        target: u64,
        source: u64,
        epoch: u64,
    ) -> Result<checkpoint::Checkpoint, Error> {
        let (decoded, _) = self.importing(target, source, epoch)?;
        self.progress = Some(Progress {
            next: 0,
            start: 0,
            limit: 0,
        });
        Ok(decoded)
    }

    /// Read durable importing state for complete native-prefix validation.
    pub fn importing(
        &self,
        target: u64,
        source: u64,
        epoch: u64,
    ) -> Result<(checkpoint::Checkpoint, u32), Error> {
        self.import_binding(target, source, epoch)?;
        if self.phase != Phase::Importing {
            return Err(Error::State);
        }
        let progress = self.progress.ok_or(Error::State)?;
        Ok((
            checkpoint::decode(&self.checkpoint, epoch).map_err(Error::Checkpoint)?,
            progress.next,
        ))
    }

    /// Advance at most 64 elements. Only the exact latest (offset, limit) request
    /// is retryable; older or changed requests refuse without changing this job.
    pub fn batch(
        &mut self,
        target: u64,
        source: u64,
        epoch: u64,
        start: u32,
        limit: u32,
    ) -> Result<Batch, Error> {
        self.import_binding(target, source, epoch)?;
        if !matches!(self.phase, Phase::Importing | Phase::Consumed) || !(1..=64).contains(&limit) {
            return Err(Error::State);
        }
        let progress = self.progress.ok_or(Error::State)?;
        let decoded = checkpoint::decode(&self.checkpoint, epoch).map_err(Error::Checkpoint)?;
        let total = decoded.values.len() as u32;
        let retry = progress.limit != 0 && start == progress.start && limit == progress.limit;
        if !retry && (self.phase != Phase::Importing || start != progress.next) {
            return Err(Error::State);
        }
        let end = start.checked_add(limit).ok_or(Error::State)?.min(total);
        let complete = end == total;
        if !retry {
            self.progress = Some(Progress {
                next: end,
                start,
                limit,
            });
            if complete {
                self.phase = Phase::Consumed;
            }
        }
        Ok(Batch {
            checkpoint: decoded,
            start,
            end,
            retry,
            complete,
        })
    }
}
