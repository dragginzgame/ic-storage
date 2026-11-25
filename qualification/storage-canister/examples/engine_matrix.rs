//! Matched engine measurements on the same exclusive allocation implementation.
#![cfg(target_arch = "wasm32")]
use ic_platform_runtime::raw as ic0;
use ic_storage::adapter::{StableBTreeMap, StableCell};
use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, Memory, MemoryAuthority,
    MemoryRequest, MemoryRuntime, RuntimeMemory, SealedDeclarationSnapshot, StableKey,
};
use std::{collections::BTreeSet, hint::black_box};

#[global_allocator]
static HEAP: peak_alloc::PeakAlloc = peak_alloc::PeakAlloc;

const KEY_MAP: &str = "app.keys.v1";
const MAP: &str = "app.map.v1";
const CELL: &str = "app.cell.v1";

struct ReferenceMemory(RuntimeMemory<DefaultMemoryImpl>);
impl ic_stable_structures::Memory for ReferenceMemory {
    fn size(&self) -> u64 {
        self.0.size()
    }
    fn grow(&self, n: u64) -> i64 {
        Memory::grow(&self.0, n)
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.0.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        self.0.write(offset, bytes);
    }
}

fn lease(host: &MemoryRuntime<DefaultMemoryImpl>, name: &str) -> RuntimeMemory<DefaultMemoryImpl> {
    host.open_memory(&StableKey::parse(name).unwrap()).unwrap()
}

fn value(key: u64, width: usize, revision: u8) -> Vec<u8> {
    vec![(key as u8).wrapping_add(revision); width]
}

struct Measurements(Vec<u8>);
impl Measurements {
    fn run<T>(&mut self, label: &str, operation: impl FnOnce() -> T) -> T {
        let pages = ic0::stable64_size();
        let wasm = core::arch::wasm32::memory_size::<0>();
        let live = HEAP.current_usage();
        HEAP.reset_peak_usage();
        let start = ic0::instruction_counter();
        let result = black_box(operation());
        let instructions = ic0::instruction_counter() - start;
        let heap = HEAP.peak_usage() - live;
        let growth = ic0::stable64_size() - pages;
        let linear = core::arch::wasm32::memory_size::<0>() - wasm;
        use std::io::Write;
        writeln!(self.0, "{label} {instructions} {heap} {growth} {linear}").unwrap();
        result
    }
}

fn owned_batch(map: &mut StableBTreeMap<u64, Vec<u8>>, keys: BTreeSet<u64>) -> u64 {
    map.remove_many(keys)
}
type ReferenceMap = ic_stable_structures::BTreeMap<u64, Vec<u8>, ReferenceMemory>;
fn reference_batch(map: &mut ReferenceMap, keys: BTreeSet<u64>) -> u64 {
    keys.into_iter().filter(|k| map.remove(k).is_some()).count() as u64
}

trait ClearMatrix {
    fn clear_matrix(&mut self);
}
impl ClearMatrix for StableBTreeMap<u64, Vec<u8>> {
    fn clear_matrix(&mut self) {
        self.clear();
    }
}
impl ClearMatrix for ReferenceMap {
    fn clear_matrix(&mut self) {
        self.clear_new();
    }
}

