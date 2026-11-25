//! Compare a one-key probe after consuming the same page on the same installed map.
#[path = "../examples/read_paths.rs"]
mod fixture;

use fixture::{HEAP, MAP, NODES, VALUES, args};
use ic_platform_runtime::raw as ic0;
use std::ops::Bound::{Excluded, Included};

#[expect(unsafe_code, reason = "IC qualification endpoint")]
#[unsafe(export_name = "canister_update lookahead")]
extern "C" fn lookahead() {
    let [mode, reverse, exhausted, width] = args();
    assert!(reverse <= 1 && exhausted <= 1 && matches!(width, 16 | 1024));
    let low = 8;
    let high = if exhausted == 1 { 22 } else { 28 };
    MAP.with_borrow(|state| {
        let rows = state.as_ref().unwrap();
        let start = ic0::instruction_counter();
        let mut page = rows.range(low..=high);
        let mut last = 0;
        for i in 0..8 {
            let entry = if reverse == 1 {
                page.next_back()
            } else {
                page.next()
            }
            .unwrap();
            let (key, value) = entry.into_pair();
            assert_eq!(
                key,
                if reverse == 1 {
                    high - 2 * i
                } else {
                    low + 2 * i
                }
            );
            assert_eq!(value.len(), width as usize);
            assert!(value.iter().all(|byte| *byte == (key / 2) as u8));
            last = key;
        }
        NODES.set(0);
        VALUES.set(0);
        let initial_heap = HEAP.current_usage();
        HEAP.reset_peak_usage();
        let probe_start = ic0::instruction_counter();
        let key = match (mode, reverse) {
            (0, 0) => page.next().map(|entry| entry.into_key()),
            (0, 1) => page.next_back().map(|entry| entry.into_key()),
            (1, 0) => rows.keys_range((Excluded(last), Included(high))).next(),
            (1, 1) => rows.keys_range((Included(low), Excluded(last))).next_back(),
            #[cfg(feature = "positioned-keys")]
            (2, 0) => page.next_key(),
            #[cfg(feature = "positioned-keys")]
            (2, 1) => page.next_back_key(),
            _ => panic!("unknown lookahead method"),
        };
        let end = ic0::instruction_counter();
        let measures = [
            key.unwrap_or(u64::MAX),
            end - start,
            end - probe_start,
            NODES.get(),
            VALUES.get(),
            (HEAP.peak_usage() - initial_heap) as u64,
        ];
        for word in measures {
            ic0::msg_reply_data_append(&word.to_le_bytes());
        }
        ic0::msg_reply();
    });
}
