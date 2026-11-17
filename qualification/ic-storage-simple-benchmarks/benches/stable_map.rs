//! StableBTreeMap backend for the installed native-map comparison only.
use ic_platform_runtime::raw as ic0;
use ic_stable_structures::{
    DefaultMemoryImpl, Memory, StableBTreeMap,
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
};
use ic_storage_simple_benchmarks::wire;
use std::cell::RefCell;

type Tree = StableBTreeMap<u64, u64, VirtualMemory<DefaultMemoryImpl>>;
struct State {
    maps: [Tree; 2],
    rows: u64,
    seed: u64,
}
thread_local! { static STATE: RefCell<Option<State>> = const {RefCell::new(None)}; }

#[expect(unsafe_code, reason = "unique benchmark setup export")]
#[unsafe(export_name = "canister_update bench_setup")]
extern "C" fn setup() {
    let [rows, seed] = wire::input();
    assert!([64, 192, 1536].contains(&rows) && (1..=3).contains(&seed));
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none());
        let manager = MemoryManager::init(DefaultMemoryImpl::default());
        let mut maps = [
            Tree::init(manager.get(MemoryId::new(0))),
            Tree::init(manager.get(MemoryId::new(1))),
        ];
        for key in 0..rows {
            maps[0].insert(key, key + 1000);
        }
        *slot = Some(State { maps, rows, seed });
    });
    wire::reply(&[]);
}

#[expect(unsafe_code, reason = "unique benchmark operation export")]
#[unsafe(export_name = "canister_update bench_run")]
extern "C" fn run() {
    let [operation, batch] = wire::input();
    assert!(operation < 6 && [1, 8, 64].contains(&batch));
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut().unwrap();
        for i in 0..batch {
            let key = (i * 17 + state.seed * 13) % state.rows;
            match operation {
                0 => {
                    core::hint::black_box(key);
                }
                1 => assert_eq!(state.maps[0].get(&key), Some(key + 1000)),
                2 => {
                    state.maps[0].insert(key, key + 2000);
                }
                3 => assert_eq!(state.maps[0].remove(&key), Some(key + 2000)),
                4 => {
                    state.maps[0].insert(key, key + 1000);
                }
                5 => {
                    state.maps[0].insert(state.rows + i, state.rows + i + 1000);
                }
                _ => unreachable!(),
            }
        }
    });
    wire::reply(&[]);
}

#[expect(unsafe_code, reason = "unique benchmark state export")]
#[unsafe(export_name = "canister_query bench_state")]
extern "C" fn state() {
    STATE.with(|slot| {
        let slot = slot.borrow();
        let state = slot.as_ref().unwrap();
        for word in [
            DefaultMemoryImpl::default().size(),
            state.maps[1].len(),
            state.maps[0].len(),
        ] {
            ic0::msg_reply_data_append(&word.to_le_bytes());
        }
        for entry in state.maps[0].iter() {
            ic0::msg_reply_data_append(&entry.key().to_le_bytes());
            ic0::msg_reply_data_append(&entry.value().to_le_bytes());
        }
    });
    ic0::msg_reply();
}
