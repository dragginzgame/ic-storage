//! Shared controller admission and exact job argument framing.
use ic_platform_runtime::raw as ic0;

pub(crate) fn controller() {
    let len = ic0::msg_caller_size();
    assert!(len <= 29);
    let mut bytes = [0; 29];
    ic0::msg_caller_copy(&mut bytes[..len], 0);
    if ic0::is_controller(&bytes[..len]) != 1 {
        ic0::trap(b"migration controller required");
    }
}

pub(crate) fn args() -> [u64; 3] {
    assert_eq!(ic0::msg_arg_data_size(), 24);
    let mut bytes = [0; 24];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
}
