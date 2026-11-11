//! Backend-specific production integration; no test-support features.
#[cfg(any(
    feature = "map",
    feature = "set",
    feature = "vector",
    feature = "cell",
    feature = "region"
))]
use ic_storage as storage;

pub(super) struct State {
    #[cfg(feature = "map")]
    map: storage::adapter::StableBTreeMap<u64, u64>,
    #[cfg(feature = "set")]
    set: storage::adapter::StableBTreeSet<u64>,
    #[cfg(feature = "vector")]
    vector: storage::adapter::StableVec<u64>,
    #[cfg(feature = "cell")]
    cell: storage::adapter::StableCell<u64>,
    #[cfg(feature = "region")]
    region: storage::adapter::BTreeMapRegion<u64, u64>,
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
        let open = {
            use storage::*;
            let mut runtime = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
            let names: &[&str] = &[
                #[cfg(feature = "map")]
                "size.map.v1",
                #[cfg(feature = "set")]
                "size.set.v1",
                #[cfg(feature = "vector")]
                "size.vector.v1",
                #[cfg(feature = "cell")]
                "size.cell.v1",
                #[cfg(feature = "region")]
                "size.region.v1",
            ];
            let requests = SealedDeclarationSnapshot::new(
                names
                    .iter()
                    .map(|name| MemoryRequest::new("size", name).unwrap())
                    .collect(),
            )
            .unwrap();
            let grants =
                AllocationGrants::new(vec![MemoryAuthority::new("size", "size.").unwrap()])
                    .unwrap();
            runtime
                .bootstrap(&requests, &grants, &GenericAllocationPolicy)
                .unwrap();
            move |name: &str| {
                runtime
                    .open_memory(&StableKey::parse(name).unwrap())
                    .unwrap()
            }
        };
        #[cfg(feature = "map")]
        let map = {
            let memory = open("size.map.v1");
            if reopen {
                storage::adapter::StableBTreeMap::load(memory).unwrap()
            } else {
                storage::adapter::StableBTreeMap::create(memory).unwrap()
            }
        };
        #[cfg(feature = "set")]
        let set = {
            let memory = open("size.set.v1");
            if reopen {
                storage::adapter::StableBTreeSet::load(memory).unwrap()
            } else {
                storage::adapter::StableBTreeSet::create(memory).unwrap()
            }
        };
        #[cfg(feature = "vector")]
        let vector = {
            let memory = open("size.vector.v1");
            if reopen {
                storage::adapter::StableVec::load(memory).unwrap()
            } else {
                storage::adapter::StableVec::create(memory).unwrap()
            }
        };
        #[cfg(feature = "cell")]
        let cell = {
            let memory = open("size.cell.v1");
            if reopen {
                storage::adapter::StableCell::load(memory, 8).unwrap()
            } else {
                storage::adapter::StableCell::create(memory, 0).unwrap()
            }
        };
        #[cfg(feature = "region")]
        let region = {
            let memory = open("size.region.v1");
            let mut region = storage::adapter::BTreeMapRegion::new(std::rc::Rc::new(memory), 0..64);
            if !reopen {
                region.initialize().unwrap();
            }
            region
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
            output.push(self.region.insert(key, value).unwrap().unwrap_or(u64::MAX));
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
            output.push(self.region.get(&key).unwrap().unwrap_or(u64::MAX));
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
            output.push(self.region.remove(&key).unwrap().unwrap_or(u64::MAX));
        }
        output
    }
    pub(super) fn extrema(&self) -> Vec<u64> {
        #[cfg(feature = "map")]
        {
            let first = self.map.first_key_value().unwrap_or((u64::MAX, u64::MAX));
            let last = self.map.last_key_value().unwrap_or((u64::MAX, u64::MAX));
            vec![first.0, first.1, last.0, last.1]
        }
        #[cfg(not(feature = "map"))]
        {
            Vec::new()
        }
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
            output.push(
                self.region
                    .keys_range_limited(start.., 16)
                    .unwrap()
                    .fold(0_u64, u64::wrapping_add),
            );
        }
        output
    }
}
