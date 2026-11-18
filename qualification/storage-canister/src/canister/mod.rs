use ic_platform_runtime::raw as ic0;
use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StableKey,
    adapter::{StableBTreeMap, StableBTreeSet, StableCell},
};
use std::cell::RefCell;

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

/// Creates the three named collections on a fresh installation.
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_init")]
pub extern "C" fn init() {
    open(true);
}

/// Reopens existing collections and optionally traps to exercise upgrade rollback.
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_post_upgrade")]
pub extern "C" fn upgrade() {
    open(false);
    if ic0::msg_arg_data_size() != 0 {
        STORES.with(|state| {
            state.borrow_mut().as_mut().unwrap().revision.set(999);
        });
        ic0::trap(b"failed target startup");
    }
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

/// Updates a row, its index and the revision in one message.
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update put")]
pub extern "C" fn put() {
    write();
}

/// Exercises query isolation by running the same writes as an update.
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_query query_put")]
pub extern "C" fn query_put() {
    write();
}

/// Removes a row and its index entry, with optional failure after both writes.
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update delete")]
pub extern "C" fn delete() {
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

/// Returns the selected row payload or an empty reply when absent.
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_query get")]
pub extern "C" fn get() {
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

/// Reports row count, index count, revision and physical stable pages.
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_query stats")]
pub extern "C" fn stats() {
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
