//! Cross-read actual collection images with the published ICSS reference.
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
fn map_images_cross_read_after_splits_overflows_and_removals() {
    let runtime = runtime();
    let mut map = StableBTreeMap::<u64, Vec<u8>>::create(memory(&runtime)).unwrap();
    let mut model = BTreeMap::new();
    for key in (0u64..300).rev() {
        let bytes = vec![
            key.to_le_bytes()[0];
            [0, 31, 256, 4096, 65536][usize::try_from(key % 5).unwrap()]
        ];
        map.insert(key, bytes.clone());
        model.insert(key, bytes);
    }
    // Replace values throughout the populated tree, including internal entries
    // and records spanning overflow pages, before admitting the same image in ICSS.
    for (&key, value) in &mut model {
        let replacement = vec![key.to_le_bytes()[0].wrapping_add(1); value.len()];
        assert_eq!(map.insert(key, replacement.clone()).as_ref(), Some(&*value));
        *value = replacement;
    }
    // Shrink, regrow and reuse overflow chains before cross-reading every
    // intermediate image with ICSS's independent chain walker.
    for size in [0, 65536, 31, 4096] {
        for key in (0..300).step_by(13) {
            let replacement = vec![0xa5; size];
            assert_eq!(
                map.insert(key, replacement.clone()),
                model.insert(key, replacement)
            );
        }
        drop(map);
        let stock = icss::BTreeMap::<Reference<u64>, Reference<Vec<u8>>, _>::load(Checked(memory(
            &runtime,
        )));
        for (&key, value) in &model {
            assert_eq!(
                stock.get(&Reference(key)).map(|v| v.0).as_ref(),
                Some(value)
            );
        }
        drop(stock);
        map = StableBTreeMap::<u64, Vec<u8>>::load(memory(&runtime)).unwrap();
    }
    drop(map);
    let mut stock =
        icss::BTreeMap::<Reference<u64>, Reference<Vec<u8>>, _>::load(Checked(memory(&runtime)));
    for (&k, v) in &model {
        assert_eq!(stock.get(&Reference(k)).map(|v| v.0).as_ref(), Some(v));
    }
    for key in (0..300).step_by(3) {
        assert_eq!(
            stock.remove(&Reference(key)).map(|v| v.0),
            model.remove(&key)
        );
    }
    stock.insert(Reference(500), Reference(vec![9; 65537]));
    model.insert(500, vec![9; 65537]);
    drop(stock);
    let map = StableBTreeMap::<u64, Vec<u8>>::load(memory(&runtime)).unwrap();
    assert_eq!(
        map.iter().map(|x| x.into_pair()).collect::<Vec<_>>(),
        model.into_iter().collect::<Vec<_>>()
    );
}
fn owned_key_image_history<K>(key: impl Fn(u32, usize) -> K)
where
    K: ic_storage::storable::Storable + Ord + Clone + std::fmt::Debug,
{
    let runtime = runtime();
    let mut stock =
        icss::BTreeMap::<Reference<K>, Reference<u64>, _>::new(Checked(memory(&runtime)));
    let mut model = BTreeMap::new();
    // Both sides of eager loading, plus keys spanning multiple overflow pages.
    for id in (0..96).rev() {
        let width = [4, 16, 17, 31, 4096, 65536][id as usize % 6];
        let key = key(id, width);
        stock.insert(Reference(key.clone()), Reference(u64::from(id)));
        model.insert(key, u64::from(id));
    }
    drop(stock);
    let mut map = StableBTreeMap::<K, u64>::load(memory(&runtime)).unwrap();
    for (key, value) in &model {
        assert_eq!(map.get(key), Some(*value));
    }
    let expected: Vec<_> = model
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    {
        let mut actual = map.iter();
        let mut expected = expected.iter();
        let mut forward = true;
        loop {
            let (a, b) = if forward {
                (actual.next(), expected.next())
            } else {
                (actual.next_back(), expected.next_back())
            };
            assert_eq!(a.map(|entry| entry.into_pair()), b.cloned());
            if b.is_none() {
                break;
            }
            forward = !forward;
        }
    }
    for (index, (key, _)) in expected.iter().enumerate() {
        if index % 3 == 0 {
            assert_eq!(map.remove(key), model.remove(key));
        } else {
            assert_eq!(
                map.insert(key.clone(), 1000),
                model.insert(key.clone(), 1000)
            );
        }
    }
    drop(map);
    let stock = icss::BTreeMap::<Reference<K>, Reference<u64>, _>::load(Checked(memory(&runtime)));
    assert_eq!(
        stock
            .iter()
            .map(|entry| {
                let (key, value) = entry.into_pair();
                (key.0, value.0)
            })
            .collect::<Vec<_>>(),
        model.into_iter().collect::<Vec<_>>()
    );
}

