//! Installed comparison of bounded ranges and point/batched reads on identical backing.
use ic_platform_runtime::raw as ic0;
use ic_storage::{
    AllocationGrants, BackingIdentity, DefaultMemoryImpl, GenericAllocationPolicy, Memory,
    MemoryAuthority, MemoryBacking, MemoryRequest, MemoryRuntime, SealedDeclarationSnapshot,
    StableKey, adapter::StableBTreeMap,
};
use std::cell::{Cell, RefCell};

#[global_allocator]
pub(crate) static HEAP: peak_alloc::PeakAlloc = peak_alloc::PeakAlloc;
thread_local! {
    pub(crate) static MAP: RefCell<Option<StableBTreeMap<u64, Vec<u8>, Counted>>> = const { RefCell::new(None) };
    pub(crate) static NODES: Cell<u64> = const { Cell::new(0) };
    pub(crate) static VALUES: Cell<u64> = const { Cell::new(0) };
}
#[derive(Default)]
pub(crate) struct Counted(DefaultMemoryImpl);
impl MemoryBacking for Counted {
    fn backing_identity(&self) -> BackingIdentity {
        self.0.backing_identity()
    }
}
impl Memory for Counted {
    fn size(&self) -> u64 {
        self.0.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        self.0.grow(pages)
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        self.0.write(offset, bytes);
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.0.read(offset, bytes);
        if bytes.len() == 20 {
            match &bytes[..8] {
                b"ICBTND01" => NODES.set(NODES.get() + 1),
                b"ICBTVAL1" => VALUES.set(VALUES.get() + 1),
                _ => {}
            }
        }
    }
}
fn open(create: bool) {
    let mut host = MemoryRuntime::new(Counted::default()).unwrap();
    let name = "app.rows.v1";
    let decl =
        SealedDeclarationSnapshot::new(vec![MemoryRequest::new("app", name).unwrap()]).unwrap();
    host.bootstrap(
        &decl,
        &AllocationGrants::new(vec![MemoryAuthority::new("app", "app.").unwrap()]).unwrap(),
        &GenericAllocationPolicy,
    )
    .unwrap();
    let lease = host.open_memory(&StableKey::parse(name).unwrap()).unwrap();
    MAP.set(Some(if create {
        StableBTreeMap::create(lease)
    } else {
        StableBTreeMap::load(lease)
    }));
}
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    open(true);
}
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn reopen() {
    open(false);
}

pub(crate) fn args() -> [u64; 4] {
    assert_eq!(ic0::msg_arg_data_size(), 32);
    let mut bytes = [0; 32];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[8 * i..8 * i + 8].try_into().unwrap()))
}
#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update seed")]
extern "C" fn seed() {
    let [start, size, _, _] = args();
    assert!(start < 512 && matches!(size, 16 | 1024 | 4096));
    MAP.with_borrow_mut(|map| {
        let map = map.as_mut().unwrap();
        for i in start..start + 32 {
            map.insert(2 * i, vec![i as u8; size as usize]);
        }
    });
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "IC entrypoint export")]
#[unsafe(export_name = "canister_update measure")]
extern "C" fn measure() {
    let [mode, count, pattern, size] = args();
    assert!(count <= 32 && pattern <= 2 && matches!(size, 16 | 4096));
    let keys: Vec<_> = (0..count)
        .map(|i| match pattern {
            0 => 64 + 2 * i,
            1 => 2 * i * 16,
            _ => 2 * i * 16 + 1,
        })
        .collect();
    MAP.with_borrow(|map| {
        let map = map.as_ref().unwrap();
        NODES.set(0);
        VALUES.set(0);
        let initial = HEAP.current_usage();
        HEAP.reset_peak_usage();
        let start = ic0::instruction_counter();
        let mut consumed = 0;
        let mut visit = |key: &u64, value: Option<Vec<u8>>| {
            assert_eq!(Some(key), keys.get(consumed));
            if key.is_multiple_of(2) {
                let bytes = value.unwrap();
                assert_eq!(bytes.len(), size as usize);
                assert!(bytes.iter().all(|&byte| byte == (key / 2) as u8));
            } else {
                assert!(value.is_none());
            }
            consumed += 1;
        };
        match mode {
            0 => {
                for key in &keys {
                    visit(key, map.get(key));
                }
            }
            #[cfg(feature = "shared-reads")]
            1 => map.visit_sorted_keys(&keys, visit),
            2 => {
                for row in map.range(64..128).take(count as usize) {
                    assert_eq!(*row.key(), 64 + 2 * consumed as u64);
                    consumed += 1;
                }
            }
            3 => {
                for key in map.keys_range(64..128).take(count as usize) {
                    assert_eq!(key, 64 + 2 * consumed as u64);
                    consumed += 1;
                }
            }
            _ => panic!("unknown mode"),
        }
        let instructions = ic0::instruction_counter() - start;
        let heap = HEAP.peak_usage() - initial;
        assert_eq!(consumed, count as usize);
        for word in [instructions, heap as u64, NODES.get(), VALUES.get()] {
            ic0::msg_reply_data_append(&word.to_le_bytes());
        }
        ic0::msg_reply();
    });
}
