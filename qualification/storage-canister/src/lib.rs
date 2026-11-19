//! Installed witness using only public allocation and upstream collection APIs.

use ic_platform_runtime::raw as ic0;
use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
    MemoryRuntime, RuntimeConstructionError, SealedDeclarationSnapshot, StableKey, VectorMemory,
    adapter::{StableBTreeMap, StableBTreeSet, StableCell},
};
use std::cell::RefCell;

// init
// Creates the three named collections on a fresh installation.
// scope  : IC installation hook
// limits : none specified
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    open(true);
}

// upgrade
// Reopens existing collections and optionally traps to exercise upgrade rollback.
// scope  : IC post-upgrade hook
// limits : traps on any nonempty argument
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn upgrade() {
    open(false);
    if ic0::msg_arg_data_size() != 0 {
        STORES.with(|state| {
            state.borrow_mut().as_mut().unwrap().revision.set(999);
        });
        ic0::trap(b"failed target startup");
    }
}

// put
// Updates a row, its index and the revision in one message.
// scope  : public
// limits : exactly 32 argument bytes, row payload up to 1 MiB
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update put")]
extern "C" fn put() {
    write();
}

// query_put
// Exercises query isolation by running the same writes as an update.
// scope  : public
// limits : exactly 32 argument bytes, row payload up to 1 MiB
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_query query_put")]
extern "C" fn query_put() {
    write();
}

// delete
// Removes a row and its index entry, with optional failure after both writes.
// scope  : public
// limits : exactly 32 argument bytes
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update delete")]
extern "C" fn delete() {
    let [key, _, _, fail] = args();
    STORES.with(|state| {
        let mut state = state.borrow_mut();
        let stores = state.as_mut().unwrap();
        stores.rows.remove(&key);
        stores.index.remove(&key);
    });
    if fail != 0 {
        ic0::trap(b"rollback deletion");
    }
    reply(&[]);
}

// get
// Returns the selected row payload or an empty reply when absent.
// scope  : public
// limits : exactly 32 argument bytes
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_query get")]
extern "C" fn get() {
    let [key, _, _, _] = args();
    STORES.with(|state| {
        reply(
            &state
                .borrow()
                .as_ref()
                .unwrap()
                .rows
                .get(&key)
                .unwrap_or_default(),
        )
    });
}

// stats
// Reports row count, index count, revision and physical stable pages.
// scope  : public
// limits : none specified
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_query stats")]
extern "C" fn stats() {
    STORES.with(|state| {
        let state = state.borrow();
        let stores = state.as_ref().unwrap();
        reply(
            &[
                stores.rows.len(),
                stores.index.len(),
                *stores.revision.get(),
                ic0::stable64_size(),
            ]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>(),
        );
    });
}

struct Stores {
    rows: StableBTreeMap<u64, Vec<u8>>,
    index: StableBTreeSet<u64>,
    revision: StableCell<u64>,
}

thread_local! {
    static STORES: RefCell<Option<Stores>> = const { RefCell::new(None) };
}

fn open(create: bool) {
    let mut host = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
    assert!(matches!(
        MemoryRuntime::new(DefaultMemoryImpl::default()),
        Err(RuntimeConstructionError::BackingAlreadyClaimed)
    ));
    let volatile = VectorMemory::default();
    let _volatile_host = MemoryRuntime::new(volatile.clone()).unwrap();
    assert!(matches!(
        MemoryRuntime::new(volatile),
        Err(RuntimeConstructionError::BackingAlreadyClaimed)
    ));
    let mut keys = vec![
        "app.rows.v1".to_string(),
        "app.index.v1".to_string(),
        "app.revision.v1".to_string(),
    ];
    keys.extend((0..4093).map(|i| format!("app.spare{i}.v1")));
    let declarations = SealedDeclarationSnapshot::new(
        keys.iter()
            .map(|key| MemoryRequest::new("app", key).unwrap())
            .collect(),
    )
    .unwrap();
    let grants = AllocationGrants::new(vec![MemoryAuthority::new("app", "app.").unwrap()]).unwrap();
    host.bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    let lease = |name| host.open_memory(&StableKey::parse(name).unwrap()).unwrap();
    let rows = lease("app.rows.v1");
    let index = lease("app.index.v1");
    let revision = lease("app.revision.v1");
    let stores = if create {
        Stores {
            rows: StableBTreeMap::create(rows),
            index: StableBTreeSet::create(index),
            revision: StableCell::create(revision, 0),
        }
    } else {
        Stores {
            rows: StableBTreeMap::load(rows),
            index: StableBTreeSet::load(index),
            revision: StableCell::load(revision).unwrap(),
        }
    };
    STORES.with(|state| *state.borrow_mut() = Some(stores));
}

fn args() -> [u64; 4] {
    assert_eq!(ic0::msg_arg_data_size(), 32);
    let mut bytes = [0; 32];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
}

fn reply(bytes: &[u8]) {
    ic0::msg_reply_data_append(bytes);
    ic0::msg_reply();
}

fn write() {
    let [key, value, size, fail] = args();
    assert!(size <= 1024 * 1024);
    STORES.with(|state| {
        let mut state = state.borrow_mut();
        let stores = state.as_mut().unwrap();
        stores.rows.insert(key, vec![value as u8; size as usize]);
        stores.index.insert(key);
        stores.revision.set(stores.revision.get() + 1);
    });
    if fail != 0 {
        ic0::trap(b"rollback rows, index, revision and allocation growth");
    }
    reply(&[]);
}
