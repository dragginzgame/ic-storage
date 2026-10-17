//! Non-published canister probe; each installation owns one fresh memory region.
#![cfg(target_arch = "wasm32")]
mod fixtures;
mod protocol;
use protocol::{MatrixOperation, SCAN_LIMIT, SEED_MAX_BATCH, WORKLOAD_PROFILES};
mod upstream;
mod workloads;
use ic_memory::{
    DefaultMemoryImpl as Memory, GenericAllocationPolicy, MemoryAllocationPool, MemoryAuthority,
    MemoryRequest, MemoryRuntime, RuntimeMemory, SealedDeclarationSnapshot,
};
use ic_storage::{BTreeMap, Config, Lexicographic};
use ic_testkit::performance::Performance;
use std::cell::RefCell;
use upstream::Map as Upstream;

enum Tree {
    Ours(BTreeMap<Memory>),
    Index(Upstream<16668, false, 1, false>),
    Journal(Upstream<12, true, 65536, false>),
    Row(Upstream<262, false, 0, false>),
}
impl Tree {
    fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        match self {
            Self::Ours(tree) => tree.insert(&key, &value).unwrap(),
            Self::Index(tree) => tree.insert(key, value),
            Self::Journal(tree) => tree.insert(key, value),
            Self::Row(tree) => tree.insert(key, value),
        }
    }
    fn get(&self, key: Vec<u8>) -> Option<Vec<u8>> {
        match self {
            Self::Ours(tree) => tree.get(&key).unwrap(),
            Self::Index(tree) => tree.get(key),
            Self::Journal(tree) => tree.get(key),
            Self::Row(tree) => tree.get(key),
        }
    }
    fn remove(&mut self, key: Vec<u8>) -> Option<Vec<u8>> {
        match self {
            Self::Ours(tree) => tree.remove(&key).unwrap(),
            Self::Index(tree) => tree.remove(key),
            Self::Journal(tree) => tree.remove(key),
            Self::Row(tree) => tree.remove(key),
        }
    }
    fn scan(&self) -> Vec<u8> {
        match self {
            Self::Ours(tree) => tree
                .iter()
                .take(SCAN_LIMIT)
                .flat_map(|e| e.unwrap().key().to_vec())
                .collect(),
            Self::Index(tree) => tree.scan(),
            Self::Journal(tree) => tree.scan(),
            Self::Row(tree) => tree.scan(),
        }
    }
    fn scan_values(&self) -> Vec<u8> {
        let mut output = Vec::new();
        match self {
            Self::Ours(tree) => tree
                .iter()
                .take(SCAN_LIMIT)
                .for_each(|e| output.extend_from_slice(&e.unwrap().value().unwrap())),
            Self::Index(tree) => tree.scan_values(&mut output),
            Self::Journal(tree) => tree.scan_values(&mut output),
            Self::Row(tree) => tree.scan_values(&mut output),
        }
        output
    }
}
struct State {
    tree: Tree,
    count: u64,
    key_len: usize,
    value_len: usize,
    seeded: u64,
    dense_keys: bool,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
fn key(n: u64, len: usize, dense: bool) -> Vec<u8> {
    let mut bytes = vec![if dense { 0xa5 } else { 0 }; len];
    bytes[..8].copy_from_slice(&n.to_be_bytes());
    bytes
}
fn governed_memory() -> RuntimeMemory<Memory> {
    let pool = MemoryAllocationPool::new(
        vec![MemoryAuthority::new("bench", "bench.").unwrap()],
        vec![],
    )
    .unwrap();
    let declarations =
        SealedDeclarationSnapshot::new(vec![MemoryRequest::new("bench", "bench.tree.v1").unwrap()])
            .unwrap();
    let mut runtime = MemoryRuntime::new(Memory::default()).unwrap();
    runtime
        .bootstrap(&declarations, &pool, &GenericAllocationPolicy)
        .unwrap();
    runtime.open_memory("bench.tree.v1").unwrap()
}
#[ic_cdk::init]
fn init(engine: u8, profile: u8, dense_keys: bool) {
    assert!(engine <= 1);
    if usize::from(profile) >= fixtures::FIXTURES.len() {
        let workload = usize::from(profile) - fixtures::FIXTURES.len();
        assert!(workload < WORKLOAD_PROFILES.len());
        workloads::init(engine, workload);
        return;
    }
    let fixture = &fixtures::FIXTURES[usize::from(profile)];
    assert_ne!(fixture.name, "");
    let (count, key_len, value_len) = (fixture.count, fixture.key_len, fixture.value_len);
    let memory = governed_memory();
    let tree = if engine == 0 {
        Tree::Ours(BTreeMap::create(memory, Config::default(), Lexicographic).unwrap())
    } else {
        match fixture.kind {
            0 => Tree::Index(Upstream::new(memory)),
            1 => Tree::Journal(Upstream::new(memory)),
            2 => Tree::Row(Upstream::new(memory)),
            _ => unreachable!(),
        }
    };
    STATE.with_borrow_mut(|state| {
        *state = Some(State {
            tree,
            count,
            key_len,
            value_len,
            seeded: 0,
            dense_keys,
        });
    });
}
#[ic_cdk::update]
fn seed(batch: u64) -> bool {
    assert!((1..=SEED_MAX_BATCH).contains(&batch));
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().unwrap();
        let end = (state.seeded + batch).min(state.count);
        for n in state.seeded..end {
            assert!(
                state
                    .tree
                    .insert(
                        key(n, state.key_len, state.dense_keys),
                        vec![7; state.value_len]
                    )
                    .is_none()
            );
        }
        state.seeded = end;
        end == state.count
    })
}
#[ic_cdk::update]
fn measure(operation: u8) -> (u128, u128, u128, u64) {
    use MatrixOperation::{
        Get, InsertNew, MissingGet, Refill, Regrow, Remove, Replace, ReplaceChanged, ScanKeys,
        ScanValues, Shrink,
    };
    let operation = MatrixOperation::try_from(operation).expect("matrix operation");
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().unwrap();
        assert_eq!(state.seeded, state.count);
        let target = if matches!(operation, MissingGet | InsertNew) {
            state.count
        } else {
            state.count / 2
        };
        let key = key(target, state.key_len, state.dense_keys);
        let value_len = if operation == Shrink {
            1
        } else {
            state.value_len
        };
        let fill = if operation == ReplaceChanged { 8 } else { 7 };
        let value = vec![fill; value_len];
        let before = Performance::counters();
        let result = match operation {
            Get | MissingGet => state.tree.get(key),
            Replace | Shrink | Regrow | Refill | InsertNew | ReplaceChanged => {
                state.tree.insert(key, value)
            }
            ScanKeys => Some(state.tree.scan()),
            Remove => state.tree.remove(key),
            ScanValues => Some(state.tree.scan_values()),
        };
        let after = Performance::counters();
        // Semantic checks and reply encoding are outside the measured interval.
        if matches!(operation, MissingGet | Refill | InsertNew) {
            assert!(result.is_none());
        } else if operation == ScanKeys {
            let expected = (0..state.count.min(SCAN_LIMIT as u64))
                .flat_map(|n| self::key(n, state.key_len, state.dense_keys))
                .collect::<Vec<_>>();
            assert_eq!(result.as_ref().unwrap(), &expected);
        } else {
            let len = match operation {
                ScanValues => {
                    usize::try_from(state.count.min(SCAN_LIMIT as u64)).unwrap() * state.value_len
                }
                Regrow => 1,
                _ => state.value_len,
            };
            assert_eq!(result.as_ref().unwrap(), &vec![7; len]);
        }
        if matches!(
            operation,
            Replace | Remove | Shrink | Regrow | Refill | InsertNew | ReplaceChanged
        ) {
            let stored = state
                .tree
                .get(self::key(target, state.key_len, state.dense_keys));
            let expected = if operation == Remove {
                None
            } else {
                Some(vec![fill; value_len])
            };
            assert_eq!(stored, expected);
        }
        // Restore the common input state outside the measured interval.
        if operation == ReplaceChanged {
            assert_eq!(
                state.tree.insert(
                    self::key(target, state.key_len, state.dense_keys),
                    vec![7; value_len]
                ),
                Some(vec![8; value_len])
            );
        }
        (
            after.instructions - before.instructions,
            after.heap_bytes,
            after.memory_bytes,
            result.as_ref().map_or(0, |bytes| bytes.len() as u64),
        )
    })
}
