//! Native IO/codec attribution. Page sets describe stable byte ranges, not IC faults or fees.
use clap::Parser;
use ic_stable_structures as icss;
use ic_storage::{Memory, MemoryBacking, RuntimeMemory, VectorMemory, testing::MeteredMemory};
use ic_storage_benchmarks::{Operation, Pattern, Request, keys};
use serde_json::{Value as Json, json};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::PathBuf,
    rc::Rc,
};

#[derive(Parser)]
#[command(about = "Attribute native Map/region IO and codecs across Storage and ICSS 0.7.2")]
struct Arguments {
    /// New JSONL output file; existing evidence is never overwritten.
    output: PathBuf,
}

#[derive(Default)]
struct Ranges {
    read: BTreeSet<u64>,
    written: BTreeSet<u64>,
}
impl Ranges {
    fn add(set: &mut BTreeSet<u64>, offset: u64, length: usize) {
        if length != 0 {
            set.extend(offset / 4096..=(offset + length as u64 - 1) / 4096);
        }
    }
    fn report(&self) -> Json {
        let accessed: BTreeSet<_> = self.read.union(&self.written).copied().collect();
        json!({"accessed_4k":accessed.len(), "written_4k":self.written.len(),
            "accessed_64k":accessed.iter().map(|p|p/16).collect::<BTreeSet<_>>().len(),
            "written_64k":self.written.iter().map(|p|p/16).collect::<BTreeSet<_>>().len()})
    }
}

#[derive(Clone, Default)]
struct Backing {
    memory: MeteredMemory<VectorMemory>,
    ranges: Rc<RefCell<Ranges>>,
}
impl Backing {
    fn reset(&self) {
        self.memory.reset();
        *self.ranges.borrow_mut() = Ranges::default();
        CODECS.set([0; 4]);
    }
    fn report(&self) -> Json {
        let c = self.memory.counts();
        assert!(!c.overflowed);
        json!({"reads":c.reads,"read_bytes":c.read_bytes,"writes":c.writes,
            "write_bytes":c.write_bytes,"grows":c.grows,"grown_pages":c.grown_pages,
            "stable_pages":self.memory.size(),"ranges":self.ranges.borrow().report(),
            "key_encodes":CODECS.get()[0],"key_decodes":CODECS.get()[1],
            "value_encodes":CODECS.get()[2],"value_decodes":CODECS.get()[3]})
    }
}
impl Memory for Backing {
    fn size(&self) -> u64 {
        self.memory.size()
    }
    fn grow(&self, n: u64) -> i64 {
        self.memory.grow(n)
    }
    fn read(&self, at: u64, bytes: &mut [u8]) {
        Ranges::add(&mut self.ranges.borrow_mut().read, at, bytes.len());
        self.memory.read(at, bytes);
    }
    fn write(&self, at: u64, bytes: &[u8]) {
        Ranges::add(&mut self.ranges.borrow_mut().written, at, bytes.len());
        self.memory.write(at, bytes);
    }
}
impl MemoryBacking for Backing {
    fn backing_identity(&self) -> ic_storage::BackingIdentity {
        self.memory.backing_identity()
    }
}
#[derive(Clone)]
struct Checked<M>(M);
impl<M: Memory> icss::Memory for Checked<M> {
    fn size(&self) -> u64 {
        self.0.size()
    }
    fn grow(&self, n: u64) -> i64 {
        self.0.grow(n)
    }
    fn read(&self, at: u64, bytes: &mut [u8]) {
        self.0.read(at, bytes);
    }
    fn write(&self, at: u64, bytes: &[u8]) {
        self.0.write(at, bytes);
    }
}