#[test]
fn string_and_byte_keys_cross_read_overflows_and_mixed_end_iteration() {
    let text = |id: u32, width: usize| format!("{id:04}{}", "x".repeat(width - 4));
    owned_key_image_history(text);
    owned_key_image_history(|id, width| text(id, width).into_bytes());
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
fn variable_vector_images_cross_read_and_update() {
    let runtime = runtime();
    let vector = StableVec::<Bytes>::create(memory(&runtime)).unwrap();
    for index in 0..1024 {
        vector.push(&Bytes(vec![
            u8::try_from(index % 251).unwrap();
            if index % 2 == 0 { 256 } else { 2048 }
        ]));
    }
    drop(vector);
    let stock = icss::Vec::<Bytes, _>::init(Checked(memory(&runtime)));
    assert_eq!(stock.len(), 1024);
    assert_eq!(stock.get(1023).unwrap().0, vec![19; 2048]);
    stock.set(0, &Bytes(vec![9; 4096]));
    stock.push(&Bytes(vec![10; 32]));
    drop(stock);
    let vector = StableVec::<Bytes>::load(memory(&runtime)).unwrap();
    assert_eq!(vector.len(), 1025);
    assert_eq!(vector.get(0).unwrap().0, vec![9; 4096]);
    assert_eq!(vector.pop().unwrap().0, vec![10; 32]);
}
#[test]
fn fixed_vector_images_cross_read_and_update() {
    let runtime = runtime();
    let vector = StableVec::<u64>::create(memory(&runtime)).unwrap();
    for key in 0..129 {
        vector.push(&key);
    }
    drop(vector);
    let stock = icss::Vec::<Reference<u64>, _>::init(Checked(memory(&runtime)));
    assert_eq!(stock.get(128).map(|v| v.0), Some(128));
    stock.set(64, &Reference(u64::MAX));
    drop(stock);
    let vector = StableVec::<u64>::load(memory(&runtime)).unwrap();
    assert_eq!(vector.get(64), Some(u64::MAX));
}
#[test]
fn cell_images_cross_read_and_update() {
    let runtime = runtime();
    drop(StableCell::create(memory(&runtime), vec![7; 4096]).unwrap());
    let mut stock = icss::Cell::<Vec<u8>, _>::init(Checked(memory(&runtime)), Vec::new());
    assert_eq!(stock.get(), &vec![7; 4096]);
    stock.set(vec![8; 65536]);
    drop(stock);
    let cell = StableCell::<Vec<u8>>::load(memory(&runtime), 65536).unwrap();
    assert_eq!(cell.get(), &vec![8; 65536]);
}
#[test]
fn set_images_cross_read_and_update() {
    let runtime = runtime();
    let mut set = StableBTreeSet::<u64>::create(memory(&runtime)).unwrap();
    for key in (0..129).rev() {
        set.insert(key);
    }
    drop(set);
    let mut stock = icss::BTreeSet::<Reference<u64>, _>::init(Checked(memory(&runtime)));
    assert!(stock.remove(&Reference(64)));
    stock.insert(Reference(500));
    drop(stock);
    let set = StableBTreeSet::<u64>::load(memory(&runtime)).unwrap();
    assert!(!set.contains(&64));
    assert!(set.contains(&500));
    assert_eq!(set.iter().next_back().unwrap(), 500);
}
#[test]
fn governed_region_images_cross_read_and_update() {
    let runtime = runtime();
    let lease = Rc::new(memory(&runtime));
    lease.as_ref().grow(1).unwrap();
    lease.write(0, b"host frame");
    let mut region =
        ic_storage::adapter::BTreeMapRegion::<u64, u64>::new(Rc::clone(&lease), 1..129);
    region.initialize().unwrap();
    region.insert(7, 11).unwrap();
    drop(region);
    let region = icss::RestrictedMemory::new(Checked(Rc::clone(&lease)), 1..129);
    let mut stock = icss::BTreeMap::<Reference<u64>, Reference<u64>, _>::load(region);
    assert_eq!(
        stock.insert(Reference(7), Reference(12)).map(|v| v.0),
        Some(11)
    );
    drop(stock);
    let region = ic_storage::adapter::BTreeMapRegion::<u64, u64>::new(Rc::clone(&lease), 1..129);
    assert_eq!(region.get(&7).unwrap(), Some(12));
    let mut frame = [0; 10];
    lease.read(0, &mut frame);
    assert_eq!(&frame, b"host frame");
}
