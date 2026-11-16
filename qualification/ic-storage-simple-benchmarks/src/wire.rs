//! Fixed little-endian benchmark protocol; no Candid or heap allocation in timing.

use ic_platform_runtime::raw as ic0;

/// Read exactly two u64 words. Invalid requests trap before a timer starts.
#[must_use]
pub fn input() -> [u64; 2] {
    if ic0::msg_arg_data_size() != 16 {
        core::arch::wasm32::unreachable();
    }
    let mut bytes = [0; 16];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    let (words, _) = bytes.as_chunks::<8>();
    [u64::from_le_bytes(words[0]), u64::from_le_bytes(words[1])]
}

/// Admit the fixed small population and seed, outside timing.
#[must_use]
pub fn setup() -> (u64, u64) {
    let [rows, seed] = input();
    if ![1, 16, 64].contains(&rows) || !(1..=3).contains(&seed) {
        core::arch::wasm32::unreachable();
    }
    (rows, seed)
}

/// Admit one operation and batch, outside timing.
#[must_use]
pub fn request() -> (usize, u64) {
    let [operation, batch] = input();
    if operation >= super::OPERATIONS.len() as u64 || ![1, 8].contains(&batch) {
        core::arch::wasm32::unreachable();
    }
    (operation as usize, batch)
}

/// Append fixed scalar replies synchronously.
pub fn reply(words: &[u64]) {
    for word in words {
        ic0::msg_reply_data_append(&word.to_le_bytes());
    }
    ic0::msg_reply();
}
