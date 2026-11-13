//! Concrete non-map workloads. Dispatch and input construction precede timing.
#[expect(
    clippy::wildcard_imports,
    reason = "Private canister modules share parent bindings selected by the benchmark backend features."
)]
use super::*;
#[cfg(feature = "owned")]
type Set<K> = ic_storage::adapter::StableBTreeSet<K>;
#[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
type Set<K> = ic_storage_benchmarks::upstream::BTreeSet<K, BenchMemory>;
#[cfg(feature = "owned")]
type Vector<V> = ic_storage::adapter::StableVec<V>;
#[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
type Vector<V> = ic_storage_benchmarks::upstream::Vec<V, BenchMemory>;
#[cfg(feature = "owned")]
type StableCell<V> = ic_storage::adapter::StableCell<V>;
#[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
type StableCell<V> = ic_storage_benchmarks::upstream::Cell<V, BenchMemory>;

pub(super) struct TypedSet<K: BenchKey + Storable> {
    case: Case,
    set: Set<K>,
}
impl<K: BenchKey + Storable> TypedSet<K> {
    pub(super) fn new(case: Case, bucket_pages: std::num::NonZeroU16) -> Self {
        #[cfg(feature = "owned")]
        let set = Set::create(new_memory(bucket_pages)).unwrap();
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        let set = Set::new(new_memory(bucket_pages));
        Self { case, set }
    }
}
impl<K: BenchKey + Storable> Benchmark for TypedSet<K> {
    fn setup(&mut self, r: Setup) -> u64 {
        for id in population_ids(&r) {
            assert!(self.set.insert(K::from_id(id)));
        }
        self.set.len()
    }
    fn verify(&self, r: Setup) -> Vec<(u64, Vec<u8>)> {
        let iter = self.set.iter();
        iter.filter(|k| k.id() >= r.start)
            .take(r.count as usize)
            .map(|k| (k.id(), vec![]))
            .collect()
    }
    fn run(&mut self, r: Request) -> Response {
        assert_eq!(r.case, self.case);
        let inputs: Vec<_> = keys(r).into_iter().map(|id| (K::from_id(id), id)).collect();
        let mut results = Vec::with_capacity(r.count as usize);
        let mut scan = Vec::with_capacity(r.count as usize);
        let start = ic0::performance_counter(0);
        if r.operation == Operation::IterLast {
            for _ in 0..r.count {
                if let Some(key) = self.set.iter().last() {
                    scan.push(key);
                }
            }
        } else if r.operation == Operation::Scan {
            for k in self.set.iter().take(r.count as usize) {
                scan.push(k);
            }
        } else if r.operation == Operation::Nth {
            for (_, index) in inputs {
                scan.push(
                    self.set
                        .iter()
                        .nth(index as usize)
                        .expect("admitted ordered index"),
                );
            }
        } else {
            for (k, id) in inputs {
                let result = match r.operation {
                    Operation::Control => {
                        std::hint::black_box(&k);
                        None
                    }
                    Operation::Get | Operation::MissAbove => Some(self.set.contains(&k)),
                    Operation::Insert | Operation::Reinsert => Some(self.set.insert(k)),
                    Operation::Remove | Operation::RemoveMiss | Operation::RemoveInserted => {
                        Some(self.set.remove(&k))
                    }
                    _ => unreachable!(),
                };
                results.push((id, result));
            }
        }
        let instructions = ic0::performance_counter(0) - start;
        let entries = if matches!(
            r.operation,
            Operation::Scan | Operation::Nth | Operation::IterLast
        ) {
            scan.into_iter().map(|k| (k.id(), Some(vec![]))).collect()
        } else {
            results
                .into_iter()
                .map(|(id, v)| (id, v.map(|v| vec![u8::from(v)])))
                .collect()
        };
        Response {
            decode: None,
            instructions,
            entries,
            length: self.set.len(),
            stable_pages: ic0::stable64_size(),
        }
    }
}

