//! Closed generation-owned map service; source-reviewed qualification only.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs, dead_code)]

#[expect(
    dead_code,
    reason = "shared controller also serves single-vector probes"
)]
mod region;
mod runtime;
#[cfg(session_bench)]
mod session_benchmark;

use core::{mem::MaybeUninit, ptr::NonNull};
use ic_storage::map::{Map, Slot};
use region::pool::{Pool, Root};
use runtime::abort;

impl region::sealed::Sealed for Slot {}
impl region::Element for Slot {}
const _: () = assert!(size_of::<Slot>() == 24 && align_of::<Slot>() == 8);

const BASE: usize = 8 * 1024 * 1024;
const PAYLOAD: usize = BASE + 128;
const ELEMENTS: usize = 8192;
const MAGIC: u64 = 0x315353455350414d;
type Owner = Map<&'static Pool<'static, Slot>>;

// Controlled startup clears these headers before target Rust runs. No private
// Rust representation lies in the retained interval or is read across upgrades.
static mut POOL: MaybeUninit<Pool<'static, Slot>> = MaybeUninit::uninit();
static mut MAPS: MaybeUninit<[Owner; 2]> = MaybeUninit::uninit();
static mut PHASE: u32 = 0; // 0 fresh, 1 closed, 2 open, 3 executing
static mut ADMISSIONS: u64 = 0;
static mut CALLS: u64 = 0;
static mut COST: u64 = 0;
#[cfg(not(session_b))]
static VERSION: u64 = 101;
#[cfg(session_b)]
static VERSION: u64 = 202;
#[cfg(session_b)]
static EXTRA: [u8; 513] = [37; 513];

// Narrow scalar diagnostic protocol. The admitted host transform connects
// these three non-reentrant functions to IC reply/counter imports.
#[expect(unsafe_code, reason = "closed diagnostic host protocol")]
#[link(wasm_import_module = "session_provider")]
unsafe extern "C" {
    safe fn reply();
    unsafe fn reply_data(pointer: *const u8, length: usize);
    safe fn counter(kind: i32) -> u64;
}

fn word(value: u64) {
    let bytes = value.to_le_bytes();
    // SAFETY: synchronous reply copies exactly this live stack value.
    #[expect(unsafe_code, reason = "bounded diagnostic scalar reply")]
    unsafe {
        reply_data(bytes.as_ptr(), bytes.len())
    }
}

// SAFETY obligations for the functions below: the admitted module is single
// threaded, exports never suspend, imports cannot re-enter, and startup zeros
// the complete transient interval before any Rust entrypoint. The retained
// allocation is disjoint from stack/data, valid for the whole module generation,
// and exclusively owned by this fixture. Phase 3 denies overlapping entry.
#[expect(
    unsafe_code,
    reason = "private retained header reads after artifact admission"
)]
fn read(index: usize) -> u64 {
    assert!(index < 12);
    unsafe { ((BASE as *const u64).add(index)).read() }
}
#[expect(
    unsafe_code,
    reason = "private retained header writes under provider custody"
)]
fn write(index: usize, value: u64) {
    assert!(index < 12);
    unsafe { ((BASE as *mut u64).add(index)).write(value) }
}

fn root(index: usize) -> (Root, usize) {
    let at = 4 + index * 4;
    let capacity = usize::try_from(read(at + 2)).unwrap_or_else(|_| abort());
    (
        Root {
            start: usize::try_from(read(at)).unwrap_or_else(|_| abort()),
            length: usize::try_from(read(at + 1)).unwrap_or_else(|_| abort()),
            capacity,
        },
        usize::try_from(read(at + 3)).unwrap_or_else(|_| abort()),
    )
}

#[expect(
    unsafe_code,
    reason = "generation-owned static controller initialization"
)]
fn open() {
    let start = counter(0);
    unsafe {
        assert!(matches!(PHASE, 0 | 1));
        assert_eq!(read(0), MAGIC);
        assert_eq!(read(1), 1); // closed, complete root publication
        assert_eq!(read(2), ELEMENTS as u64);
        assert_eq!(read(3), 1); // native slot/schema contract
        assert_eq!(core::arch::wasm32::memory_size::<0>(), 132);
        let roots = [root(0), root(1)];
        PHASE = 3;
        // No old owner exists: initial startup, successful close, or an upgrade
        // whose source published closed roots. The raw allocation is private to
        // this generation, not a stack borrow with an extended lifetime.
        let pool = Pool::claim(
            NonNull::new(PAYLOAD as *mut u8).unwrap(),
            ELEMENTS,
            &roots.map(|(root, _)| root),
        )
        .unwrap_or_else(|_| abort());
        (&raw mut POOL).write(MaybeUninit::new(pool));
        let pool = (&raw const POOL).as_ref().unwrap().assume_init_ref();
        let maps = core::array::from_fn(|index| {
            Map::admit(
                pool.restore(index).unwrap_or_else(|_| abort()),
                roots[index].1,
                3072,
            )
            .unwrap_or_else(|_| abort())
        });
        (&raw mut MAPS).write(MaybeUninit::new(maps));
        ADMISSIONS += 1;
        write(1, 0); // old descriptors are unavailable while the service owns runs
        PHASE = 2;
        COST = counter(0) - start;
    }
}

