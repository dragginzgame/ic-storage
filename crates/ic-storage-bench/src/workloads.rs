//! Source-informed canonical-store workloads; not an `IcyDB` codec or overlay.
use super::protocol::WorkloadOperation::{
    FoldDelete, FoldPut, KeysForward, KeysReverse, ReadMix, Refill, ValuesForward, ValuesReverse,
};
use super::{
    Tree, Upstream,
    protocol::{
        SCAN_LIMIT, WORKLOAD_BATCH, WORKLOAD_OPERATIONS, WORKLOAD_PROFILES, WORKLOAD_RECORDS,
        WORKLOAD_ROUNDS, WORKLOAD_SEED_BATCH, WorkloadOperation,
    },
};
use ic_storage::{BTreeMap, Config, Lexicographic};
use ic_testkit::performance::Performance;
use std::{cell::RefCell, collections::BTreeMap as Model, ops::Bound};

struct State {
    tree: Tree,
    model: Model<Vec<u8>, Vec<u8>>,
    index: bool,
    seeded: u64,
    step: usize,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn init(engine: u8, profile: usize) {
    assert!(engine <= 1 && profile < WORKLOAD_PROFILES.len());
    let index = profile == 1;
    let memory = super::governed_memory();
    let tree = match (engine, index) {
        (0, _) => Tree::Ours(BTreeMap::create(memory, Config::default(), Lexicographic).unwrap()),
        (_, true) => Tree::Index(Upstream::new(memory)),
        (_, false) => Tree::Row(Upstream::new(memory)),
    };
    STATE.with_borrow_mut(|state| {
        *state = Some(State {
            tree,
            model: Model::new(),
            index,
            seeded: 0,
            step: 0,
        });
    });
}

fn key(n: u64) -> Vec<u8> {
    let len = [24, 32, 33, 64, 128, 262][usize::try_from(n % 6).unwrap()];
    let mut bytes = vec![0xa5; len];
    bytes[..8].copy_from_slice(&n.to_be_bytes());
    bytes
}

fn value(n: u64, generation: u8, index: bool) -> Vec<u8> {
    let len = if index {
        1
    } else {
        [1, 16, 17, 32, 256, 1024, 4072, 4073, 8192, 65536]
            [usize::try_from((n + u64::from(generation)) % 10).unwrap()]
    };
    (0..len)
        .map(|i| u8::try_from((i + usize::from(generation)) % 251).unwrap() + 1)
        .collect()
}

fn targets(round: u8, operation: WorkloadOperation) -> impl Iterator<Item = u64> {
    (0..WORKLOAD_BATCH).map(move |i| {
        let n = (i * 149 + u64::from(round) * 43) % WORKLOAD_RECORDS;
        if operation == ReadMix && i % 4 == 0 {
            n + WORKLOAD_RECORDS
        } else {
            n
        }
    })
}

#[ic_cdk::update]
fn workload_seed() -> bool {
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().unwrap();
        let end = (state.seeded + WORKLOAD_SEED_BATCH).min(WORKLOAD_RECORDS);
        for i in state.seeded..end {
            // A full permutation gives a reproducible nonsequential insertion order.
            let n = (i * 149 + 37) % WORKLOAD_RECORDS;
            let key = key(n);
            let value = value(n, 0, state.index);
            assert_eq!(state.tree.insert(key.clone(), value.clone()), None);
            assert_eq!(state.model.insert(key, value), None);
        }
        state.seeded = end;
        end == WORKLOAD_RECORDS
    })
}

// Both engines copy just the requested projection into the output. Upstream's
// key-only cursor never calls value(); neither path clones a key into a Vec first.
fn scan(tree: &Tree, lower: Vec<u8>, upper: Vec<u8>, reverse: bool, values: bool) -> Vec<u8> {
    let mut output = Vec::new();
    match tree {
        Tree::Ours(tree) => {
            let mut iter = tree
                .range(Bound::Included(&lower), Bound::Excluded(&upper))
                .unwrap();
            for _ in 0..SCAN_LIMIT {
                let Some(entry) = (if reverse {
                    iter.next_back()
                } else {
                    iter.next()
                }) else {
                    break;
                };
                let entry = entry.unwrap();
                if values {
                    output.extend_from_slice(&entry.value().unwrap());
                } else {
                    output.extend_from_slice(entry.key());
                }
            }
        }
        Tree::Index(tree) => tree.range_scan(lower, upper, reverse, values, &mut output),
        Tree::Row(tree) => tree.range_scan(lower, upper, reverse, values, &mut output),
        Tree::Journal(_) => unreachable!(),
    }
    output
}

