//! Two governed cells; the shared job codec owns framing and cross-part integrity.
use ic_storage_legacy::{DefaultMemoryImpl, Memory, MemoryBacking, StableKey, adapter::StableCell};
use ic_storage_native_feasibility::{
    checkpoint::maps,
    migration_job::{METADATA_BYTES, MapJob},
};

const METADATA: &str = "native.checkpoint.maps.job.metadata.v1";
const CHECKPOINT: &str = "native.checkpoint.maps.job.checkpoint.v1";

pub(crate) struct Store<M: Memory = DefaultMemoryImpl> {
    metadata: StableCell<Vec<u8>, M>,
    checkpoint: StableCell<Vec<u8>, M>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open(create: bool) -> Store {
    Store::open(DefaultMemoryImpl::default(), create)
}

impl<M: MemoryBacking> Store<M> {
    fn open(memory: M, create: bool) -> Self {
        let runtime = super::runtime(memory, &[METADATA, CHECKPOINT], create);
        let cell = |key, limit| {
            let memory = runtime
                .open_memory(&StableKey::parse(key).unwrap())
                .unwrap();
            if create {
                StableCell::create(memory, Vec::new()).unwrap()
            } else {
                StableCell::load(memory, limit).unwrap()
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

    // IC message rollback, not the cell write order, provides atomicity across
    // both allocations and the later native publication. A mixed pair refuses.
    pub(crate) fn write(&mut self, job: &MapJob) {
        self.write_with_fault(job, false);
    }

    pub(crate) fn write_with_fault(&mut self, job: &MapJob, after_checkpoint: bool) {
        let (metadata, checkpoint) = job.encode_split().unwrap();
        if self.checkpoint.get() != checkpoint {
            self.checkpoint.set(checkpoint.to_vec());
        }
        assert!(
            !after_checkpoint,
            "map job after checkpoint write, before metadata"
        );
        if self.metadata.get() != &metadata {
            self.metadata.set(metadata.to_vec());
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
        cell.set(bytes);
    }
}

#[cfg(test)]
mod tests;
