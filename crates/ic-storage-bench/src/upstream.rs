//! Shared upstream operations; codec bounds remain distinct for each fixture.
use super::{Memory, protocol::SCAN_LIMIT};
use ic_memory::RuntimeMemory;
use ic_storage::adapter::{StableBTreeMap, Storable, storable::Bound};
use std::borrow::Cow;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Bytes<const MAX: u32, const FIXED: bool>(Vec<u8>);
impl<const MAX: u32, const FIXED: bool> Storable for Bytes<MAX, FIXED> {
    const BOUND: Bound = if MAX == 0 {
        Bound::Unbounded
    } else {
        Bound::Bounded {
            max_size: MAX,
            is_fixed_size: FIXED,
        }
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

pub(super) struct Map<const K: u32, const KF: bool, const V: u32, const VF: bool> {
    inner: StableBTreeMap<Bytes<K, KF>, Bytes<V, VF>, Memory>,
}
impl<const K: u32, const KF: bool, const V: u32, const VF: bool> Map<K, KF, V, VF> {
    pub fn new(memory: RuntimeMemory<Memory>) -> Self {
        Self {
            inner: StableBTreeMap::new(memory),
        }
    }
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        self.inner.insert(Bytes(key), Bytes(value)).map(|v| v.0)
    }
    pub fn get(&self, key: Vec<u8>) -> Option<Vec<u8>> {
        self.inner.get(&Bytes(key)).map(|v| v.0)
    }
    pub fn remove(&mut self, key: Vec<u8>) -> Option<Vec<u8>> {
        self.inner.remove(&Bytes(key)).map(|v| v.0)
    }
    pub fn scan(&self) -> Vec<u8> {
        self.inner
            .iter()
            .take(SCAN_LIMIT)
            .flat_map(|e| e.key().0.clone())
            .collect()
    }
    pub fn scan_values(&self, output: &mut Vec<u8>) {
        self.inner
            .iter()
            .take(SCAN_LIMIT)
            .for_each(|e| output.extend_from_slice(&e.value().0));
    }
    pub fn range_scan(
        &self,
        lower: Vec<u8>,
        upper: Vec<u8>,
        reverse: bool,
        values: bool,
        output: &mut Vec<u8>,
    ) {
        let mut iter = self.inner.range(Bytes(lower)..Bytes(upper));
        for _ in 0..SCAN_LIMIT {
            let Some(entry) = (if reverse {
                iter.next_back()
            } else {
                iter.next()
            }) else {
                break;
            };
            if values {
                output.extend_from_slice(&entry.value().0);
            } else {
                output.extend_from_slice(&entry.key().0);
            }
        }
    }
}
