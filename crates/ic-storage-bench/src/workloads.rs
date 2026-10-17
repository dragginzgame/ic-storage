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
        let lower = key(128 + u64::from(round));
        let upper = key(384 + u64::from(round));
        let is_scan = matches!(
            operation,
            KeysForward | KeysReverse | ValuesForward | ValuesReverse
        );
        let reverse = matches!(operation, KeysReverse | ValuesReverse);
        let values = matches!(operation, ValuesForward | ValuesReverse);
        // Allocate inputs and calculate independent expected results before timing.
        let inputs = (0..WORKLOAD_BATCH)
            .map(|i| {
                let n = (i * 149 + u64::from(round) * 43) % WORKLOAD_RECORDS;
                let n = if operation == ReadMix && i % 4 == 0 {
                    n + WORKLOAD_RECORDS
                } else {
                    n
                };
                (key(n), value(n, round + 1, state.index))
            })
            .collect::<Vec<_>>();
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
            inputs
                .iter()
                .map(|(key, _)| state.model.get(key).cloned())
                .collect::<Vec<_>>()
        };
        let before = Performance::counters();
        let result = if is_scan {
            vec![Some(scan(&state.tree, lower, upper, reverse, values))]
        } else {
            inputs
                .into_iter()
                .map(|(key, value)| match operation {
                    ReadMix => state.tree.get(key),
                    FoldPut | Refill => state.tree.insert(key, value),
                    FoldDelete => state.tree.remove(key),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>()
        };
        let after = Performance::counters();
        assert_eq!(result, expected);
        for i in 0..WORKLOAD_BATCH {
            let n = (i * 149 + u64::from(round) * 43) % WORKLOAD_RECORDS;
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
