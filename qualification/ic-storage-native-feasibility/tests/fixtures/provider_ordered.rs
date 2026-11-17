//! Closed installed ordered-root experiment; no retained Rust headers or allocator.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs, dead_code)]
extern crate alloc;

mod ordered_runtime;
mod ordered_session;
#[expect(
    dead_code,
    reason = "shared owner also supplies other closed collection profiles"
)]
mod region;

use alloc::vec::Vec;
use core::ops::Bound::{Excluded, Included};
use core::sync::atomic::{AtomicU64, Ordering};
use ic_platform_runtime::raw as ic0;
use ic_storage::{
    checkpoint::{Memory, Record, Specification, Store},
    ordered::{Mutation, Node},
};
use ordered_runtime::{A, B, Stable, enter, header, publish};

type Element = Node<512, 4096>;
impl region::sealed::Sealed for Element {}
impl region::Element for Element {}
const _: () = assert!(size_of::<Element>() == 56 && align_of::<Element>() == 8);
const ELEMENTS: usize = 256;
const PAYLOAD_OFFSET: usize = 16 * 1024;
const PAYLOAD_BYTES: usize = 1024 * 1024;
const WINDOW: usize = PAYLOAD_OFFSET + PAYLOAD_BYTES;
const _: () = assert!(ELEMENTS * size_of::<Element>() <= PAYLOAD_OFFSET);
const _: () = assert!(WINDOW <= B - A && B + WINDOW <= 8 * 1024 * 1024);
const LIMIT: usize = 128;
const MAGIC: u64 = 0x4f52444e41543033;
const BUILD: u64 = if cfg!(ordered_b) { 2 } else { 1 };
const BASE: usize = if cfg!(ordered_recovery) { B } else { A };
static LAST_COST: AtomicU64 = AtomicU64::new(0);
static LAST_SCRATCH: AtomicU64 = AtomicU64::new(0);

fn args() -> [u64; 3] {
    assert_eq!(ic0::msg_arg_data_size(), 24);
    let mut bytes = [0; 24];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    core::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
}
fn cell(create: bool) -> Record<Vec<u8>, Stable> {
    let specs = [Specification {
        id: 40,
        bytes: 16 + 1024 * 1024,
    }];
    let mut store = if create {
        Store::create(Stable, &specs)
    } else {
        Store::open(Stable, &specs)
    }
    .unwrap();
    let region = store.take(40).unwrap();
    if create {
        Record::create(region, Vec::new())
    } else {
        Record::load(region, 1024 * 1024)
    }
    .unwrap()
}
fn checked() -> [u64; 14] {
    let words = header();
    assert_eq!(words[0], MAGIC);
    assert_eq!(words[1], BASE as u64);
    assert!(words[9] <= 1 && words[13] <= 1);
    words
}
fn finish_cost(start: u64) {
    LAST_COST.store(ic0::instruction_counter() - start, Ordering::Relaxed);
    LAST_SCRATCH.store(ordered_runtime::scratch_bytes(), Ordering::Relaxed);
}

fn with_map<R>(
    words: &mut [u64; 14],
    operation: impl FnOnce(&mut ordered_session::Owner) -> R,
) -> R {
    assert_eq!(*words, header());
    ordered_session::with(operation)
}

fn primary(id: u64) -> [u8; 9] {
    let mut key = [1; 9];
    key[1..].copy_from_slice(&id.to_be_bytes());
    key
}

#[expect(
    unsafe_code,
    reason = "measured exact lookup on an already admitted owner"
)]
#[unsafe(export_name = "canister_update lookup")]
extern "C" fn lookup() {
    enter();
    let start = ic0::instruction_counter();
    let [id, expected, _] = args();
    let value = ordered_session::with(|map| {
        map.with(|view| {
            let value = view.get(&primary(id)).unwrap();
            assert_eq!(
                u64::from_le_bytes(value.as_slice()[..8].try_into().unwrap()),
                expected
            );
            u64::from_le_bytes(value.as_slice()[..8].try_into().unwrap())
        })
        .unwrap()
    });
    finish_cost(start);
    ic0::msg_reply_data_append(&value.to_le_bytes());
    ic0::msg_reply();
}
fn index(id: u64, value: u64) -> [u8; 10] {
    let mut key = [2; 10];
    key[1] = (value % 8) as u8;
    key[2..].copy_from_slice(&id.to_be_bytes());
    key
}