pub(super) struct TypedVector<V: BenchValue + Storable> {
    pub(super) case: Case,
    pub(super) vector: Vector<V>,
}
impl<V: BenchValue + Storable> TypedVector<V> {
    pub(super) fn new(case: Case, bucket_pages: std::num::NonZeroU16) -> Self {
        #[cfg(feature = "owned")]
        let vector = Vector::create(new_memory(bucket_pages)).unwrap();
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        let vector = Vector::new(new_memory(bucket_pages));
        Self { case, vector }
    }
}
impl<V: BenchValue + Storable> Benchmark for TypedVector<V> {
    fn setup(&mut self, r: Setup) -> u64 {
        for id in r.start..r.start + u64::from(r.count) {
            self.vector.push(&V::make(id, r.payload, false));
        }
        self.vector.len()
    }
    fn verify(&self, r: Setup) -> Vec<(u64, Vec<u8>)> {
        (r.start..self.vector.len().min(r.start + u64::from(r.count)))
            .map(|i| (i, self.vector.get(i).unwrap().canonical()))
            .collect()
    }
    fn run(&mut self, r: Request) -> Response {
        assert_eq!(r.case, self.case);
        let len = self.vector.len();
        let inputs: Vec<_> = keys(r)
            .into_iter()
            .enumerate()
            .map(|(i, id)| {
                let id = if r.operation == Operation::Push {
                    len + i as u64
                } else if r.operation == Operation::Last {
                    len.saturating_sub(1)
                } else if r.operation == Operation::NthBack {
                    len - 1 - id
                } else if r.operation == Operation::Pop {
                    len.saturating_sub(i as u64 + 1)
                } else {
                    id
                };
                (
                    id,
                    if matches!(r.operation, Operation::Set | Operation::Push) {
                        Some(V::make(id, r.payload, replacement(r.operation)))
                    } else {
                        None
                    },
                )
            })
            .collect();
        let changes: Vec<_> = if r.operation == Operation::ScanSet {
            (1..u64::from(r.count).min(len))
                .map(|id| V::make(id, r.payload, true))
                .collect()
        } else {
            Vec::new()
        };
        let encoded: Vec<_> = if r.operation == Operation::Decode {
            inputs
                .iter()
                .map(|(id, _)| {
                    let value = V::make(*id, r.payload, false);
                    codec!(value.to_bytes()).into_owned()
                })
                .collect()
        } else {
            Vec::new()
        };
        if r.operation == Operation::WarmGet {
            for (id, _) in &inputs {
                std::hint::black_box(self.vector.get(*id));
            }
        }
        if r.case.measures_decode(r.operation) {
            reset_decode_metrics();
        }
        let mut entries = Vec::with_capacity(r.count as usize);
        let start = ic0::performance_counter(0);
        if r.operation == Operation::Scan {
            for (i, v) in self.vector.iter().take(r.count as usize).enumerate() {
                entries.push((i as u64, Some(v)));
            }
        } else if r.operation == Operation::ReverseScan {
            for (i, v) in self.vector.iter().rev().take(r.count as usize).enumerate() {
                entries.push((len - 1 - i as u64, Some(v)));
            }
        } else if r.operation == Operation::AlternatingScan {
            let mut iter = self.vector.iter();
            for step in 0..u64::from(r.count).min(len) {
                let (id, value) = if step % 2 == 0 {
                    (step / 2, iter.next())
                } else {
                    (len - 1 - step / 2, iter.next_back())
                };
                entries.push((id, Some(value.unwrap())));
            }
        } else if r.operation == Operation::ScanSet {
            let mut iter = self.vector.iter();
            for id in 0..u64::from(r.count).min(len) {
                if id != 0 {
                    self.vector.set(id, &changes[(id - 1) as usize]);
                }
                entries.push((id, Some(iter.next().unwrap())));
            }
        } else {
            for (at, (id, value)) in inputs.into_iter().enumerate() {
                let result = match r.operation {
                    Operation::Control => {
                        std::hint::black_box(id);
                        None
                    }
                    Operation::Get | Operation::WarmGet | Operation::MissAbove => {
                        self.vector.get(id)
                    }
                    Operation::Decode => Some(codec!(V::from_bytes(std::borrow::Cow::Borrowed(
                        &encoded[at]
                    )))),
                    Operation::Nth => Some(
                        self.vector
                            .iter()
                            .nth(id as usize)
                            .expect("admitted vector index"),
                    ),
                    Operation::NthBack => Some(
                        self.vector
                            .iter()
                            .nth_back((len - 1 - id) as usize)
                            .expect("admitted reverse vector index"),
                    ),
                    Operation::Last => Some(self.vector.iter().last().expect("populated vector")),
                    Operation::Set => {
                        self.vector.set(id, &value.unwrap());
                        None
                    }
                    Operation::Push => {
                        self.vector.push(&value.unwrap());
                        None
                    }
                    Operation::Pop => self.vector.pop(),
                    _ => unreachable!(),
                };
                entries.push((id, result));
            }
        }
        let instructions = ic0::performance_counter(0) - start;
        Response {
            decode: (r.case.measures_decode(r.operation)).then(decode_metrics),
            instructions,
            entries: entries
                .into_iter()
                .map(|(id, v)| (id, v.map(V::canonical)))
                .collect(),
            length: self.vector.len(),
            stable_pages: ic0::stable64_size(),
        }
    }
}

