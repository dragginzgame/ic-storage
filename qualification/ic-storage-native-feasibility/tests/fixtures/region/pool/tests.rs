use super::*;
use ic_storage::map::{Error as MapError, ImportError, Map, Slot};
use std::{io::Read, mem};

pub(super) struct Backing<T: Element>(pub(super) Vec<MaybeUninit<T>>);

impl<T: Element> Backing<T> {
    pub(super) fn new(elements: usize, value: T) -> Self {
        Self(vec![MaybeUninit::new(value); elements])
    }

    pub(super) fn claim(&mut self, roots: &[Root]) -> Result<Pool<'_, T>, Error> {
        // SAFETY: this owner loans its real stable allocation exclusively for
        // the returned borrow. All slots are valid initialized T; tests only
        // reissue backing after successful finish and the previous scope ends.
        #[expect(unsafe_code, reason = "host owner supplies a real exclusive pool loan")]
        unsafe {
            Pool::claim(
                NonNull::new(self.0.as_mut_ptr().cast()).unwrap(),
                self.0.len(),
                roots,
            )
        }
    }
}

#[test]
fn multiple_maps_grow_reopen_and_portably_recover_in_one_pool() {
    exercise_maps([10_000, 4_000, 1_700], 65_536);
}

#[test]
#[ignore = "million-record provider pool; run make native-map-pool-firehose"]
fn million_records_share_a_pool_with_other_roots() {
    let start = std::time::Instant::now();
    exercise_maps([1_000_000, 16_384, 8_192], 4_194_304);
    println!(
        "verified=true roots=3 records=1024576 backing_bytes=100663296 seconds={:.6}",
        start.elapsed().as_secs_f64()
    );
}

fn exercise_maps(sizes: [usize; 3], elements: usize) {
    let limit = sizes.into_iter().max().unwrap() + 1;
    let mut backing = Backing::new(elements, Slot::EMPTY);
    let mut roots = Vec::new();
    let mut checkpoints = Vec::new();
    {
        let pool = backing.claim(&[]).unwrap();
        let mut maps = Vec::new();
        for (id, size) in sizes.into_iter().enumerate() {
            let mut map = Map::new_in(limit, &pool);
            map.with(|mut view| {
                for key in 0..size as u64 {
                    view.insert(key, key ^ (id as u64 + 100)).unwrap();
                }
            })
            .unwrap();
            maps.push(map);
        }
        let live_slots: usize = maps
            .iter()
            .map(|map| map.allocated_bytes().unwrap() / size_of::<Slot>())
            .sum();
        assert_eq!(pool.reusable_elements(), elements - live_slots);
        for map in maps {
            checkpoints.push(map.checkpoint(7).unwrap());
            let (slots, _) = map.release().unwrap();
            roots.push(
                pool.release(slots)
                    .unwrap_or_else(|_| panic!("release"))
                    .unwrap(),
            );
        }
        pool.finish().unwrap();
    }
    {
        let pool = backing.claim(&roots).unwrap();
        // Reopen out of order: an unopened reservation remains unavailable.
        for id in (0..sizes.len()).rev() {
            let mut map = Map::admit(pool.restore(id).unwrap(), sizes[id], limit)
                .unwrap_or_else(|_| panic!("admission"));
            assert_eq!(map.checkpoint(7).unwrap(), checkpoints[id]);
            map.with(|mut view| {
                for key in 0..sizes[id] as u64 {
                    assert_eq!(view.get(key), Some(key ^ (id as u64 + 100)));
                    if key % 3 == 0 {
                        assert!(view.remove(key).is_some());
                        view.insert(key + sizes[id] as u64, u64::MAX - key).unwrap();
                    }
                }
            })
            .unwrap();
            checkpoints[id] = map.checkpoint(8).unwrap();
            let (slots, _) = map.release().unwrap();
            assert_eq!(
                pool.release(slots).unwrap_or_else(|_| panic!("release")),
                Some(roots[id])
            );
        }
        pool.finish().unwrap();
    }
    drop(backing); // no source heap or native descriptor is used for recovery
    let mut fresh = Backing::new(elements, Slot::EMPTY);
    let pool = fresh.claim(&[]).unwrap();
    let mut restored = Vec::new();
    for bytes in &checkpoints {
        let mut input = bytes.as_slice();
        restored.push(
            Map::from_reader_in(bytes.len(), 8, limit, &pool, |chunk| {
                input.read_exact(chunk)
            })
            .unwrap(),
        );
    }
    for (id, mut map) in restored.into_iter().enumerate() {
        assert_eq!(map.len(), Ok(sizes[id]));
        map.with(|mut view| {
            for key in 0..sizes[id] as u64 {
                if key % 3 == 0 {
                    assert_eq!(view.get(key), None);
                    assert_eq!(view.get(key + sizes[id] as u64), Some(u64::MAX - key));
                } else {
                    assert_eq!(view.get(key), Some(key ^ (id as u64 + 100)));
                }
            }
            view.insert(u64::MAX, id as u64).unwrap();
        })
        .unwrap();
    }
    assert_eq!(pool.reusable_elements(), elements);
    pool.finish().unwrap();
}