#[expect(unsafe_code, reason = "fixture-owned initialization entry")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    enter();
    assert_eq!(ic0::msg_arg_data_size(), 0);
    assert_eq!(header(), [0; 14]);
    let _ = cell(true);
    publish(
        [MAGIC, BASE as u64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        false,
    );
    ordered_session::open();
}

#[expect(unsafe_code, reason = "fixture-owned native validation before upgrade")]
#[unsafe(export_name = "canister_pre_upgrade")]
extern "C" fn pre_upgrade() {
    enter();
    #[cfg(not(ordered_no_close))]
    {
        if !ordered_session::is_open() {
            ordered_session::open();
        }
        ordered_session::close(cfg!(ordered_fail_pre));
    }
}

#[expect(
    unsafe_code,
    reason = "fixture-owned compatible or explicit replacement admission"
)]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn post_upgrade() {
    enter();
    let start = ic0::instruction_counter();
    let [mode, epoch, fault] = args();
    assert!(fault <= 3);
    if mode == 0 {
        ordered_session::open();
        ordered_session::with(|map| {
            if fault != 0 {
                map.with(|mut view| view.insert(&primary(0), &[255; 4096]).unwrap())
                    .unwrap();
                assert!(fault != 1);
            }
        });
    } else {
        assert_eq!(mode, 1);
        assert_eq!(header(), [0; 14], "replacement required");
        let checkpoint = cell(false);
        publish(
            [MAGIC, BASE as u64, 0, 0, 0, 0, 0, 0, epoch, 0, 0, 0, 0, 1],
            false,
        );
        ordered_session::open();
        ordered_session::with(|map| {
            map.restore(checkpoint.get(), epoch).unwrap();
            assert!(fault != 1);
        });
    }
    assert!(fault != 2);
    if fault == 3 {
        ordered_session::close(true);
    }
    finish_cost(start);
}

