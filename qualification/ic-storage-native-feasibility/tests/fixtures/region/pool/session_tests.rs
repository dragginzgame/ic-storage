//! Host-only service lifetime: owners outlive calls, never the provider loan.
use super::{tests::Backing, *};
use ic_storage::map::{Error as MapError, Map, Slot, View};
use std::panic::{AssertUnwindSafe, catch_unwind};

// A private qualification caller, not a new persisted header or public runtime.
// The embedding service owns the pool separately: no self-referential owner,
// leaked allocation, forged 'static lifetime or cached validation flag.
struct Session<A: Allocator + Clone> {
    maps: [Map<A>; 2],
    healthy: bool,
}

impl<A: Allocator + Clone> Session<A> {
    fn new(maps: [Map<A>; 2]) -> Self {
        Self {
            maps,
            healthy: true,
        }
    }

    fn call<R>(
        &mut self,
        operation: impl for<'scope> FnOnce(View<'scope, A>, View<'scope, A>) -> R,
    ) -> Result<R, MapError> {
        if !self.healthy {
            return Err(MapError::Poisoned);
        }
        self.healthy = false;
        let [first, second] = &mut self.maps;
        let result = first.with(|first| second.with(|second| operation(first, second)))??;
        self.healthy = true;
        Ok(result)
    }

    // Refusal preserves both owners. A host catching an unwind cannot publish
    // a healthy root beside a partially mutated root or silently reset either.
    fn close(self) -> Result<[Map<A>; 2], Self> {
        if !self.healthy || self.maps.iter().any(|map| map.len().is_err()) {
            return Err(self);
        }
        Ok(self.maps)
    }
}

fn close(pool: &Pool<'_, Slot>, session: Session<&Pool<'_, Slot>>) -> [(Root, usize); 2] {
    let maps = session
        .close()
        .unwrap_or_else(|_| panic!("healthy session"));
    let roots = maps.map(|map| {
        let (slots, len) = map.release().unwrap();
        (
            pool.release(slots)
                .unwrap_or_else(|_| panic!("same pool"))
                .unwrap(),
            len,
        )
    });
    // No descriptor is returned for publication before complete settlement.
    pool.finish().unwrap();
    roots
}

#[test]
fn live_session_reuses_two_owners_then_closes_before_pool_relocation() {
    let mut backing = Backing::new(8192, Slot::EMPTY);
    let roots = {
        let pool = backing.claim(&[]).unwrap();
        let mut session = Session::new([Map::new_in(1536, &pool), Map::new_in(1536, &pool)]);
        // Each invocation returns fully; the owner and allocator remain alive
        // at the service boundary. Admission never appears in this dispatcher.
        for key in 0..1536 {
            session
                .call(|mut first, mut second| {
                    assert_eq!(first.insert(key, key + 1000), Ok(None));
                    assert_eq!(second.insert(key, key + 2000), Ok(None));
                })
                .unwrap();
        }
        let occupied = pool.reusable_elements();
        for key in 0..1536 {
            assert_eq!(pool.finish(), Err(Error::State));
            assert!(matches!(pool.restore(0), Err(Error::State)));
            session
                .call(|mut first, mut second| {
                    assert_eq!(first.get(key), Some(key + 1000));
                    assert_eq!(second.remove(key), Some(key + 2000));
                    assert_eq!(first.insert(key, key + 3000), Ok(Some(key + 1000)));
                    assert_eq!(second.insert(key, key + 4000), Ok(None));
                })
                .unwrap();
        }
        assert_eq!(pool.reusable_elements(), occupied);
        close(&pool, session)
    };

    // The old loan, Vec headers, allocator references and session have ended.
    // Move initialized slots to disjoint backing while both allocations exist.
    let mut relocated = Backing::new(16384, Slot::EMPTY);
    assert_ne!(backing.0.as_ptr(), relocated.0.as_ptr());
    relocated.0[..backing.0.len()].copy_from_slice(&backing.0);
    drop(backing);
    let pool = relocated.claim(&roots.map(|(root, _)| root)).unwrap();
    let maps = core::array::from_fn(|index| {
        Map::admit(pool.restore(index).unwrap(), roots[index].1, 1536)
            .unwrap_or_else(|_| panic!("fresh admission after relocation"))
    });
    let mut session = Session::new(maps);
    for key in 0..1536 {
        session
            .call(|first, second| {
                assert_eq!(first.get(key), Some(key + 3000));
                assert_eq!(second.get(key), Some(key + 4000));
            })
            .unwrap();
    }
    assert_eq!(close(&pool, session), roots);
}

#[test]
fn expected_call_refusal_keeps_owners_and_prior_writes_available() {
    let mut backing = Backing::new(40, Slot::EMPTY);
    let pool = backing.claim(&[]).unwrap();
    let mut session = Session::new([Map::new_in(64, &pool), Map::new_in(64, &pool)]);
    session
        .call(|mut first, mut second| {
            first.reserve(12).unwrap();
            second.reserve(12).unwrap();
            for key in 0..12 {
                first.insert(key, key).unwrap();
                second.insert(key, key + 100).unwrap();
            }
        })
        .unwrap();
    assert_eq!(
        session.call(|mut first, mut second| {
            assert_eq!(second.insert(0, 999), Ok(Some(100)));
            first.insert(12, 12)
        }),
        Ok(Err(MapError::Allocation))
    );
    session
        .call(|first, second| {
            assert_eq!(first.get(12), None);
            assert_eq!(second.get(0), Some(999));
            for key in 1..12 {
                assert_eq!(first.get(key), Some(key));
                assert_eq!(second.get(key), Some(key + 100));
            }
        })
        .unwrap();
    assert_eq!(close(&pool, session).map(|(_, len)| len), [12, 12]);
}

#[test]
fn caught_unwind_quarantines_the_whole_session_and_refuses_publication() {
    let mut backing = Backing::new(64, Slot::EMPTY);
    let pool = backing.claim(&[]).unwrap();
    let mut session = Session::new([Map::new_in(12, &pool), Map::new_in(12, &pool)]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = session.call(|mut first, mut second| {
                first.insert(1, 10).unwrap();
                second.insert(2, 20).unwrap();
                panic!("after both writes");
            });
        }))
        .is_err()
    );
    let occupied = pool.reusable_elements();
    assert!(occupied < 64);
    assert_eq!(
        session.call(|_, _| panic!("must not enter")),
        Err(MapError::Poisoned)
    );
    let session = session
        .close()
        .err()
        .expect("must preserve unavailable owners");
    assert_eq!(pool.reusable_elements(), occupied);
    assert_eq!(pool.finish(), Err(Error::State));
    // Explicitly discard this failed host generation; this is not IC rollback,
    // publication, automatic reset or permission to recover using stale roots.
    drop(session);
    assert_eq!(pool.reusable_elements(), 64);
    pool.finish().unwrap();
}