fn vector<'pool, 'backing>(
    pool: &'pool Pool<'backing, u64>,
    capacity: usize,
    value: u64,
) -> Vec<u64, &'pool Pool<'backing, u64>> {
    let mut values = Vec::new_in(pool);
    values.try_reserve_exact(capacity).unwrap();
    values.resize(capacity, value);
    values
}

#[test]
fn fragmentation_refuses_then_adjacent_free_runs_are_reused() {
    let mut backing = Backing::new(64, 0u64);
    let pool = backing.claim(&[]).unwrap();
    let first = vector(&pool, 16, 1);
    let second = vector(&pool, 16, 2);
    let third = vector(&pool, 16, 3);
    let fourth = vector(&pool, 16, 4);
    drop(first);
    drop(third);
    assert_eq!(pool.reusable_elements(), 32);
    let mut candidate = Vec::<u64, _>::new_in(&pool);
    assert!(candidate.try_reserve_exact(24).is_err());
    assert_eq!(pool.reusable_elements(), 32);
    assert_eq!(second.as_slice(), &[2; 16]);
    assert_eq!(fourth.as_slice(), &[4; 16]);
    drop(second);
    candidate.try_reserve_exact(32).unwrap();
    candidate.resize(32, 9);
    assert_eq!(fourth.as_slice(), &[4; 16]);
    drop(candidate);
    drop(fourth);
    assert_eq!(pool.reusable_elements(), 64);
    pool.finish().unwrap();
}

#[test]
fn full_map_growth_and_failed_recovery_preserve_other_roots() {
    let mut backing = Backing::new(40, Slot::EMPTY);
    let pool = backing.claim(&[]).unwrap();
    let mut first = Map::new_in(64, &pool);
    let mut second = Map::new_in(64, &pool);
    for map in [&mut first, &mut second] {
        map.with(|mut view| {
            view.reserve(12).unwrap();
            for key in 0..12 {
                view.insert(key, key + 17).unwrap();
            }
        })
        .unwrap();
    }
    let saved = first.checkpoint(7).unwrap();
    assert_eq!(
        first.with(|mut view| view.insert(99, 100)),
        Ok(Err(MapError::Allocation))
    );
    assert_eq!(first.checkpoint(7).unwrap(), saved);
    assert_eq!(second.checkpoint(7).unwrap(), saved);
    drop(second);
    let available = pool.reusable_elements();
    let mut bad = saved.clone();
    *bad.last_mut().unwrap() ^= 1;
    let mut input = bad.as_slice();
    assert!(matches!(
        Map::from_reader_in(bad.len(), 7, 64, &pool, |chunk| input.read_exact(chunk)),
        Err(ImportError::Map(MapError::Integrity))
    ));
    assert_eq!(pool.reusable_elements(), available);
    assert_eq!(first.checkpoint(7).unwrap(), saved);
    first
        .with(|mut view| {
            view.remove(0);
            view.insert(99, 100).unwrap();
        })
        .unwrap();
    drop(first);
    assert_eq!(pool.reusable_elements(), 40);
    pool.finish().unwrap();
}

