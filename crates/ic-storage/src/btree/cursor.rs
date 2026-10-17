use super::page::{MAX_HEIGHT, Node, Record};
use super::{BTreeMap, Comparator, Error, Memory};
use std::{cmp::Ordering, iter::FusedIterator, ops::Bound};

/// One scanned key with an unread value descriptor.
///
/// Holding an entry borrows the tree, preventing mutation through that handle.
/// Inspecting the key never reads the value payload.
pub struct Entry<'a, M, C = super::Lexicographic> {
    tree: &'a BTreeMap<M, C>,
    key: Vec<u8>,
    record: Record,
}
impl<M: Memory, C: Comparator> Entry<'_, M, C> {
    /// Borrow the encoded key.
    #[must_use]
    pub fn key(&self) -> &[u8] {
        &self.key
    }
    /// Encoded value size, without loading its payload.
    #[must_use]
    pub const fn value_len(&self) -> u64 {
        self.record.value_len
    }
    /// Load and validate this entry's value bytes.
    /// # Errors
    /// Returns an error on observed payload corruption.
    pub fn value(&self) -> Result<Vec<u8>, Error> {
        self.tree.value(self.record)
    }
    /// Consume the entry and load its value.
    /// # Errors
    /// Returns an error on observed payload corruption.
    pub fn into_pair(self) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let value = self.value()?;
        Ok((self.key, value))
    }
}
struct Frame {
    node: Node,
    position: usize,
}
struct Cursor {
    frames: Vec<Frame>,
    reverse: bool,
    started: bool,
    pending_child: Option<u64>,
}
impl Cursor {
    const fn new(reverse: bool) -> Self {
        Self {
            frames: Vec::new(),
            reverse,
            started: false,
            pending_child: None,
        }
    }
    fn seek<M: Memory, C: Comparator>(
        &mut self,
        tree: &BTreeMap<M, C>,
        bound: &Bound<Vec<u8>>,
    ) -> Result<(), Error> {
        self.started = true;
        let (Bound::Included(key) | Bound::Excluded(key)) = bound else {
            return self.descend(tree, tree.header.root);
        };
        let inclusive = matches!(bound, Bound::Included(_));
        let mut id = tree.header.root;
        for _ in 0..MAX_HEIGHT {
            let node = tree.node(id)?;
            let (at, found) = tree.locate(&node, key)?;
            let position = at + usize::from(found && (self.reverse == inclusive));
            let child = if found && inclusive {
                None
            } else {
                node.children.get(position).copied()
            };
            self.frames.push(Frame { node, position });
            if let Some(child) = child {
                id = child;
            } else {
                return Ok(());
            }
        }
        Err(Error::Corrupt)
    }
    fn descend<M: Memory, C: Comparator>(
        &mut self,
        tree: &BTreeMap<M, C>,
        mut id: u64,
    ) -> Result<(), Error> {
        while self.frames.len() < MAX_HEIGHT {
            let node = tree.node(id)?;
            let position = if self.reverse { node.entries.len() } else { 0 };
            let child = node.children.get(position).copied();
            self.frames.push(Frame { node, position });
            if let Some(child) = child {
                id = child;
            } else {
                return Ok(());
            }
        }
        Err(Error::Corrupt)
    }
    fn next<M: Memory, C: Comparator>(
        &mut self,
        tree: &BTreeMap<M, C>,
    ) -> Result<Option<Record>, Error> {
        if let Some(child) = self.pending_child.take() {
            self.descend(tree, child)?;
        }
        while let Some(frame) = self.frames.last_mut() {
            if (self.reverse && frame.position == 0)
                || (!self.reverse && frame.position == frame.node.entries.len())
            {
                self.frames.pop();
                continue;
            }
            let index = if self.reverse {
                frame.position - 1
            } else {
                frame.position
            };
            let record = frame.node.entries[index];
            frame.position = if self.reverse { index } else { index + 1 };
            let child = frame.node.children.get(frame.position).copied();
            self.pending_child = child;
            return Ok(Some(record));
        }
        Ok(None)
    }
}

/// Fallible, double-ended traversal with bounded tree-path memory.
///
/// Each direction seeks once, then walks the tree in order. The first observed
/// error ends iteration. Values are loaded only by [`Entry::value`].
pub struct Range<'a, M, C = super::Lexicographic> {
    tree: &'a BTreeMap<M, C>,
    lower: Bound<Vec<u8>>,
    upper: Bound<Vec<u8>>,
    front: Cursor,
    back: Cursor,
    done: bool,
}
impl<'a, M: Memory, C: Comparator> Range<'a, M, C> {
    pub(super) fn all(tree: &'a BTreeMap<M, C>) -> Self {
        Self {
            tree,
            lower: Bound::Unbounded,
            upper: Bound::Unbounded,
            front: Cursor::new(false),
            back: Cursor::new(true),
            done: false,
        }
    }
    pub(super) fn new(
        tree: &'a BTreeMap<M, C>,
        lower: Bound<&[u8]>,
        upper: Bound<&[u8]>,
    ) -> Result<Self, Error> {
        for bound in [&lower, &upper] {
            if let Bound::Included(key) | Bound::Excluded(key) = bound {
                tree.check_key(key)?;
            }
        }
        if let (
            Bound::Included(left) | Bound::Excluded(left),
            Bound::Included(right) | Bound::Excluded(right),
        ) = (lower, upper)
            && tree.order.compare(left, right)? == Ordering::Greater
        {
            return Err(Error::InvalidRange);
        }
        Ok(Self {
            lower: lower.map(<[u8]>::to_vec),
            upper: upper.map(<[u8]>::to_vec),
            ..Self::all(tree)
        })
    }
    fn advance(&mut self, reverse: bool) -> Result<Option<Entry<'a, M, C>>, Error> {
        let (cursor, bound) = if reverse {
            (&mut self.back, &self.upper)
        } else {
            (&mut self.front, &self.lower)
        };
        if !cursor.started {
            cursor.seek(self.tree, bound)?;
        }
        let Some(record) = cursor.next(self.tree)? else {
            return Ok(None);
        };
        let key = self.tree.key(record)?;
        self.tree.check_key(&key)?;
        for (bound, forbidden) in [
            (&self.lower, Ordering::Less),
            (&self.upper, Ordering::Greater),
        ] {
            if let Bound::Included(limit) | Bound::Excluded(limit) = bound {
                let comparison = self.tree.order.compare(&key, limit)?;
                if comparison == forbidden
                    || (comparison == Ordering::Equal && matches!(bound, Bound::Excluded(_)))
                {
                    return Ok(None);
                }
            }
        }
        let boundary = if reverse {
            &mut self.upper
        } else {
            &mut self.lower
        };
        if let Bound::Excluded(previous) = boundary {
            previous.clone_from(&key);
        } else {
            *boundary = Bound::Excluded(key.clone());
        }
        Ok(Some(Entry {
            tree: self.tree,
            key,
            record,
        }))
    }
    fn step(&mut self, reverse: bool) -> Option<Result<Entry<'a, M, C>, Error>> {
        if self.done {
            return None;
        }
        match self.advance(reverse) {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(error) => {
                self.done = true;
                Some(Err(error))
            }
        }
    }
}
impl<'a, M: Memory, C: Comparator> Iterator for Range<'a, M, C> {
    type Item = Result<Entry<'a, M, C>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        self.step(false)
    }
}
impl<M: Memory, C: Comparator> DoubleEndedIterator for Range<'_, M, C> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.step(true)
    }
}
impl<M: Memory, C: Comparator> FusedIterator for Range<'_, M, C> {}
