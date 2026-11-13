//! Native capacity/IO experiment over one backing; never an IC instruction benchmark.
use clap::Parser;
use ic_stable_structures as icss;
use ic_storage::{Memory, MemoryBacking, MemoryRuntime, VectorMemory, testing::MeteredMemory};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
mod allocation_histories;

#[derive(Parser)]
#[command(about = "Measure shared-backing capacity and IO; does not measure IC instructions")]
struct Arguments {
    /// Write the JSON report to a new file, preserving existing reports.
    output: PathBuf,
    /// Measure small-record boundaries and reuse instead of the large mixed study.
    #[arg(long)]
    small_records: bool,
    /// Compare fixed, growing and mixed-size histories over shared backing.
    #[arg(long, conflicts_with = "small_records")]
    allocation_histories: bool,
}

#[derive(Clone, Default)]
struct Backing(MeteredMemory<VectorMemory>);
impl Memory for Backing {
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
impl MemoryBacking for Backing {
    fn backing_identity(&self) -> ic_storage::BackingIdentity {
        self.0.backing_identity()
    }
}
impl icss::Memory for Backing {
    fn size(&self) -> u64 {
        Memory::size(self)
    }
    fn grow(&self, n: u64) -> i64 {
        Memory::grow(self, n)
    }
    fn read(&self, at: u64, bytes: &mut [u8]) {
        Memory::read(self, at, bytes);
    }
    fn write(&self, at: u64, bytes: &[u8]) {
        Memory::write(self, at, bytes);
    }
}
trait Stores {
    fn init(&mut self);
    fn get(&self, store: usize, key: u64) -> Option<Vec<u8>>;
    fn put(&mut self, store: usize, key: u64, value: Vec<u8>) -> Option<Vec<u8>>;
    fn remove(&mut self, store: usize, key: u64) -> Option<Vec<u8>>;
    fn stats(&self) -> Value;
}
struct Owned {
    runtime: MemoryRuntime<Backing>,
    maps: Vec<ic_storage::adapter::StableBTreeMap<u64, Vec<u8>, Backing>>,
    count: usize,
}
impl Owned {
    fn new(backing: Backing, count: usize) -> Self {
        use ic_storage::{
            AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
            MemoryRuntime, SealedDeclarationSnapshot,
        };
        let mut runtime = MemoryRuntime::new(backing).unwrap();
        let declarations = SealedDeclarationSnapshot::new(
            (0..count)
                .map(|i| {
                    MemoryRequest::new("experiment", &format!("experiment.store{i:03}.v1")).unwrap()
                })
                .collect(),
        )
        .unwrap();
        runtime
            .bootstrap(
                &declarations,
                &AllocationGrants::new(vec![
                    MemoryAuthority::new("experiment", "experiment.").unwrap(),
                ])
                .unwrap(),
                &GenericAllocationPolicy,
            )
            .unwrap();
        Self {
            runtime,
            maps: vec![],
            count,
        }
    }
}
impl Stores for Owned {
    fn init(&mut self) {
        for i in 0..self.count {
            let memory = self
                .runtime
                .open_memory(
                    &ic_storage::StableKey::parse(format!("experiment.store{i:03}.v1")).unwrap(),
                )
                .unwrap();
            self.maps
                .push(ic_storage::adapter::StableBTreeMap::create(memory).unwrap());
        }
    }
    fn get(&self, s: usize, k: u64) -> Option<Vec<u8>> {
        self.maps[s].get(&k)
    }
    fn put(&mut self, s: usize, k: u64, v: Vec<u8>) -> Option<Vec<u8>> {
        self.maps[s].insert(k, v)
    }
    fn remove(&mut self, s: usize, k: u64) -> Option<Vec<u8>> {
        self.maps[s].remove(&k)
    }
    fn stats(&self) -> Value {
        json!({"directory":self.runtime.diagnostics().unwrap(), "allocations":self.runtime.memory_allocations().unwrap()})
    }
}
type Manager = icss::memory_manager::MemoryManager<Backing>;
struct Icss {
    manager: Manager,
    maps: Vec<icss::BTreeMap<u64, Vec<u8>, icss::memory_manager::VirtualMemory<Backing>>>,
    count: usize,
}
impl Stores for Icss {
    fn init(&mut self) {
        for i in 0..self.count {
            self.maps.push(icss::BTreeMap::init(
                self.manager
                    .get(icss::memory_manager::MemoryId::new(i as u8)),
            ));
        }
    }
    fn get(&self, s: usize, k: u64) -> Option<Vec<u8>> {
        self.maps[s].get(&k)
    }
    fn put(&mut self, s: usize, k: u64, v: Vec<u8>) -> Option<Vec<u8>> {
        self.maps[s].insert(k, v)
    }
    fn remove(&mut self, s: usize, k: u64) -> Option<Vec<u8>> {
        self.maps[s].remove(&k)
    }
    fn stats(&self) -> Value {
        let pages: Vec<_> = (0..self.count)
            .map(|i| {
                icss::Memory::size(
                    &self
                        .manager
                        .get(icss::memory_manager::MemoryId::new(i as u8)),
                )
            })
            .collect();
        json!({"allocation_pages":pages,"logical_pages":pages.iter().sum::<u64>(),"manager_metadata_pages":1})
    }
}
fn value(store: usize, key: u64) -> Vec<u8> {
    let size = if store.is_multiple_of(8) { 65_536 } else { 256 };
    (0..size)
        .map(|i| (i as u8).wrapping_add(store as u8).wrapping_add(key as u8))
        .collect()
}
fn checkpoint(
    stores: &dyn Stores,
    backing: &Backing,
    phase: &str,
    logical_payload_bytes: usize,
) -> Value {
    let io = backing.0.counts();
    assert!(!io.overflowed);
    let result = json!({"phase":phase,"logical_payload_bytes":logical_payload_bytes,"physical_pages":Memory::size(backing),"state":stores.stats(),"io":{"reads":io.reads,"read_bytes":io.read_bytes,"writes":io.writes,"write_bytes":io.write_bytes,"grows":io.grows,"grown_pages":io.grown_pages}});
    backing.0.reset(); // Diagnostic observation is excluded from the next phase.
    result
}
fn exercise(stores: &mut dyn Stores, backing: &Backing, count: usize) -> Vec<Value> {
    let payload_bytes = |rows| {
        (0..count)
            .map(|s| rows * if s % 8 == 0 { 65536 } else { 256 })
            .sum()
    };
    let mut report = vec![checkpoint(stores, backing, "bootstrap", 0)];
    stores.init();
    report.push(checkpoint(stores, backing, "initialized", 0));
    for (phase, start, end) in [("populated", 0, 32), ("grown", 32, 96)] {
        for key in start..end {
            for s in 0..count {
                assert_eq!(stores.put(s, key, value(s, key)), None);
            }
        }
        report.push(checkpoint(
            stores,
            backing,
            phase,
            payload_bytes(end as usize),
        ));
    }
    for round in 0..4 {
        for key in (0..96).step_by(2) {
            for s in 0..count {
                assert_eq!(stores.remove(s, key), Some(value(s, key)));
            }
        }
        report.push(checkpoint(
            stores,
            backing,
            &format!("deleted-{round}"),
            payload_bytes(48),
        ));
        for key in (0..96).rev().filter(|k| k % 2 == 0) {
            for s in 0..count {
                assert_eq!(stores.put(s, key, value(s, key)), None);
            }
        }
        report.push(checkpoint(
            stores,
            backing,
            &format!("reinserted-{round}"),
            payload_bytes(96),
        ));
    }
    for key in 0..96 {
        for s in 0..count {
            assert_eq!(stores.get(s, key), Some(value(s, key)));
        }
    }
    report.push(checkpoint(stores, backing, "verified", payload_bytes(96)));
    // Keep the original same-size study intact, then cross inline and external
    // size-class boundaries with exactly the same replacements on both engines.
    let sizes = [31, 32, 33, 236, 237, 4076, 4077, 65516, 65517];
    let changed = |s: usize, k: u64, round: usize| {
        vec![
            (s as u8).wrapping_add(k as u8).wrapping_add(round as u8);
            sizes[(s + k as usize + round) % sizes.len()]
        ]
    };
    for round in 0..6 {
        let mut live = 0;
        for key in 0..96 {
            for s in 0..count {
                let old = if round == 0 {
                    value(s, key)
                } else {
                    changed(s, key, round - 1)
                };
                let new = changed(s, key, round);
                live += new.len();
                assert_eq!(stores.put(s, key, new), Some(old));
            }
        }
        report.push(checkpoint(
            stores,
            backing,
            &format!("size-churn-{round}"),
            live,
        ));
    }
    for key in 0..96 {
        for s in 0..count {
            assert_eq!(stores.get(s, key), Some(changed(s, key, 5)));
        }
    }
    report
}
fn main() {
    let args = Arguments::parse();
    let output = args.output;
    assert!(!output.exists(), "preserve earlier results");
    if args.allocation_histories {
        fs::write(
            output,
            serde_json::to_vec_pretty(&allocation_histories::histories()).unwrap(),
        )
        .unwrap();
        return;
    }
    if args.small_records {
        fs::write(output, serde_json::to_vec_pretty(&small_records()).unwrap()).unwrap();
        return;
    }
    let mut runs = vec![];
    for count in [32, 64] {
        for bucket in [0, 1, 8, 128] {
            let backing = Backing::default();
            let mut engine: Box<dyn Stores> = if bucket == 0 {
                Box::new(Owned::new(backing.clone(), count))
            } else {
                Box::new(Icss {
                    manager: Manager::init_with_bucket_size(backing.clone(), bucket),
                    maps: vec![],
                    count,
                })
            };
            let phases = exercise(engine.as_mut(), &backing, count);
            runs.push(json!({"backend":if bucket==0 {"ic-storage"} else {"ic-stable-structures-0.7.2"},"allocations":count,"bucket_pages":if bucket==0 {None} else {Some(bucket)},"phases":phases}));
        }
    }
    fs::write(output,serde_json::to_vec_pretty(&json!({"scope":"Native capacity and backing IO; not IC instructions. Diagnostics excluded from phase IO. One backing, interleaved operations; four delete/reinsert rounds.","runs":runs})).unwrap()).unwrap();
}

fn small_records() -> Value {
    let mut runs = vec![];
    for payload in [33, 44, 45, 80, 108, 109, 235, 236, 237] {
        let backing = Backing::default();
        let mut stores = Owned::new(backing.clone(), 4);
        stores.init();
        backing.0.reset();
        for key in 0..1024 {
            for store in 0..4 {
                assert!(stores.put(store, key, vec![7; payload]).is_none());
            }
        }
        let mut phases = vec![checkpoint(&stores, &backing, "populated", payload * 4096)];
        for round in 0..6 {
            for key in (0..1024).step_by(2) {
                for store in 0..4 {
                    assert_eq!(stores.remove(store, key), Some(vec![7; payload]));
                }
            }
            phases.push(checkpoint(
                &stores,
                &backing,
                &format!("deleted-{round}"),
                payload * 2048,
            ));
            for key in (0..1024).rev().filter(|key| key % 2 == 0) {
                for store in 0..4 {
                    assert!(stores.put(store, key, vec![7; payload]).is_none());
                }
            }
            phases.push(checkpoint(
                &stores,
                &backing,
                &format!("reused-{round}"),
                payload * 4096,
            ));
        }
        for key in 0..1024 {
            for store in 0..4 {
                assert_eq!(stores.get(store, key), Some(vec![7; payload]));
            }
        }
        runs.push(json!({"payload_bytes":payload,"allocations":4,"records_per_allocation":1024,"phases":phases}));
    }
    json!({"scope":"Native capacity and backing IO including allocation metadata; no IC instruction or timing claim. Four shared-backing maps, six delete/reinsert rounds, full final-value validation.","runs":runs})
}
