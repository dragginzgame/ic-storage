extern crate std;

use super::*;
use alloc::{vec, vec::Vec};
use core::ops::Bound::{self, Excluded, Included, Unbounded};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

fn records<const K: usize, const V: usize>(map: &mut Map<K, V>) -> Vec<(Vec<u8>, Vec<u8>)> {
    map.with(|view| {
        view.range(Unbounded, Unbounded)
            .map(|(key, value)| (key.as_slice().to_vec(), value.as_slice().to_vec()))
            .collect()
    })
    .unwrap()
}

#[test]
fn mixed_operations_balance_reclaim_and_match_independent_ordered_oracle() {
    let mut map = Map::<8, 16>::new(1024);
    let mut oracle = BTreeMap::new();
    let mut seed = 0x4913_9876_45ab_2013u64;
    for step in 0..12_000u64 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let key = (seed % 1024).to_be_bytes();
        let value = step.to_le_bytes();
        map.with(|mut view| {
            if seed & 3 == 0 {
                assert_eq!(
                    view.remove(&key).map(|v| v.as_slice().to_vec()),
                    oracle.remove(key.as_slice())
                );
            } else {
                assert_eq!(
                    view.insert(&key, &value)
                        .unwrap()
                        .map(|v| v.as_slice().to_vec()),
                    oracle.insert(key.to_vec(), value.to_vec())
                );
            }
            assert_eq!(
                view.get(&key).map(ByteSlice::as_slice),
                oracle.get(key.as_slice()).map(Vec::as_slice)
            );
            assert_eq!(view.len(), oracle.len());
        })
        .unwrap();
        if step % 101 == 0 {
            let image = map.release().unwrap();
            map = Map::admit(image, 1024).unwrap_or_else(|_| panic!("valid AVL refused at {step}"));
            assert_eq!(
                records(&mut map),
                oracle
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Vec<_>>()
            );
        }
    }
    let allocated = map.image.nodes.len();
    for key in oracle.keys() {
        map.with(|mut view| {
            view.remove(key);
        })
        .unwrap();
    }
    assert!(map.with(|v| v.is_empty()).unwrap());
    let image = map.release().unwrap();
    map = Map::admit(image, 1024).unwrap_or_else(|_| panic!("empty free list refused"));
    for key in 0..allocated as u64 {
        map.with(|mut v| v.insert(&key.to_be_bytes(), b"reused"))
            .unwrap()
            .unwrap();
    }
    assert_eq!(map.image.nodes.len(), allocated);
    tree::validate(&map.image, 1024).unwrap();
}

#[test]
fn monotone_growth_and_deletion_keep_logarithmic_height() {
    for reverse in [false, true] {
        let mut map = Map::<8, 0>::new(4096);
        for index in 0..4096u64 {
            let key = if reverse { 4095 - index } else { index };
            map.with(|mut view| view.insert(&key.to_be_bytes(), b""))
                .unwrap()
                .unwrap();
        }
        tree::validate(&map.image, 4096).unwrap();
        assert!(map.node(map.image.root).height <= 18);
        for index in 0..4096u64 {
            assert!(
                map.with(|mut view| view.remove(&index.to_be_bytes()))
                    .unwrap()
                    .is_some()
            );
            if index % 127 == 0 {
                tree::validate(&map.image, 4096).unwrap();
            }
        }
        tree::validate(&map.image, 4096).unwrap();
    }
}

fn within(key: &[u8], lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
    (match lower {
        Included(k) => key >= k,
        Excluded(k) => key > k,
        Unbounded => true,
    }) && (match upper {
        Included(k) => key <= k,
        Excluded(k) => key < k,
        Unbounded => true,
    })
}

