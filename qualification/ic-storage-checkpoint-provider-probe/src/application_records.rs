//! Supplier qualification through the same public API offered to applications.
//! No raw memory partitioner, checkpoint-region handle or native provider arena.
use ic_platform_runtime::raw as ic0;
use ic_storage::records::{Error, Participant, Plan, Root, StableMemory, Store};
use std::cell::RefCell;

const BUILD: u64 = if cfg!(feature = "version-b") {
    202
} else {
    101
};
struct State {
    store: Store<StableMemory>,
    root: Participant<StableMemory>,
    entry: Participant<StableMemory>,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

fn plan(schema: u64) -> Plan {
    Plan::new(
        &[
            Root {
                id: 1,
                participant: 10,
                schema,
                capacity: 16_384,
            },
            Root {
                id: 2,
                participant: 10,
                schema: 1,
                capacity: 2 * 524_288 + 24,
            },
            Root {
                id: 3,
                participant: 20,
                schema: 1,
                capacity: 128,
            },
        ],
        64,
    )
    .unwrap()
}
fn words<const N: usize>() -> [u64; N] {
    assert_eq!(ic0::msg_arg_data_size() as usize, N * 8);
    let mut bytes = vec![0; N * 8];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    std::array::from_fn(|index| {
        u64::from_le_bytes(bytes[index * 8..index * 8 + 8].try_into().unwrap())
    })
}
fn open(fresh: bool, schema: u64) {
    let memory = StableMemory::take().unwrap();
    assert!(matches!(StableMemory::take(), Err(Error::Claimed)));
    let plan = plan(schema);
    assert_eq!(plan.physical_pages(), 17);
    let mut store = if fresh {
        Store::create(memory, &plan, &[b"intent", b"custody", b""]).unwrap()
    } else {
        Store::open(memory, &plan).unwrap()
    };
    let root = store.take(10).unwrap();
    let entry = store.take(20).unwrap();
    assert!(matches!(store.take(10), Err(Error::Claimed)));
    entry
        .read(|read| assert_eq!(read.get(1), Err(Error::Authority)))
        .unwrap();
    STATE.with_borrow_mut(|state| {
        assert!(state.is_none());
        *state = Some(State { store, root, entry });
    });
}
#[expect(
    unsafe_code,
    reason = "fixture-owned ordinary application initialization export"
)]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    assert_eq!(ic0::msg_arg_data_size(), 0);
    open(true, 1);
}

#[expect(
    unsafe_code,
    reason = "fixture-owned ordinary application reopen export"
)]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn post_upgrade() {
    let [schema, fail] = words();
    assert!(fail <= 1);
    open(false, schema);
    if fail == 1 {
        STATE.with_borrow_mut(|state| {
            state
                .as_mut()
                .unwrap()
                .root
                .update(|mut write| write.set(1, b"failed target write"))
                .unwrap()
                .unwrap()
        });
        ic0::trap(b"target after record write");
    }
}

fn reply_state() {
    ic0::msg_reply_data_append(&BUILD.to_le_bytes());
    STATE.with_borrow(|state| {
        let state = state.as_ref().unwrap();
        state
            .store
            .read(&[&state.root, &state.entry], |read| {
                for id in 1..=3 {
                    let bytes = read.get(id).unwrap();
                    ic0::msg_reply_data_append(&(bytes.len() as u64).to_le_bytes());
                    ic0::msg_reply_data_append(bytes);
                }
            })
            .unwrap();
    });
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "fixture-owned cached application query")]
#[unsafe(export_name = "canister_query state")]
extern "C" fn state() {
    assert_eq!(ic0::msg_arg_data_size(), 0);
    reply_state();
}

fn change(first: usize, second: usize, fail: u64) -> Result<(), Error> {
    assert!(first <= 16_385 && second <= 2 * 524_288 + 25 && fail <= 4);
    STATE.with_borrow_mut(|state| {
        let State { store, root, entry } = state.as_mut().unwrap();
        store
            .update(&[root, entry], |mut write| {
                write.set(1, &vec![7; first])?;
                if fail == 1 {
                    ic0::trap(b"after first record");
                }
                write.set(2, &vec![9; second])?;
                if fail == 2 {
                    ic0::trap(b"after second record");
                }
                write.set(3, b"validated worker binding")?;
                if fail == 3 {
                    ic0::trap(b"after participant record");
                }
                Ok(())
            })
            .unwrap()
    })
}
fn encode_reply(refused: bool, fail: bool) -> Result<Vec<u8>, &'static str> {
    if fail {
        Err("reply encoder refuses after writes")
    } else {
        Ok(u64::from(refused).to_le_bytes().to_vec())
    }
}
#[expect(
    unsafe_code,
    reason = "fixture-owned composed synchronous application write"
)]
#[unsafe(export_name = "canister_update write")]
extern "C" fn write() {
    let started = ic0::instruction_counter();
    let [first, second, fail] = words();
    let result = change(first as usize, second as usize, fail);
    assert!(result.is_ok() || result == Err(Error::Capacity));
    // Deliberately outside Storage's scope: the IC must roll back all earlier
    // record/cached-root changes when an application's response encoder fails.
    let reply = encode_reply(result.is_err(), fail == 4)
        .unwrap_or_else(|message| ic0::trap(message.as_bytes()));
    let instructions = ic0::instruction_counter() - started;
    ic0::msg_reply_data_append(&reply);
    ic0::msg_reply_data_append(&instructions.to_le_bytes());
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "fixture-owned speculative query rollback diagnostic"
)]
#[unsafe(export_name = "canister_query speculative")]
extern "C" fn speculative() {
    change(31, 41, 0).unwrap();
    reply_state();
}
