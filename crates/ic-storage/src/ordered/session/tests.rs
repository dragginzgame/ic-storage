use super::*;
use alloc::vec;
use core::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
extern crate std;

fn roots() -> [Root; 2] {
    [
        Root {
            id: 1,
            participant: 10,
            schema: 11,
            entries: 4,
            key_bytes: 8,
            value_bytes: 32,
        },
        Root {
            id: 2,
            participant: 20,
            schema: 12,
            entries: 4,
            key_bytes: 4,
            value_bytes: 8,
        },
    ]
}
fn fresh() -> Session<8, 32> {
    let roots = roots();
    Session::from_maps(
        vec![(roots[0], Map::new(4)), (roots[1], Map::new(4))],
        &roots,
    )
    .unwrap()
}

#[test]
fn independent_grants_compose_and_return_maps_only_after_all_grants_end() {
    let mut session = fresh();
    let a = session.take(10).unwrap();
    let b = session.take(20).unwrap();
    session
        .update(&[&a, &b], |mut view| {
            view.insert(1, b"row", b"payload").unwrap();
            view.insert(2, b"key", b"row").unwrap();
        })
        .unwrap();
    a.read(|view| {
        assert_eq!(view.get(1, b"row").unwrap().unwrap().as_slice(), b"payload");
        assert_eq!(view.get(2, b"key"), Err(Error::Authority));
        assert_eq!(view.range(1, Unbounded, Unbounded).unwrap().count(), 1);
    })
    .unwrap();
    session = session
        .into_maps()
        .err()
        .expect("live grants retain custody");
    drop(a);
    assert!(matches!(session.take(10), Err(Error::Claimed)));
    session = session
        .into_maps()
        .err()
        .expect("second grant is still live");
    drop(b);
    let maps = session.into_maps().ok().unwrap();
    let saved = maps
        .iter()
        .map(|(_, map)| map.checkpoint(3).unwrap())
        .collect::<Vec<_>>();
    // Explicitly end and re-admit each map's live allocations before a generation.
    let maps = maps
        .into_iter()
        .map(|(root, map)| {
            let map = Map::admit(map.release().unwrap(), root.entries)
                .unwrap_or_else(|_| panic!("admission"));
            (root, map)
        })
        .collect();
    let mut reopened = Session::from_maps(maps, &roots()).unwrap();
    let a = reopened.take(10).unwrap();
    let b = reopened.take(20).unwrap();
    reopened
        .read(&[&a, &b], |view| {
            assert_eq!(view.checkpoint(1, 3).unwrap(), saved[0]);
            assert_eq!(view.checkpoint(2, 3).unwrap(), saved[1]);
        })
        .unwrap();
    drop(reopened); // bootstrap need not outlive participants
    a.read(|view| assert_eq!(view.len(1), Ok(1))).unwrap();
    b.read(|view| assert_eq!(view.len(2), Ok(1))).unwrap();
}

#[test]
#[expect(
    clippy::forget_non_drop,
    reason = "prove forgotten scoped borrows cannot leak custody"
)]
fn ordinary_refusal_preserves_prior_writes_and_forgotten_views_are_harmless() {
    let mut session = fresh();
    let a = session.take(10).unwrap();
    let b = session.take(20).unwrap();
    let result = session.update(&[&a, &b], |mut view| {
        view.insert(1, b"a", b"committed")?;
        view.insert(2, b"toolong", b"x")
    });
    assert_eq!(result, Ok(Err(Error::Collection(MapError::Limit))));
    session
        .read(&[&a, &b], |view| {
            assert_eq!(view.get(1, b"a").unwrap().unwrap().as_slice(), b"committed");
            assert_eq!(view.len(2), Ok(0));
        })
        .unwrap();
    session.update(&[&a, &b], |view| mem::forget(view)).unwrap();
    a.read(|view| mem::forget(view)).unwrap();
    session
        .update(&[&a, &b], |mut view| view.insert(2, b"a", b"ok"))
        .unwrap()
        .unwrap();
    drop((a, b));
    assert!(session.into_maps().is_ok());
}

#[test]
fn foreign_duplicate_and_reentrant_access_refuse_and_unwind_poison_is_shared() {
    let mut session = fresh();
    let mut foreign = fresh();
    let a = session.take(10).unwrap();
    let mut b = session.take(20).unwrap();
    let other = foreign.take(10).unwrap();
    for grants in [&[][..], &[&a, &a][..], &[&other][..]] {
        assert_eq!(session.read(grants, |_| ()), Err(Error::Authority));
    }
    a.read(|_| assert_eq!(b.update(|_| ()), Err(Error::Busy)))
        .unwrap();
    b.update(|_| assert_eq!(a.read(|_| ()), Err(Error::Busy)))
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = session.update(&[&a, &b], |mut view| {
                view.insert(1, b"a", b"partial").unwrap();
                panic!("before index write");
            });
        }))
        .is_err()
    );
    assert_eq!(a.read(|_| ()), Err(Error::Poisoned));
    assert_eq!(b.update(|_| ()), Err(Error::Poisoned));
    drop((a, b));
    assert!(session.into_maps().is_err());
    assert!(other.read(|_| ()).is_ok());
}