#[test]
fn catalog_refuses_overlaps_overflow_and_invalid_prefixes() {
    let mut backing = Backing::new(32, 17u64);
    let good = Root {
        start: 8,
        length: 3,
        capacity: 8,
    };
    for root in [
        Root {
            start: usize::MAX,
            ..good
        },
        Root { start: 30, ..good },
        Root { length: 9, ..good },
        Root {
            capacity: 0,
            length: 0,
            ..good
        },
    ] {
        assert!(matches!(backing.claim(&[root]), Err(Error::Root)));
    }
    assert!(matches!(backing.claim(&[good, good]), Err(Error::Root)));
    assert!(matches!(backing.claim(&[good; RUNS + 1]), Err(Error::Root)));
    let pool = backing.claim(&[good]).unwrap();
    assert_eq!(pool.finish(), Err(Error::State));
    assert!(pool.restore(1).is_err());
    let mut values = pool.restore(0).unwrap();
    assert_eq!(values.as_slice(), &[17; 3]);
    assert!(pool.restore(0).is_err());
    // Default allocator growth and shrink also preserve the initialized prefix.
    values.try_reserve_exact(10).unwrap();
    assert_eq!(values.as_slice(), &[17; 3]);
    values.shrink_to_fit();
    assert_eq!(values.as_slice(), &[17; 3]);
    let root = pool
        .release(values)
        .unwrap_or_else(|_| panic!("release"))
        .unwrap();
    assert_eq!(root.length, 3);
    assert!(pool.restore(0).is_err());
    pool.finish().unwrap();
    assert_eq!(pool.finish(), Err(Error::State));
    assert!(pool.allocate(Layout::array::<u64>(1).unwrap()).is_err());
}

#[test]
fn run_budget_and_forgotten_owners_never_authorize_reuse() {
    let mut backing = Backing::new(64, 0u64);
    let pool = backing.claim(&[]).unwrap();
    let mut owners: Vec<_> = (0..RUNS).map(|_| vector(&pool, 1, 42)).collect();
    assert_eq!(pool.reusable_elements(), 32);
    assert!(pool.allocate(Layout::array::<u64>(1).unwrap()).is_err());
    drop(owners.pop());
    let replacement = vector(&pool, 1, 99);
    assert!(owners.iter().all(|values| values.as_slice() == [42]));
    drop(owners);
    assert_eq!(pool.finish(), Err(Error::State));
    mem::forget(replacement);
    assert_eq!(pool.finish(), Err(Error::State));
    assert_eq!(pool.reusable_elements(), 63);
    // This loan is deliberately not settled or reissued after the lost owner.
}

#[test]
fn release_requires_the_issuing_controller_and_sealed_runs_stay_reserved() {
    let mut a = Backing::new(64, 0u64);
    let mut b = Backing::new(64, 0u64);
    let root = {
        let first = a.claim(&[]).unwrap();
        let other = b.claim(&[]).unwrap();
        let values = vector(&first, 16, 123);
        let (error, values) = other.release(values).unwrap_err();
        assert_eq!(error, Error::State);
        assert_eq!(values.as_slice(), &[123; 16]);
        let root = first
            .release(values)
            .unwrap_or_else(|_| panic!("release"))
            .unwrap();
        let rest = vector(&first, 48, 456);
        assert_eq!(first.reusable_elements(), 0);
        assert_eq!(first.finish(), Err(Error::State));
        drop(rest);
        assert_eq!(first.reusable_elements(), 48);
        first.finish().unwrap();
        other.finish().unwrap();
        root
    };
    let pool = a.claim(&[root]).unwrap();
    let values = pool.restore(0).unwrap();
    assert_eq!(values.as_slice(), &[123; 16]);
    drop(values);
    assert_eq!(pool.reusable_elements(), 64);
    pool.finish().unwrap();
}
