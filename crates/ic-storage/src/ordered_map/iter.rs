//! Checked bound admission seeds two traversal paths; rank jumps skip entries.

use std::{
    cmp::Ordering,
    iter::FusedIterator,
    ops::{Bound, RangeBounds},
};

use crate::{Memory, storable::Storable};

use super::{
    EncodedValue, OrderedMap, check_depth,
    storage::{MAX_HEIGHT, Node},
};

struct Cursor<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> {
    map: &'a OrderedMap<K, V, M>,
    front: u64,
    back: u64,
    forward: Option<Vec<Node>>,
    backward: Option<Vec<Node>>,
    forward_consumed: bool,
    backward_consumed: bool,
}

impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> Cursor<'a, K, V, M> {
    const fn new(map: &'a OrderedMap<K, V, M>, front: u64, back: u64) -> Self {
        Self {
            map,
            front,
            back,
            forward: None,
            backward: None,
            forward_consumed: false,
            backward_consumed: false,
        }
    }

    fn range(map: &'a OrderedMap<K, V, M>, range: &impl RangeBounds<K>) -> Self {
        let start = range.start_bound();
        let end = range.end_bound();
        if let (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b)) =
            (start, end)
        {
            assert!(a <= b, "ordered-map range starts after its end");
            assert!(
                a != b || !matches!((start, end), (Bound::Excluded(_), Bound::Excluded(_))),
                "ordered-map range excludes equal endpoints"
            );
        }
        let mut cursor = Self::new(map, 0, map.len());
        if let Bound::Included(key) | Bound::Excluded(key) = start {
            let (rank, path) = cursor.bound_path(key, matches!(start, Bound::Excluded(_)), false);
            cursor.front = rank;
            cursor.forward = Some(path);
        }
        if let Bound::Included(key) | Bound::Excluded(key) = end {
            let (rank, path) = cursor.bound_path(key, matches!(end, Bound::Included(_)), true);
            cursor.back = rank;
            cursor.backward = Some(path);
        }
        cursor.front = cursor.front.min(cursor.back);
        cursor
    }

    fn bound_path(&self, key: &K, after: bool, reverse: bool) -> (u64, Vec<Node>) {
        let mut path = Vec::with_capacity(16);
        let mut current = self.map.store.root_node();
        let mut rank = 0_u64;
        let mut depth = 0;
        while let Some(node) = current {
            check_depth(depth);
            let ordering = self.map.key(node).cmp(key);
            let right = ordering == Ordering::Less || (after && ordering == Ordering::Equal);
            // A left turn retains a forward successor; a right turn retains a
            // reverse predecessor. The final candidate is at the top, ready
            // for take(), without repeating this checked descent by rank.
            if right == reverse {
                path.push(node);
            }
            if right {
                rank = rank
                    .checked_add(node.left.count)
                    .and_then(|n| n.checked_add(1))
                    .expect("ordered-map rank overflow");
                current = self.map.store.child_node(node.right);
            } else {
                current = self.map.store.child_node(node.left);
            }
            depth += 1;
        }
        assert!(rank <= self.map.len(), "invalid ordered-map rank");
        (rank, path)
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
                self.backward_consumed = false;
            } else {
                self.forward = Some(path);
                self.forward_consumed = false;
            }
        }
        let (path, consumed) = if reverse {
            (&mut self.backward, &mut self.backward_consumed)
        } else {
            (&mut self.forward, &mut self.forward_consumed)
        };
        let path = path.as_mut().expect("initialized cursor");
        if *consumed {
            // Keep the yielded top until this direction is requested again.
            // Unused lookahead does no IO; every visited child is still checked.
            let node = path.pop().expect("invalid ordered-map cursor path");
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
        *consumed = true;
        Some(*path.last().expect("invalid ordered-map cursor path"))
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
    pub(super) fn range(map: &'a OrderedMap<K, V, M>, range: &impl RangeBounds<K>) -> Self {
        Self {
            cursor: Cursor::range(map, range),
        }
    }

    pub(super) const fn new(map: &'a OrderedMap<K, V, M>, front: u64, back: u64) -> Self {
        Self {
            cursor: Cursor::new(map, front, back),
        }
    }
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> Iterator for Entries<'_, K, V, M> {
    type Item = (K, EncodedValue);
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
    pub(super) fn range(map: &'a OrderedMap<K, V, M>, range: &impl RangeBounds<K>) -> Self {
        Self {
            cursor: Cursor::range(map, range),
        }
    }

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