pub(super) struct TypedCell<V: BenchValue + Storable> {
    pub(super) case: Case,
    pub(super) cell: Option<StableCell<V>>,
    pub(super) bucket_pages: std::num::NonZeroU16,
}
impl<V: BenchValue + Storable> TypedCell<V> {
    pub(super) fn new(case: Case, bucket_pages: std::num::NonZeroU16) -> Self {
        Self {
            case,
            cell: None,
            bucket_pages,
        }
    }
}
impl<V: BenchValue + Storable> Benchmark for TypedCell<V> {
    fn setup(&mut self, r: Setup) -> u64 {
        assert!(self.cell.is_none() && r.start == 0 && r.count == 1);
        let value = V::make(0, r.payload, false);
        #[cfg(feature = "owned")]
        let cell = StableCell::create(new_memory(self.bucket_pages), value).unwrap();
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        let cell = StableCell::new(new_memory(self.bucket_pages), value);
        self.cell = Some(cell);
        1
    }
    fn verify(&self, _: Setup) -> Vec<(u64, Vec<u8>)> {
        vec![(0, self.cell.as_ref().unwrap().get().clone().canonical())]
    }
    fn run(&mut self, r: Request) -> Response {
        assert_eq!(r.case, self.case);
        let cell = self.cell.as_mut().unwrap();
        // Read phases have no replacement input. Iterating over Option<V> here
        // can copy the full inline V even for None, obscuring cached-read costs.
        let inputs: Vec<_> = if replacement(r.operation) {
            (0..r.count)
                .map(|_| V::make(0, payload_for(r), true))
                .collect()
        } else {
            Vec::new()
        };
        let mut inputs = inputs.into_iter();
        let mut entries = Vec::with_capacity(r.count as usize);
        let start = ic0::performance_counter(0);
        for _ in 0..r.count {
            let result = match r.operation {
                Operation::Control => {
                    std::hint::black_box(cell.get());
                    None
                }
                Operation::Get => Some(cell.get().clone()),
                Operation::Replace | Operation::Grow | Operation::Shrink => {
                    Some(cell.set(inputs.next().unwrap()))
                }
                _ => unreachable!(),
            };
            entries.push((0, result));
        }
        let instructions = ic0::performance_counter(0) - start;
        Response {
            decode: None,
            instructions,
            entries: entries
                .into_iter()
                .map(|(id, v)| (id, v.map(V::canonical)))
                .collect(),
            length: 1,
            stable_pages: ic0::stable64_size(),
        }
    }
}

