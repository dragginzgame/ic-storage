//! Matched scalar map matrix; no legacy Storage dependency is selected.
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
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

#[expect(unsafe_code, reason = "unique benchmark setup export")]
#[unsafe(export_name = "canister_update bench_setup")]
extern "C" fn setup() {
    let [shape, seed] = wire::input();
    let rows = shape & 0xffff_ffff;
    let other = shape >> 32;
    assert!(
        [64, 192, 1536].contains(&rows) && [0, 192].contains(&other) && (1..=3).contains(&seed)
    );
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none());
        let manager = MemoryManager::init(DefaultMemoryImpl::default());
        let mut maps = [
            Tree::init(manager.get(MemoryId::new(0))),
            Tree::init(manager.get(MemoryId::new(1))),
        ];
        for (map, count) in maps.iter_mut().zip([rows, other]) {
            for key in 0..count {
                assert_eq!(map.insert(key, key + 1000), None);
            }
        }
        *slot = Some(State { maps, rows, seed });
    });
    wire::reply(&[]);
}

#[expect(unsafe_code, reason = "unique benchmark operation export")]
#[unsafe(export_name = "canister_update bench_run")]
extern "C" fn run() {
    let [operation, batch] = wire::input();
    assert!(operation < 9 && [1, 8, 64].contains(&batch));
    let sum = STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut().unwrap();
        let map = &mut state.maps[0];
        let mut sum = 0u64;
        if operation == 8 {
            for entry in map.iter() {
                sum = sum.wrapping_add(entry.value());
            }
        } else {
            for i in 0..batch {
                let key = (17 * i + 13 * state.seed) % state.rows;
                match operation {
                    0 => {
                        core::hint::black_box(key);
                    }
                    1 => assert_eq!(map.get(&key), Some(key + 1000)),
                    2 => assert_eq!(map.get(&(state.rows + 1024 + i)), None),
                    3 => assert_eq!(map.insert(key, key + 2000), Some(key + 1000)),
                    4 => assert_eq!(map.remove(&key), Some(key + 2000)),
                    5 => assert_eq!(map.insert(key, key + 1000), None),
                    6 => assert_eq!(map.insert(state.rows + i, state.rows + i + 1000), None),
                    7 => {
                        assert_eq!(map.remove(&key), Some(key + 1000));
                        assert_eq!(
                            map.insert(state.rows + 64 + i, state.rows + 64 + i + 1000),
                            None
                        );
                    }
                    _ => unreachable!(),
                }
            }
        }
        sum
    });
    wire::reply(&[sum]);
}

#[expect(unsafe_code, reason = "unique benchmark state export")]
#[unsafe(export_name = "canister_query bench_state")]
extern "C" fn state() {
    STATE.with(|slot| {
        let slot = slot.borrow();
        let state = slot.as_ref().unwrap();
        for word in [
            core::arch::wasm32::memory_size::<0>() as u64,
            DefaultMemoryImpl::default().size(),
            0,
        ] {
            ic0::msg_reply_data_append(&word.to_le_bytes());
        }
        for map in &state.maps {
            ic0::msg_reply_data_append(&map.len().to_le_bytes());
            for entry in map.iter() {
                ic0::msg_reply_data_append(&entry.key().to_le_bytes());
                ic0::msg_reply_data_append(&entry.value().to_le_bytes());
            }
        }
    });
    ic0::msg_reply();
}
