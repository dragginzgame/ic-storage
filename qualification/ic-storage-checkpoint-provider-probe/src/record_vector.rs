//! Governed record-vector checkpoint bridge; only scalar fields cross domains.

use crate::provider::record_vector_cell as cell;
use ic_platform_runtime::raw as ic0;
use ic_storage_native_feasibility::checkpoint::record_vector as codec;
#[cfg(feature = "native-record-vector-v2")]
use ic_storage_native_feasibility::record::V2 as Record;
#[cfg(not(feature = "native-record-vector-v2"))]
use ic_storage_native_feasibility::record::narrow::V3 as Record;
#[cfg(not(feature = "native-record-vector-v2"))]
type Counter = u32;
#[cfg(feature = "native-record-vector-v2")]
type Counter = u64;

#[expect(
    unsafe_code,
    reason = "scalar imports bound to reviewed provider callbacks"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn freeze() -> u32;
    safe fn record_value(index: u32, field: u32) -> u64;
    safe fn fresh();
    safe fn begin();
    safe fn record_put(index: u32, counter: Counter, ceiling: u64);
    safe fn finish(length: u32);
}

pub(super) fn args() -> [u64; 2] {
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
    reason = "private initialization bound by the final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_create() {
    drop(cell(true));
}

#[expect(unsafe_code, reason = "private export bound by the final linker")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    let [epoch, fault] = args();
    assert!(fault <= 1);
    let count = freeze();
    assert!(count <= codec::LIMIT as u32);
    let records: Vec<_> = (0..count)
        .map(|index| {
            Record::new(
                Counter::try_from(record_value(index, 0)).unwrap(),
                record_value(index, 1),
            )
            .unwrap()
        })
        .collect();
    #[cfg(not(feature = "native-record-vector-v2"))]
    let encoded = codec::encode(&records, epoch);
    #[cfg(feature = "native-record-vector-v2")]
    let encoded = codec::encode_v2(&records, epoch);
    cell(false).set(encoded.unwrap());
    if fault == 1 {
        ic0::trap(b"record vector export after stable write");
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private recovery bound by the final linker")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore() {
    restore(false);
}

#[cfg(not(feature = "native-record-vector-v2"))]
#[expect(
    unsafe_code,
    reason = "explicit vector conversion bound by the final linker"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_vector_narrow() {
    restore(true);
}

fn restore(narrow: bool) {
    let [epoch, fault] = args();
    assert!(fault <= 3);
    fresh();
    let mut cell = cell(false);
    #[cfg(not(feature = "native-record-vector-v2"))]
    let decoded = if narrow {
        codec::decode_narrow_v2(cell.get(), epoch)
    } else {
        codec::decode(cell.get(), epoch)
    };
    #[cfg(feature = "native-record-vector-v2")]
    let decoded = {
        assert!(!narrow);
        codec::decode_v2(cell.get(), epoch)
    };
    let (records, count) = decoded.unwrap_or_else(|error| match error {
        codec::Error::Conversion { .. } => ic0::trap(b"record vector narrowing out of range"),
        other => panic!("record vector checkpoint: {other:?}"),
    });
    if fault == 1 {
        cell.set(codec::encode(&[], epoch).unwrap());
        ic0::trap(b"record vector restore after stable write");
    }
    // Complete decoding precedes target initialization. These callbacks cannot
    // enter native Rust; the owning Vec is reconstructed only after this returns.
    begin();
    for (index, record) in records.into_iter().take(count).enumerate() {
        let record = record.unwrap();
        record_put(index as u32, record.counter(), record.ceiling());
        if fault == 2 {
            ic0::trap(b"record vector restore during construction");
        }
    }
    finish(count as u32);
    if fault == 3 {
        ic0::trap(b"record vector restore after publication");
    }
}

#[expect(unsafe_code, reason = "private query bound by the final linker")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_bytes() {
    ic0::msg_reply_data_append(cell(false).get());
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "closed checkpoint corruption fixture")]
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
        1 => bytes[32] ^= 1,
        2 => bytes[16] = 255,
        _ => panic!("unknown checkpoint damage"),
    }
    cell.set(bytes);
    ic0::msg_reply();
}
