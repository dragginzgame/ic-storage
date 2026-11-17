//! DB-shaped contracts: lazy payload access, cleanup and four named roots.
use super::*;
use alloc::vec::Vec;
use core::{
    cell::Cell,
    ops::Bound::{Excluded, Included, Unbounded},
};
extern crate std;

std::thread_local! { static VALUE_ACCESSES: Cell<usize> = const { Cell::new(0) }; }
pub(super) fn observe_value_access() {
    VALUE_ACCESSES.with(|count| count.set(count.get() + 1));
}
fn reset_accesses() {
    VALUE_ACCESSES.with(|count| count.set(0));
}
fn accesses() -> usize {
    VALUE_ACCESSES.with(Cell::get)
}

#[test]
fn key_preflight_stops_before_payload_and_retries_the_unconsumed_candidate() {
    for reverse in [false, true] {
        let mut map = Map::<512, 4096>::new(16);
        let keys = (1..=5)
            .map(|id| {
                let mut k = [id; 512];
                k[0] = b'r';
                k
            })
            .collect::<Vec<_>>();
        map.with(|mut view| {
            for (index, key) in keys.iter().enumerate() {
                view.insert(key, &[index as u8; 4096]).unwrap();
            }
        })
        .unwrap();
        let mut consumed = Vec::new();
        let mut refused = Vec::new();
        map.with(|view| {
            reset_accesses();
            assert!(view.contains_key(&keys[2]));
            assert!(!view.contains_key(b"missing"));
            let mut entries = view.prefix(b"r").entries();
            for index in 0..3 {
                let entry = if reverse {
                    entries.next_back()
                } else {
                    entries.next()
                }
                .unwrap();
                assert_eq!(entry.value_len(), 4096);
                assert_eq!(accesses(), index.min(2));
                if index == 2 {
                    refused = entry.key().as_slice().to_vec();
                    break; // DB's budget refused: do not advance its logical cursor.
                }
                assert_eq!(entry.value().as_slice().len(), 4096);
                consumed.push(entry.key().as_slice().to_vec());
            }
            assert_eq!(accesses(), 2);
        })
        .unwrap();
        let cursor = consumed.last().unwrap();
        map.with(|mut view| assert!(view.remove_discard(cursor)))
            .unwrap();
        map.with(|view| {
            reset_accesses();
            let mut remaining = if reverse {
                view.range(Unbounded, Excluded(cursor.as_slice())).entries()
            } else {
                view.range(Excluded(cursor.as_slice()), Unbounded).entries()
            };
            let candidate = if reverse {
                remaining.next_back()
            } else {
                remaining.next()
            }
            .unwrap();
            assert_eq!(candidate.key().as_slice(), refused);
            assert_eq!(accesses(), 0);
            assert_eq!(candidate.value().as_slice().len(), 4096);
            assert_eq!(accesses(), 1);
        })
        .unwrap();
    }
}