#[expect(
    unsafe_code,
    reason = "end every owner before root publication and pool teardown"
)]
fn close(fail: bool) {
    unsafe {
        assert!(PHASE == 2);
        PHASE = 3;
        let maps = (&raw const MAPS).read().assume_init();
        let pool = (&raw const POOL).as_ref().unwrap().assume_init_ref();
        let roots = maps.map(|map| {
            let (slots, len) = map.release().unwrap_or_else(|_| abort());
            (
                pool.release(slots)
                    .unwrap_or_else(|_| abort())
                    .unwrap_or_else(|| abort()),
                len,
            )
        });
        pool.finish().unwrap_or_else(|_| abort());
        // No allocator reference or map survives this drop.
        (&raw mut POOL).as_mut().unwrap().assume_init_drop();
        for (index, (root, len)) in roots.into_iter().enumerate() {
            for (field, value) in [root.start, root.length, root.capacity, len]
                .into_iter()
                .enumerate()
            {
                write(4 + index * 4 + field, value as u64);
            }
            if fail && index == 0 {
                abort();
            }
        }
        write(1, 1);
        PHASE = 1;
    }
}

#[expect(
    unsafe_code,
    reason = "single-threaded generation owner with non-reentrant scalar callbacks"
)]
fn access(operation: u32) {
    unsafe {
        assert!(PHASE == 2);
        PHASE = 3;
        let start = counter(0);
        let maps = (&raw mut MAPS).as_mut().unwrap().assume_init_mut();
        for (id, map) in maps.iter_mut().enumerate() {
            map.with(|mut view| match operation {
                0 => {
                    for key in 0..1536 {
                        view.insert(key, key + 1000 + id as u64).unwrap();
                    }
                }
                1 => assert!(view.get(7).is_some()),
                2 | 5 => {
                    view.insert(7, 9000 + id as u64).unwrap();
                }
                3 => {
                    assert!(view.remove(7).is_some());
                }
                4 => {
                    assert_eq!(view.insert(7, 1007 + id as u64).unwrap(), None);
                }
                6 => {
                    word(view.len() as u64);
                    view.for_each(|key, value| {
                        word(key);
                        word(value);
                    });
                }
                _ => abort(),
            })
            .unwrap();
        }
        if operation == 5 {
            abort();
        }
        COST = counter(0) - start;
        CALLS += 1;
        PHASE = 2;
    }
}

#[expect(
    unsafe_code,
    reason = "closed lifecycle initialization of the retained allocation"
)]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    unsafe {
        assert!(PHASE == 0);
        assert_eq!(core::arch::wasm32::memory_grow::<0>(3), 129);
        (PAYLOAD as *mut Slot).write_bytes(0, ELEMENTS);
    }
    for (index, value) in [MAGIC, 1, ELEMENTS as u64, 1, 0, 8, 8, 0, 8, 8, 8, 0]
        .into_iter()
        .enumerate()
    {
        write(index, value);
    }
    open();
}

#[expect(
    unsafe_code,
    reason = "source closes its generation before a compatible upgrade"
)]
#[unsafe(export_name = "canister_pre_upgrade")]
extern "C" fn pre() {
    #[cfg(not(session_no_close))]
    close(cfg!(session_fail_pre));
}

#[expect(
    unsafe_code,
    reason = "controlled startup has discarded all old transient headers"
)]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn post() {
    #[cfg(session_corrupt_post)]
    write(7, read(7) + 1);
    open();
    #[cfg(session_fail_post)]
    abort();
}

#[expect(unsafe_code, reason = "bounded transient service status query")]
#[unsafe(export_name = "canister_query session_status")]
extern "C" fn status() {
    #[cfg(session_b)]
    core::hint::black_box(&EXTRA);
    unsafe {
        for value in [VERSION, PHASE as u64, ADMISSIONS, CALLS, COST] {
            word(value);
        }
    }
    reply();
}

#[expect(
    unsafe_code,
    reason = "non-reentrant diagnostic copies initialized retained bytes while no access frame exists"
)]
#[unsafe(export_name = "canister_query session_bytes")]
extern "C" fn bytes() {
    unsafe {
        assert!(matches!(PHASE, 1 | 2));
        reply_data(BASE as *const u8, 128 + ELEMENTS * size_of::<Slot>());
    }
    reply();
}

macro_rules! operation {
    ($function:ident, $export:literal, $operation:expr) => {
        #[expect(unsafe_code, reason = "closed synchronous diagnostic entrypoint")]
        #[unsafe(export_name = $export)]
        extern "C" fn $function() {
            access($operation);
            reply();
        }
    };
}
operation!(fill, "canister_update session_fill", 0);
operation!(lookup, "canister_update session_lookup", 1);
operation!(replace, "canister_update session_replace", 2);
operation!(delete, "canister_update session_delete", 3);
operation!(reinsert, "canister_update session_reinsert", 4);
operation!(trap, "canister_update session_trap", 5);
operation!(state, "canister_query session_state", 6);
operation!(query_write, "canister_query session_query_write", 2);

#[expect(unsafe_code, reason = "explicit diagnostic close boundary")]
#[unsafe(export_name = "canister_update session_close")]
extern "C" fn park() {
    close(false);
    reply();
}
#[expect(unsafe_code, reason = "diagnostic partial-publication trap")]
#[unsafe(export_name = "canister_update session_close_trap")]
extern "C" fn park_trap() {
    close(true);
    reply();
}
#[expect(unsafe_code, reason = "explicit diagnostic readmission boundary")]
#[unsafe(export_name = "canister_update session_open")]
extern "C" fn reopen() {
    open();
    reply();
}