#[expect(
    unsafe_code,
    reason = "fixture-owned atomic row and secondary-index update"
)]
#[unsafe(export_name = "canister_update put")]
extern "C" fn put() {
    enter();
    let start = ic0::instruction_counter();
    let [id, value, fault] = args();
    assert!(fault <= 3);
    let mut words = checked();
    assert_eq!(words[9], 0, "frozen");
    with_map(&mut words, |map| {
        map.with(|mut view| {
            let key = primary(id);
            let next = index(id, value);
            let old = view
                .get(&key)
                .map(|bytes| u64::from_le_bytes(bytes.as_slice()[..8].try_into().unwrap()));
            let mut row = [0; 4096];
            row[..8].copy_from_slice(&value.to_le_bytes());
            row[8..].fill((id ^ value) as u8);
            if fault == 1 {
                view.insert(&key, &row).unwrap();
                panic!("between row and index");
            }
            if let Some(old) = old {
                view.apply(&[
                    Mutation::Remove(&index(id, old)),
                    Mutation::Put(&key, &row),
                    Mutation::Put(&next, &key),
                ])
                .unwrap();
            } else {
                view.apply(&[Mutation::Put(&key, &row), Mutation::Put(&next, &key)])
                    .unwrap();
            }
        })
        .unwrap()
    });
    assert!(fault != 2);
    publish(words, fault == 3);
    finish_cost(start);
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "fixture-owned row and secondary-index deletion")]
#[unsafe(export_name = "canister_update delete")]
extern "C" fn delete() {
    enter();
    let [id, _, fault] = args();
    assert!(fault <= 1);
    let mut words = checked();
    assert_eq!(words[9], 0);
    with_map(&mut words, |map| {
        map.with(|mut view| {
            let key = primary(id);
            if let Some(value) = view.get(&key) {
                let value = u64::from_le_bytes(value.as_slice()[..8].try_into().unwrap());
                view.apply(&[Mutation::Remove(&key), Mutation::Remove(&index(id, value))])
                    .unwrap();
            }
            assert_eq!(fault, 0);
        })
        .unwrap()
    });
    publish(words, false);
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "fixture-owned stable publication and source freeze"
)]
#[unsafe(export_name = "canister_update checkpoint")]
extern "C" fn checkpoint() {
    enter();
    let start = ic0::instruction_counter();
    let [epoch, _, fault] = args();
    assert!(fault <= 3);
    let mut words = checked();
    assert_eq!(words[9], 0);
    assert!(epoch > words[8]);
    let mut record = cell(false);
    with_map(&mut words, |map| {
        record.set(map.checkpoint(epoch).unwrap()).unwrap();
        assert!(fault != 1);
    });
    words[8] = epoch;
    words[9] = 1;
    assert!(fault != 2);
    publish(words, fault == 3);
    finish_cost(start);
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "fixture-owned read-only logical evidence")]
#[unsafe(export_name = "canister_query records")]
extern "C" fn records() {
    enter();
    let mut words = checked();
    with_map(&mut words, |map| {
        ic0::msg_reply_data_append(&map.checkpoint(0).unwrap());
    });
    assert_eq!(words, header());
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "fixture-owned bounded secondary-index pagination"
)]
#[unsafe(export_name = "canister_query page")]
extern "C" fn page() {
    enter();
    let [group, after, flags] = args();
    assert!(group < 8 && flags <= 3);
    let reverse = flags & 1;
    let has_cursor = flags & 2 != 0;
    let mut words = checked();
    with_map(&mut words, |map| {
        map.with(|view| {
            let prefix = [2, group as u8];
            let end = [2, group as u8 + 1];
            let cursor = index(after, group);
            let lower = if reverse == 0 && has_cursor {
                Excluded(cursor.as_slice())
            } else {
                Included(prefix.as_slice())
            };
            let upper = if reverse == 1 && has_cursor {
                Excluded(cursor.as_slice())
            } else {
                Excluded(end.as_slice())
            };
            let mut range = view.range(lower, upper);
            for _ in 0..5 {
                let entry = if reverse == 0 {
                    range.next()
                } else {
                    range.next_back()
                };
                let Some((key, owner)) = entry else {
                    break;
                };
                let id = u64::from_be_bytes(key.as_slice()[2..].try_into().unwrap());
                let value = view.get(owner.as_slice()).unwrap();
                ic0::msg_reply_data_append(&id.to_le_bytes());
                ic0::msg_reply_data_append(&value.as_slice()[..8]);
            }
        })
        .unwrap()
    });
    assert_eq!(words, header());
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "fixture-owned raw evidence without interpreting damaged nodes"
)]
#[unsafe(export_name = "canister_query observe")]
extern "C" fn observe() {
    assert!(ordered_session::is_open() || header()[13] == 1);
    for word in header() {
        ic0::msg_reply_data_append(&word.to_le_bytes());
    }
    ordered_runtime::reply_native(BASE, WINDOW);
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "fixture-owned measured entry-body cost evidence"
)]
#[unsafe(export_name = "canister_query costs")]
extern "C" fn costs() {
    let stats = ordered_session::statistics();
    for word in [
        BUILD,
        LAST_COST.load(Ordering::Relaxed),
        LAST_SCRATCH.load(Ordering::Relaxed),
        Stable.size(),
        8 * 1024 * 1024,
        WINDOW as u64,
        stats[3],
        stats[4],
    ] {
        ic0::msg_reply_data_append(&word.to_le_bytes());
    }
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "closed corruption injection; next admission must refuse"
)]
#[unsafe(export_name = "canister_update damage")]
extern "C" fn damage() {
    enter();
    let [kind, _, _] = args();
    ordered_session::close(false);
    let mut words = checked();
    if kind == 0 {
        words[5] = u64::MAX;
        publish(words, false);
    } else if kind == 2 {
        // The former inline-node representation must never be reinterpreted.
        words[0] = 0x4f52444e41543031;
        publish(words, false);
    } else if kind == 3 {
        // Valid byte-pool geometry, but a truncated owned payload partition.
        words[11] -= 1;
        publish(words, false);
    } else if kind == 4 {
        // Compact per-message owners used a different publication contract.
        words[0] = 0x4f52444e41543032;
        publish(words, false);
    } else {
        assert_eq!(kind, 1);
        let mut record = cell(false);
        let mut bytes = record.get().clone();
        bytes[0] ^= 1;
        record.set(bytes).unwrap();
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "diagnostic session state without owner access")]
#[unsafe(export_name = "canister_query session_status")]
extern "C" fn session_status() {
    for word in ordered_session::statistics() {
        ic0::msg_reply_data_append(&word.to_le_bytes());
    }
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "explicit end of both native owners")]
#[unsafe(export_name = "canister_update session_close")]
extern "C" fn session_close() {
    enter();
    let [fault, _, _] = args();
    assert!(fault <= 1);
    ordered_session::close(fault != 0);
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "explicit readmission after complete settlement")]
#[unsafe(export_name = "canister_update session_open")]
extern "C" fn session_open() {
    enter();
    ordered_session::open();
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "query isolation of native data and generation owners"
)]
#[unsafe(export_name = "canister_query query_put")]
extern "C" fn query_put() {
    put();
}
