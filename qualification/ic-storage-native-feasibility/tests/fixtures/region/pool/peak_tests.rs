//! Host allocation evidence, not installed-Wasm or whole-heap peak evidence.

use super::{tests::Backing, *};
use ic_storage_native::map::{
    Error as MapError, Map, Slot,
    paged::{Manifest, PAGE_BYTES, PAGE_RECORDS, Reader},
};

#[test]
fn peaks_include_reserved_sealed_and_resize_candidates() {
    let mut backing = Backing::new(64, 17u64);
    let roots = [
        Root {
            start: 0,
            length: 3,
            capacity: 8,
        },
        Root {
            start: 8,
            length: 3,
            capacity: 8,
        },
    ];
    let pool = backing.claim(&roots).unwrap();
    assert_eq!(pool.peak_elements.get(), 16);
    let first = pool.restore(0).unwrap();
    assert_eq!(pool.peak_elements.get(), 16); // issuance is not a new span
    pool.release(first).unwrap_or_else(|_| panic!("release"));
    let mut second = pool.restore(1).unwrap();
    second.try_reserve_exact(13).unwrap(); // 8 sealed + 8 old + 16 candidate
    assert_eq!(second.as_slice(), &[17; 3]);
    assert_eq!(pool.peak_elements.get(), 32);
    assert_eq!(pool.reusable_elements(), 40);
    pool.peak_elements.set(24); // reset with outstanding allocations
    second.shrink_to_fit(); // 8 sealed + 16 old + 3 candidate
    assert_eq!(pool.peak_elements.get(), 27);
    assert_eq!(second.as_slice(), &[17; 3]);
    assert_eq!(pool.reusable_elements(), 53);
    let before = (pool.reusable_elements(), pool.peak_elements.get());
    assert!(second.try_reserve_exact(62).is_err());
    assert_eq!((pool.reusable_elements(), pool.peak_elements.get()), before);
    assert_eq!(second.as_slice(), &[17; 3]);
    drop(second);
    assert_eq!(pool.reusable_elements(), 56); // sealed root remains unavailable
    assert_eq!(pool.peak_elements.get(), 27);
    pool.finish().unwrap();
}

#[test]
fn fragmented_refusal_does_not_raise_peak_or_change_owners() {
    let mut backing = Backing::new(64, 0u64);
    let pool = backing.claim(&[]).unwrap();
    let mut owners: Vec<_> = (0..4u64)
        .map(|value| {
            let mut values = Vec::new_in(&pool);
            values.try_reserve_exact(16).unwrap();
            values.resize(16, value);
            Some(values)
        })
        .collect();
    drop(owners[0].take());
    drop(owners[2].take());
    pool.peak_elements.set(32);
    let mut candidate = Vec::<u64, _>::new_in(&pool);
    assert!(candidate.try_reserve_exact(24).is_err());
    assert_eq!(pool.reusable_elements(), 32);
    assert_eq!(pool.peak_elements.get(), 32);
    assert_eq!(owners[1].as_ref().unwrap().as_slice(), &[1; 16]);
    assert_eq!(owners[3].as_ref().unwrap().as_slice(), &[3; 16]);
    drop(owners[1].take());
    candidate.try_reserve_exact(32).unwrap();
    assert_eq!(pool.peak_elements.get(), 48);
    drop(candidate);
    drop(owners);
    assert_eq!(pool.reusable_elements(), 64);
    pool.finish().unwrap();
}

struct Source {
    manifest: Manifest,
    pages: Vec<[u8; PAGE_BYTES]>,
}

impl Source {
    fn new(count: usize, id: usize) -> Self {
        let mut map = Map::new(6144);
        map.with(|mut view| {
            for key in 0..count as u64 {
                view.insert(key, key + 1000 + id as u64).unwrap();
            }
        })
        .unwrap();
        let mut pages = vec![[0; PAGE_BYTES]; count.div_ceil(PAGE_RECORDS)];
        let manifest = map
            .export_pages(7, id as u64 + 1, |index, bytes| {
                pages[index] = *bytes;
                Ok::<_, ()>(())
            })
            .unwrap();
        Self { manifest, pages }
    }
}

struct Session {
    backing: Backing<Slot>,
    roots: [Root; 2],
    counts: [usize; 2],
    peak: usize,
}

impl Session {
    fn new() -> Self {
        Self {
            backing: Backing::new(1024, Slot::EMPTY),
            roots: [
                Root {
                    start: 0,
                    length: 8,
                    capacity: 8,
                },
                Root {
                    start: 8,
                    length: 8,
                    capacity: 8,
                },
            ],
            counts: [0; 2],
            peak: 16,
        }
    }

