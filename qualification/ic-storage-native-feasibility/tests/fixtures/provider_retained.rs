//! Application-shaped witness: public Storage/Platform APIs only, no allocator,
//! raw retained pointer, copied startup, pool or private native admission code.
#![deny(unsafe_code, dead_code)]
use ic_platform_runtime::raw as ic0;
use ic_storage::{
    checkpoint::{Memory, Record, Specification, Store},
    ordered::{Map, Mutation},
    retained::{Bank, Canister, Geometry, Grant, Root},
};
use std::cell::RefCell;
const ROOTS: [Root; 4] = [
    Root {
        id: 1,
        participant: 1,
        schema: 1,
        entries: 128,
        key_bytes: 512,
        value_bytes: 4096,
    },
    Root {
        id: 2,
        participant: 2,
        schema: 1,
        entries: 32,
        key_bytes: 512,
        value_bytes: 4096,
    },
    Root {
        id: 3,
        participant: 3,
        schema: 1,
        entries: 32,
        key_bytes: 512,
        value_bytes: 4096,
    },
    Root {
        id: 4,
        participant: 4,
        schema: 1,
        entries: 17,
        key_bytes: 1,
        value_bytes: 16384,
    },
];
fn geometry() -> Geometry {
    Geometry {
        bank: if cfg!(retained_b) { Bank::B } else { Bank::A },
        nodes: 512,
        bytes: 2 * 1024 * 1024,
    }
}
std::thread_local! {static GRANTS:RefCell<Vec<Grant>>=const{RefCell::new(Vec::new())};static TRANSIENT:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};}
fn issue() {
    GRANTS.with(|g| {
        *g.borrow_mut() = ROOTS
            .map(|root| Canister::grant(root.participant).unwrap())
            .into()
    });
}
fn args() -> [u64; 3] {
    assert_eq!(ic0::msg_arg_data_size(), 24);
    let mut b = [0; 24];
    ic0::msg_arg_data_copy(&mut b, 0);
    std::array::from_fn(|i| u64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap()))
}
fn with<R>(f: impl FnOnce(&[&Grant]) -> R) -> R {
    GRANTS.with(|g| {
        let g = g.borrow();
        f(&g.iter().collect::<Vec<_>>())
    })
}
#[derive(Clone, Copy)]
struct Stable;
impl Memory for Stable {
    fn size(&self) -> u64 {
        ic0::stable64_size()
    }
    fn grow(&self, n: u64) -> Option<u64> {
        ic0::stable64_grow(n)
    }
    fn read(&self, offset: u64, b: &mut [u8]) {
        ic0::stable64_read(offset, b)
    }
    fn write(&self, offset: u64, b: &[u8]) {
        ic0::stable64_write(offset, b)
    }
}
fn cell(create: bool) -> Record<Vec<u8>, Stable> {
    let specs = [Specification {
        id: 1,
        bytes: 1024 * 1024 + 16,
    }];
    let mut store = if create {
        Store::create(Stable, &specs)
    } else {
        Store::open(Stable, &specs)
    }
    .unwrap();
    let region = store.take(1).unwrap();
    if create {
        Record::create(region, Vec::new())
    } else {
        Record::load(region, 1024 * 1024)
    }
    .unwrap()
}
fn snapshot() -> Vec<u8> {
    with(|g| {
        Canister::read(g, |v| {
            let mut out = Vec::new();
            for root in ROOTS {
                let b = v.checkpoint(root.id, 1).unwrap();
                out.extend_from_slice(&(b.len() as u64).to_le_bytes());
                out.extend_from_slice(&b);
            }
            out
        })
        .unwrap()
    })
}
#[expect(unsafe_code, reason = "application-owned IC initialization export")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    Canister::create(geometry(), &ROOTS).unwrap();
    issue();
    let _ = cell(true);
}
#[expect(
    unsafe_code,
    reason = "application-owned close hook delegates custody to Storage"
)]
#[unsafe(export_name = "canister_pre_upgrade")]
extern "C" fn pre() {
    if !cfg!(retained_no_close) {
        Canister::close().unwrap();
    }
    if cfg!(retained_fail_close) {
        ic0::trap(b"failed source close");
    }
}
#[expect(
    unsafe_code,
    reason = "application-owned compatible or explicit portable recovery hook"
)]
#[unsafe(export_name = "canister_post_upgrade")]
extern "C" fn post() {
    let [mode, fault, _] = args();
    if mode == 0 {
        Canister::reopen(geometry(), &ROOTS).unwrap();
        issue();
    } else {
        assert_eq!(mode, 1);
        let snapshot = cell(false);
        Canister::create(geometry(), &ROOTS).unwrap();
        issue();
        let mut bytes = snapshot.get().as_slice();
        for root in ROOTS {
            let n = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
            bytes = &bytes[8..];
            let mut decoded = Map::<512, 16384>::new(root.entries);
            decoded.restore(&bytes[..n], 1).unwrap();
            decoded
                .with(|view| {
                    with(|g| {
                        Canister::update(g, |mut target| {
                            for (key, value) in
                                view.range(std::ops::Bound::Unbounded, std::ops::Bound::Unbounded)
                            {
                                target
                                    .insert(root.id, key.as_slice(), value.as_slice())
                                    .unwrap();
                            }
                        })
                        .unwrap()
                    });
                })
                .unwrap();
            bytes = &bytes[n..];
        }
        assert!(bytes.is_empty());
    }
    assert_eq!(fault, 0);
}
#[expect(unsafe_code, reason = "application-owned four-root write endpoint")]
#[unsafe(export_name = "canister_update put")]
extern "C" fn put() {
    let [slot, value, fault] = args();
    assert!(slot < 17);
    let key = [slot as u8; 512];
    with(|g| {
        Canister::update(g, |mut view| {
            for root in ROOTS {
                let k = if root.id == 4 { &key[..1] } else { &key[..] };
                view.apply(
                    root.id,
                    &[Mutation::Put(k, &vec![value as u8; root.value_bytes])],
                )
                .unwrap();
                assert_ne!(fault, root.id);
            }
        })
        .unwrap()
    });
    assert_ne!(fault, 5);
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "application-owned logical snapshot query")]
#[unsafe(export_name = "canister_query records")]
extern "C" fn records() {
    ic0::msg_reply_data_append(&snapshot());
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "application-owned explicit portable checkpoint endpoint"
)]
#[unsafe(export_name = "canister_update checkpoint")]
extern "C" fn checkpoint() {
    let mut record = cell(false);
    record.set(snapshot()).unwrap();
    ic0::msg_reply();
}
#[expect(
    unsafe_code,
    reason = "application-owned close/reopen and stale-token witness"
)]
#[unsafe(export_name = "canister_update cycle")]
extern "C" fn cycle() {
    Canister::close().unwrap();
    Canister::reopen(geometry(), &ROOTS).unwrap();
    with(|g| assert!(Canister::read(g, |_| ()).is_err()));
    issue();
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "ordinary Global heap lifetime across messages")]
#[unsafe(export_name = "canister_update heap")]
extern "C" fn heap() {
    let [length, byte, _] = args();
    TRANSIENT.with(|v| *v.borrow_mut() = vec![byte as u8; length as usize]);
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "ordinary Global heap query witness")]
#[unsafe(export_name = "canister_query heap_len")]
extern "C" fn heap_len() {
    TRANSIENT.with(|v| ic0::msg_reply_data_append(&(v.borrow().len() as u64).to_le_bytes()));
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "observe ordinary Wasm memory high-water mark")]
#[unsafe(export_name = "canister_query memory_pages")]
extern "C" fn memory_pages() {
    ic0::msg_reply_data_append(&(std::arch::wasm32::memory_size(0) as u64).to_le_bytes());
    ic0::msg_reply();
}
#[expect(unsafe_code, reason = "query isolation through the public runtime")]
#[unsafe(export_name = "canister_query query_put")]
extern "C" fn query_put() {
    put();
}
