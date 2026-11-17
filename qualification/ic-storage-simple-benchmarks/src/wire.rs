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

/// Append fixed scalar replies synchronously.
pub fn reply(words: &[u64]) {
    for word in words {
        ic0::msg_reply_data_append(&word.to_le_bytes());
    }
    ic0::msg_reply();
}