    // Each operation freshly admits both owners, releases both, and finishes
    // the loan. No pointer/allocator survives provider resizing or a reopen.
    fn with<R>(
        &mut self,
        operation: impl for<'pool, 'backing> FnOnce(
            &mut [Map<&'pool Pool<'backing, Slot>>; 2],
            &'pool Pool<'backing, Slot>,
        ) -> R,
    ) -> R {
        let pool = self.backing.claim(&self.roots).unwrap();
        let mut maps = core::array::from_fn(|id| {
            Map::admit(pool.restore(id).unwrap(), self.counts[id], 6144)
                .unwrap_or_else(|_| panic!("admission"))
        });
        let result = operation(&mut maps, &pool);
        for (id, map) in maps.into_iter().enumerate() {
            let (slots, count) = map.release().unwrap();
            self.counts[id] = count;
            self.roots[id] = pool
                .release(slots)
                .unwrap_or_else(|_| panic!("release"))
                .unwrap();
        }
        pool.finish().unwrap();
        let end: usize = self.roots.iter().map(|root| root.capacity).sum();
        assert_eq!(pool.elements - pool.reusable_elements(), end);
        assert!(pool.peak_elements.get() >= end);
        assert!(pool.peak_elements.get() <= pool.elements);
        self.peak = self.peak.max(pool.peak_elements.get());
        result
    }

    fn recover(&mut self, sources: &[Source; 2]) -> usize {
        let mut refusals = 0;
        for (id, source) in sources.iter().enumerate() {
            let mut reader = source.manifest.reader();
            while let Some(index) = reader.next_page() {
                let records = reader.read_page(&source.pages[index]).unwrap();
                while self
                    .with(|maps, _| {
                        if maps[id].capacity().unwrap() - maps[id].len().unwrap() >= records.len() {
                            return Ok(());
                        }
                        let before = maps.each_ref().map(|map| map.checkpoint(7).unwrap());
                        let result = maps[id]
                            .with(|mut view| view.reserve(records.len()))
                            .unwrap();
                        if result.is_err() {
                            assert_eq!(result, Err(MapError::Allocation));
                            assert_eq!(
                                maps.each_ref().map(|map| map.checkpoint(7).unwrap()),
                                before
                            );
                        }
                        result
                    })
                    .is_err()
                {
                    refusals += 1;
                    let old = self.backing.0.len();
                    assert!(old < 8192, "bounded recovery pool exhausted");
                    // All native owners and the old loan have ended. A host
                    // relocation here models provider growth, not IC rollback.
                    self.backing
                        .0
                        .resize(old * 2, MaybeUninit::new(Slot::EMPTY));
                }
                self.with(|maps, _| {
                    maps[id]
                        .with(|mut view| {
                            for &(key, value) in &records {
                                assert_eq!(view.insert(key, value), Ok(None));
                            }
                        })
                        .unwrap();
                });
                reader = Reader::resume(&source.manifest, &reader.checkpoint()).unwrap();
            }
        }
        self.with(|maps, _| {
            for (id, map) in maps.iter_mut().enumerate() {
                assert_eq!(map.len(), Ok(sources[id].manifest.len()));
                map.with(|view| {
                    for key in 0..sources[id].manifest.len() as u64 {
                        assert_eq!(view.get(key), Some(key + 1000 + id as u64));
                    }
                })
                .unwrap();
            }
        });
        refusals
    }

    fn reset(&mut self) {
        for id in 0..2 {
            self.with(|maps, pool| {
                maps[id] = Map::new_in(6144, pool);
                maps[id].with(|mut view| view.reserve(6)).unwrap().unwrap();
            });
        }
    }
}

#[test]
fn paged_recovery_measures_rehash_overlap_across_pool_growth_and_reopens() {
    assert_eq!(size_of::<Slot>(), 24);
    for (sizes, expected_peak, expected_end, expected_pool, expected_refusals) in [
        ([0, 0], 384, 384, 24_576, 0),
        ([192, 192], 15_360, 12_288, 24_576, 0),
        ([3072, 1536], 172_032, 147_456, 196_608, 3),
    ] {
        let sources = core::array::from_fn(|id| Source::new(sizes[id], id));
        let mut session = Session::new();
        let refusals = session.recover(&sources);
        let end: usize = session.roots.iter().map(|root| root.capacity).sum();
        assert_eq!(
            (
                session.peak * 24,
                end * 24,
                session.backing.0.len() * 24,
                refusals
            ),
            (
                expected_peak,
                expected_end,
                expected_pool,
                expected_refusals
            ),
        );
        println!(
            "native_pool_host records={}+{} peak_bytes={} end_bytes={} reserved_bytes={} growth_refusals={refusals}",
            sizes[0],
            sizes[1],
            session.peak * 24,
            end * 24,
            session.backing.0.len() * 24
        );
        session.reset();
        assert_eq!(session.counts, [0, 0]);
        assert_eq!(
            session
                .roots
                .iter()
                .map(|root| root.capacity)
                .sum::<usize>(),
            16
        );
        session.peak = 16;
        assert_eq!(session.recover(&sources), 0);
        assert_eq!(session.peak * 24, expected_peak);
        assert_eq!(
            session
                .roots
                .iter()
                .map(|root| root.capacity)
                .sum::<usize>()
                * 24,
            expected_end
        );
        assert_eq!(session.backing.0.len() * 24, expected_pool);
        println!(
            "native_pool_host_regrow records={}+{} peak_bytes={} end_bytes={} reserved_bytes={}",
            sizes[0],
            sizes[1],
            session.peak * 24,
            end * 24,
            session.backing.0.len() * 24
        );
    }
}
