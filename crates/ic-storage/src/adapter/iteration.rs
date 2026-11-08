//! Owned entries and iterator views over the copied ICSS engine.
use super::{
    Memory, Storable,
    bridge::{Codec, TreeMemory},
};
use crate::{DefaultMemoryImpl, engine};
use std::{borrow::Cow, marker::PhantomData};

///
/// `MapEntry`
///
/// An owned key and encoded value from an ordered scan. Decoding failures trap.
///
pub struct MapEntry<K, V> {
    key: K,
    bytes: Vec<u8>,
    value: PhantomData<V>,
}
impl<K, V: Storable> MapEntry<K, V> {
    /// Borrow the decoded key.
    pub const fn key(&self) -> &K {
        &self.key
    }
    /// Decode the value. Malformed stored bytes trap.
    pub fn value(&self) -> V {
        <Codec<V> as engine::Storable>::from_bytes(Cow::Borrowed(&self.bytes)).0
    }
    /// Consume the entry and decode its value.
    pub fn into_pair(self) -> (K, V) {
        (
            self.key,
            <Codec<V> as engine::Storable>::from_bytes(Cow::Owned(self.bytes)).0,
        )
    }
    /// Consume only the key without decoding its value.
    pub fn into_key(self) -> K {
        self.key
    }
    /// Consume and decode only the value.
    pub fn into_value(self) -> V {
        <Codec<V> as engine::Storable>::from_bytes(Cow::Owned(self.bytes)).0
    }
}
///
/// `MapIter`
///
/// Bidirectional ICSS traversal retaining its governed map borrow.
///
pub struct MapIter<'a, K: Storable + Ord + Clone, V: Storable, M: Memory = DefaultMemoryImpl>(
    pub(super) engine::btreemap::Iter<'a, Codec<K>, Codec<V>, TreeMemory<M>>,
);
fn entry<K: Storable + Ord + Clone, V: Storable, M: Memory>(
    entry: engine::btreemap::LazyEntry<'_, Codec<K>, Codec<V>, TreeMemory<M>>,
) -> MapEntry<K, V> {
    let (key, bytes) = entry.into_raw();
    MapEntry {
        key: key.0,
        bytes,
        value: PhantomData,
    }
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> Iterator for MapIter<'_, K, V, M> {
    type Item = MapEntry<K, V>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(entry)
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth(n).map(entry)
    }
    fn last(mut self) -> Option<Self::Item> {
        // The reverse cursor already respects bounds and consumption from either end.
        self.next_back()
    }
    fn count(self) -> usize {
        self.0.count()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> DoubleEndedIterator
    for MapIter<'_, K, V, M>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(entry)
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth_back(n).map(entry)
    }
}
