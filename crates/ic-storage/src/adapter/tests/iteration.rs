//! Terminal entry traversal respects the remaining bidirectional range.
use super::*;
use crate::testing::MeteredMemory;
use std::{fmt::Debug, ops::Bound as RangeBound};

fn remaining_last<I, T>(
    mut actual: I,
    expected: &[T],
    front: usize,
    back: usize,
    decode: impl Fn(I::Item) -> T,
) where
    I: DoubleEndedIterator,
    T: Clone + Debug + PartialEq,
{
    let mut expected = expected.iter().cloned();
    if front > 0 {
        assert_eq!(actual.nth(front - 1).map(&decode), expected.nth(front - 1));
    }
    if back > 0 {
        assert_eq!(
            actual.nth_back(back - 1).map(&decode),
            expected.nth_back(back - 1)
        );
    }
    assert_eq!(Iterator::last(actual).map(decode), expected.next_back());
}

fn terminal_history<K: Storable + Ord + Clone + Debug>(key: impl Fn(u64) -> K) {
    use RangeBound::{Excluded, Included, Unbounded};
    for population in [0, 1, 97] {
        let host = runtime(VectorMemory::default());
        let mut map = StableBTreeMap::<K, u64>::create(
            host.open_memory(&crate::StableKey::parse(KEYS[0]).unwrap())
                .unwrap(),
        )
        .unwrap();
        let mut set = StableBTreeSet::<K>::create(
            host.open_memory(&crate::StableKey::parse(KEYS[1]).unwrap())
                .unwrap(),
        )
        .unwrap();
        let mut model = std::collections::BTreeMap::new();
        for n in (0..population).rev() {
            map.insert(key(n), n * 3);
            set.insert(key(n));
            model.insert(key(n), n * 3);
        }
        let pairs: Vec<_> = model.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let keys: Vec<_> = model.keys().cloned().collect();
        for (front, back) in [(0, 0), (1, 0), (0, 1), (2, 3), (100, 0), (0, 100)] {
            remaining_last(map.iter(), &pairs, front, back, MapEntry::into_pair);
            remaining_last(set.iter(), &keys, front, back, std::convert::identity);
            let reversed: Vec<_> = pairs.iter().rev().cloned().collect();
            remaining_last(
                map.iter().rev(),
                &reversed,
                front,
                back,
                MapEntry::into_pair,
            );
            for bounds in [
                (Unbounded, Included(key(90))),
                (Included(key(7)), Excluded(key(82))),
                (Excluded(key(7)), Included(key(82))),
                (Included(key(7)), Excluded(key(7))),
                (Included(key(200)), Unbounded),
            ] {
                let expected: Vec<_> = model
                    .range(bounds.clone())
                    .map(|(k, v)| (k.clone(), *v))
                    .collect();
                remaining_last(
                    map.range(bounds),
                    &expected,
                    front,
                    back,
                    MapEntry::into_pair,
                );
            }
        }
    }
}

#[test]
fn last_preserves_empty_bounded_and_partially_consumed_results() {
    terminal_history(|n| n);
    terminal_history(|n| format!("{n:06}"));
    terminal_history(|n| format!("{n:06}{}", "x".repeat(4096)));
    terminal_history(u64::to_be_bytes);
}

#[test]
fn last_reads_a_tree_path_without_walking_the_population() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let host = runtime(backing.clone());
    let mut map = StableBTreeMap::<u64, u64, _>::create(
        host.open_memory(&crate::StableKey::parse(KEYS[0]).unwrap())
            .unwrap(),
    )
    .unwrap();
    for n in 0..512 {
        map.insert(n, n * 3);
    }
    backing.reset();
    for (n, entry) in map.iter().enumerate() {
        assert_eq!(entry.into_pair(), (n as u64, n as u64 * 3));
    }
    let scan = backing.counts();
    backing.reset();
    assert_eq!(map.iter().last().unwrap().into_pair(), (511, 1533));
    let last = backing.counts();
    assert!(!scan.overflowed && !last.overflowed);
    assert_eq!((last.writes, last.grows), (0, 0));
    assert!(last.reads * 8 < scan.reads, "last={last:?}, scan={scan:?}");
    assert!(
        last.read_bytes * 8 < scan.read_bytes,
        "last={last:?}, scan={scan:?}"
    );
}

#[test]
fn skipped_entries_do_not_read_large_value_payloads() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let host = runtime(backing.clone());
    let mut map = StableBTreeMap::<u64, Vec<u8>, _>::create(
        host.open_memory(&crate::StableKey::parse(KEYS[0]).unwrap())
            .unwrap(),
    )
    .unwrap();
    for n in 0..97 {
        map.insert(n, vec![7; 4096]);
    }
    // Exhausting either direction must pay only for routing and keys, even
    // when the iterator's item owns the encoded value of yielded entries.
    for reverse in [false, true] {
        backing.reset();
        let mut keys = map.keys();
        let result = if reverse {
            keys.nth_back(97)
        } else {
            keys.nth(97)
        };
        assert!(result.is_none());
        let expected = backing.counts();
        backing.reset();
        let mut entries = map.iter();
        let result = if reverse {
            entries.nth_back(97)
        } else {
            entries.nth(97)
        };
        assert!(result.is_none());
        assert_eq!(backing.counts(), expected);
    }
}
