//! Two-map portable bridge: scalar callbacks only, never native Rust reentry.
#[cfg(not(feature = "native-map-job"))]
use crate::provider::map_cell;
#[cfg(feature = "native-map-job")]
mod job;
use ic_platform_runtime::raw as ic0;
use ic_storage_native::map::Map;
use ic_storage_native_feasibility::checkpoint::maps;

#[expect(
    unsafe_code,
    reason = "scalar imports bound to non-reentrant provider callbacks"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn freeze();
    safe fn map_count(id: u32) -> u32;
    safe fn map_word(id: u32, index: u32, field: u32) -> u64;
    safe fn fresh();
    safe fn map_begin(a: u32, b: u32, fault: u32);
    safe fn map_put(id: u32, index: u32, key: u64, value: u64);
    safe fn map_finish();
}
#[cfg(not(feature = "native-map-job"))]
fn args() -> [u64; 2] {
    assert_eq!(ic0::msg_arg_data_size(), 16);
    let mut bytes = [0; 16];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    [
        u64::from_le_bytes(bytes[..8].try_into().unwrap()),
        u64::from_le_bytes(bytes[8..].try_into().unwrap()),
    ]
}
#[cfg(not(feature = "native-map-job"))]
#[expect(unsafe_code, reason = "private governed map initialization export")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_create() {
    drop(map_cell(true));
}
#[cfg(not(feature = "native-map-job"))]
#[expect(unsafe_code, reason = "private governed map export entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    let [epoch, fault] = args();
    assert!(fault <= 1);
    freeze(); // The wrapper has already ended native admission and frozen both roots.
    map_cell(false).set(maps::encode(&read_source(), epoch).unwrap());
    if fault == 1 {
        ic0::trap(b"map export after stable write");
    }
    ic0::msg_reply();
}
fn read_source() -> [Map; 2] {
    let mut source = [Map::new(maps::LIMIT), Map::new(maps::LIMIT)];
    for (id, map) in source.iter_mut().enumerate() {
        let count = map_count(id as u32);
        assert!(count <= maps::LIMIT as u32);
        map.with(|mut view| {
            for index in 0..count {
                assert!(
                    view.insert(map_word(id as u32, index, 0), map_word(id as u32, index, 1))
                        .unwrap()
                        .is_none()
                );
            }
        })
        .unwrap();
    }
    source
}
#[cfg(not(feature = "native-map-job"))]
#[expect(unsafe_code, reason = "private explicit fresh-heap recovery entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore() {
    let [epoch, fault] = args();
    assert!(fault <= 4);
    fresh();
    let mut cell = map_cell(false);
    let decoded = maps::decode(cell.get(), epoch).unwrap();
    if fault == 1 {
        cell.set(Vec::new());
        ic0::trap(b"map recovery after stable write");
    }
    stage(decoded, fault);
}
fn stage(mut decoded: [Map; 2], fault: u64) {
    map_begin(
        decoded[0].len().unwrap() as u32,
        decoded[1].len().unwrap() as u32,
        fault as u32,
    );
    for (id, map) in decoded.iter_mut().enumerate() {
        let mut index = 0;
        map.with(|view| {
            view.for_each(|key, value| {
                map_put(id as u32, index, key, value);
                index += 1;
                if fault == 2 {
                    ic0::trap(b"map recovery during staging");
                }
            })
        })
        .unwrap();
    }
    map_finish(); // Only staging is complete. Wrapper builds native maps after return.
}
#[cfg(not(feature = "native-map-job"))]
#[expect(unsafe_code, reason = "private checkpoint evidence query")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_bytes() {
    ic0::msg_reply_data_append(map_cell(false).get());
    ic0::msg_reply();
}
#[cfg(not(feature = "native-map-job"))]
#[expect(unsafe_code, reason = "private governed checkpoint corruption fixture")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_damage() {
    let [mode, reserved] = args();
    assert_eq!(reserved, 0);
    freeze();
    let mut cell = map_cell(false);
    let mut bytes = cell.get().clone();
    match mode {
        0 => {
            bytes.pop().unwrap();
        }
        1 => bytes[64] ^= 1,
        2 => bytes[24] = 255,
        _ => panic!("unknown damage"),
    }
    cell.set(bytes);
    ic0::msg_reply();
}