// Expand the same typed operations for both backends. All assertions use generated
// expectations independent of the engine; full record checks are outside timings.
macro_rules! suite {
    ($host:ident,$n:ident,$width:ident,$out:ident,$new_map:expr,$load_map:expr,$new_cell:expr,$load_cell:expr,$batch:ident) => {{
        let n = $n;
        let width = $width;
        let out = &mut $out;
        let mut map = out.run("map_create", || ($new_map)(lease(&$host, MAP)));
        out.run("insert_ordered", || {
            for k in 0..n {
                assert!(map.insert(k, value(k, width, 0)).is_none());
            }
        });
        out.run("insert_permuted", || {
            for i in 0..n {
                let k = n + (i * 157) % n;
                assert!(map.insert(k, value(k, width, 0)).is_none());
            }
        });
        out.run("overwrite", || {
            for i in 0..n {
                let k = n + (i * 157) % n;
                assert_eq!(map.insert(k, value(k, width, 1)).unwrap().len(), width);
            }
        });
        out.run("hit", || {
            for i in 0..n {
                black_box(map.get(&((i * 157) % n)).unwrap());
            }
        });
        out.run("miss", || {
            for k in 2 * n..3 * n {
                assert!(map.get(&k).is_none());
            }
        });
        out.run("contains", || {
            for k in n..2 * n {
                assert!(map.contains_key(&k));
            }
        });
        out.run("keys", || {
            for k in map.keys() {
                black_box(k);
            }
        });
        out.run("values", || {
            for v in map.values() {
                black_box(v);
            }
        });
        out.run("range", || {
            for e in map.range(123..187) {
                black_box(e.into_pair());
            }
        });
        out.run("reverse", || {
            for e in map.range(123..187).rev() {
                black_box(e.into_pair());
            }
        });
        out.run("endpoints", || {
            black_box(map.first_key_value());
            black_box(map.last_key_value());
        });
        drop(map);
        let mut map = out.run("map_reopen", || ($load_map)(lease(&$host, MAP)));
        let pairs = map.iter().map(|e| e.into_pair());
        assert!(pairs.eq((0..2 * n).map(|k| (k, value(k, width, u8::from(k >= n))))));
        out.run("delete_point", || {
            for k in 0..64 {
                assert!(map.remove(&k).is_some());
            }
        });
        out.run("delete_adjacent", || {
            assert_eq!($batch(&mut map, (64..128).collect()), 64)
        });
        out.run("delete_interleaved", || {
            assert_eq!($batch(&mut map, (128..256).step_by(2).collect()), 64)
        });
        out.run("retry", || {
            assert_eq!($batch(&mut map, (128..256).step_by(2).collect()), 0)
        });
        out.run("pop_ends", || {
            assert_eq!(map.pop_first().unwrap().0, 129);
            assert_eq!(map.pop_last().unwrap().0, 2 * n - 1);
        });
        out.run("reinsert", || {
            for k in 0..64 {
                assert!(map.insert(k, value(k, width, 2)).is_none());
            }
        });
        let expected = (0..2 * n).filter(|k| {
            *k < 64 || (*k > 129 && *k < 256 && k % 2 == 1) || (256..2 * n - 1).contains(k)
        });
        assert!(map.iter().map(|e| e.into_pair()).eq(expected.map(|k| (
            k,
            value(k, width, if k < 64 { 2 } else { u8::from(k >= n) })
        ))));
        out.run("clear", || map.clear_matrix());
        assert!(map.is_empty());
        out.run("refill", || {
            for k in 0..64 {
                assert!(map.insert(k, value(k, width, 3)).is_none());
            }
        });
        assert!(
            map.iter()
                .map(|e| e.into_pair())
                .eq((0..64).map(|k| (k, value(k, width, 3))))
        );
        let mut cell = out.run("cell_create", || {
            ($new_cell)(lease(&$host, CELL), value(1, width, 0))
        });
        out.run("cell_get", || {
            for _ in 0..n {
                black_box(cell.get());
            }
        });
        out.run("cell_replace", || {
            for k in 0..32 {
                black_box(cell.set(value(k, width, 0)));
            }
        });
        out.run("cell_grow", || {
            black_box(cell.set(value(7, width + 65536, 0)));
        });
        out.run("cell_shrink", || {
            black_box(cell.set(value(8, width, 0)));
        });
        drop(cell);
        let cell = out.run("cell_reopen", || ($load_cell)(lease(&$host, CELL)));
        assert_eq!(cell.get(), &value(8, width, 0));
    }};
}

fn open_host() -> MemoryRuntime<DefaultMemoryImpl> {
    let mut host = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
    let declarations = SealedDeclarationSnapshot::new(
        [MAP, CELL, KEY_MAP]
            .iter()
            .map(|k| MemoryRequest::new("app", k).unwrap())
            .collect(),
    )
    .unwrap();
    host.bootstrap(
        &declarations,
        &AllocationGrants::new(vec![MemoryAuthority::new("app", "app.").unwrap()]).unwrap(),
        &GenericAllocationPolicy,
    )
    .unwrap();
    host
}

// Size-only builds specialize every exported entrypoint at compile time.
const SIZE_ENGINE: Option<u64> = match option_env!("STORAGE_MATRIX_SIZE_ENGINE") {
    Some(name) => match name.as_bytes() {
        b"owned" => Some(0),
        b"reference" => Some(1),
        _ => None,
    },
    None => None,
};

fn arguments() -> (u64, u64, usize) {
    assert_eq!(ic0::msg_arg_data_size(), 24);
    let mut bytes = [0; 24];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    let engine = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let width = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let n = u64::from_le_bytes(bytes[16..].try_into().unwrap());
    assert!(engine <= 1);
    assert!(matches!(n, 256 | 2048) && width <= 8192);
    (engine, n, width)
}

