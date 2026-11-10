//! Two bounded traversal paths with rank jumps for skipped entries.

use std::{cmp::Ordering, iter::FusedIterator};

use crate::{Memory, storable::Storable};

use super::{
    OrderedMap,
    storage::{MAX_HEIGHT, Node},
};

struct Cursor<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> {
    map: &'a OrderedMap<K, V, M>,
    front: u64,
    back: u64,
    forward: Option<Vec<Node>>,
    backward: Option<Vec<Node>>,
}

impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> Cursor<'a, K, V, M> {
    const fn new(map: &'a OrderedMap<K, V, M>, front: u64, back: u64) -> Self {
        Self {
            map,
            front,
            back,
            forward: None,
            backward: None,
        }
    }

    fn seek(&self, mut rank: u64, reverse: bool) -> Vec<Node> {
        let mut path = Vec::with_capacity(16);
        let mut current = self.map.store.root_node();
        for _ in 0..MAX_HEIGHT {
            let node = current.expect("invalid ordered-map iterator rank");
            assert!(
                rank < node.link().count,
                "invalid ordered-map iterator rank"
            );
            match rank.cmp(&node.left.count) {
                Ordering::Equal => {
                    path.push(node);
                    return path;
                }
                Ordering::Less => {
                    if !reverse {
                        path.push(node);
                    }
                    current = self.map.store.child_node(node.left);
                }
                Ordering::Greater => {
                    if reverse {
                        path.push(node);
                    }
                    rank -= node.left.count + 1;
                    current = self.map.store.child_node(node.right);
                }
            }
        }
        panic!("ordered-map cursor exceeds AVL height bound");
    }

    fn take(&mut self, reverse: bool, skip: usize) -> Option<Node> {
        let skip = u64::try_from(skip).unwrap_or(u64::MAX);
        if skip >= self.back - self.front {
            self.front = self.back;
            return None;
        }
        let rank = if reverse {
            self.back -= skip + 1;
            self.back
        } else {
            self.front += skip + 1;
            self.front - 1
        };
        if skip != 0
            || if reverse {
                self.backward.is_none()
            } else {
                self.forward.is_none()
            }
        {
            let path = self.seek(rank, reverse);
            if reverse {
                self.backward = Some(path);
            } else {
                self.forward = Some(path);
            }
        }
        let path = if reverse {
            self.backward.as_mut()
        } else {
            self.forward.as_mut()
        }
        .expect("initialized cursor");
        let node = path.pop().expect("invalid ordered-map cursor path");
        if self.front != self.back {
            let mut link = if reverse { node.left } else { node.right };
            let mut steps = 0;
            while link.address != 0 {
                assert!(
                    steps < MAX_HEIGHT
                        && path.len() < usize::try_from(MAX_HEIGHT).expect("height fits host"),
                    "ordered-map cursor exceeds AVL height bound"
                );
                let descendant = self
                    .map
                    .store
                    .child_node(link)
                    .expect("missing cursor child");
                path.push(descendant);
                link = if reverse {
                    descendant.right
                } else {
                    descendant.left
                };
                steps += 1;
            }
        }
        Some(node)
    }

    fn hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (
            usize::try_from(remaining).unwrap_or(usize::MAX),
            usize::try_from(remaining).ok(),
        )
    }

    fn count(&self) -> usize {
        usize::try_from(self.back - self.front)
            .expect("ordered-map iterator count exceeds host address space")
    }
}

///
/// Entries
///
/// Double-ended cursor returning owned keys and encoded values.
pub struct Entries<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> {
    cursor: Cursor<'a, K, V, M>,
}

impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> Entries<'a, K, V, M> {
    pub(super) const fn new(map: &'a OrderedMap<K, V, M>, front: u64, back: u64) -> Self {
        Self {
            cursor: Cursor::new(map, front, back),
        }
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> Iterator for Entries<'_, K, V, M> {
    type Item = (K, Vec<u8>);
    fn next(&mut self) -> Option<Self::Item> {
        self.nth(0)
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let node = self.cursor.take(false, n)?;
        Some((self.cursor.map.key(node), self.cursor.map.value_bytes(node)))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.cursor.hint()
    }
    fn count(self) -> usize {
        self.cursor.count()
    }
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> DoubleEndedIterator
    for Entries<'_, K, V, M>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.nth_back(0)
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let node = self.cursor.take(true, n)?;
        Some((self.cursor.map.key(node), self.cursor.map.value_bytes(node)))
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> FusedIterator for Entries<'_, K, V, M> {}

///
/// Keys
///
/// Key-only cursor that never reads value payloads.
pub struct Keys<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> {
    cursor: Cursor<'a, K, V, M>,
}

impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> Keys<'a, K, V, M> {
    pub(super) const fn new(map: &'a OrderedMap<K, V, M>, front: u64, back: u64) -> Self {
        Self {
            cursor: Cursor::new(map, front, back),
        }
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> Iterator for Keys<'_, K, V, M> {
    type Item = K;
    fn next(&mut self) -> Option<K> {
        self.nth(0)
    }
    fn nth(&mut self, n: usize) -> Option<K> {
        let node = self.cursor.take(false, n)?;
        Some(self.cursor.map.key(node))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.cursor.hint()
    }
    fn count(self) -> usize {
        self.cursor.count()
    }
    fn last(mut self) -> Option<K> {
        self.next_back()
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> DoubleEndedIterator for Keys<'_, K, V, M> {
    fn next_back(&mut self) -> Option<K> {
        self.nth_back(0)
    }
    fn nth_back(&mut self, n: usize) -> Option<K> {
        let node = self.cursor.take(true, n)?;
        Some(self.cursor.map.key(node))
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> FusedIterator for Keys<'_, K, V, M> {}

///
/// Values
///
/// Ordered value cursor; skipped entries do not invoke a codec.
pub struct Values<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> {
    cursor: Cursor<'a, K, V, M>,
}

impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> Values<'a, K, V, M> {
    pub(super) const fn new(map: &'a OrderedMap<K, V, M>, front: u64, back: u64) -> Self {
        Self {
            cursor: Cursor::new(map, front, back),
        }
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> Iterator for Values<'_, K, V, M> {
    type Item = V;
    fn next(&mut self) -> Option<V> {
        self.nth(0)
    }
    fn nth(&mut self, n: usize) -> Option<V> {
        let node = self.cursor.take(false, n)?;
        Some(self.cursor.map.value(node))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.cursor.hint()
    }
    fn count(self) -> usize {
        self.cursor.count()
    }
    fn last(mut self) -> Option<V> {
        self.next_back()
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> DoubleEndedIterator
    for Values<'_, K, V, M>
{
    fn next_back(&mut self) -> Option<V> {
        self.nth_back(0)
    }
    fn nth_back(&mut self, n: usize) -> Option<V> {
        let node = self.cursor.take(true, n)?;
        Some(self.cursor.map.value(node))
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> FusedIterator for Values<'_, K, V, M> {}
