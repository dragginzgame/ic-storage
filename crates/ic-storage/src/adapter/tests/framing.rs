//! Framing rejection must precede allocator access and must never initialize data.
use super::*;
use crate::testing::MeteredMemory;

#[test]
fn map_and_region_admit_framing_once_without_writes() {
    for corrupt in [None, Some((0, 0)), Some((3, 9)), Some((8, 0))] {
        let backing = MeteredMemory::<VectorMemory>::default();
        let runtime = runtime(backing.clone());
        let key = crate::StableKey::parse(KEYS[0]).unwrap();
        let map =
            StableBTreeMap::<u64, u64, _>::create(runtime.open_memory(&key).unwrap()).unwrap();
        drop(map);
        let memory = runtime.open_memory(&key).unwrap();
        if let Some((at, byte)) = corrupt {
            memory.write(at, &[byte]);
        }
        backing.reset();
        let result = StableBTreeMap::<u64, u64, _>::load(memory);
        if corrupt.is_some() {
            assert!(matches!(result, Err(CollectionError::Format)));
        } else {
            assert!(result.as_ref().unwrap().is_empty());
        }
        assert_eq!(backing.counts().reads, 1);
        assert!(backing.counts().read_bytes <= 1024);
        assert_eq!((backing.counts().writes, backing.counts().grows), (0, 0));
        drop(result);
        let memory = Rc::new(runtime.open_memory(&key).unwrap());
        let region = BTreeMapRegion::<u64, u64, _>::new(memory, 0..16);
        backing.reset();
        let result = region.len();
        if corrupt.is_some() {
            assert!(matches!(result, Err(CollectionError::Format)));
        } else {
            assert_eq!(result.unwrap(), 0);
        }
        assert_eq!(backing.counts().reads, 1);
        assert!(backing.counts().read_bytes <= 1024);
        assert_eq!((backing.counts().writes, backing.counts().grows), (0, 0));
    }
}

#[test]
fn empty_memory_is_rejected_without_reading_or_initializing() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let key = crate::StableKey::parse(KEYS[0]).unwrap();
    let memory = runtime.open_memory(&key).unwrap();
    backing.reset();
    assert!(matches!(
        StableBTreeMap::<u64, u64, _>::load(memory),
        Err(CollectionError::Format)
    ));
    assert_eq!(backing.counts(), crate::testing::MemoryIoCounts::default());
    let memory = Rc::new(runtime.open_memory(&key).unwrap());
    let region = BTreeMapRegion::<u64, u64, _>::new(memory, 0..16);
    backing.reset();
    assert!(matches!(region.len(), Err(CollectionError::Format)));
    assert_eq!(backing.counts(), crate::testing::MemoryIoCounts::default());
}
