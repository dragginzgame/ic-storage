//! Ordinary Rust Vec<V2> source for explicit portable collection conversion.
//! No native retained-layout or source EOP claim is made by this fixture.

use crate::provider::record_vector_cell as cell;
use ic_platform_runtime::raw as ic0;
use ic_storage_native_feasibility::{checkpoint::record_vector as codec, record::V2};
use std::cell::RefCell;

struct State {
    values: Vec<V2>,
    frozen: bool,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

fn args<const N: usize>() -> [u64; N] {
    assert_eq!(ic0::msg_arg_data_size() as usize, N * 8);
    let mut bytes = vec![0; N * 8];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
}

#[expect(unsafe_code, reason = "ordinary source installation fixture")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    let [count, special_index, special_value] = args();
    assert!(count <= codec::LIMIT as u64);
    assert!(special_index < count || special_index == u64::MAX);
    let values = (0..count)
        .map(|index| {
            V2::new(
                if index == special_index {
                    special_value
                } else {
                    17 + index
                },
                u64::MAX,
            )
            .unwrap()
        })
        .collect();
    drop(cell(true));
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            values,
            frozen: false,
        })
    });
}

#[expect(
    unsafe_code,
    reason = "ordinary source requires a completed frozen export"
)]
#[unsafe(export_name = "canister_pre_upgrade")]
extern "C" fn pre_upgrade() {
    STATE.with(|state| assert!(state.borrow().as_ref().unwrap().frozen));
}

#[expect(unsafe_code, reason = "ordinary source logical field update fixture")]
#[unsafe(export_name = "canister_update set_counter")]
extern "C" fn set_counter() {
    let [index, value] = args();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut().unwrap();
        assert!(!state.frozen && index < state.values.len() as u64);
        let old = state.values[index as usize];
        state.values[index as usize] = V2::new(value, old.ceiling()).unwrap();
    });
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "ordinary source governed checkpoint publication"
)]
#[unsafe(export_name = "canister_update export_checkpoint")]
extern "C" fn export_checkpoint() {
    let [epoch, fault] = args();
    assert!(fault <= 1);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut().unwrap();
        cell(false).set(codec::encode_v2(&state.values, epoch).unwrap());
        state.frozen = true;
    });
    if fault == 1 {
        ic0::trap(b"V2 vector export after stable write");
    }
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "explicit source repair after refused conversion"
)]
#[unsafe(export_name = "canister_update cancel_export")]
extern "C" fn cancel_export() {
    assert_eq!(ic0::msg_arg_data_size(), 0);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.as_mut().unwrap();
        assert!(state.frozen);
        // Clear the obsolete checkpoint before allowing newer application writes.
        cell(false).set(Vec::new());
        state.frozen = false;
    });
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "ordinary source logical-state diagnostic")]
#[unsafe(export_name = "canister_query observe")]
extern "C" fn observe() {
    STATE.with(|state| {
        let state = state.borrow();
        let state = state.as_ref().unwrap();
        ic0::msg_reply_data_append(&u64::from(state.frozen).to_le_bytes());
        ic0::msg_reply_data_append(&(state.values.len() as u64).to_le_bytes());
        for value in &state.values {
            ic0::msg_reply_data_append(&value.store_encode().unwrap());
        }
    });
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "governed source checkpoint evidence query")]
#[unsafe(export_name = "canister_query checkpoint_bytes")]
extern "C" fn checkpoint_bytes() {
    ic0::msg_reply_data_append(cell(false).get());
    ic0::msg_reply();
}
