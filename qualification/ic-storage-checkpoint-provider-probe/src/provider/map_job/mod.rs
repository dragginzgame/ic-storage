//! Two governed cells; the shared job codec owns framing and cross-part integrity.
use super::memory::StableMemory;
use ic_storage::checkpoint::{Memory, Record};
use ic_storage_native_feasibility::{
    checkpoint::maps,
    migration_job::{Batch, METADATA_BYTES, MapJob, MapPair},
};

const METADATA: u64 = 10;
const CHECKPOINT: u64 = 11;

pub(crate) struct Store<M: Memory = StableMemory> {
    metadata: Record<Vec<u8>, M>,
    checkpoint: Record<Vec<u8>, M>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open(create: bool) -> Store {
    Store::open(StableMemory::default(), create)
}

impl<M: Memory> Store<M> {
    fn open(memory: M, create: bool) -> Self {
        let mut backing = super::backing(memory, &[METADATA, CHECKPOINT], create);
        let mut cell = |key, limit| {
            let memory = backing.take(key).unwrap();
            if create {
                Record::create(memory, Vec::new()).unwrap()
            } else {
                Record::load(memory, limit).unwrap()
            }
        };
        Self {
            metadata: cell(METADATA, METADATA_BYTES),
            checkpoint: cell(CHECKPOINT, maps::MAX_BYTES),
        }
    }
}

impl<M: Memory> Store<M> {
    pub(crate) fn read(&self) -> MapJob {
        MapJob::decode_split(self.metadata.get(), self.checkpoint.get()).unwrap()
    }

    pub(crate) fn batch(
        &self,
        target: u64,
        source: u64,
        epoch: u64,
        start: u32,
        limit: u32,
    ) -> (MapJob, Batch<MapPair>) {
        MapJob::decode_split_batch(
            self.metadata.get(),
            self.checkpoint.get(),
            target,
            source,
            epoch,
            start,
            limit,
        )
        .unwrap()
    }

    // IC message rollback, not the cell write order, provides atomicity across
    // both allocations and the later native publication. A mixed pair refuses.
    pub(crate) fn write(&mut self, job: &MapJob) {
        self.write_with_fault(job, false);
    }

    pub(crate) fn write_with_fault(&mut self, job: &MapJob, after_checkpoint: bool) {
        let (metadata, checkpoint) = job.encode_split().unwrap();
        if self.checkpoint.get() != checkpoint {
            self.checkpoint.set(checkpoint.to_vec()).unwrap();
        }
        assert!(
            !after_checkpoint,
            "map job after checkpoint write, before metadata"
        );
        if self.metadata.get() != &metadata {
            self.metadata.set(metadata.to_vec()).unwrap();
        }
    }

    pub(crate) fn damage(&mut self, part: u64) {
        let cell = match part {
            0 => &mut self.metadata,
            1 => &mut self.checkpoint,
            _ => panic!("unknown stored job part"),
        };
        let mut bytes = cell.get().clone();
        bytes[24] ^= 1;
        cell.set(bytes).unwrap();
    }
}

#[cfg(test)]
mod tests;
