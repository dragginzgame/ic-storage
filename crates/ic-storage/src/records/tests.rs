extern crate std;
use super::*;
use alloc::vec;
use std::panic::{AssertUnwindSafe, catch_unwind};

const ROOTS: [Root; 3] = [
    Root {
        id: 1,
        participant: 10,
        schema: 1,
        capacity: 16_384,
    },
    Root {
        id: 2,
        participant: 10,
        schema: 2,
        capacity: 2 * 524_288 + 24,
    },
    Root {
        id: 3,
        participant: 20,
        schema: 1,
        capacity: 128,
    },
];
fn plan() -> Plan {
    Plan::new(&ROOTS, 64).unwrap()
}
fn backing(store: Store<HostMemory>) -> HostMemory {
    store
        .into_backing()
        .unwrap_or_else(|_| panic!("ended grants"))
}

#[test]
fn canic_sized_roots_share_one_owner_and_reopen_without_flush() {
    let plan = plan();
    assert_eq!(plan.physical_pages(), 17);
    let mut store =
        Store::create(HostMemory::default(), &plan, &[b"journal", b"custody", b""]).unwrap();
    let root = store.take(10).unwrap();
    let entry = store.take(20).unwrap();
    assert!(matches!(store.take(10), Err(Error::Claimed)));
    assert!(matches!(store.take(99), Err(Error::Authority)));
    root.read(|read| {
        assert_eq!(read.get(1).unwrap(), b"journal");
        assert_eq!(read.get(3), Err(Error::Authority));
    })
    .unwrap();
    store
        .update(&[&root, &entry], |mut write| {
            write.set(1, &vec![7; 16_384]).unwrap();
            write.set(2, &vec![9; 2 * 524_288 + 24]).unwrap();
            write.set(3, b"bound worker").unwrap();
            assert_eq!(write.get(3).unwrap(), b"bound worker");
        })
        .unwrap();
    let store = store
        .into_backing()
        .err()
        .expect("live grants retain ownership");
    drop(root);
    drop(entry);
    let bytes = backing(store).into_bytes();
    assert_eq!(bytes.len(), 17 * 65536);
    let before = bytes.clone();
    let mut reopened = Store::open(HostMemory::from_bytes(bytes).unwrap(), &plan).unwrap();
    let root = reopened.take(10).unwrap();
    let entry = reopened.take(20).unwrap();
    reopened
        .read(&[&root, &entry], |read| {
            assert_eq!(read.get(1).unwrap(), &[7; 16_384]);
            assert_eq!(read.get(2).unwrap().len(), 2 * 524_288 + 24);
            assert_eq!(read.get(3).unwrap(), b"bound worker");
        })
        .unwrap();
    drop(root);
    drop(entry);
    assert_eq!(backing(reopened).into_bytes(), before);
}

#[test]
fn capacity_preparation_and_normal_errors_preserve_the_declared_contract() {
    assert!(Plan::new(&ROOTS, 16).is_err());
    let plan = plan();
    let error = Store::create(HostMemory::default(), &plan, &[b"one"])
        .err()
        .unwrap();
    assert!(error.backing.bytes().is_empty());
    let too_large = vec![0; 16_385];
    let error = Store::create(error.backing, &plan, &[&too_large, b"", b""])
        .err()
        .unwrap();
    assert_eq!(error.error, Error::Capacity);
    assert!(error.backing.bytes().is_empty());
    let mut store = Store::create(error.backing, &plan, &[b"old", b"custody", b""]).unwrap();
    let mut grant = store.take(10).unwrap();
    let returned = grant
        .update(|mut write| {
            write.set(1, b"committed intent")?;
            write.set(1, &too_large)?;
            Ok::<_, Error>(())
        })
        .unwrap();
    assert_eq!(returned, Err(Error::Capacity));
    grant
        .read(|read| assert_eq!(read.get(1).unwrap(), b"committed intent"))
        .unwrap();
    drop(grant);
    let mut reopened = Store::open(backing(store), &plan).unwrap();
    reopened
        .take(10)
        .unwrap()
        .read(|read| assert_eq!(read.get(1).unwrap(), b"committed intent"))
        .unwrap();
}

#[test]
fn missing_corrupt_foreign_and_unsupported_roots_never_initialize() {
    let plan = plan();
    let error = Store::open(HostMemory::default(), &plan).err().unwrap();
    assert!(error.backing.bytes().is_empty());
    let store = Store::create(error.backing, &plan, &[b"journal", b"custody", b"binding"]).unwrap();
    let bytes = backing(store).into_bytes();
    let error = Store::create(
        HostMemory::from_bytes(bytes.clone()).unwrap(),
        &plan,
        &[b"", b"", b""],
    )
    .err()
    .unwrap();
    assert_eq!(error.backing.bytes(), bytes);
    for offset in [0, 8, 16, 24, 120, 128, 136, 144, 152, 160, 168, 176, 181] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        let error = Store::open(HostMemory::from_bytes(bad.clone()).unwrap(), &plan)
            .err()
            .unwrap();
        assert_eq!(error.backing.bytes(), bad, "offset {offset}");
    }
    for field in 0..4 {
        let mut changed = ROOTS;
        match field {
            0 => changed[0].id = 0,
            1 => changed[0].participant += 1,
            2 => changed[0].schema += 1,
            _ => changed[0].capacity += 1,
        }
        if let Ok(other) = Plan::new(&changed, 64) {
            let error = Store::open(HostMemory::from_bytes(bytes.clone()).unwrap(), &other)
                .err()
                .unwrap();
            assert_eq!(error.backing.bytes(), bytes);
        }
    }
}

#[test]
fn grants_cannot_mix_owners_or_reissue_after_drop_and_unwind_poisons_all() {
    let plan = plan();
    let mut store = Store::create(HostMemory::default(), &plan, &[b"", b"", b""]).unwrap();
    let mut other = Store::create(HostMemory::default(), &plan, &[b"", b"", b""]).unwrap();
    let mut root = store.take(10).unwrap();
    let entry = store.take(20).unwrap();
    let foreign = other.take(20).unwrap();
    assert_eq!(
        store.read(&[&root, &foreign], |_| ()),
        Err(Error::Authority)
    );
    assert_eq!(store.read(&[&root, &root], |_| ()), Err(Error::Authority));
    assert_eq!(store.read(&[], |_| ()), Err(Error::Authority));
    root.update(|_| assert_eq!(entry.read(|_| ()), Err(Error::Busy)))
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| root.update(|mut write| {
            write.set(1, b"partial host write").unwrap();
            panic!("host unwind");
        })))
        .is_err()
    );
    assert_eq!(entry.read(|_| ()), Err(Error::Poisoned));
    assert_eq!(root.update(|_| ()), Err(Error::Poisoned));
    drop(root);
    drop(entry);
    assert!(store.into_backing().is_err());
    drop(foreign);
    assert!(matches!(other.take(20), Err(Error::Claimed)));
    let forgotten = other.take(10).unwrap();
    core::mem::forget(forgotten);
    assert!(other.into_backing().is_err());
}
