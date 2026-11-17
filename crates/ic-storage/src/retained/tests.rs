use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
fn roots() -> [Root; 4] {
    std::array::from_fn(|i| Root {
        id: i as u64 + 1,
        participant: i as u64 + 1,
        schema: 1,
        entries: 32,
        key_bytes: 512,
        value_bytes: if i == 3 { 16384 } else { 4096 },
    })
}
fn geometry() -> Geometry {
    Geometry {
        bank: Bank::A,
        nodes: 256,
        bytes: 1536 * 1024,
    }
}
#[test]
fn host_reopens_all_roots_and_revokes_stale_grants() {
    let roots = roots();
    let runtime = Runtime::host(geometry(), &roots).unwrap();
    let grants = roots.map(|r| runtime.grant(r.participant).unwrap());
    runtime
        .update(&grants.each_ref(), |mut view| {
            for r in roots {
                view.insert(r.id, b"k", &[r.id as u8; 32]).unwrap();
            }
        })
        .unwrap();
    assert!(runtime.grant(1).is_err());
    runtime.close().unwrap();
    let directory = runtime.backing.header();
    let mut changed = roots;
    changed[3].schema += 1;
    assert_eq!(runtime.reopen(&changed), Err(Error::Declaration));
    assert_eq!(runtime.backing.header(), directory);
    runtime.reopen(&roots).unwrap();
    assert_eq!(runtime.read(&[&grants[0]], |_| ()), Err(Error::Authority));
    let next = roots.map(|r| runtime.grant(r.participant).unwrap());
    runtime
        .read(&next.each_ref(), |view| {
            for r in roots {
                assert_eq!(
                    view.get(r.id, b"k").unwrap().unwrap().as_slice(),
                    &[r.id as u8; 32]
                );
            }
        })
        .unwrap();
    assert!(
        runtime
            .update(&[&next[0]], |mut v| v.insert(2, b"x", b"bad"))
            .unwrap()
            .is_err()
    );
    runtime.close().unwrap();
}
#[test]
fn panic_poison_and_reentry_do_not_fake_rollback() {
    let runtime = Runtime::host(geometry(), &roots()).unwrap();
    let g = runtime.grant(1).unwrap();
    runtime
        .read(&[&g], |_| assert_eq!(runtime.close(), Err(Error::Busy)))
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| runtime.update(&[&g], |mut v| {
            v.insert(1, b"x", b"y").unwrap();
            panic!("host")
        })))
        .is_err()
    );
    assert_eq!(
        runtime.read(&[&g], |_| ()),
        Err(Error::Session(session::Error::Poisoned))
    );
    assert_eq!(
        runtime.close(),
        Err(Error::Session(session::Error::Poisoned))
    );
}
#[test]
fn corrupt_and_overlapping_closed_roots_refuse_unchanged() {
    let runtime = Runtime::host(geometry(), &roots()).unwrap();
    let g = runtime.grant(1).unwrap();
    let h = runtime.grant(2).unwrap();
    runtime
        .update(&[&g, &h], |mut v| {
            v.insert(1, b"k", b"v").unwrap();
            v.insert(2, b"other", b"root").unwrap();
        })
        .unwrap();
    runtime.close().unwrap();
    let original = runtime.backing.header();
    let mut bad = original;
    bad[100] ^= 1;
    runtime.backing.publish(bad);
    assert_eq!(runtime.reopen(&roots()), Err(Error::Directory));
    assert_eq!(runtime.backing.header(), bad);
    let mut images = directory::decode(&original, geometry(), &roots()).unwrap();
    images[1].nodes = images[0].nodes;
    let overlap = directory::encode(geometry(), &roots(), &images[..4], true);
    runtime.backing.publish(overlap);
    assert_eq!(runtime.reopen(&roots()), Err(Error::Geometry));
    assert_eq!(runtime.backing.header(), overlap);
    runtime.backing.publish(original);
    runtime.reopen(&roots()).unwrap();
    let next = runtime.grant(2).unwrap();
    runtime
        .read(&[&next], |v| {
            assert_eq!(v.get(2, b"other").unwrap().unwrap().as_slice(), b"root");
        })
        .unwrap();
    runtime.close().unwrap();
}

#[test]
fn zero_directory_cannot_reset_occupied_backing() {
    let runtime = Runtime::host(geometry(), &roots()).unwrap();
    let g = runtime.grant(1).unwrap();
    runtime
        .update(&[&g], |mut v| v.insert(1, b"k", b"v").unwrap())
        .unwrap();
    runtime.close().unwrap();
    let original = runtime.backing.header();
    runtime.backing.publish([0; DIRECTORY_BYTES]);
    assert!(matches!(
        Runtime::create(runtime.backing.clone(), &roots()),
        Err(Error::NotFresh)
    ));
    assert_eq!(runtime.backing.header(), [0; DIRECTORY_BYTES]);
    runtime.backing.publish(original);
    runtime.reopen(&roots()).unwrap();
}

#[test]
fn capacity_refusal_preserves_roots_and_reuses_deleted_payloads() {
    let runtime = Runtime::host(
        Geometry {
            bytes: 8192,
            ..geometry()
        },
        &roots(),
    )
    .unwrap();
    let g = runtime.grant(1).unwrap();
    let job = runtime.grant(4).unwrap();
    runtime
        .update(&[&g], |mut v| v.insert(1, b"k", b"v").unwrap())
        .unwrap();
    assert!(
        runtime
            .update(&[&job], |mut v| v.insert(4, b"j", &[7; 16384]))
            .unwrap()
            .is_err()
    );
    for _ in 0..64 {
        runtime
            .update(&[&g], |mut v| {
                v.insert(1, b"temporary", &[3; 1024]).unwrap();
                assert!(v.remove_discard(1, b"temporary").unwrap());
            })
            .unwrap();
    }
    runtime.close().unwrap();
    runtime.reopen(&roots()).unwrap();
    let next = runtime.grant(1).unwrap();
    runtime
        .read(&[&next], |v| {
            assert_eq!(v.get(1, b"k").unwrap().unwrap().as_slice(), b"v");
            assert!(!v.contains_key(1, b"temporary").unwrap());
        })
        .unwrap();
}

#[test]
fn foreign_duplicate_and_forgotten_grants_never_gain_authority() {
    let runtime = Runtime::host(geometry(), &roots()).unwrap();
    let other = Runtime::host(geometry(), &roots()).unwrap();
    let g = runtime.grant(1).unwrap();
    let foreign = other.grant(1).unwrap();
    assert_eq!(runtime.read(&[&foreign], |_| ()), Err(Error::Authority));
    assert_eq!(runtime.read(&[&g, &g], |_| ()), Err(Error::Authority));
    let result = runtime
        .update(&[&g], |mut v| {
            v.insert(1, b"k", b"v").unwrap();
            Err::<(), _>("business error")
        })
        .unwrap();
    assert_eq!(result, Err("business error"));
    std::mem::forget(g);
    runtime.close().unwrap();
    runtime.reopen(&roots()).unwrap();
    let next = runtime.grant(1).unwrap();
    runtime
        .read(&[&next], |v| {
            assert_eq!(v.get(1, b"k").unwrap().unwrap().as_slice(), b"v");
        })
        .unwrap();
}
