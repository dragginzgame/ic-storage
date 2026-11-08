//! Identical application endpoints, with no catalogue, timer or verification code.
use crate::backend::State;
use ic_platform_runtime::ic0;
use std::cell::RefCell;

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

fn input<const N: usize>() -> [u8; N] {
    assert_eq!(ic0::msg_arg_data_size(), N);
    let mut bytes = [0; N];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    bytes
}
fn reply(values: &[u64]) {
    for value in values {
        ic0::msg_reply_data_append(&value.to_le_bytes());
    }
    ic0::msg_reply();
}
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    STATE.with(|slot| *slot.borrow_mut() = Some(State::open(false)));
}
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn reopen() {
    STATE.with(|slot| *slot.borrow_mut() = Some(State::open(true)));
}
#[unsafe(export_name = "canister_update put")]
extern "C" fn put() {
    let bytes = input::<16>();
    let key = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let value = u64::from_le_bytes(bytes[8..].try_into().unwrap());
    STATE.with(|slot| reply(&slot.borrow_mut().as_mut().unwrap().put(key, value)));
}
#[unsafe(export_name = "canister_query get")]
extern "C" fn get() {
    let key = u64::from_le_bytes(input::<8>());
    STATE.with(|slot| reply(&slot.borrow().as_ref().unwrap().get(key)));
}
#[unsafe(export_name = "canister_update remove")]
extern "C" fn remove() {
    let key = u64::from_le_bytes(input::<8>());
    STATE.with(|slot| reply(&slot.borrow_mut().as_mut().unwrap().remove(key)));
}
#[unsafe(export_name = "canister_query scan")]
extern "C" fn scan() {
    let _ = input::<0>();
    STATE.with(|slot| reply(&slot.borrow().as_ref().unwrap().scan()));
}
