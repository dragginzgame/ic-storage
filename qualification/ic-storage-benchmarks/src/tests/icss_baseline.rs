//! Compare independent images and refuse external ICSS formats without writes.
use ic_stable_structures as icss;
use ic_storage::{
    Memory, RuntimeMemory, StableKey, StorageConfig,
    adapter::{StableBTreeMap, StableBTreeSet, StableCell, StableVec},
    testing::fixture_runtime,
};
use std::{borrow::Cow, collections::BTreeMap, rc::Rc};
struct Checked<M>(M);
impl<M: Memory> icss::Memory for Checked<M> {
    fn size(&self) -> u64 {
        self.0.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        self.0.grow(pages)
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.0.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        self.0.write(offset, bytes);
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Reference<T>(T);
impl<T: ic_storage::storable::Storable> icss::Storable for Reference<T> {
    const BOUND: icss::storable::Bound = match T::BOUND {
        ic_storage::storable::Bound::Unbounded => icss::storable::Bound::Unbounded,
        ic_storage::storable::Bound::Bounded {
            max_size,
            is_fixed_size,
        } => icss::storable::Bound::Bounded {
            max_size,
            is_fixed_size,
        },
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0.to_bytes_checked().unwrap()
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes_checked().unwrap()
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(T::from_bytes(bytes).unwrap())
    }
}
fn runtime() -> ic_storage::MemoryRuntime<ic_storage::VectorMemory> {
    fixture_runtime(
        ic_storage::VectorMemory::default(),
        "baseline",
        "baseline.",
        &["baseline.data.v1"],
        StorageConfig::default(),
    )
    .unwrap()
}
fn memory(
    runtime: &ic_storage::MemoryRuntime<ic_storage::VectorMemory>,
) -> RuntimeMemory<ic_storage::VectorMemory> {
    runtime
        .open_memory(&StableKey::parse("baseline.data.v1").unwrap())
        .unwrap()
}
#[test]
fn separate_map_images_match_the_same_mutation_history() {
    let owned_host = runtime();
    let reference_host = runtime();
    let mut map = StableBTreeMap::<u64, Vec<u8>>::create(memory(&owned_host)).unwrap();
    let mut stock = icss::BTreeMap::<Reference<u64>, Reference<Vec<u8>>, _>::new(Checked(memory(
        &reference_host,
    )));
    let mut model = BTreeMap::new();
    for key in (0_u64..300).rev() {
        let value = vec![
            key.to_le_bytes()[0];
            [0, 31, 256, 4096, 65536][usize::try_from(key % 5).unwrap()]
        ];
        assert_eq!(
            map.insert(key, value.clone()),
            stock
                .insert(Reference(key), Reference(value.clone()))
                .map(|v| v.0)
        );
        model.insert(key, value);
    }
    for size in [0, 65537, 31, 4096] {
        for key in (0..300).step_by(13) {
            let value = vec![0xa5; size];
            let expected = model.insert(key, value.clone());
            assert_eq!(map.insert(key, value.clone()), expected);
            assert_eq!(
                stock.insert(Reference(key), Reference(value)).map(|v| v.0),
                expected
            );
        }
        drop(map);
        map = StableBTreeMap::<u64, Vec<u8>>::load(memory(&owned_host)).unwrap();
        for (&key, value) in &model {
            assert_eq!(map.get(&key).as_ref(), Some(value));
            assert_eq!(
                stock.get(&Reference(key)).map(|v| v.0).as_ref(),
                Some(value)
            );
        }
    }
    for key in (0..300).step_by(3) {
        let expected = model.remove(&key);
        assert_eq!(map.remove(&key), expected);
        assert_eq!(stock.remove(&Reference(key)).map(|v| v.0), expected);
    }
    let expected: Vec<_> = model.into_iter().collect();
    assert_eq!(
        map.iter()
            .map(|entry| entry.into_pair())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        stock
            .iter()
            .map(|entry| {
                let (k, v) = entry.into_pair();
                (k.0, v.0)
            })
            .collect::<Vec<_>>(),
        expected
    );
}

fn image(runtime: &ic_storage::MemoryRuntime<ic_storage::VectorMemory>) -> Vec<u8> {
    let allocation = memory(runtime);
    let mut bytes = vec![0; usize::try_from(allocation.size() * 65_536).unwrap()];
    allocation.read(0, &mut bytes);
    bytes
}

fn reject_ordered_reference<K>(key: impl Fn(u32, usize) -> K)
where
    K: ic_storage::storable::Storable + Ord + Clone + std::fmt::Debug,
{
    let runtime = runtime();
    let mut stock =
        icss::BTreeMap::<Reference<K>, Reference<u64>, _>::new(Checked(memory(&runtime)));
    let mut expected = BTreeMap::new();
    for id in (0..96).rev() {
        let key = key(id, [4, 16, 17, 31, 4096, 65536][id as usize % 6]);
        stock.insert(Reference(key.clone()), Reference(u64::from(id)));
        expected.insert(key, u64::from(id));
    }
    drop(stock);
    let before = image(&runtime);
    assert!(StableBTreeMap::<K, u64>::load(memory(&runtime)).is_err());
    assert_eq!(image(&runtime), before);
    let stock = icss::BTreeMap::<Reference<K>, Reference<u64>, _>::load(Checked(memory(&runtime)));
    for (key, value) in &expected {
        assert_eq!(
            stock.get(&Reference(key.clone())).map(|v| v.0),
            Some(*value)
        );
    }
    drop(stock);
    let mut map = StableBTreeMap::<K, u64>::reset(memory(&runtime)).unwrap();
    for (key, value) in &expected {
        map.insert(key.clone(), *value);
    }
    assert_eq!(
        map.iter()
            .map(|entry| entry.into_pair())
            .collect::<Vec<_>>(),
        expected.into_iter().collect::<Vec<_>>()
    );
}

#[test]
fn reference_map_images_are_rejected_without_writes() {
    reject_ordered_reference(|id, _| u64::from(id));
    reject_ordered_reference(|id, width| format!("{id:04}{}", "x".repeat(width - 4)));
    reject_ordered_reference(|id, width| format!("{id:04}{}", "x".repeat(width - 4)).into_bytes());
}

#[derive(Clone)]
struct Bytes(Vec<u8>);
impl ic_storage::storable::Storable for Bytes {
    const BOUND: ic_storage::storable::Bound = ic_storage::storable::Bound::Bounded {
        max_size: 4096,
        is_fixed_size: false,
    };
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, ic_storage::storable::CodecError> {
        Ok(Cow::Borrowed(&self.0))
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, ic_storage::storable::CodecError> {
        Ok(Self(bytes.into_owned()))
    }
}
impl icss::Storable for Bytes {
    const BOUND: icss::storable::Bound = icss::storable::Bound::Bounded {
        max_size: 4096,
        is_fixed_size: false,
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&self.0)
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(bytes.into_owned())
    }
}
#[test]
fn variable_vector_rejects_reference_images_until_explicit_reset() {
    let runtime = runtime();
    let stock = icss::Vec::<Bytes, _>::init(Checked(memory(&runtime)));
    for index in 0..1024 {
        stock.push(&Bytes(vec![u8::try_from(index % 251).unwrap(); 256]));
    }
    drop(stock);
    assert!(StableVec::<Bytes>::load(memory(&runtime)).is_err());
    let stock = icss::Vec::<Bytes, _>::init(Checked(memory(&runtime)));
    assert_eq!(stock.len(), 1024);
    assert_eq!(stock.get(1023).unwrap().0, vec![19; 256]);
    drop(stock);
    let vector = StableVec::<Bytes>::reset(memory(&runtime)).unwrap();
    assert!(vector.is_empty());
    vector.push(&Bytes(vec![9; 4096]));
    drop(vector);
    let vector = StableVec::<Bytes>::load(memory(&runtime)).unwrap();
    assert_eq!(vector.pop().unwrap().0, vec![9; 4096]);
}
#[test]
fn fixed_vector_rejection_preserves_the_complete_reference_image() {
    let runtime = runtime();
    let stock = icss::Vec::<Reference<u64>, _>::init(Checked(memory(&runtime)));
    for value in 0..129 {
        stock.push(&Reference(value));
    }
    drop(stock);
    let allocation = memory(&runtime);
    let mut before = vec![0; usize::try_from(allocation.size() * 65_536).unwrap()];
    allocation.read(0, &mut before);
    assert!(StableVec::<u64>::load(allocation).is_err());
    let allocation = memory(&runtime);
    let mut after = vec![0; before.len()];
    allocation.read(0, &mut after);
    assert_eq!(after, before);
    let vector = StableVec::<u64>::reset(allocation).unwrap();
    vector.push(&u64::MAX);
    drop(vector);
    assert_eq!(
        StableVec::<u64>::load(memory(&runtime)).unwrap().get(0),
        Some(u64::MAX)
    );
}
#[test]
fn cell_rejects_reference_images_until_explicit_reset() {
    for length in [0, 1, 4096, 65_536] {
        let runtime = runtime();
        drop(icss::Cell::new(
            Checked(memory(&runtime)),
            vec![7_u8; length],
        ));
        let allocation = memory(&runtime);
        let mut before = vec![0; usize::try_from(allocation.size() * 65_536).unwrap()];
        allocation.read(0, &mut before);
        assert!(StableCell::<Vec<u8>>::load(allocation, usize::MAX).is_err());
        let allocation = memory(&runtime);
        let mut after = vec![0; before.len()];
        allocation.read(0, &mut after);
        assert_eq!(after, before, "rejected reference image changed");
        drop(StableCell::reset(allocation, vec![9_u8; length]).unwrap());
        let cell = StableCell::<Vec<u8>>::load(memory(&runtime), length).unwrap();
        assert_eq!(cell.get(), &vec![9_u8; length]);
    }
}
#[test]
fn set_reference_images_require_explicit_reset() {
    let runtime = runtime();
    let mut stock = icss::BTreeSet::<Reference<u64>, _>::init(Checked(memory(&runtime)));
    for key in (0..129).rev() {
        stock.insert(Reference(key));
    }
    drop(stock);
    let before = image(&runtime);
    assert!(StableBTreeSet::<u64>::load(memory(&runtime)).is_err());
    assert_eq!(image(&runtime), before);
    let mut set = StableBTreeSet::<u64>::reset(memory(&runtime)).unwrap();
    set.insert(500);
    drop(set);
    let set = StableBTreeSet::<u64>::load(memory(&runtime)).unwrap();
    assert!(!set.contains(&64));
    assert!(set.contains(&500));
    assert_eq!(set.iter().next_back().unwrap(), 500);
}
#[test]
fn governed_region_rejects_reference_image_and_preserves_neighbor() {
    let runtime = runtime();
    let lease = Rc::new(memory(&runtime));
    lease.as_ref().grow(1).unwrap();
    lease.write(0, b"host frame");
    let region = icss::RestrictedMemory::new(Checked(Rc::clone(&lease)), 1..129);
    let mut stock = icss::BTreeMap::<Reference<u64>, Reference<u64>, _>::new(region);
    assert_eq!(stock.insert(Reference(7), Reference(12)).map(|v| v.0), None);
    drop(stock);
    let region = ic_storage::adapter::BTreeMapRegion::<u64, u64>::new(Rc::clone(&lease), 1..129);
    let mut before = vec![0; usize::try_from(lease.size() * 65_536).unwrap()];
    lease.read(0, &mut before);
    assert!(region.get(&7).is_err());
    assert!(region.len().is_err());
    let mut after = vec![0; before.len()];
    lease.read(0, &mut after);
    assert_eq!(after, before);
    let mut frame = [0; 10];
    lease.read(0, &mut frame);
    assert_eq!(&frame, b"host frame");
}

#[test]
fn cell_resize_io_separates_engine_from_allocation_costs() {
    use ic_storage::testing::MeteredMemory;

    for initial in [65_536, 1_048_576] {
        let grown = (initial * 4).min(1_536 * 1024);
        let mut governed_counts = Vec::new();
        for reference_engine in [false, true] {
            let backing = MeteredMemory::<ic_storage::VectorMemory>::default();
            let runtime = fixture_runtime(
                backing.clone(),
                "baseline",
                "baseline.",
                &["baseline.data.v1"],
                StorageConfig::default(),
            )
            .unwrap();
            let memory = runtime
                .open_memory(&StableKey::parse("baseline.data.v1").unwrap())
                .unwrap();
            let mut observed = Vec::new();
            // Use the published engine over the exact same governed backing to
            // distinguish Cell serialization from allocation publication work.
            if reference_engine {
                let mut cell = icss::Cell::new(Checked(memory), vec![1; initial]);
                for length in [grown, 0, grown] {
                    let previous = cell.get().clone();
                    backing.reset();
                    assert_eq!(cell.set(vec![2; length]), previous);
                    observed.push(backing.counts());
                    assert_eq!(cell.get(), &vec![2; length]);
                }
            } else {
                let mut cell = StableCell::create(memory, vec![1; initial]).unwrap();
                for length in [grown, 0, grown] {
                    let previous = cell.get().clone();
                    backing.reset();
                    assert_eq!(cell.set(vec![2; length]), previous);
                    observed.push(backing.counts());
                    assert_eq!(cell.get(), &vec![2; length]);
                }
            }
            assert!(observed.iter().all(|counts| !counts.overflowed));
            assert_eq!(observed[0].reads, 0);
            assert_eq!(observed[0].grows, u64::from(initial > 65_536));
            for counts in &observed[1..] {
                assert_eq!((counts.reads, counts.grows), (0, 0));
            }
            governed_counts.push(observed);
        }
        for (owned, reference) in governed_counts[0].iter().zip(&governed_counts[1]) {
            // Different Cell envelopes need not have equal serialized bytes or
            // write counts. Compare the shared allocation effects separately.
            assert_eq!(
                (owned.reads, owned.grows),
                (reference.reads, reference.grows)
            );
        }

        let backing = MeteredMemory::<ic_storage::VectorMemory>::default();
        let manager =
            icss::memory_manager::MemoryManager::init_with_bucket_size(Checked(backing.clone()), 8);
        let mut cell = icss::Cell::new(
            manager.get(icss::memory_manager::MemoryId::new(0)),
            vec![1; initial],
        );
        let mut reference_counts = Vec::new();
        for length in [grown, 0, grown] {
            let previous = cell.get().clone();
            backing.reset();
            assert_eq!(cell.set(vec![2; length]), previous);
            let counts = backing.counts();
            assert!(!counts.overflowed);
            assert_eq!(counts.reads, 0);
            reference_counts.push(counts);
        }
        println!(
            "Cell {initial} -> {grown} -> 0 -> {grown}: governed={:?}; ICSS eight-page manager={reference_counts:?}",
            governed_counts[0]
        );
    }
}
