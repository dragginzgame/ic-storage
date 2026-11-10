//! Owned entries and iterator views over the independent ordered engine.
use super::{Memory, Storable, bridge::TreeMemory};
use crate::{DefaultMemoryImpl, ordered_map};
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
        ordered_map::decode(Cow::Borrowed(&self.bytes))
    }
    /// Consume the entry and decode its value.
    pub fn into_pair(self) -> (K, V) {
        (self.key, ordered_map::decode(Cow::Owned(self.bytes)))
    }
    /// Consume only the key without decoding its value.
    pub fn into_key(self) -> K {
        self.key
    }
    /// Consume and decode only the value.
    pub fn into_value(self) -> V {
        ordered_map::decode(Cow::Owned(self.bytes))
    }
}
///
/// `MapIter`
///
/// Bidirectional ordered traversal retaining its governed map borrow.
///
pub struct MapIter<'a, K: Storable + Ord + Clone, V: Storable, M: Memory = DefaultMemoryImpl>(
    pub(super) ordered_map::Entries<'a, K, V, TreeMemory<M>>,
);
fn entry<K, V>((key, bytes): (K, Vec<u8>)) -> MapEntry<K, V> {
    MapEntry {
        key,
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
