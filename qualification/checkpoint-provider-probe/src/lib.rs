//! Ordinary-upgrade checkpoint provider fixture. All stable access is governed.
//! No EOP retention marker, retained Rust pointers or raw stable IO is used here.

use std::{
    cell::RefCell,
    sync::atomic::{AtomicU64, Ordering},
};

use ic_platform_runtime::ic0;
use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StableKey, adapter::StableCell,
};
use ic_storage_native_feasibility::checkpoint;

const OWNER: &str = "native-checkpoint";
const PREFIX: &str = "native.checkpoint.";
const KEY: &str = "native.checkpoint.scalar.v1";
const MAX_CHECKPOINT_BYTES: usize = 248;
const BUILD: u64 = if cfg!(feature = "version-b") {
    202
} else {
    101
};
static TRANSIENT: AtomicU64 = AtomicU64::new(BUILD);

struct State {
    cell: StableCell<Vec<u8>>,
    counter: u64,
    values: Vec<u64>,
    epoch: u64,
    frozen: bool,
    restored: bool,
}

thread_local! {static STATE: RefCell<Option<State>> = const {RefCell::new(None)};}

fn cell(create: bool) -> StableCell<Vec<u8>> {
    // Runtime and leases are always new transient objects. There is one owner
    // per backing. Post-upgrade requires the existing named allocation before
    // bootstrap; it never creates or resets a missing checkpoint cell.
    let mut runtime = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
    let key = StableKey::parse(KEY).unwrap();
    if !create {
        assert!(
            runtime.allocation_id(&key).is_some(),
            "missing checkpoint locator"
        );
    }
    let declarations =
        SealedDeclarationSnapshot::new(vec![MemoryRequest::new(OWNER, KEY).unwrap()]).unwrap();
    let grants = AllocationGrants::new(vec![MemoryAuthority::new(OWNER, PREFIX).unwrap()]).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    let memory = runtime.open_memory(&key).unwrap();
    if create {
        StableCell::create(memory, Vec::new()).unwrap()
    } else {
        StableCell::load(memory, MAX_CHECKPOINT_BYTES).unwrap()
    }
}

fn args<const N: usize>() -> [u64; N] {
    let length = N * 8;
    assert_eq!(ic0::msg_arg_data_size() as usize, length);
    let mut bytes = vec![0; length];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().unwrap()))
}

fn reply_words(words: &[u64]) {
    for value in words {
        ic0::msg_reply_data_append(&value.to_le_bytes());
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "fixture-owned IC initialization export")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    assert_eq!(ic0::msg_arg_data_size(), 0);
    STATE.with(|slot| {
        *slot.borrow_mut() = Some(State {
            cell: cell(true),
            counter: 41,
            values: vec![3, 5, 8],
            epoch: 7,
            frozen: false,
            restored: false,
        })
    });
}

#[expect(unsafe_code, reason = "fixture-owned ordinary upgrade import export")]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn post_upgrade() {
    let [epoch, fail_after_write] = args();
    assert!(fail_after_write <= 1);
    assert_eq!(
        TRANSIENT.load(Ordering::Relaxed),
        BUILD,
        "old heap was not discarded"
    );
    STATE.with(|slot| assert!(slot.borrow().is_none(), "old state must be unavailable"));
    let mut cell = cell(false);
    let decoded = checkpoint::decode(cell.get(), epoch).unwrap();
    if fail_after_write == 1 {
        cell.set(checkpoint::encode(99999, &decoded.values, epoch).unwrap());
        ic0::trap(b"injected failure after stable checkpoint mutation");
    }
    STATE.with(|slot| {
        *slot.borrow_mut() = Some(State {
            cell,
            counter: decoded.counter,
            values: decoded.values,
            epoch: decoded.source_epoch,
            frozen: false,
            restored: true,
        })
    });
}

#[expect(unsafe_code, reason = "fixture-owned mutation export")]
#[unsafe(export_name = "canister_update append")]
extern "C" fn append() {
    let [value] = args();
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut().unwrap();
        assert!(!state.frozen && state.values.len() < 16);
        state.counter = state.counter.checked_add(1).unwrap();
        state.epoch = state.epoch.checked_add(1).unwrap();
        state.values.push(value);
    });
    TRANSIENT.store(999, Ordering::Relaxed);
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "fixture-owned checkpoint publication export")]
#[unsafe(export_name = "canister_update export_checkpoint")]
extern "C" fn export_checkpoint() {
    assert_eq!(ic0::msg_arg_data_size(), 0);
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut().unwrap();
        state
            .cell
            .set(checkpoint::encode(state.counter, &state.values, state.epoch).unwrap());
        state.frozen = true;
    });
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "fixture-only corruption through its governed cell"
)]
#[unsafe(export_name = "canister_update damage_checkpoint")]
extern "C" fn damage_checkpoint() {
    let [mode] = args();
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut().unwrap();
        assert!(state.frozen);
        let mut bytes = state.cell.get().clone();
        match mode {
            0 => {
                bytes.pop().unwrap();
            }
            1 => {
                bytes[80] ^= 1;
            }
            2 => {
                bytes[24] = 2;
            }
            _ => panic!("unknown corruption mode"),
        }
        state.cell.set(bytes);
    });
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "fixture-owned logical state query")]
#[unsafe(export_name = "canister_query state")]
extern "C" fn state() {
    STATE.with(|slot| {
        let slot = slot.borrow();
        let state = slot.as_ref().unwrap();
        let mut words = vec![
            BUILD,
            TRANSIENT.load(Ordering::Relaxed),
            u64::from(state.restored),
            u64::from(state.frozen),
            state.counter,
            state.epoch,
            state.values.len() as u64,
        ];
        words.extend_from_slice(&state.values);
        reply_words(&words);
    });
}

#[expect(unsafe_code, reason = "fixture-owned checkpoint evidence query")]
#[unsafe(export_name = "canister_query checkpoint_bytes")]
extern "C" fn checkpoint_bytes() {
    STATE.with(|slot| ic0::msg_reply_data_append(slot.borrow().as_ref().unwrap().cell.get()));
    ic0::msg_reply();
}
