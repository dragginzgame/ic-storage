use super::{ByteSlice, Bytes, Map};
use core::{alloc::Allocator, iter::FusedIterator, ops::Bound};

/// Borrowed, double-ended byte range. Each step performs a root-to-key seek;
/// traversal neither buffers all entries nor walks unrelated prefixes.
pub struct Range<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> {
    map: &'a Map<K, V, A, P>,
    front: u64,
    back: u64,
}

/// A borrowed candidate whose key and declared value length can be inspected
/// before accessing value bytes. Neither this handle nor its borrows may outlive
/// the collection scope. It does not copy an owning payload.
pub struct Entry<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> {
    map: &'a Map<K, V, A, P>,
    id: u64,
}
impl<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Entry<'a, K, V, A, P>
{
    /// Borrow only this candidate's key.
    #[must_use]
    pub fn key(&self) -> ByteSlice<'a> {
        ByteSlice(self.map.key(self.id))
    }
    /// Read the admitted value length from scalar node metadata, without accessing
    /// the value's byte span. Applications may use it for preflight accounting.
    #[must_use]
    pub fn value_len(&self) -> usize {
        self.map.node(self.id).value.length as usize
    }
    /// Borrow the value only after caller-owned filtering/budget preflight accepts it.
    #[must_use]
    pub fn value(&self) -> ByteSlice<'a> {
        ByteSlice(self.map.value(self.id))
    }
}

/// Double-ended, fused traversal that yields candidates without visiting values.
/// Keep a logical continuation at the last consumed key: a candidate refused by
/// a byte/work budget must not become the next page's exclusive cursor.
pub struct Entries<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> {
    range: Range<'a, K, V, A, P>,
}

impl<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Range<'a, K, V, A, P>
{
    /// Continue this range's remaining traversal with lazy value access. Supports
    /// both directions and preserves any already-consumed front/back positions.
    ///
    /// ```compile_fail
    /// let mut map = ic_storage::ordered::Map::<512, 4096>::new(16);
    /// let entry = map.with(|view| view.range(core::ops::Bound::Unbounded,
    ///     core::ops::Bound::Unbounded).entries().next());
    /// ```
    #[must_use]
    pub fn entries(self) -> Entries<'a, K, V, A, P> {
        Entries { range: self }
    }

    fn advance(&mut self, forward: bool) -> Option<Entry<'a, K, V, A, P>> {
        let id = if forward { self.front } else { self.back };
        if id == 0 {
            return None;
        }
        if self.front == self.back {
            self.front = 0;
            self.back = 0;
        } else if forward {
            self.front = seek(self.map, Bound::Excluded(self.map.key(id)), true);
        } else {
            self.back = seek(self.map, Bound::Excluded(self.map.key(id)), false);
        }
        Some(Entry { map: self.map, id })
    }
    pub(super) fn new(map: &'a Map<K, V, A, P>, lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> Self {
        let front = seek(map, lower, true);
        let back = seek(map, upper, false);
        let mut range = Self { map, front, back };
        if front == 0 || back == 0 || map.key(front) > map.key(back) {
            range.front = 0;
            range.back = 0;
        }
        range
    }
    pub(super) fn prefix(map: &'a Map<K, V, A, P>, prefix: &[u8]) -> Self {
        let Ok(mut upper) = Bytes::<K>::new(prefix) else {
            return Self {
                map,
                front: 0,
                back: 0,
            };
        };
        for index in (0..prefix.len()).rev() {
            if upper.data[index] != 255 {
                upper.data[index] += 1;
                // Truncate directly; this temporary is used only as a bound.
                return Self::new(
                    map,
                    Bound::Included(prefix),
                    Bound::Excluded(&upper.data[..=index]),
                );
            }
        }
        Self::new(map, Bound::Included(prefix), Bound::Unbounded)
    }
}

fn seek<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>(
    map: &Map<K, V, A, P>,
    bound: Bound<&[u8]>,
    forward: bool,
) -> u64 {
    let mut id = map.image.root;
    let mut candidate = 0;
    while id != 0 {
        let node = map.node(id);
        let qualifies = match bound {
            Bound::Unbounded => true,
            Bound::Included(key) => {
                if forward {
                    map.key(id) >= key
                } else {
                    map.key(id) <= key
                }
            }
            Bound::Excluded(key) => {
                if forward {
                    map.key(id) > key
                } else {
                    map.key(id) < key
                }
            }
        };
        if qualifies {
            candidate = id;
            id = if forward { node.left } else { node.right };
        } else {
            id = if forward { node.right } else { node.left };
        }
    }
    candidate
}

impl<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> Iterator
    for Range<'a, K, V, A, P>
{
    type Item = (ByteSlice<'a>, ByteSlice<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        self.advance(true).map(|entry| (entry.key(), entry.value()))
    }
}
impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> DoubleEndedIterator
    for Range<'_, K, V, A, P>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.advance(false)
            .map(|entry| (entry.key(), entry.value()))
    }
}
impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> FusedIterator
    for Range<'_, K, V, A, P>
{
}

impl<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> Iterator
    for Entries<'a, K, V, A, P>
{
    type Item = Entry<'a, K, V, A, P>;
    fn next(&mut self) -> Option<Self::Item> {
        self.range.advance(true)
    }
}
impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> DoubleEndedIterator
    for Entries<'_, K, V, A, P>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.range.advance(false)
    }
}
impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> FusedIterator
    for Entries<'_, K, V, A, P>
{
}
