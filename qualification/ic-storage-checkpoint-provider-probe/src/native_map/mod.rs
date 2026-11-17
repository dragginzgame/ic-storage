//! Two-map portable bridge: scalar callbacks only, never native Rust reentry.
#[cfg(not(feature = "native-map-job"))]
use crate::provider::map_cell;
#[cfg(all(feature = "native-map-job", not(feature = "native-map-paged")))]
mod job;
#[cfg(feature = "native-map-paged")]
mod paged_job;
#[cfg(feature = "native-map-job")]
mod staging;
#[cfg(not(feature = "native-map-paged"))]
use ic_platform_runtime::raw as ic0;
use ic_storage::map::Map;
#[cfg(not(feature = "native-map-paged"))]
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
    #[cfg(not(feature = "native-map-paged"))]
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
    map_cell(false)
        .set(maps::encode(&read_source(maps::LIMIT), epoch).unwrap())
        .unwrap();
    if fault == 1 {
        ic0::trap(b"map export after stable write");
    }
    ic0::msg_reply();
}
fn read_source(limit: usize) -> [Map; 2] {
    let mut source = [Map::new(limit), Map::new(limit)];
    for (id, map) in source.iter_mut().enumerate() {
        // Starts a fresh transient cursor. The callback contract requires each
        // key immediately followed by its value, with increasing record indices.
        let count = map_count(id as u32);
        assert!(count as usize <= limit);
        map.with(|mut view| {
            for index in 0..count {
                let key = map_word(id as u32, index, 0);
                let value = map_word(id as u32, index, 1);
                assert!(view.insert(key, value).unwrap().is_none());
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
        cell.set(Vec::new()).unwrap();
        ic0::trap(b"map recovery after stable write");
    }
    stage(&decoded, fault);
}
#[cfg(not(feature = "native-map-paged"))]
fn stage(decoded: &maps::Records, fault: u64) {
    map_begin(
        decoded[0].len() as u32,
        decoded[1].len() as u32,
        fault as u32,
    );
    for (id, records) in decoded.iter().enumerate() {
        for (index, &(key, value)) in records.iter().enumerate() {
            map_put(id as u32, index as u32, key, value);
            if fault == 2 {
                ic0::trap(b"map recovery during staging");
            }
        }
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
    cell.set(bytes).unwrap();
    ic0::msg_reply();
}