// Upstream has no BTreeMapRegion: this control recreates a map view from the
// same virtual memory for each operation, matching Storage's root-load lifetime.
pub(super) struct Region<K: BenchKey + Storable, V: BenchValue + Storable> {
    case: Case,
    record: std::marker::PhantomData<(K, V)>,
    #[cfg(feature = "owned")]
    map: ic_storage::adapter::BTreeMapRegion<K, V>,
    #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
    memory: BenchMemory,
}
impl<K: BenchKey + Storable, V: BenchValue + Storable> Region<K, V> {
    pub(super) fn new(case: Case, bucket_pages: std::num::NonZeroU16) -> Self {
        #[cfg(feature = "owned")]
        {
            let mut map = ic_storage::adapter::BTreeMapRegion::new(
                std::rc::Rc::new(new_memory(bucket_pages)),
                0..4_194_304,
            );
            map.initialize().unwrap();
            Self {
                case,
                record: std::marker::PhantomData,
                map,
            }
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        {
            let memory = new_memory(bucket_pages);
            drop(Map::<K, V>::new(memory.clone()));
            Self {
                case,
                record: std::marker::PhantomData,
                memory,
            }
        }
    }
    fn get(&self, key: &K) -> Option<V> {
        #[cfg(feature = "owned")]
        {
            self.map.get(key).unwrap()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        {
            Map::<K, V>::load(self.memory.clone()).get(key)
        }
    }
    fn insert(&mut self, key: K, value: V) -> Option<V> {
        #[cfg(feature = "owned")]
        {
            self.map.insert(key, value).unwrap()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        {
            Map::<K, V>::load(self.memory.clone()).insert(key, value)
        }
    }
    fn remove(&mut self, key: &K) -> Option<V> {
        #[cfg(feature = "owned")]
        {
            self.map.remove(key).unwrap()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        {
            Map::<K, V>::load(self.memory.clone()).remove(key)
        }
    }
    fn insert_discard(&mut self, key: K, value: V) -> bool {
        #[cfg(all(feature = "owned", not(feature = "returning-mutations")))]
        {
            self.map.insert_discard(key, value).unwrap()
        }
        #[cfg(any(
            feature = "dfinity",
            feature = "dfinity-head",
            feature = "returning-mutations"
        ))]
        {
            self.insert(key, value).is_some()
        }
    }
    fn remove_discard(&mut self, key: &K) -> bool {
        #[cfg(all(feature = "owned", not(feature = "returning-mutations")))]
        {
            self.map.remove_discard(key).unwrap()
        }
        #[cfg(any(
            feature = "dfinity",
            feature = "dfinity-head",
            feature = "returning-mutations"
        ))]
        {
            self.remove(key).is_some()
        }
    }
    fn len(&self) -> u64 {
        #[cfg(feature = "owned")]
        {
            self.map.len().unwrap()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        {
            Map::<K, V>::load(self.memory.clone()).len()
        }
    }
    fn keys(&self, start: K, count: usize) -> Vec<K> {
        #[cfg(feature = "owned")]
        {
            self.map
                .keys_range_limited(start.., count)
                .unwrap()
                .collect()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        {
            Map::<K, V>::load(self.memory.clone())
                .keys_range(start..)
                .take(count)
                .collect()
        }
    }
}
impl<K: BenchKey + Storable, V: BenchValue + Storable> Benchmark for Region<K, V> {
    fn setup(&mut self, r: Setup) -> u64 {
        for id in population_ids(&r) {
            assert!(
                self.insert(K::from_id(id), V::make(id, r.payload, false))
                    .is_none()
            );
        }
        self.len()
    }
    fn verify(&self, r: Setup) -> Vec<(u64, Vec<u8>)> {
        self.keys(K::from_id(r.start), r.count as usize)
            .into_iter()
            .map(|k| (k.id(), self.get(&k).unwrap().canonical()))
            .collect()
    }
    fn run(&mut self, r: Request) -> Response {
        assert_eq!(r.case, self.case);
        let ids = keys(r);
        let scan_start = K::from_id(ids[0]);
        let inputs: Vec<_> = ids
            .into_iter()
            .map(|id| {
                (
                    K::from_id(id),
                    if matches!(r.operation, Operation::Insert | Operation::Reinsert)
                        || replacement(r.operation)
                    {
                        Some(V::make(id, payload_for(r), replacement(r.operation)))
                    } else {
                        None
                    },
                )
            })
            .collect();
        let mut entries = Vec::with_capacity(r.count as usize);
        let mut key_entries = None;
        let mut booleans = Vec::with_capacity(r.count as usize);
        let start = ic0::performance_counter(0);
        if r.operation == Operation::KeysScan {
            key_entries = Some(self.keys(scan_start, r.count as usize));
        } else {
            for (key, value) in inputs {
                let result = match r.operation {
                    Operation::ReplaceDiscard => {
                        let result = self.insert_discard(key.clone(), value.unwrap());
                        booleans.push((key, result));
                        continue;
                    }
                    Operation::RemoveDiscard => {
                        let result = self.remove_discard(&key);
                        booleans.push((key, result));
                        continue;
                    }
                    Operation::Control => {
                        std::hint::black_box(&key);
                        None
                    }
                    Operation::Get | Operation::MissAbove => self.get(&key),
                    Operation::Insert
                    | Operation::Replace
                    | Operation::Reinsert
                    | Operation::Grow
                    | Operation::Shrink => self.insert(key.clone(), value.unwrap()),
                    Operation::Remove | Operation::RemoveMiss | Operation::RemoveInserted => {
                        self.remove(&key)
                    }
                    _ => unreachable!(),
                };
                entries.push((key, result));
            }
        }
        let instructions = ic0::performance_counter(0) - start;
        Response {
            decode: None,
            instructions,
            entries: if matches!(
                r.operation,
                Operation::ReplaceDiscard | Operation::RemoveDiscard
            ) {
                booleans
                    .into_iter()
                    .map(|(key, result)| (key.id(), Some(vec![u8::from(result)])))
                    .collect()
            } else if let Some(keys) = key_entries {
                keys.into_iter()
                    .map(|key| (key.id(), Some(Vec::new())))
                    .collect()
            } else {
                entries
                    .into_iter()
                    .map(|(key, value)| (key.id(), value.map(BenchValue::canonical)))
                    .collect()
            },
            length: self.len(),
            stable_pages: ic0::stable64_size(),
        }
    }
}
