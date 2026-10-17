//! Non-published canister probe; each installation owns one fresh memory region.
#![cfg(target_arch = "wasm32")]
mod fixtures;
mod workloads;
use ic_memory::{
    GenericAllocationPolicy, MemoryAllocationPool, MemoryAuthority, MemoryRequest, MemoryRuntime,
    RuntimeMemory, SealedDeclarationSnapshot, ic_stable_structures::DefaultMemoryImpl as Memory,
};
use ic_storage::adapter::{Bound, StableBTreeMap as Upstream, Storable};
use ic_storage::{BTreeMap, Config, Lexicographic};
use ic_testkit::performance::Performance;
use std::{borrow::Cow, cell::RefCell};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Bytes<const MAX: u32, const FIXED: bool>(Vec<u8>);
impl<const MAX: u32, const FIXED: bool> Storable for Bytes<MAX, FIXED> {
    const BOUND: Bound = if MAX == 0 {
        Bound::Unbounded
    } else {
        Bound::Bounded {
            max_size: MAX,
            is_fixed_size: FIXED,
        }
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&self.0)
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(bytes.into_owned())
    }
}
enum Tree {
    Ours(BTreeMap<Memory>),
    Index(Upstream<Bytes<16668, false>, Bytes<1, false>, Memory>),
    Journal(Upstream<Bytes<12, true>, Bytes<65536, false>, Memory>),
    Row(Upstream<Bytes<262, false>, Bytes<0, false>, Memory>),
}
impl Tree {
    fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        match self {
            Self::Ours(tree) => tree.insert(&key, &value).unwrap(),
            Self::Index(tree) => tree.insert(Bytes(key), Bytes(value)).map(|v| v.0),
            Self::Journal(tree) => tree.insert(Bytes(key), Bytes(value)).map(|v| v.0),
            Self::Row(tree) => tree.insert(Bytes(key), Bytes(value)).map(|v| v.0),
        }
    }
    fn get(&self, key: Vec<u8>) -> Option<Vec<u8>> {
        match self {
            Self::Ours(tree) => tree.get(&key).unwrap(),
            Self::Index(tree) => tree.get(&Bytes(key)).map(|v| v.0),
            Self::Journal(tree) => tree.get(&Bytes(key)).map(|v| v.0),
            Self::Row(tree) => tree.get(&Bytes(key)).map(|v| v.0),
        }
    }
    fn remove(&mut self, key: Vec<u8>) -> Option<Vec<u8>> {
        match self {
            Self::Ours(tree) => tree.remove(&key).unwrap(),
            Self::Index(tree) => tree.remove(&Bytes(key)).map(|v| v.0),
            Self::Journal(tree) => tree.remove(&Bytes(key)).map(|v| v.0),
            Self::Row(tree) => tree.remove(&Bytes(key)).map(|v| v.0),
        }
    }
    fn scan(&self) -> Vec<u8> {
        match self {
            Self::Ours(tree) => tree
                .iter()
                .take(16)
                .flat_map(|e| e.unwrap().key().to_vec())
                .collect(),
            Self::Index(tree) => tree
                .iter()
                .take(16)
                .flat_map(|e| e.key().0.clone())
                .collect(),
            Self::Journal(tree) => tree
                .iter()
                .take(16)
                .flat_map(|e| e.key().0.clone())
                .collect(),
            Self::Row(tree) => tree
                .iter()
                .take(16)
                .flat_map(|e| e.key().0.clone())
                .collect(),
        }
    }
    fn scan_values(&self) -> Vec<u8> {
        let mut output = Vec::new();
        match self {
            Self::Ours(tree) => tree
                .iter()
                .take(16)
                .for_each(|e| output.extend_from_slice(&e.unwrap().value().unwrap())),
            Self::Index(tree) => tree
                .iter()
                .take(16)
                .for_each(|e| output.extend_from_slice(&e.value().0)),
            Self::Journal(tree) => tree
                .iter()
                .take(16)
                .for_each(|e| output.extend_from_slice(&e.value().0)),
            Self::Row(tree) => tree
                .iter()
                .take(16)
                .for_each(|e| output.extend_from_slice(&e.value().0)),
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
        workloads::init(engine, usize::from(profile) - fixtures::FIXTURES.len());
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
    assert!((1..=32).contains(&batch));
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
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().unwrap();
        assert_eq!(state.seeded, state.count);
        let target = if operation == 4 || operation == 9 {
            state.count
        } else {
            state.count / 2
        };
        let key = key(target, state.key_len, state.dense_keys);
        let value_len = if operation == 6 { 1 } else { state.value_len };
        let fill = if operation == 10 { 8 } else { 7 };
        let value = vec![fill; value_len];
        let before = Performance::counters();
        let result = match operation {
            0 | 4 => state.tree.get(key),
            1 | 6 | 7 | 8 | 9 | 10 => state.tree.insert(key, value),
            2 => Some(state.tree.scan()),
            3 => state.tree.remove(key),
            5 => Some(state.tree.scan_values()),
            _ => panic!("invalid operation"),
        };
        let after = Performance::counters();
        // Semantic checks and reply encoding are outside the measured interval.
        if matches!(operation, 4 | 8 | 9) {
            assert!(result.is_none());
        } else if operation == 2 {
            let expected = (0..state.count.min(16))
                .flat_map(|n| self::key(n, state.key_len, state.dense_keys))
                .collect::<Vec<_>>();
            assert_eq!(result.as_ref().unwrap(), &expected);
        } else {
            let len = match operation {
                5 => usize::try_from(state.count.min(16)).unwrap() * state.value_len,
                7 => 1,
                _ => state.value_len,
            };
            assert_eq!(result.as_ref().unwrap(), &vec![7; len]);
        }
        if matches!(operation, 1 | 3 | 6 | 7 | 8 | 9 | 10) {
            let stored = state
                .tree
                .get(self::key(target, state.key_len, state.dense_keys));
            let expected = if operation == 3 {
                None
            } else {
                Some(vec![fill; value_len])
            };
            assert_eq!(stored, expected);
        }
        // Restore the common input state outside the measured interval.
        if operation == 10 {
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