#[test]
fn binary_ranges_prefixes_and_double_ended_exhaustion() {
    let keys: &[&[u8]] = &[
        b"",
        b"\0",
        b"\0\0",
        b"a",
        b"a\0",
        b"ab",
        b"b",
        b"\xff",
        b"\xff\xff",
    ];
    let mut map = Map::<4, 0>::new(keys.len());
    for key in keys {
        map.with(|mut view| view.insert(key, b"")).unwrap().unwrap();
    }
    let bounds: Vec<_> = core::iter::once(Unbounded)
        .chain(keys.iter().flat_map(|k| [Included(*k), Excluded(*k)]))
        .collect();
    for &lower in &bounds {
        for &upper in &bounds {
            let expected: Vec<_> = keys
                .iter()
                .filter(|key| within(key, lower, upper))
                .map(|key| key.to_vec())
                .collect();
            map.with(|view| {
                assert_eq!(
                    view.range(lower, upper)
                        .map(|(k, _)| k.as_slice().to_vec())
                        .collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(
                    view.range(lower, upper)
                        .rev()
                        .map(|(k, _)| k.as_slice().to_vec())
                        .collect::<Vec<_>>(),
                    expected.iter().rev().cloned().collect::<Vec<_>>()
                );
                let mut range = view.range(lower, upper);
                let mut front = 0;
                let mut back = expected.len();
                while front < back {
                    assert_eq!(range.next().unwrap().0.as_slice(), expected[front]);
                    front += 1;
                    if front < back {
                        back -= 1;
                        assert_eq!(range.next_back().unwrap().0.as_slice(), expected[back]);
                    }
                }
                assert!(range.next().is_none());
                assert!(range.next_back().is_none());
            })
            .unwrap();
        }
    }
    for prefix in keys
        .iter()
        .copied()
        .chain([b"aaa".as_slice(), b"12345".as_slice()])
    {
        let expected: Vec<_> = keys
            .iter()
            .filter(|key| key.starts_with(prefix))
            .map(|key| key.to_vec())
            .collect();
        map.with(|view| {
            assert_eq!(
                view.prefix(prefix)
                    .map(|(k, _)| k.as_slice().to_vec())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                view.prefix(prefix)
                    .rev()
                    .map(|(k, _)| k.as_slice().to_vec())
                    .collect::<Vec<_>>(),
                expected.iter().rev().cloned().collect::<Vec<_>>()
            );
        })
        .unwrap();
    }
}

#[test]
fn continuation_uses_exclusive_key_under_intervening_mutation() {
    let mut map = Map::<8, 8>::new(10);
    for key in [b"a", b"c", b"e"] {
        map.with(|mut v| v.insert(key, key)).unwrap().unwrap();
    }
    let cursor = map
        .with(|v| {
            v.range(Unbounded, Unbounded)
                .nth(1)
                .unwrap()
                .0
                .as_slice()
                .to_vec()
        })
        .unwrap();
    map.with(|mut v| {
        v.remove(b"c");
        v.insert(b"b", b"behind").unwrap();
        v.insert(b"d", b"ahead").unwrap();
        assert_eq!(
            v.range(Excluded(cursor.as_slice()), Unbounded)
                .map(|(k, _)| k.as_slice().to_vec())
                .collect::<Vec<_>>(),
            vec![b"d".to_vec(), b"e".to_vec()]
        );
    })
    .unwrap();
}

#[test]
fn batches_refuse_whole_plan_and_handle_duplicate_keys_at_capacity() {
    let mut map = Map::<4, 4>::new(2);
    map.with(|mut v| v.apply(&[Mutation::Put(b"a", b"1"), Mutation::Put(b"b", b"2")]))
        .unwrap()
        .unwrap();
    let before = map.checkpoint(1).unwrap();
    for batch in [
        vec![
            Mutation::Remove(b"a"),
            Mutation::Put(b"c", b"3"),
            Mutation::Put(b"d", b"4"),
        ],
        vec![Mutation::Remove(b"a"), Mutation::Put(b"c", b"large")],
        vec![Mutation::Put(b"c", b"3"), Mutation::Remove(b"a")],
        vec![Mutation::Remove(b"a"); 2049],
    ] {
        assert_eq!(
            map.with(|mut v| v.apply(&batch)).unwrap(),
            Err(Error::Limit)
        );
        assert_eq!(map.checkpoint(1).unwrap(), before);
    }
    map.with(|mut v| {
        v.apply(&[
            Mutation::Remove(b"a"),
            Mutation::Put(b"c", b"3"),
            Mutation::Remove(b"c"),
            Mutation::Put(b"c", b"4"),
            Mutation::Put(b"b", b"5"),
        ])
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        records(&mut map),
        vec![
            (b"b".to_vec(), b"5".to_vec()),
            (b"c".to_vec(), b"4".to_vec())
        ]
    );
    assert_eq!(map.image.nodes.len(), 2);
}

#[test]
fn db_sized_encoded_rows_secondary_entries_and_portable_reconstruction() {
    // Consumer-shaped data, independently authored: namespace + big-endian IDs;
    // a separate namespace sorts encoded secondary keys before their owner suffix.
    let mut map = Map::<512, 4096>::new(1024);
    for id in (0..256u64).rev() {
        let mut primary = vec![1];
        primary.extend_from_slice(&id.to_be_bytes());
        let mut secondary = vec![2, (id % 7) as u8];
        secondary.extend_from_slice(&id.to_be_bytes());
        let row = vec![id as u8; if id == 255 { 4096 } else { 17 + id as usize }];
        map.with(|mut v| {
            v.apply(&[
                Mutation::Put(&primary, &row),
                Mutation::Put(&secondary, &primary),
            ])
        })
        .unwrap()
        .unwrap();
    }
    let max_key = [255; 512];
    map.with(|mut v| v.insert(&max_key, b"full key"))
        .unwrap()
        .unwrap();
    map.with(|v| {
        assert_eq!(v.prefix(&[1]).count(), 256);
        for (_, owner) in v.prefix(&[2, 3]) {
            let row = v.get(owner.as_slice()).unwrap();
            assert!(!row.as_slice().is_empty());
        }
    })
    .unwrap();
    let expected = records(&mut map);
    let checkpoint = map.checkpoint(81).unwrap();
    drop(map);
    let mut recovered =
        Map::<512, 4096>::from_checkpoint_in(&checkpoint, 81, 1024, Global).unwrap();
    assert_eq!(records(&mut recovered), expected);
    assert_eq!(recovered.checkpoint(81).unwrap(), checkpoint);
    assert!(Map::<511, 4096>::from_checkpoint_in(&checkpoint, 81, 1024, Global).is_err());
    assert!(Map::<512, 4095>::from_checkpoint_in(&checkpoint, 81, 1024, Global).is_err());
    recovered
        .with(|mut v| {
            v.remove(&max_key);
            v.insert(b"new", b"continued").unwrap();
        })
        .unwrap();
    let image = recovered.release().unwrap();
    let pointer = image.nodes.as_ptr();
    let reopened = Map::admit(image, 1024).unwrap_or_else(|_| panic!("valid DB image"));
    assert_eq!(reopened.release().unwrap().nodes.as_ptr(), pointer);
}

#[test]
fn native_admission_rejects_corruption_without_mutating_allocation() {
    let mut map = Map::<8, 16>::new(8);
    for id in 0..7u8 {
        map.with(|mut v| v.insert(&[id], &[id])).unwrap().unwrap();
    }
    map.with(|mut v| v.remove(&[0])).unwrap();
    let original = map.release().unwrap();
    for case in 0..12 {
        let mut image = Image {
            nodes: original.nodes.clone(),
            payload: original.payload.clone(),
            root: original.root,
            free: original.free,
            len: original.len,
        };
        let root = image.root as usize - 1;
        let free = image.free as usize - 1;
        match case {
            0 => image.root = u64::MAX,
            1 => image.nodes[root].left = image.root,
            2 => image.free = image.root,
            3 => image.len += 1,
            4 => image.nodes[root].height += 1,
            5 => image.nodes[root].key.length = u64::MAX,
            6 => image.payload[image.nodes[root].value.offset as usize + 7] = 1,
            7 => image.nodes[root].key.offset = u64::MAX,
            8 => image.nodes[free].left = image.free,
            9 => image.nodes[free].value = image.nodes[root].value,
            10 => image.nodes[root].key = arena::Span::EMPTY,
            11 => image.free = 0,
            _ => unreachable!(),
        }
        let pointer = image.nodes.as_ptr();
        let fingerprint = image.nodes.clone();
        let payload = image.payload.clone();
        let err = match Map::admit(image, 8) {
            Ok(_) => panic!("accepted case {case}"),
            Err(err) => err,
        };
        assert_eq!(err.error, Error::NativeImage);
        assert_eq!(err.image.nodes.as_ptr(), pointer);
        assert_eq!(err.image.nodes, fingerprint);
        assert_eq!(err.image.payload, payload);
    }
}

#[test]
fn db_entity_batch_expansion_and_aggregate_job_value_bounds() {
    let mut map = Map::<512, 4096>::new(400);
    let mut entries = Vec::new();
    for id in 0..32u64 {
        let mut primary = vec![1];
        primary.extend_from_slice(&id.to_be_bytes());
        entries.push((primary.clone(), vec![id as u8; 4096]));
        for index in 0..8u8 {
            let mut key = vec![2, index];
            key.extend_from_slice(&id.to_be_bytes());
            entries.push((key, primary.clone()));
        }
    }
    let mutations: Vec<_> = entries
        .iter()
        .map(|(key, value)| Mutation::Put(key, value))
        .collect();
    map.with(|mut view| view.apply(&mutations))
        .unwrap()
        .unwrap();
    assert_eq!(map.with(|view| view.len()).unwrap(), 288);
    map.with(|view| {
        for (_, primary) in view.prefix(&[2]) {
            assert_eq!(view.get(primary.as_slice()).unwrap().as_slice().len(), 4096);
        }
    })
    .unwrap();
    let mut jobs = Map::<1, 16384>::new(17);
    jobs.with(|mut view| {
        view.insert(&[0], b"incarnation/generation").unwrap();
        for id in 1..=16u8 {
            view.insert(&[id], &vec![id; 16384]).unwrap();
        }
        assert_eq!(view.insert(&[1], &vec![0; 16385]), Err(Error::Limit));
        assert_eq!(view.get(&[1]).unwrap().as_slice(), vec![1; 16384]);
    })
    .unwrap();
    let checkpoint = jobs.checkpoint(12).unwrap();
    drop(jobs);
    let mut jobs = Map::<1, 16384>::from_checkpoint_in(&checkpoint, 12, 17, Global).unwrap();
    jobs.with(|mut view| {
        assert_eq!(view.range(Unbounded, Unbounded).count(), 17);
        assert_eq!(view.remove(&[16]).unwrap().as_slice(), vec![16; 16384]);
        view.insert(&[16], b"retired and reused").unwrap();
    })
    .unwrap();
}

fn seal(bytes: &mut [u8]) {
    let split = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..split]);
    bytes[split..].copy_from_slice(&digest);
}

