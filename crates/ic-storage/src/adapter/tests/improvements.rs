//! Baseline ordered projections and caller-codec behavior.
use super::*;
use std::{
    borrow::Cow,
    panic::{AssertUnwindSafe, catch_unwind},
};
#[test]
fn bounded_map_keys_and_mixed_end_ranges_preserve_order() {
    let runtime = runtime(VectorMemory::default());
    let mut map = StableBTreeMap::<u64, Vec<u8>>::create(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    for key in 0..100 {
        map.insert(key, vec![7; 16384]);
    }
    let mut keys = map.keys_range(10..14);
    assert_eq!(keys.next().unwrap(), 10);
    assert_eq!(keys.next_back().unwrap(), 13);
    assert_eq!(keys.collect::<Vec<_>>(), vec![11, 12]);
}

fn fixed_key_history<const N: usize>() {
    let backing = VectorMemory::default();
    let host = runtime(backing.clone());
    let binding = crate::StableKey::parse(KEYS[0]).unwrap();
    let mut map =
        StableBTreeMap::<[u8; N], ()>::create(host.open_memory(&binding).unwrap()).unwrap();
    let mut model = std::collections::BTreeSet::new();
    // Wide fixed keys with zero-sized values exercise densely occupied nodes;
    // N=0 also verifies empty slices without chunks_exact(0) or extra key IO.
    for number in (0..240_u8).rev() {
        let key = [number; N];
        assert_eq!(map.insert(key, ()).is_none(), model.insert(key));
    }
    drop(map);
    drop(host);
    let host = runtime(backing);
    let mut map = StableBTreeMap::<[u8; N], ()>::load(host.open_memory(&binding).unwrap()).unwrap();
    for number in 0..240_u8 {
        let key = [number; N];
        assert_eq!(map.get(&key), model.contains(&key).then_some(()));
        if number % 3 == 0 {
            assert_eq!(map.remove(&key).is_some(), model.remove(&key));
        }
    }
    assert_eq!(
        map.iter().map(MapEntry::into_pair).collect::<Vec<_>>(),
        model.into_iter().map(|key| (key, ())).collect::<Vec<_>>()
    );
}

#[test]
fn fixed_key_batch_boundaries_preserve_splits_removals_and_reopening() {
    fixed_key_history::<0>();
    fixed_key_history::<1>();
    fixed_key_history::<8>();
    fixed_key_history::<15>();
    fixed_key_history::<16>();
    fixed_key_history::<17>();
}
#[derive(Clone)]
struct RejectDecode(u64);
impl Storable for RejectDecode {
    const BOUND: Bound = u64::BOUND;
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
        self.0.to_bytes()
    }
    fn from_bytes(_: Cow<'_, [u8]>) -> Result<Self, CodecError> {
        Err(CodecError::Invalid("injected stored decode refusal"))
    }
}
#[test]
fn stored_value_decode_failure_traps_and_keys_remain_readable() {
    let backing = VectorMemory::default();
    let runtime = runtime(backing.clone());
    let mut map = StableBTreeMap::<u64, RejectDecode>::create(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    map.insert(7, RejectDecode(42));
    let before = backing.borrow().clone();
    assert!(catch_unwind(AssertUnwindSafe(|| map.get(&7))).is_err());
    assert_eq!(map.keys().collect::<Vec<_>>(), vec![7]);
    assert_eq!(*backing.borrow(), before);
    // A mutation's codec panic must poison the shared owner; IC executions use
    // rollback instead, independently of when the engine decodes the old value.
    assert!(catch_unwind(AssertUnwindSafe(|| map.insert(7, RejectDecode(43)))).is_err());
    assert!(
        runtime
            .open_memory(&crate::StableKey::parse("test.set.v1").unwrap())
            .is_err()
    );
}