#[test]
fn declaration_refusal_preserves_every_map_and_checks_admitted_record_bounds() {
    let original = roots();
    let mut first = Map::<8, 32>::new(4);
    first
        .with(|mut v| v.insert(b"longkey", b"payload"))
        .unwrap()
        .unwrap();
    let saved = first.checkpoint(1).unwrap();
    let mut maps = vec![(original[0], first), (original[1], Map::new(4))];
    for bad in [
        Root {
            schema: 99,
            ..original[0]
        },
        Root {
            participant: 99,
            ..original[0]
        },
        Root {
            entries: 5,
            ..original[0]
        },
    ] {
        let refusal = Session::from_maps(maps, &[bad, original[1]]).err().unwrap();
        assert_eq!(refusal.error, Error::Declaration);
        maps = refusal.maps;
        assert_eq!(maps[0].1.checkpoint(1).unwrap(), saved);
    }
    maps[0].0.key_bytes = 2;
    let expected = [maps[0].0, maps[1].0];
    let refusal = Session::from_maps(maps, &expected).err().unwrap();
    assert_eq!(refusal.error, Error::Collection(MapError::Limit));
    assert_eq!(refusal.maps[0].1.checkpoint(1).unwrap(), saved);
    assert!(matches!(
        Session::<8, 32>::from_maps(Vec::new(), &[]),
        Err(AdmissionError {
            error: Error::Declaration,
            ..
        })
    ));
}

#[test]
fn forgotten_grants_prevent_reissuance_and_explicit_quarantine_cannot_be_reset() {
    let mut session = fresh();
    let a = session.take(10).unwrap();
    mem::forget(a);
    assert!(matches!(session.take(10), Err(Error::Claimed)));
    session = session.into_maps().err().unwrap();
    let b = session.take(20).unwrap();
    session.quarantine().unwrap();
    assert_eq!(b.read(|_| ()), Err(Error::Poisoned));
    drop(b);
    assert!(session.into_maps().is_err());
}

#[test]
fn declaration_limits_and_last_grant_bit_are_checked_before_issuance() {
    let original = roots();
    for bad in [
        Root {
            id: 0,
            ..original[0]
        },
        Root {
            id: 2,
            ..original[0]
        },
        Root {
            participant: 0,
            ..original[0]
        },
        Root {
            schema: 0,
            ..original[0]
        },
        Root {
            entries: 5,
            ..original[0]
        },
        Root {
            key_bytes: 9,
            ..original[0]
        },
        Root {
            value_bytes: 33,
            ..original[0]
        },
    ] {
        let expected = [bad, original[1]];
        let maps = vec![(bad, Map::<8, 32>::new(4)), (original[1], Map::new(4))];
        assert_eq!(
            Session::from_maps(maps, &expected).err().unwrap().error,
            Error::Declaration
        );
    }
    let roots = (1..=64)
        .map(|id| Root {
            id,
            participant: 7,
            schema: 1,
            entries: 0,
            key_bytes: 0,
            value_bytes: 0,
        })
        .collect::<Vec<_>>();
    let maps = roots
        .iter()
        .map(|root| (*root, Map::<0, 0>::new(0)))
        .collect();
    let mut session = Session::from_maps(maps, &roots).unwrap();
    let grant = session.take(7).unwrap();
    grant
        .read(|view| {
            assert_eq!(view.len(1), Ok(0));
            assert_eq!(view.len(64), Ok(0));
        })
        .unwrap();
    drop(grant);
    let mut maps = session.into_maps().ok().unwrap();
    let root = Root { id: 65, ..roots[0] };
    maps.push((root, Map::new(0)));
    let mut expected = roots;
    expected.push(root);
    assert_eq!(
        Session::from_maps(maps, &expected).err().unwrap().error,
        Error::Declaration
    );
    let mut poisoned = Map::<8, 32>::new(4);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = poisoned.with::<()>(|_| panic!("map failure"));
        }))
        .is_err()
    );
    let maps = vec![(original[0], poisoned), (original[1], Map::new(4))];
    let refusal = Session::from_maps(maps, &original).err().unwrap();
    assert_eq!(refusal.error, Error::Collection(MapError::Poisoned));
    assert_eq!(refusal.maps[0].1.checkpoint(1), Err(MapError::Poisoned));
}

#[test]
fn granted_paged_export_binds_identity_and_preserves_native_records_on_sink_failure() {
    let root = Root {
        id: 7,
        participant: 8,
        schema: 1,
        entries: 64,
        key_bytes: 8,
        value_bytes: 4096,
    };
    let mut map = Map::<8, 4096>::new(64);
    for key in 0..16u64 {
        map.with(|mut view| view.insert(&key.to_be_bytes(), &[9; 4096]).unwrap())
            .unwrap();
    }
    let before = map.checkpoint(3).unwrap();
    let mut owner = Session::from_maps(vec![(root, map)], &[root]).unwrap();
    let grant = owner.take(8).unwrap();
    grant
        .read(|view| {
            assert_eq!(
                view.export_pages(9, 3, |_, _| -> Result<(), ()> {
                    panic!("unauthorized sink")
                }),
                Err(ExportError::Session(Error::Authority))
            );
            assert_eq!(
                view.export_pages(7, 3, |_, _| Err(42)),
                Err(ExportError::Pages(
                    crate::ordered::paged::ExportError::Sink(42)
                ))
            );
            assert_eq!(view.checkpoint(7, 3).unwrap(), before);
            let mut pages = Vec::new();
            let manifest = view
                .export_pages(7, 3, |index, page| {
                    pages.push((index, page.to_vec()));
                    Ok::<_, ()>(())
                })
                .unwrap();
            let manifest =
                crate::ordered::paged::Manifest::decode(&manifest.encode(), 3, 7, 64).unwrap();
            let mut reader = manifest.reader::<8, 4096>();
            let mut count = 0;
            for (index, page) in pages.iter().rev() {
                assert_eq!(reader.next_page(), Some(*index));
                count += reader.read_page(page).unwrap().records().count();
            }
            assert_eq!(count, 16);
            assert_eq!(reader.next_page(), None);
        })
        .unwrap();
}
