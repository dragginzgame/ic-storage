extern crate std;

use super::{Error, Map, Slot, table};
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    vec::Vec,
};

mod admission;
mod checkpoint;

fn entries(map: &mut Map) -> BTreeMap<u64, u64> {
    map.with(|view| {
        let mut result = BTreeMap::new();
        view.for_each(|key, value| {
            assert!(result.insert(key, value).is_none());
        });
        result
    })
    .unwrap()
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn native_rebuild_preserves_source_and_has_an_independent_portable_path() {
    let mut source = Map::new(100);
    source
        .with(|mut view| {
            for key in [0, 7, 99, u64::MAX] {
                view.insert(key, key.wrapping_add(17)).unwrap();
            }
        })
        .unwrap();
    let original = entries(&mut source);
    let saved = source.checkpoint(7).unwrap();
    assert!(matches!(
        source.rebuild_in(101, std::alloc::Global),
        Err(Error::Limit)
    ));
    assert_eq!(source.checkpoint(7).unwrap(), saved);
    let mut candidate = source.rebuild_in(64, std::alloc::Global).unwrap();
    assert!(candidate.capacity().unwrap() >= 64);
    assert_eq!(entries(&mut candidate), original);
    candidate
        .with(|mut view| {
            view.remove(7);
            view.insert(8, 9).unwrap();
        })
        .unwrap();
    assert_eq!(entries(&mut source), original);
    let checkpoint = candidate.checkpoint(8).unwrap();
    let mut recovered = Map::from_checkpoint(&checkpoint, 8, 100).unwrap();
    assert_eq!(entries(&mut recovered), entries(&mut candidate));
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = source.with(|_| panic!("poison"));
    }));
    assert!(matches!(
        source.rebuild_in(8, std::alloc::Global),
        Err(Error::Poisoned)
    ));
}

#[test]
fn differential_mixed_histories_survive_growth_deletion_admission_and_recovery() {
    for seed in [1, 0x5eed, 0xdead_beef, u64::MAX] {
        let mut state = seed;
        let mut actual = Map::new(4096);
        let mut expected = BTreeMap::new();
        for step in 0..100_000 {
            let choice = next(&mut state);
            let key = next(&mut state) % 4096;
            let value = next(&mut state);
            actual
                .with(|mut view| match choice % 5 {
                    0 | 1 => assert_eq!(view.insert(key, value), Ok(expected.insert(key, value))),
                    2 => assert_eq!(view.remove(key), expected.remove(&key)),
                    _ => assert_eq!(view.get(key), expected.get(&key).copied()),
                })
                .unwrap();
            if step % 8192 == 0 {
                assert_eq!(entries(&mut actual), expected);
                let bytes = actual.checkpoint(step).unwrap();
                actual = Map::from_checkpoint(&bytes, step, 4096).unwrap();
                let (slots, len) = actual.release().unwrap();
                actual =
                    Map::admit(slots, len, 4096).unwrap_or_else(|_| panic!("native admission"));
                assert_eq!(entries(&mut actual), expected);
            }
        }
        assert_eq!(entries(&mut actual), expected);
        assert_eq!(actual.len(), Ok(expected.len()));
    }
}

#[test]
fn deletion_closes_wrapping_collision_chains_and_reuses_capacity() {
    let keys: Vec<_> = (0..1000)
        .filter(|key| table::home(*key, 7) == 7)
        .take(6)
        .collect();
    assert_eq!(keys.len(), 6);
    for removed in 0..keys.len() {
        let mut map = Map::new(6);
        map.with(|mut view| {
            for key in &keys {
                assert_eq!(view.insert(*key, *key + 1), Ok(None));
            }
            assert_eq!(view.remove(keys[removed]), Some(keys[removed] + 1));
            assert_eq!(view.remove(keys[removed]), None);
            for (index, key) in keys.iter().enumerate() {
                assert_eq!(
                    view.get(*key),
                    if index == removed {
                        None
                    } else {
                        Some(*key + 1)
                    }
                );
            }
            assert_eq!(view.insert(keys[removed], u64::MAX), Ok(None));
            assert_eq!(view.get(keys[removed]), Some(u64::MAX));
        })
        .unwrap();
        assert_eq!(map.allocated_bytes(), Ok(8 * 24));
    }
}

#[test]
fn limits_allow_updates_and_refusals_preserve_prior_writes() {
    let mut map = Map::new(2);
    assert_eq!(
        map.with(|mut view| {
            view.insert(0, u64::MAX)?;
            view.insert(u64::MAX, 0)?;
            assert_eq!(view.insert(0, 17), Ok(Some(u64::MAX)));
            assert_eq!(view.reserve(usize::MAX), Err(Error::Limit));
            view.insert(9, 10)
        }),
        Ok(Err(Error::Limit))
    );
    assert_eq!(entries(&mut map), BTreeMap::from([(0, 17), (u64::MAX, 0)]));
    let allocated = map.allocated_bytes();
    map.with(|mut view| {
        view.clear();
        assert!(view.is_empty());
        assert_eq!(view.insert(1, 2), Ok(None));
    })
    .unwrap();
    assert_eq!(map.allocated_bytes(), allocated);
    let mut zero = Map::new(0);
    assert_eq!(
        zero.with(|mut view| view.insert(0, 0)),
        Ok(Err(Error::Limit))
    );
    assert_eq!(zero.allocated_bytes(), Ok(0));
}

#[test]
fn forgetting_a_view_finishes_but_caught_unwind_requires_explicit_recovery() {
    let mut map = Map::new(8);
    map.with(|mut view| {
        view.insert(1, 2).unwrap();
        #[expect(
            clippy::forget_non_drop,
            reason = "forget must not bypass owner finalization"
        )]
        std::mem::forget(view);
    })
    .unwrap();
    let saved = map.checkpoint(7).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = map.with(|mut view| {
            view.insert(1, 9).unwrap();
            view.insert(2, 3).unwrap();
            panic!("after partial mutation");
        });
    }));
    assert!(result.is_err());
    assert_eq!(map.len(), Err(Error::Poisoned));
    assert_eq!(map.checkpoint(7), Err(Error::Poisoned));
    assert_eq!(
        map.with(|_| panic!("poisoned callback ran")),
        Err(Error::Poisoned)
    );
    assert_eq!(map.recover(&saved, 8), Err(Error::Epoch));
    assert_eq!(map.len(), Err(Error::Poisoned));
    map.recover(&saved, 7).unwrap();
    assert_eq!(entries(&mut map), BTreeMap::from([(1, 2)]));
}