// This specialization also exercises the reference engine's bounded page layout.
macro_rules! fixed_suite {
    ($map:expr,$out:ident) => {{
        let mut map = $map;
        $out.run("fixed_insert", || {
            for i in 0..512 {
                let k = (i * 157) % 512;
                assert_eq!(map.insert(k, k + 10), None);
            }
        });
        $out.run("fixed_get", || {
            for i in 0..512 {
                assert_eq!(map.get(&i), Some(i + 10));
            }
        });
        $out.run("fixed_overwrite", || {
            for i in 0..512 {
                assert_eq!(map.insert(i, i + 20), Some(i + 10));
            }
        });
        $out.run("fixed_range", || {
            let mut entries = map.range(96..160);
            for i in 0..32 {
                assert_eq!(entries.next().unwrap().into_pair(), (96 + i, 116 + i));
                assert_eq!(entries.next_back().unwrap().into_pair(), (159 - i, 179 - i));
            }
            assert!(entries.next().is_none());
        });
        $out.run("fixed_remove", || {
            for i in (0..512).step_by(2) {
                assert_eq!(map.remove(&i), Some(i + 20));
            }
        });
        assert!(
            map.iter()
                .map(|e| e.into_pair())
                .eq((1..512).step_by(2).map(|k| (k, k + 20)))
        );
    }};
}

macro_rules! key_suite {
    ($map:ident, $out:ident, $bytes:ident, $width:ident) => {{
        let mut map = $map;
        $out.run(&format!("key{}_insert", $width), || {
            for i in 0..256 {
                let k = i * 157 % 256;
                assert!(map.insert($bytes(k), value(k, 32, 0)).is_none());
            }
        });
        $out.run(&format!("key{}_hit", $width), || {
            for i in 0..256 {
                black_box(map.get(&$bytes(i)).unwrap());
            }
        });
        $out.run(&format!("key{}_range", $width), || {
            for e in map.range($bytes(96)..$bytes(160)) {
                black_box(e.into_pair());
            }
        });
        assert!(
            map.iter()
                .map(|e| e.into_pair())
                .eq((0..256).map(|k| ($bytes(k), value(k, 32, 0))))
        );
    }};
}

// Use the same operations and generated expectations for each concrete map type.
macro_rules! typed_suite {
    ($map:expr, $out:ident, $name:literal, $key:expr, $value:expr) => {{
        let mut map = $map;
        let key = $key;
        let value = $value;
        $out.run(concat!($name, "_insert"), || {
            for i in 0..256u32 {
                let k = i * 157 % 256;
                assert_eq!(map.insert(key(k), value(k)), None);
            }
        });
        $out.run(concat!($name, "_hit"), || {
            for i in 0..256 {
                assert_eq!(map.get(&key(i)), Some(value(i)));
            }
        });
        $out.run(concat!($name, "_range"), || {
            assert!(
                map.range(key(96)..key(160))
                    .map(|e| e.into_pair())
                    .eq((96..160).map(|i| (key(i), value(i))))
            );
        });
        $out.run(concat!($name, "_remove"), || {
            for i in (0..256).step_by(2) {
                assert_eq!(map.remove(&key(i)), Some(value(i)));
            }
        });
        assert!(
            map.iter()
                .map(|e| e.into_pair())
                .eq((1..256).step_by(2).map(|i| (key(i), value(i))))
        );
    }};
}

macro_rules! compare_type {
    ($engine:ident, $host:ident, $out:ident, $name:literal, $k:ty, $v:ty, $key:expr, $value:expr) => {{
        let memory = lease(&$host, KEY_MAP);
        if $engine == 0 {
            typed_suite!(
                StableBTreeMap::<$k, $v>::reset(memory),
                $out,
                $name,
                $key,
                $value
            );
        } else {
            typed_suite!(
                ic_stable_structures::BTreeMap::<$k, $v, _>::new(ReferenceMemory(memory)),
                $out,
                $name,
                $key,
                $value
            );
        }
    }};
}

