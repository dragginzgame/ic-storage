//! Matched on-demand workload over the generation-owned service.
use super::*;

static mut ROWS: u64 = 0;
static mut SEED: u64 = 0;

#[expect(unsafe_code, reason = "closed scalar request-copy protocol")]
#[link(wasm_import_module = "session_provider")]
unsafe extern "C" {
    safe fn arg_size() -> usize;
    unsafe fn arg_copy(destination: *mut u8, offset: usize, length: usize);
}

fn input() -> [u64; 2] {
    assert_eq!(arg_size(), 16);
    let mut bytes = [0u8; 16];
    // SAFETY: exact bounded copy into a live stack buffer; no host reentry.
    #[expect(unsafe_code, reason = "bounded diagnostic request copy")]
    unsafe {
        arg_copy(bytes.as_mut_ptr(), 0, bytes.len());
    }
    let words = bytes.as_chunks::<8>().0;
    [u64::from_le_bytes(words[0]), u64::from_le_bytes(words[1])]
}

#[expect(
    unsafe_code,
    reason = "exclusive service setup with admitted live owners"
)]
#[unsafe(export_name = "canister_update bench_setup")]
extern "C" fn setup() {
    let [shape, seed] = input();
    let rows = shape & 0xffff_ffff;
    let other = shape >> 32;
    assert!(
        [64, 192, 1536].contains(&rows) && [0, 192].contains(&other) && (1..=3).contains(&seed)
    );
    unsafe {
        assert!(PHASE == 2 && ROWS == 0);
        PHASE = 3;
        let maps = (&raw mut MAPS).as_mut().unwrap().assume_init_mut();
        for (map, count) in maps.iter_mut().zip([rows, other]) {
            map.with(|mut view| {
                view.reserve(count as usize).unwrap();
                for key in 0..count {
                    assert_eq!(view.insert(key, key + 1000).unwrap(), None);
                }
            })
            .unwrap();
        }
        ROWS = rows;
        SEED = seed;
        PHASE = 2;
    }
    reply();
}

#[expect(
    unsafe_code,
    reason = "exclusive generation-owned batch; no admission or suspension"
)]
#[unsafe(export_name = "canister_update bench_run")]
extern "C" fn run() {
    let [operation, batch] = input();
    assert!(operation < 9 && [1, 8, 64].contains(&batch));
    let result = unsafe {
        assert!(PHASE == 2 && ROWS != 0);
        PHASE = 3;
        let maps = (&raw mut MAPS).as_mut().unwrap().assume_init_mut();
        let rows = ROWS;
        let seed = SEED;
        let result = maps[0]
            .with(|mut view| {
                let mut sum = 0u64;
                if operation == 8 {
                    view.for_each(|_, value| sum = sum.wrapping_add(value));
                } else {
                    for i in 0..batch {
                        let key = (17 * i + 13 * seed) % rows;
                        match operation {
                            0 => {
                                core::hint::black_box(key);
                            }
                            1 => assert_eq!(view.get(key), Some(key + 1000)),
                            2 => assert_eq!(view.get(rows + 1024 + i), None),
                            3 => {
                                assert_eq!(view.insert(key, key + 2000).unwrap(), Some(key + 1000))
                            }
                            4 => assert_eq!(view.remove(key), Some(key + 2000)),
                            5 => assert_eq!(view.insert(key, key + 1000).unwrap(), None),
                            6 => assert_eq!(view.insert(rows + i, rows + i + 1000).unwrap(), None),
                            7 => {
                                assert_eq!(view.remove(key), Some(key + 1000));
                                assert_eq!(
                                    view.insert(rows + 64 + i, rows + 64 + i + 1000).unwrap(),
                                    None
                                );
                            }
                            _ => abort(),
                        }
                    }
                }
                sum
            })
            .unwrap();
        PHASE = 2;
        result
    };
    word(result);
    reply();
}

#[expect(
    unsafe_code,
    reason = "non-reentrant full-state diagnostic outside timed updates"
)]
#[unsafe(export_name = "canister_query bench_state")]
extern "C" fn state() {
    unsafe {
        assert!(PHASE == 2);
        PHASE = 3;
        let maps = (&raw mut MAPS).as_mut().unwrap().assume_init_mut();
        word(core::arch::wasm32::memory_size::<0>() as u64);
        word(0); // no stable-memory allocation
        word(
            maps.iter()
                .map(|map| map.allocated_bytes().unwrap() as u64)
                .sum(),
        );
        for map in maps {
            map.with(|view| {
                word(view.len() as u64);
                view.for_each(|key, value| {
                    word(key);
                    word(value);
                });
            })
            .unwrap();
        }
        PHASE = 2;
    }
    reply();
}