thread_local! { static CODECS: Cell<[u64; 4]> = const { Cell::new([0;4]) }; }
fn codec(index: usize) {
    let mut c = CODECS.get();
    c[index] += 1;
    CODECS.set(c);
}
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Key(u64);
impl ic_storage::adapter::Storable for Key {
    const BOUND: ic_storage::adapter::Bound = ic_storage::adapter::Bound::Bounded {
        max_size: 8,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, ic_storage::adapter::CodecError> {
        codec(0);
        Ok(Cow::Owned(self.0.to_le_bytes().to_vec()))
    }
    fn from_bytes(b: Cow<'_, [u8]>) -> Result<Self, ic_storage::adapter::CodecError> {
        codec(1);
        Ok(Self(u64::from_le_bytes(b.as_ref().try_into().unwrap())))
    }
}
impl icss::Storable for Key {
    const BOUND: icss::storable::Bound = icss::storable::Bound::Bounded {
        max_size: 8,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        <Self as ic_storage::adapter::Storable>::to_bytes(self).unwrap()
    }
    fn into_bytes(self) -> Vec<u8> {
        <Self as icss::Storable>::to_bytes(&self).into_owned()
    }
    fn from_bytes(b: Cow<'_, [u8]>) -> Self {
        <Self as ic_storage::adapter::Storable>::from_bytes(b).unwrap()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Value<const FIXED: bool>(Vec<u8>);
impl<const FIXED: bool> ic_storage::adapter::Storable for Value<FIXED> {
    const BOUND: ic_storage::adapter::Bound = if FIXED {
        ic_storage::adapter::Bound::Bounded {
            max_size: 8,
            is_fixed_size: true,
        }
    } else {
        ic_storage::adapter::Bound::Unbounded
    };
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, ic_storage::adapter::CodecError> {
        codec(2);
        Ok(Cow::Borrowed(&self.0))
    }
    fn from_bytes(b: Cow<'_, [u8]>) -> Result<Self, ic_storage::adapter::CodecError> {
        codec(3);
        Ok(Self(b.into_owned()))
    }
}
impl<const FIXED: bool> icss::Storable for Value<FIXED> {
    const BOUND: icss::storable::Bound = if FIXED {
        icss::storable::Bound::Bounded {
            max_size: 8,
            is_fixed_size: true,
        }
    } else {
        icss::storable::Bound::Unbounded
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        <Self as ic_storage::adapter::Storable>::to_bytes(self).unwrap()
    }
    fn into_bytes(self) -> Vec<u8> {
        codec(2);
        self.0
    }
    fn from_bytes(b: Cow<'_, [u8]>) -> Self {
        <Self as ic_storage::adapter::Storable>::from_bytes(b).unwrap()
    }
}

type Governed = Checked<Rc<RuntimeMemory<Backing>>>;
type Virtual = icss::memory_manager::VirtualMemory<Checked<Backing>>;
enum Store<const F: bool> {
    Storage(ic_storage::adapter::StableBTreeMap<Key, Value<F>, Backing>),
    StorageRegion(ic_storage::adapter::BTreeMapRegion<Key, Value<F>, Backing>),
    Governed(icss::BTreeMap<Key, Value<F>, Governed>),
    GovernedRegion(Governed),
    Icss(icss::BTreeMap<Key, Value<F>, Virtual>),
    IcssRegion(Virtual),
}
impl<const F: bool> Store<F> {
    fn new(backend: &str, region: bool, backing: Backing, bucket: u16) -> Self {
        if backend == "icss-manager" {
            let memory = icss::memory_manager::MemoryManager::init_with_bucket_size(
                Checked(backing),
                bucket,
            )
            .get(icss::memory_manager::MemoryId::new(0));
            let map = icss::BTreeMap::new(memory.clone());
            if region {
                drop(map);
                Self::IcssRegion(memory)
            } else {
                Self::Icss(map)
            }
        } else {
            let runtime = ic_storage::testing::fixture_runtime(
                backing,
                "probe",
                "probe.",
                &["probe.map.v1"],
                ic_storage::StorageConfig::default(),
            )
            .unwrap();
            let memory = runtime
                .open_memory(&ic_storage::StableKey::parse("probe.map.v1").unwrap())
                .unwrap();
            match (backend, region) {
                ("storage", false) => {
                    Self::Storage(ic_storage::adapter::StableBTreeMap::create(memory).unwrap())
                }
                ("storage", true) => {
                    let mut map =
                        ic_storage::adapter::BTreeMapRegion::new(Rc::new(memory), 0..4_194_304);
                    map.initialize().unwrap();
                    Self::StorageRegion(map)
                }
                ("icss-governed", _) => {
                    let memory = Checked(Rc::new(memory));
                    let map = icss::BTreeMap::new(memory.clone());
                    if region {
                        drop(map);
                        Self::GovernedRegion(memory)
                    } else {
                        Self::Governed(map)
                    }
                }
                _ => unreachable!(),
            }
        }
    }
    fn get(&self, key: u64) -> Option<Value<F>> {
        let k = Key(key);
        match self {
            Self::Storage(m) => m.get(&k),
            Self::StorageRegion(m) => m.get(&k).unwrap(),
            Self::Governed(m) => m.get(&k),
            Self::Icss(m) => m.get(&k),
            Self::GovernedRegion(m) => icss::BTreeMap::load(m.clone()).get(&k),
            Self::IcssRegion(m) => icss::BTreeMap::load(m.clone()).get(&k),
        }
    }
    fn put(&mut self, key: u64, value: Value<F>) -> Option<Value<F>> {
        let k = Key(key);
        match self {
            Self::Storage(m) => m.insert(k, value),
            Self::StorageRegion(m) => m.insert(k, value).unwrap(),
            Self::Governed(m) => m.insert(k, value),
            Self::Icss(m) => m.insert(k, value),
            Self::GovernedRegion(m) => icss::BTreeMap::load(m.clone()).insert(k, value),
            Self::IcssRegion(m) => icss::BTreeMap::load(m.clone()).insert(k, value),
        }
    }
    fn remove(&mut self, key: u64) -> Option<Value<F>> {
        let k = Key(key);
        match self {
            Self::Storage(m) => m.remove(&k),
            Self::StorageRegion(m) => m.remove(&k).unwrap(),
            Self::Governed(m) => m.remove(&k),
            Self::Icss(m) => m.remove(&k),
            Self::GovernedRegion(m) => icss::BTreeMap::load(m.clone()).remove(&k),
            Self::IcssRegion(m) => icss::BTreeMap::load(m.clone()).remove(&k),
        }
    }
}
fn measure<const F: bool>(output: &mut fs::File, payload: usize) {
    for rows in [256, 4096] {
        for region in [false, true] {
            for pattern in [
                Pattern::Uniform,
                Pattern::Hot16,
                Pattern::Sequential,
                Pattern::First,
                Pattern::Last,
            ] {
                for count in [1, 32] {
                    for backend in ["storage", "icss-governed", "icss-manager"] {
                        let backing = Backing::default();
                        let mut store = Store::<F>::new(backend, region, backing.clone(), 8);
                        let mut model = BTreeMap::new();
                        for id in 0..u64::from(rows) {
                            let v = Value(ic_storage_benchmarks::value(id, payload as u32, false));
                            assert!(store.put(id, v.clone()).is_none());
                            model.insert(id, v);
                        }
                        let selected = keys(Request {
                            case: ic_storage_benchmarks::case!(Map, U64, U64),
                            operation: Operation::Get,
                            rows,
                            payload: 8,
                            count,
                            seed: 1,
                            pattern,
                        });
                        for phase in [
                            "Get",
                            "MissAbove",
                            "Replace",
                            "Grow",
                            "GetAfterGrow",
                            "Shrink",
                            "GetAfterShrink",
                            "Remove",
                            "GetAfterRemove",
                            "RemoveMiss",
                            "Reinsert",
                            "GetAfterReinsert",
                        ] {
                            if F && matches!(
                                phase,
                                "Grow" | "GetAfterGrow" | "Shrink" | "GetAfterShrink"
                            ) {
                                continue;
                            }
                            // Prepare inputs and expected results before the observation window.
                            let inputs: Vec<_> = selected
                                .iter()
                                .map(|&id| {
                                    let size = match phase {
                                        "Grow" => payload * 4,
                                        "Shrink" => 0,
                                        _ => payload,
                                    };
                                    (
                                        if phase == "MissAbove" {
                                            id + u64::from(rows)
                                        } else {
                                            id
                                        },
                                        Value(ic_storage_benchmarks::value(id, size as u32, true)),
                                    )
                                })
                                .collect();
                            backing.reset();
                            let actual: Vec<_> = inputs
                                .iter()
                                .map(|(id, v)| {
                                    if matches!(phase, "Replace" | "Grow" | "Shrink" | "Reinsert") {
                                        store.put(*id, v.clone())
                                    } else if matches!(phase, "Remove" | "RemoveMiss") {
                                        store.remove(*id)
                                    } else {
                                        store.get(*id)
                                    }
                                })
                                .collect();
                            let observation = backing.report();
                            for ((id, v), actual) in inputs.into_iter().zip(actual) {
                                let expected = if matches!(
                                    phase,
                                    "Replace" | "Grow" | "Shrink" | "Reinsert"
                                ) {
                                    model.insert(id, v)
                                } else if matches!(phase, "Remove" | "RemoveMiss") {
                                    model.remove(&id)
                                } else {
                                    model.get(&id).cloned()
                                };
                                assert_eq!(actual, expected);
                            }
                            writeln!(output,"{}",json!({"backend":backend,"collection":if region {"Region"} else {"Map"},"rows":rows,"payload":payload,"fixed":F,"batch":count,"pattern":pattern,"phase":phase,"bucket_pages":8,"counts":observation})).unwrap();
                        }
                        for (id, expected) in model {
                            assert_eq!(store.get(id), Some(expected));
                        }
                    }
                }
            }
        }
    }
}
fn main() {
    let args = Arguments::parse();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.output)
        .unwrap();
    measure::<true>(&mut output, 8);
    measure::<false>(&mut output, 32);
    measure::<false>(&mut output, 4096);
}