#[test]
fn lazy_ranges_preserve_bounds_prefixes_and_mixed_end_exhaustion() {
    let mut map = Map::<4, 8>::new(16);
    let keys: &[&[u8]] = &[b"", b"a", b"a\0", b"ab", b"b", &[255], &[255, 255]];
    map.with(|mut view| {
        for key in keys {
            view.insert(key, b"payload").unwrap();
        }
        for lower in [
            Unbounded,
            Included(b"a".as_slice()),
            Excluded(b"a".as_slice()),
            Included(b"z".as_slice()),
        ] {
            for upper in [
                Unbounded,
                Included(b"b".as_slice()),
                Excluded(b"b".as_slice()),
                Included(b"a".as_slice()),
            ] {
                let eager = view
                    .range(lower, upper)
                    .map(|(key, _)| key.as_slice().to_vec())
                    .collect::<Vec<_>>();
                reset_accesses();
                let lazy = view
                    .range(lower, upper)
                    .entries()
                    .map(|entry| entry.key().as_slice().to_vec())
                    .collect::<Vec<_>>();
                assert_eq!(eager, lazy);
                let backward = view
                    .range(lower, upper)
                    .entries()
                    .rev()
                    .map(|entry| entry.key().as_slice().to_vec())
                    .collect::<Vec<_>>();
                assert_eq!(backward, eager.into_iter().rev().collect::<Vec<_>>());
                assert_eq!(accesses(), 0);
            }
        }
        for prefix in [b"".as_slice(), b"a", &[255], &[255, 255], b"absent"] {
            let expected = keys
                .iter()
                .filter(|key| key.starts_with(prefix))
                .map(|key| key.to_vec())
                .collect::<Vec<_>>();
            let actual = view
                .prefix(prefix)
                .entries()
                .map(|entry| entry.key().as_slice().to_vec())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
        let mut range = view.range(Unbounded, Unbounded);
        assert_eq!(range.next().unwrap().0.as_slice(), b"");
        let mut lazy = range.entries();
        for key in [keys[6], keys[1], keys[5], keys[2], keys[4], keys[3]]
            .into_iter()
            .enumerate()
        {
            let entry = if key.0 % 2 == 0 {
                lazy.next_back()
            } else {
                lazy.next()
            }
            .unwrap();
            assert_eq!(entry.key().as_slice(), key.1);
        }
        assert!(lazy.next().is_none() && lazy.next_back().is_none() && lazy.next().is_none());
    })
    .unwrap();
}

#[test]
fn discard_and_batch_cleanup_do_not_materialize_old_job_payloads() {
    let mut map = Map::<1, 16384>::new(17);
    map.with(|mut view| {
        for id in 0..17 {
            view.insert(&[id], &[id; 16384]).unwrap();
        }
    })
    .unwrap();
    let usage = map.allocation_usage().unwrap();
    map.with(|mut view| {
        reset_accesses();
        // Include internal AVL nodes with two children and an absent key.
        for id in [8, 4, 12, 0] {
            assert!(view.remove_discard(&[id]));
        }
        assert!(!view.remove_discard(&[99]));
        view.apply(&[
            Mutation::Remove(&[1]),
            Mutation::Remove(&[2]),
            Mutation::Remove(&[1]),
        ])
        .unwrap();
        assert_eq!(accesses(), 0);
        assert_eq!(view.len(), 11);
        for id in [8, 4, 12, 0, 1, 2] {
            view.insert(&[id], &[id; 16384]).unwrap();
        }
    })
    .unwrap();
    assert_eq!(map.allocation_usage().unwrap(), usage);
    let saved = map.checkpoint(9).unwrap();
    let mut reopened =
        Map::admit(map.release().unwrap(), 17).unwrap_or_else(|_| panic!("reclaimed image"));
    assert_eq!(reopened.checkpoint(9).unwrap(), saved);
    reopened
        .with(|view| {
            for id in 0..17 {
                assert_eq!(view.get(&[id]).unwrap().as_slice(), &[id; 16384]);
            }
        })
        .unwrap();
}

#[test]
fn public_session_supports_db_main_source_target_and_separate_job_bounds() {
    use session::{Error as SessionError, Root, Session};
    let roots = core::array::from_fn::<_, 4, _>(|index| Root {
        id: index as u64 + 1,
        participant: index as u64 + 10,
        schema: 1,
        entries: if index == 3 { 17 } else { 128 },
        key_bytes: if index == 3 { 1 } else { 512 },
        value_bytes: if index == 3 { 16384 } else { 4096 },
    });
    let maps = roots
        .iter()
        .map(|root| (*root, Map::<512, 16384>::new(root.entries)))
        .collect();
    let mut session = Session::from_maps(maps, &roots).unwrap();
    let main = session.take(10).unwrap();
    let source = session.take(11).unwrap();
    let target = session.take(12).unwrap();
    let jobs = session.take(13).unwrap();
    let participants = [&main, &source, &target, &jobs];
    let row = [7; 4096];
    session
        .update(&participants, |mut view| {
            view.apply(
                1,
                &[
                    Mutation::Put(b"row:1", &row),
                    Mutation::Put(b"idx:a:1", b"row:1"),
                    Mutation::Put(b"count", &1u64.to_le_bytes()),
                    Mutation::Put(b"revision", &1u64.to_le_bytes()),
                ],
            )
            .unwrap();
            view.insert(2, b"row:1", &row).unwrap();
            for id in 0..17 {
                view.insert(4, &[id], &[id; 16384]).unwrap();
            }
            assert_eq!(
                view.insert(4, &[17], b"overflow"),
                Err(SessionError::Collection(Error::Limit))
            );
        })
        .unwrap();
    session
        .read(&participants, |view| {
            reset_accesses();
            for entry in view.prefix(1, b"idx:").unwrap().entries().rev() {
                assert_eq!(entry.key().as_slice(), b"idx:a:1");
                assert!(view.contains_key(1, b"row:1").unwrap());
                assert_eq!(accesses(), 0);
                let primary = entry.value();
                assert_eq!(
                    view.get(1, primary.as_slice()).unwrap().unwrap().as_slice(),
                    row
                );
            }
            for entry in view.prefix(2, b"row:").unwrap().entries() {
                assert!(!view.contains_key(3, entry.key().as_slice()).unwrap());
            }
            assert_eq!(view.len(4).unwrap(), 17);
        })
        .unwrap();
    let before = main.read(|view| view.checkpoint(1, 1)).unwrap().unwrap();
    session
        .update(&participants, |mut view| {
            assert_eq!(
                view.apply(
                    1,
                    &[
                        Mutation::Put(b"revision", b"bad"),
                        Mutation::Put(b"too-large", &[9; 4097])
                    ]
                ),
                Err(SessionError::Collection(Error::Limit))
            );
            assert_eq!(view.read().checkpoint(1, 1).unwrap(), before);
            view.apply(3, &[Mutation::Put(b"row:1", &row)]).unwrap();
            assert!(view.remove_discard(4, &[0]).unwrap());
            view.insert(4, &[0], b"progress").unwrap();
        })
        .unwrap();
    main.read(|view| assert_eq!(view.contains_key(4, &[0]), Err(SessionError::Authority)))
        .unwrap();
    drop((main, source, target, jobs));
    assert!(session.into_maps().is_ok());
}
