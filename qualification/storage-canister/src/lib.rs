//! Installed witness using only public allocation and upstream collection APIs.

use ic_platform_runtime::raw as ic0;
use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
    MemoryRuntime, RuntimeConstructionError, SealedDeclarationSnapshot, StableKey, VectorMemory,
    adapter::{StableBTreeMap, StableBTreeSet, StableCell},
};
use std::{cell::RefCell, collections::BTreeSet};

// init
// Creates the three named collections on a fresh installation.
// limits : default
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    open(true);
}

// upgrade
// Reopens existing collections and traps on a nonempty upgrade argument to exercise rollback.
// limits : request must be empty for a successful upgrade
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
// Writes up to 1 MiB to a row and updates its index and revision in one message.
// limits : request exactly 32 bytes, empty reply
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update put")]
extern "C" fn put() {
    write();
}

// query_put
// Exercises query isolation with the same writes and 1 MiB row limit as an update.
// limits : request exactly 32 bytes, empty reply
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_query query_put")]
extern "C" fn query_put() {
    write();
}

// delete
// Removes a row and its index entry, with optional failure after both writes.
// limits : request exactly 32 bytes, empty reply
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
// limits : request exactly 32 bytes, reply up to 1 MiB
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
// limits : default request, 32-byte reply
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

// seed_delete_rows
// Inserts a bounded chunk of the physical records for matched deletion measurements.
// limits : request exactly 32 bytes, empty reply
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update seed_delete_rows")]
extern "C" fn seed_delete_rows() {
    let [start, count, _, _] = args();
    assert!(count <= 64 && start <= 2048 - count);
    STORES.with_borrow_mut(|state| {
        let rows = &mut state.as_mut().unwrap().rows;
        for key in start..start + count {
            let size = if key % 13 == 0 { 8192 } else { 15 };
            rows.insert(key, vec![key as u8; size]);
        }
    });
    reply(&[]);
}

// measure_delete
// Deletes up to 1,022 prepared physical keys, trapping on an unexpected removal count.
// limits : request exactly 32 bytes, 32-byte reply
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update measure_delete")]
extern "C" fn measure_delete() {
    let [count, stride, bulk, expected] = args();
    assert!(count <= 1022 && matches!(stride, 1 | 2) && bulk <= 1);
    let initial_heap = HEAP.current_usage();
    HEAP.reset_peak_usage();
    let initial_pages = core::arch::wasm32::memory_size::<0>();
    let start = ic0::instruction_counter();
    let keys: BTreeSet<_> = (0..count).map(|i| i * stride).collect();

    let removed = STORES.with_borrow_mut(|state| {
        let rows = &mut state.as_mut().unwrap().rows;
        if bulk == 1 {
            rows.remove_many(keys)
        } else {
            let mut removed = 0;
            for key in keys {
                removed += u64::from(rows.remove(&key).is_some());
            }

            removed
        }
    });
    if removed != expected {
        ic0::trap(b"prepared removal count changed");
    }
    let instructions = ic0::instruction_counter() - start;
    let growth = core::arch::wasm32::memory_size::<0>() - initial_pages;
    let working_heap = HEAP.peak_usage() - initial_heap;
    let words = [removed, instructions, growth as u64, working_heap as u64];
    reply(
        &words
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>(),
    );
}

#[global_allocator]
static HEAP: peak_alloc::PeakAlloc = peak_alloc::PeakAlloc;

///
/// Stores
/// The row, index and revision collections opened by the installed witness.
///

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