#[test]
fn portable_format_refuses_corruption_resealed_bad_order_and_limits() {
    let mut map = Map::<8, 8>::new(4);
    map.with(|mut v| v.apply(&[Mutation::Put(b"a", b"1"), Mutation::Put(b"b", b"2")]))
        .unwrap()
        .unwrap();
    let bytes = map.checkpoint(9).unwrap();
    let mut golden = b"ORDMAP01\x09\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0".to_vec();
    golden.extend_from_slice(b"\x01\0\0\0\x01\0\0\0a1\x01\0\0\0\x01\0\0\0b2");
    golden.extend_from_slice(&Sha256::digest(&golden));
    assert_eq!(bytes, golden);
    for cut in 0..bytes.len() {
        assert!(Map::<8, 8>::from_checkpoint_in(&bytes[..cut], 9, 4, Global).is_err());
    }
    for index in 0..bytes.len() {
        let mut corrupt = bytes.clone();
        corrupt[index] ^= 1;
        assert!(map.restore(&corrupt, 9).is_err());
        assert_eq!(map.checkpoint(9).unwrap(), bytes);
    }
    assert!(matches!(
        Map::<8, 8>::from_checkpoint_in(&bytes, 10, 4, Global),
        Err(Error::Epoch)
    ));
    assert!(matches!(
        Map::<8, 8>::from_checkpoint_in(&bytes, 9, 1, Global),
        Err(Error::Limit)
    ));
    for index in [24, 28, 42] {
        let mut corrupt = bytes.clone();
        corrupt[index] = if index == 42 { b'a' } else { 255 };
        seal(&mut corrupt);
        assert!(Map::<8, 8>::from_checkpoint_in(&corrupt, 9, 4, Global).is_err());
    }
    let mut trailing = bytes;
    trailing.insert(trailing.len() - 32, 0);
    seal(&mut trailing);
    assert!(Map::<8, 8>::from_checkpoint_in(&trailing, 9, 4, Global).is_err());
}

#[test]
fn callback_errors_forgetting_and_poison_recovery_preserve_custody() {
    let mut map = Map::<8, 8>::new(8);
    assert_eq!(
        map.with(|mut v| {
            v.insert(b"a", b"1").unwrap();
            Err::<(), _>("ordinary")
        })
        .unwrap(),
        Err("ordinary")
    );
    let checkpoint = map.checkpoint(2).unwrap();
    #[expect(
        clippy::forget_non_drop,
        reason = "prove settlement does not rely on a view destructor"
    )]
    map.with(|view| core::mem::forget(view)).unwrap();
    assert_eq!(map.checkpoint(2).unwrap(), checkpoint);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = map.with(|mut v| {
                v.insert(b"b", b"2").unwrap();
                panic!("after write");
            });
        }))
        .is_err()
    );
    assert_eq!(map.with(|_| ()), Err(Error::Poisoned));
    assert_eq!(map.allocation_usage(), Err(Error::Poisoned));
    assert!(map.restore(b"invalid", 2).is_err());
    assert_eq!(map.checkpoint(2), Err(Error::Poisoned));
    map.restore(&checkpoint, 2).unwrap();
    assert_eq!(records(&mut map), vec![(b"a".to_vec(), b"1".to_vec())]);
}
