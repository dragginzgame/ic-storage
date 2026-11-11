//! Backend-specific production integration; no test-support features.
#[cfg(any(
    feature = "map",
    feature = "set",
    feature = "vector",
    feature = "cell",
    feature = "region"
))]
use ic_stable_structures as storage;
#[cfg(any(
    feature = "map",
    feature = "set",
    feature = "vector",
    feature = "cell",
    feature = "region"
))]
type Memory = storage::memory_manager::VirtualMemory<storage::DefaultMemoryImpl>;

pub(super) struct State {
    #[cfg(feature = "map")]
    map: storage::BTreeMap<u64, u64, Memory>,
    #[cfg(feature = "set")]
    set: storage::BTreeSet<u64, Memory>,
    #[cfg(feature = "vector")]
    vector: storage::Vec<u64, Memory>,
    #[cfg(feature = "cell")]
    cell: storage::Cell<u64, Memory>,
    #[cfg(feature = "region")]
    region: storage::RestrictedMemory<Memory>,
}
impl State {
    pub(super) fn open(reopen: bool) -> Self {
        let _ = reopen;
        #[cfg(any(
            feature = "map",
            feature = "set",
            feature = "vector",
            feature = "cell",
            feature = "region"
        ))]
        let manager =
            storage::memory_manager::MemoryManager::init(storage::DefaultMemoryImpl::default());
        #[cfg(feature = "map")]
        let map = {
            let memory = manager.get(storage::memory_manager::MemoryId::new(0));
            storage::BTreeMap::init(memory)
        };
        #[cfg(feature = "set")]
        let set = {
            let memory = manager.get(storage::memory_manager::MemoryId::new(1));
            storage::BTreeSet::init(memory)
        };
        #[cfg(feature = "vector")]
        let vector = {
            let memory = manager.get(storage::memory_manager::MemoryId::new(2));
            storage::Vec::init(memory)
        };
        #[cfg(feature = "cell")]
        let cell = {
            let memory = manager.get(storage::memory_manager::MemoryId::new(3));
            storage::Cell::init(memory, 0)
        };
        #[cfg(feature = "region")]
        let region = {
            let memory = manager.get(storage::memory_manager::MemoryId::new(4));
            let memory = storage::RestrictedMemory::new(memory, 0..64);
            if !reopen {
                storage::BTreeMap::<u64, u64, _>::new(memory.clone());
            }
            memory
        };
        Self {
            #[cfg(feature = "map")]
            map,
            #[cfg(feature = "set")]
            set,
            #[cfg(feature = "vector")]
            vector,
            #[cfg(feature = "cell")]
            cell,
            #[cfg(feature = "region")]
            region,
        }
    }
    pub(super) fn put(&mut self, key: u64, value: u64) -> Vec<u64> {
        let _ = (key, value);
        let mut output = Vec::new();
        let _ = &mut output;
        #[cfg(feature = "map")]
        {
            output.push(self.map.insert(key, value).unwrap_or(u64::MAX));
        }
        #[cfg(feature = "set")]
        {
            output.push(u64::from(self.set.insert(key)));
        }
        #[cfg(feature = "vector")]
        {
            output.push({
                self.vector.push(&value);
                self.vector.len()
            });
        }
        #[cfg(feature = "cell")]
        {
            output.push(self.cell.set(value));
        }
        #[cfg(feature = "region")]
        {
            let mut region = storage::BTreeMap::<u64, u64, _>::load(self.region.clone());
            output.push(region.insert(key, value).unwrap_or(u64::MAX));
        }
        output
    }
    pub(super) fn get(&self, key: u64) -> Vec<u64> {
        let _ = key;
        let mut output = Vec::new();
        let _ = &mut output;
        #[cfg(feature = "map")]
        {
            output.push(self.map.get(&key).unwrap_or(u64::MAX));
        }
        #[cfg(feature = "set")]
        {
            output.push(u64::from(self.set.contains(&key)));
        }
        #[cfg(feature = "vector")]
        {
            output.push(self.vector.get(key).unwrap_or(u64::MAX));
        }
        #[cfg(feature = "cell")]
        {
            output.push(*self.cell.get());
        }
        #[cfg(feature = "region")]
        {
            let region = storage::BTreeMap::<u64, u64, _>::load(self.region.clone());
            output.push(region.get(&key).unwrap_or(u64::MAX));
        }
        output
    }
    pub(super) fn remove(&mut self, key: u64) -> Vec<u64> {
        let _ = key;
        let mut output = Vec::new();
        let _ = &mut output;
        #[cfg(feature = "map")]
        {
            output.push(self.map.remove(&key).unwrap_or(u64::MAX));
        }
        #[cfg(feature = "set")]
        {
            output.push(u64::from(self.set.remove(&key)));
        }
        #[cfg(feature = "vector")]
        {
            output.push(self.vector.pop().unwrap_or(u64::MAX));
        }
        #[cfg(feature = "cell")]
        {
            output.push(self.cell.set(0));
        }
        #[cfg(feature = "region")]
        {
            let mut region = storage::BTreeMap::<u64, u64, _>::load(self.region.clone());
            output.push(region.remove(&key).unwrap_or(u64::MAX));
        }
        output
    }
    pub(super) fn scan(&self, start: u64) -> Vec<u64> {
        let _ = start;
        let mut output = Vec::new();
        let _ = &mut output;
        #[cfg(feature = "map")]
        {
            output.push(
                self.map
                    .range(start..)
                    .take(16)
                    .map(|entry| *entry.key() ^ entry.value())
                    .fold(0_u64, u64::wrapping_add),
            );
        }
        #[cfg(feature = "set")]
        {
            output.push(self.set.iter().take(16).fold(0_u64, u64::wrapping_add));
        }
        #[cfg(feature = "vector")]
        {
            output.push(self.vector.iter().take(16).fold(0_u64, u64::wrapping_add));
        }
        #[cfg(feature = "cell")]
        {
            output.push(*self.cell.get());
        }
        #[cfg(feature = "region")]
        {
            let region = storage::BTreeMap::<u64, u64, _>::load(self.region.clone());
            output.push(
                region
                    .keys_range(start..)
                    .take(16)
                    .fold(0_u64, u64::wrapping_add),
            );
        }
        output
    }
}