#[ic_cdk::update]
fn workload_measure(operation: u8) -> (u128, u128, u128, u64) {
    let operation = WorkloadOperation::try_from(operation).expect("workload operation");
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().unwrap();
        assert_eq!(state.seeded, WORKLOAD_RECORDS);
        assert!(state.step < usize::from(WORKLOAD_ROUNDS) * WORKLOAD_OPERATIONS.len());
        assert_eq!(
            operation,
            WORKLOAD_OPERATIONS[state.step % WORKLOAD_OPERATIONS.len()].0
        );
        let round = u8::try_from(state.step / WORKLOAD_OPERATIONS.len()).unwrap();
        let is_scan = matches!(
            operation,
            KeysForward | KeysReverse | ValuesForward | ValuesReverse
        );
        let reverse = matches!(operation, KeysReverse | ValuesReverse);
        let values = matches!(operation, ValuesForward | ValuesReverse);
        // Allocate only the inputs consumed by this operation, before timing.
        let (lower, upper) = if is_scan {
            (key(128 + u64::from(round)), key(384 + u64::from(round)))
        } else {
            (Vec::new(), Vec::new())
        };
        let keys = if is_scan {
            Vec::new()
        } else {
            targets(round, operation).map(key).collect::<Vec<_>>()
        };
        let values_to_write = if matches!(operation, FoldPut | Refill) {
            targets(round, operation)
                .map(|n| value(n, round + 1, state.index))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        // Calculate independent expected results outside the measured interval.
        let expected = if is_scan {
            let mut iter = state.model.range(lower.clone()..upper.clone());
            let mut output = Vec::new();
            for _ in 0..SCAN_LIMIT {
                let Some((key, value)) = (if reverse {
                    iter.next_back()
                } else {
                    iter.next()
                }) else {
                    break;
                };
                output.extend_from_slice(if values { value } else { key });
            }
            vec![Some(output)]
        } else {
            keys.iter()
                .map(|key| state.model.get(key).cloned())
                .collect::<Vec<_>>()
        };
        let before = Performance::counters();
        let result = match operation {
            KeysForward | KeysReverse | ValuesForward | ValuesReverse => {
                vec![Some(scan(&state.tree, lower, upper, reverse, values))]
            }
            ReadMix => keys.into_iter().map(|key| state.tree.get(key)).collect(),
            FoldDelete => keys.into_iter().map(|key| state.tree.remove(key)).collect(),
            FoldPut | Refill => keys
                .into_iter()
                .zip(values_to_write)
                .map(|(key, value)| state.tree.insert(key, value))
                .collect(),
        };
        let after = Performance::counters();
        assert_eq!(result, expected);
        for n in targets(round, operation) {
            match operation {
                FoldPut | Refill => {
                    state.model.insert(key(n), value(n, round + 1, state.index));
                }
                FoldDelete => {
                    state.model.remove(&key(n));
                }
                _ => {}
            }
        }
        if matches!(operation, FoldPut | FoldDelete | Refill) {
            // Check all rows after every mutation batch, outside the measured window.
            for n in 0..WORKLOAD_RECORDS {
                assert_eq!(state.tree.get(key(n)), state.model.get(&key(n)).cloned());
            }
            if let Tree::Ours(tree) = &state.tree {
                tree.validate().unwrap();
            }
        }
        state.step += 1;
        (
            after.instructions - before.instructions,
            after.heap_bytes,
            after.memory_bytes,
            result
                .iter()
                .flatten()
                .map(|v| u64::try_from(v.len()).unwrap())
                .sum(),
        )
    })
}
