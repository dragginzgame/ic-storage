//! Scalar-only foreign bridge. No native pointer or owner enters this runtime.

use crate::provider::cell;
use ic_platform_runtime::raw as ic0;
use ic_storage_native_feasibility::checkpoint;

#[cfg(feature = "native-record")]
mod records;

// These scalar imports validate arguments and custody in trusted Wasm. Their
// closed call graph cannot invoke Rust while this runtime is live.
#[expect(
    unsafe_code,
    reason = "scalar imports bound to reviewed provider functions"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn freeze() -> u32;
    safe fn counter() -> u64;
    safe fn value(index: u32) -> u64;
    safe fn fresh();
    safe fn begin(counter: u64);
    safe fn put(index: u32, value: u64);
    safe fn finish(length: u32);
}

fn args() -> [u64; 2] {
    assert_eq!(ic0::msg_arg_data_size(), 16);
    let mut bytes = [0; 16];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    [
        u64::from_le_bytes(bytes[..8].try_into().unwrap()),
        u64::from_le_bytes(bytes[8..].try_into().unwrap()),
    ]
}

#[expect(
    unsafe_code,
    reason = "private bridge initialization entry bound by final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_create() {
    drop(cell(true));
}

#[expect(
    unsafe_code,
    reason = "private native export entry bound by final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    let [epoch, fault] = args();
    assert!(fault <= 1);
    let count = freeze();
    assert!(count <= checkpoint::V2_MAX_VALUES as u32);
    let values: Vec<_> = (0..count).map(|index| value(index)).collect();
    #[cfg(not(feature = "native-record"))]
    let encoded = checkpoint::encode_v2(counter(), &values, epoch).unwrap();
    #[cfg(feature = "native-record")]
    let encoded = records::encode(counter(), &values, epoch);
    cell(false).set(encoded).unwrap();
    if fault == 1 {
        ic0::trap(b"native export fault after stable write");
    }
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "private explicit recovery entry bound by final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore() {
    restore_checkpoint(Recovery::Exact);
}

#[cfg(feature = "native-record")]
#[expect(
    unsafe_code,
    reason = "private portable widening entry bound by final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore_widen() {
    restore_checkpoint(Recovery::Widen);
}

#[cfg(feature = "native-record")]
#[expect(
    unsafe_code,
    reason = "private portable narrowing entry bound by final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore_narrow() {
    restore_checkpoint(Recovery::Narrow);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Recovery {
    Exact,
    #[cfg(feature = "native-record")]
    Widen,
    #[cfg(feature = "native-record")]
    Narrow,
}

fn restore_checkpoint(mode: Recovery) {
    let [epoch, fault] = args();
    assert!(fault <= if mode != Recovery::Exact { 3 } else { 2 });
    fresh();
    let mut cell = cell(false);
    #[cfg(not(feature = "native-record"))]
    let decoded = checkpoint::decode(cell.get(), epoch).unwrap();
    #[cfg(feature = "native-record")]
    let decoded = {
        use ic_storage_native_feasibility::record::{Record, V2, narrow::V3};
        let mut decoded = checkpoint::decode_record(cell.get(), epoch).unwrap();
        if mode == Recovery::Widen {
            let Record::V3(source) = decoded.record else {
                ic0::trap(b"portable widening requires schema three");
            };
            decoded.record = Record::V2(V2::from(source));
        }
        if mode == Recovery::Narrow {
            let Record::V2(source) = decoded.record else {
                ic0::trap(b"portable narrowing requires schema two");
            };
            decoded.record = Record::V3(
                V3::try_from(source)
                    .unwrap_or_else(|_| ic0::trap(b"portable narrowing out of range")),
            );
        }
        decoded
    };
    if fault == 1 {
        cell.set(checkpoint::encode_v2(999, &decoded.values, epoch).unwrap())
            .unwrap();
        ic0::trap(b"native restore fault after stable write");
    }
    #[cfg(not(feature = "native-record"))]
    begin(decoded.counter);
    #[cfg(feature = "native-record")]
    {
        begin(decoded.record.counter());
        records::restore(decoded.record);
    }
    for (index, value) in decoded.values.iter().enumerate() {
        put(index as u32, *value);
        if fault == 2 {
            ic0::trap(b"native restore fault during construction");
        }
    }
    finish(decoded.values.len() as u32);
    if fault == 3 {
        ic0::trap(b"portable record conversion after publication");
    }
}

#[expect(
    unsafe_code,
    reason = "private checkpoint evidence query bound by final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_bytes() {
    ic0::msg_reply_data_append(cell(false).get());
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "private governed checkpoint fault injection entry"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_damage() {
    let [mode, reserved] = args();
    assert_eq!(reserved, 0);
    freeze();
    let mut cell = cell(false);
    let mut bytes = cell.get().clone();
    match mode {
        0 => {
            bytes.pop().unwrap();
        }
        1 => bytes[80] ^= 1,
        // Record schemas 1–3 are valid, so use an unknown version.
        2 => {
            bytes[24] = if cfg!(feature = "native-record") {
                255
            } else {
                2
            }
        }
        _ => panic!("unknown damage mode"),
    }
    cell.set(bytes).unwrap();
    ic0::msg_reply();
}
