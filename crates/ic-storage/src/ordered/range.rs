use super::{ByteSlice, Bytes, Map};
use core::{alloc::Allocator, iter::FusedIterator, ops::Bound};

/// Borrowed, double-ended byte range. Each step performs a root-to-key seek;
/// traversal neither buffers all entries nor walks unrelated prefixes.
pub struct Range<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> {
    map: &'a Map<K, V, A, P>,
    front: u64,
    back: u64,
}

impl<'a, const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    Range<'a, K, V, A, P>
{
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
        if self.front == 0 {
            return None;
        }
        let id = self.front;
        if self.front == self.back {
            self.front = 0;
            self.back = 0;
        } else {
            self.front = seek(self.map, Bound::Excluded(self.map.key(id)), true);
        }
        Some((ByteSlice(self.map.key(id)), ByteSlice(self.map.value(id))))
    }
}
impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> DoubleEndedIterator
    for Range<'_, K, V, A, P>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.back == 0 {
            return None;
        }
        let id = self.back;
        if self.front == self.back {
            self.front = 0;
            self.back = 0;
        } else {
            self.back = seek(self.map, Bound::Excluded(self.map.key(id)), false);
        }
        Some((ByteSlice(self.map.key(id)), ByteSlice(self.map.value(id))))
    }
}
impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> FusedIterator
    for Range<'_, K, V, A, P>
{
}