#[expect(unsafe_code, reason = "IC test entrypoint")]
#[unsafe(export_name = "canister_update matrix")]
extern "C" fn matrix() {
    std::panic::set_hook(Box::new(|info| ic0::trap(info.to_string().as_bytes())));
    let (engine, n, width) = arguments();
    let engine = SIZE_ENGINE.unwrap_or(engine);
    let host = open_host();
    let mut out = Measurements(Vec::new());
    match engine {
        0 => suite!(
            host,
            n,
            width,
            out,
            StableBTreeMap::<u64, Vec<u8>>::create,
            StableBTreeMap::load,
            StableCell::create,
            |m| StableCell::<Vec<u8>>::load(m).unwrap(),
            owned_batch
        ),
        1 => suite!(
            host,
            n,
            width,
            out,
            |m| ReferenceMap::new(ReferenceMemory(m)),
            |m| ReferenceMap::load(ReferenceMemory(m)),
            |m, v| ic_stable_structures::Cell::new(ReferenceMemory(m), v),
            |m| ic_stable_structures::Cell::init(ReferenceMemory(m), Vec::<u8>::new()),
            reference_batch
        ),
        _ => unreachable!(),
    }
    if n == 256 && width == 16 {
        if engine == 0 {
            fixed_suite!(
                StableBTreeMap::<u64, u64>::reset(lease(&host, KEY_MAP)),
                out
            );
        } else {
            fixed_suite!(
                ic_stable_structures::BTreeMap::<u64, u64, _>::new(ReferenceMemory(lease(
                    &host, KEY_MAP
                ))),
                out
            );
        }
        for key_width in [16, 64, 256] {
            let bytes = |i: u64| {
                let mut key = vec![42; key_width];
                key[key_width - 8..].copy_from_slice(&i.to_be_bytes());
                key
            };
            if engine == 0 {
                let map = StableBTreeMap::<Vec<u8>, Vec<u8>>::reset(lease(&host, KEY_MAP));
                key_suite!(map, out, bytes, key_width);
            } else {
                let map = ic_stable_structures::BTreeMap::<Vec<u8>, Vec<u8>, _>::new(
                    ReferenceMemory(lease(&host, KEY_MAP)),
                );
                key_suite!(map, out, bytes, key_width);
            }
        }
        compare_type!(
            engine,
            host,
            out,
            "text_option",
            String,
            Option<u32>,
            |i| format!("café/{i:03}"),
            Some
        );
        compare_type!(
            engine,
            host,
            out,
            "tuple_array",
            (u16, u32),
            [u8; 64],
            |i: u32| ((i / 16) as u16, i % 16),
            |i| [i as u8; 64]
        );
        compare_type!(
            engine,
            host,
            out,
            "u128_string",
            u128,
            String,
            |i: u32| (u128::from(i) << 96) + 1,
            |i| format!("résultat/{i}")
        );
    }
    ic0::msg_reply_data_append(&out.0);
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "IC test entrypoint")]
#[unsafe(export_name = "canister_update optional_none")]
extern "C" fn optional_none() {
    std::panic::set_hook(Box::new(|info| ic0::trap(info.to_string().as_bytes())));
    let (engine, _, _) = arguments();
    let host = open_host();
    let memory = lease(&host, KEY_MAP);
    if SIZE_ENGINE.unwrap_or(engine) == 0 {
        let mut map = StableBTreeMap::<String, Option<u32>>::reset(memory);
        assert_eq!(map.insert(String::from("absent"), None), None);
        assert_eq!(map.get(&String::from("absent")), Some(None));
    } else {
        let mut map =
            ic_stable_structures::BTreeMap::<String, Option<u32>, _>::new(ReferenceMemory(memory));
        assert_eq!(map.insert(String::from("absent"), None), None);
        assert_eq!(map.get(&String::from("absent")), Some(None));
    }
    ic0::msg_reply();
}

macro_rules! verify_records {
    ($map:expr,$cell:expr,$width:expr) => {{
        let map = $map;
        let cell = $cell;
        assert!(
            map.iter()
                .map(|e| e.into_pair())
                .eq((0..64).map(|k| (k, value(k, $width, 3))))
        );
        assert_eq!(cell.get(), &value(8, $width, 0));
    }};
}
#[expect(unsafe_code, reason = "IC test entrypoint")]
#[unsafe(export_name = "canister_query verify")]
extern "C" fn verify() {
    let (engine, _, width) = arguments();
    let engine = SIZE_ENGINE.unwrap_or(engine);
    let host = open_host();
    if engine == 0 {
        verify_records!(
            StableBTreeMap::<u64, Vec<u8>>::load(lease(&host, MAP)),
            StableCell::<Vec<u8>>::load(lease(&host, CELL)).unwrap(),
            width
        );
    } else {
        verify_records!(
            ReferenceMap::load(ReferenceMemory(lease(&host, MAP))),
            ic_stable_structures::Cell::init(ReferenceMemory(lease(&host, CELL)), Vec::<u8>::new()),
            width
        );
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "IC test entrypoint")]
#[unsafe(export_name = "canister_update rollback")]
extern "C" fn rollback() {
    let (engine, _, width) = arguments();
    let engine = SIZE_ENGINE.unwrap_or(engine);
    let host = open_host();
    if engine == 0 {
        let mut map = StableBTreeMap::<u64, Vec<u8>>::load(lease(&host, MAP));
        let mut cell = StableCell::<Vec<u8>>::load(lease(&host, CELL)).unwrap();
        assert_eq!(owned_batch(&mut map, (0..32).collect()), 32);
        cell.set(value(99, width + 65536, 0));
    } else {
        let mut map = ReferenceMap::load(ReferenceMemory(lease(&host, MAP)));
        let mut cell =
            ic_stable_structures::Cell::init(ReferenceMemory(lease(&host, CELL)), Vec::<u8>::new());
        assert_eq!(reference_batch(&mut map, (0..32).collect()), 32);
        cell.set(value(99, width + 65536, 0));
    }
    ic0::trap(b"matrix rollback after writes");
}
