use super::page::{self, DEGREE, Header, MAX, MAX_HEIGHT, Node, PAYLOAD, Page, Record, SIZE};
use super::{BTreeMap, Comparator, Error, Memory};
use std::{
    cmp::Ordering,
    collections::{BTreeMap as Pages, BTreeSet},
};

pub(super) struct Transaction<'a, M, C> {
    tree: &'a BTreeMap<M, C>,
    pub header: Header,
    // Keep balancing the staging index from moving entire 4 KiB page buffers.
    pages: Pages<u64, Box<Page>>,
    allocated: BTreeSet<u64>,
}
impl<'a, M: Memory, C: Comparator> Transaction<'a, M, C> {
    pub fn new(tree: &'a BTreeMap<M, C>) -> Self {
        Self {
            tree,
            header: tree.header,
            pages: Pages::new(),
            allocated: BTreeSet::new(),
        }
    }
    fn read_into(&self, id: u64, output: &mut Page) -> Result<(), Error> {
        if let Some(page) = self.pages.get(&id) {
            output.copy_from_slice(page.as_slice());
            return Ok(());
        }
        page::read_into(&self.tree.memory, self.tree.header.next, id, output)
    }
    pub fn allocate(&mut self) -> Result<u64, Error> {
        let id;
        if self.header.free == 0 {
            id = self.header.next;
            self.header.next = id.checked_add(1).ok_or(Error::Capacity)?;
        } else {
            id = self.header.free;
            let mut page = [0; SIZE];
            self.read_into(id, &mut page)?;
            if page[0] != page::FREE || !self.allocated.insert(id) {
                return Err(Error::Corrupt);
            }
            self.header.free = page::number(&page, 8);
            if self.header.free >= self.header.next {
                return Err(Error::Corrupt);
            }
        }
        Ok(id)
    }
    fn free(&mut self, id: u64) -> Result<(), Error> {
        let mut page = [0; SIZE];
        self.free_into(id, &mut page)
    }
    // Keep the validated prior bytes in the caller's buffer for chain traversal.
    fn free_into(&mut self, id: u64, previous: &mut Page) -> Result<(), Error> {
        if id == 0 || id >= self.header.next {
            return Err(Error::Corrupt);
        }
        self.read_into(id, previous)?;
        if previous[0] == page::FREE {
            return Err(Error::Corrupt);
        }
        self.stage_free(id);
        Ok(())
    }
    // The caller has already checked this page is in range and live in this
    // transaction. Keep that validation separate from staging its free record.
    fn stage_free(&mut self, id: u64) {
        let page = page::free(self.header.free);
        self.pages.insert(id, page);
        self.header.free = id;
    }
    fn node(&self, id: u64) -> Result<Node, Error> {
        let mut page = [0; SIZE];
        self.read_into(id, &mut page)?;
        Node::decode(&page, self.header.next, self.header.config)
    }
    pub fn put_node(&mut self, id: u64, node: &Node) -> Result<(), Error> {
        self.pages.insert(id, node.encode()?);
        Ok(())
    }
    fn key(&self, record: Record) -> Result<Vec<u8>, Error> {
        if record.key == 0 {
            return Ok(record.key_inline
                [..usize::try_from(record.key_len).map_err(|_| Error::Corrupt)?]
                .to_vec());
        }
        page::read_blob_with(record.key, record.key_len, |id, output| {
            self.read_into(id, output)
        })
    }
    fn locate(&self, node: &Node, key: &[u8]) -> Result<(usize, bool), Error> {
        super::search::locate(
            node,
            key,
            &self.tree.order,
            |id, buffer| self.read_into(id, buffer),
            |record| self.key(record),
        )
    }
    fn store_blob(&mut self, bytes: &[u8]) -> Result<u64, Error> {
        let mut next = 0;
        for index in (0..bytes.len().div_ceil(PAYLOAD)).rev() {
            let chunk = &bytes[index * PAYLOAD..bytes.len().min((index + 1) * PAYLOAD)];
            let id = self.allocate()?;
            self.pages.insert(id, page::encode_blob(next, chunk));
            next = id;
        }
        Ok(next)
    }
    fn free_blob(
        &mut self,
        mut id: u64,
        mut len: u64,
        mut output: Option<&mut Vec<u8>>,
    ) -> Result<(), Error> {
        if id == 0 {
            return Ok(());
        }
        let mut page = [0; SIZE];
        while len != 0 {
            self.free_into(id, &mut page)?;
            if page[0] != page::BLOB {
                return Err(Error::Corrupt);
            }
            if let Some(bytes) = output.as_mut() {
                let take = usize::try_from(len.min(PAYLOAD as u64)).map_err(|_| Error::Corrupt)?;
                bytes.extend_from_slice(&page[16..16 + take]);
            }
            let next = page::number(&page, 8);
            len = len.saturating_sub(PAYLOAD as u64);
            id = next;
        }
        if id != 0 {
            return Err(Error::Corrupt);
        }
        Ok(())
    }
    fn split(&mut self, parent: &mut Node, at: usize, mut left: Node) -> Result<(), Error> {
        let left_id = parent.children[at];
        if left.entries.len() != MAX {
            return Err(Error::Corrupt);
        }
        let right_entries = left.entries.split_off(DEGREE);
        let median = left.entries.pop().ok_or(Error::Corrupt)?;
        let children = if left.children.is_empty() {
            Vec::new()
        } else {
            left.children.split_off(DEGREE)
        };
        let right_id = self.allocate()?;
        self.put_node(left_id, &left)?;
        self.put_node(
            right_id,
            &Node {
                entries: right_entries,
                children,
            },
        )?;
        parent.entries.insert(at, median);
        parent.children.insert(at + 1, right_id);
        Ok(())
    }
    fn replace_value(&mut self, record: &mut Record, value: &[u8]) -> Result<Vec<u8>, Error> {
        let mut ids = Vec::new();
        let mut id = record.value;
        let mut remaining = record.value_len;
        let old_len = usize::try_from(remaining).map_err(|_| Error::Corrupt)?;
        let mut previous = Vec::with_capacity(old_len);
        if id != 0 {
            let mut page = [0; SIZE];
            while remaining != 0 {
                self.read_into(id, &mut page)?;
                if page[0] != page::BLOB {
                    return Err(Error::Corrupt);
                }
                let take = (old_len - previous.len()).min(PAYLOAD);
                previous.extend_from_slice(&page[16..16 + take]);
                ids.push(id);
                id = page::number(&page, 8);
                remaining = remaining.saturating_sub(PAYLOAD as u64);
            }
            if id != 0 {
                return Err(Error::Corrupt);
            }
        } else {
            previous.extend_from_slice(&record.value_inline[..old_len]);
        }
        let needed = if value.len() <= page::VALUE_INLINE_BYTES {
            0
        } else {
            value.len().div_ceil(PAYLOAD)
        };
        while ids.len() > needed {
            // The complete old chain was validated above. Keep the existing
            // reverse reclamation order without reading each page a second time.
            self.stage_free(ids.pop().ok_or(Error::Corrupt)?);
        }
        while ids.len() < needed {
            ids.push(self.allocate()?);
        }
        for (index, id) in ids.iter().copied().enumerate() {
            let chunk = &value[index * PAYLOAD..value.len().min((index + 1) * PAYLOAD)];
            let next = ids.get(index + 1).copied().unwrap_or(0);
            self.pages.insert(id, page::encode_blob(next, chunk));
        }
        record.set_value(value, ids.first().copied().unwrap_or(0));
        Ok(previous)
    }
    pub fn replace(
        &mut self,
        id: u64,
        mut node: Node,
        at: usize,
        value: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let original = node.entries[at];
        let previous = self.replace_value(&mut node.entries[at], value)?;
        // Same-size overflow updates change payload pages, not their record.
        if node.entries[at] != original {
            self.put_node(id, &node)?;
        }
        Ok(previous)
    }
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let mut node = self.node(self.header.root)?;
        if node.entries.len() == MAX {
            let id = self.allocate()?;
            let mut parent = Node {
                entries: Vec::new(),
                children: vec![self.header.root],
            };
            self.split(&mut parent, 0, node)?;
            self.put_node(id, &parent)?;
            self.header.root = id;
            node = parent;
        }
        let mut id = self.header.root;
        for _ in 0..MAX_HEIGHT {
            let (mut at, found) = self.locate(&node, key)?;
            if found {
                return Err(Error::Corrupt);
            }
            if node.children.is_empty() {
                let key_page = if key.len() <= page::KEY_INLINE_BYTES {
                    0
                } else {
                    self.store_blob(key)?
                };
                let value_page = if value.len() <= page::VALUE_INLINE_BYTES {
                    0
                } else {
                    self.store_blob(value)?
                };
                let record = Record::new(key, value, key_page, value_page);
                node.entries.insert(at, record);
                return self.put_node(id, &node);
            }
            let mut child = self.node(node.children[at])?;
            if child.entries.len() == MAX {
                self.split(&mut node, at, child)?;
                let comparison = self.tree.order.compare(key, &self.key(node.entries[at])?)?;
                self.put_node(id, &node)?;
                if comparison == Ordering::Equal {
                    return Err(Error::Corrupt);
                }
                if comparison == Ordering::Greater {
                    at += 1;
                }
                child = self.node(node.children[at])?;
            }
            id = node.children[at];
            node = child;
        }
        Err(Error::Corrupt)
    }
    // Merge adjacent children around their parent separator; return the survivor.
    fn merge(&mut self, parent: &mut Node, at: usize) -> Result<u64, Error> {
        let left_id = parent.children[at];
        let right_id = parent.children.remove(at + 1);
        let mut left = self.node(left_id)?;
        let right = self.node(right_id)?;
        if left.children.is_empty() != right.children.is_empty() {
            return Err(Error::Corrupt);
        }
        left.entries.push(parent.entries.remove(at));
        left.entries.extend(right.entries);
        left.children.extend(right.children);
        self.put_node(left_id, &left)?;
        self.free(right_id)?;
        Ok(left_id)
    }
    fn prepare_child(&mut self, parent: &mut Node, at: usize) -> Result<u64, Error> {
        let id = parent.children[at];
        let mut child = self.node(id)?;
        if child.entries.len() >= DEGREE {
            return Ok(id);
        }
        if child.entries.len() != DEGREE - 1 {
            return Err(Error::Corrupt);
        }
        if at > 0 {
            let left_id = parent.children[at - 1];
            let mut left = self.node(left_id)?;
            if left.entries.len() >= DEGREE {
                child.entries.insert(0, parent.entries[at - 1]);
                parent.entries[at - 1] = left.entries.pop().ok_or(Error::Corrupt)?;
                if !left.children.is_empty() {
                    child
                        .children
                        .insert(0, left.children.pop().ok_or(Error::Corrupt)?);
                }
                self.put_node(left_id, &left)?;
                self.put_node(id, &child)?;
                return Ok(id);
            }
        }
        if at + 1 < parent.children.len() {
            let right_id = parent.children[at + 1];
            let mut right = self.node(right_id)?;
            if right.entries.len() >= DEGREE {
                child.entries.push(parent.entries[at]);
                parent.entries[at] = right.entries.remove(0);
                if !right.children.is_empty() {
                    child.children.push(right.children.remove(0));
                }
                self.put_node(right_id, &right)?;
                self.put_node(id, &child)?;
                return Ok(id);
            }
            return self.merge(parent, at);
        }
        self.merge(parent, at.checked_sub(1).ok_or(Error::Corrupt)?)
    }
    fn extreme(&self, mut id: u64, last: bool) -> Result<Record, Error> {
        for _ in 0..MAX_HEIGHT {
            let node = self.node(id)?;
            if node.children.is_empty() {
                return (if last {
                    node.entries.last()
                } else {
                    node.entries.first()
                })
                .copied()
                .ok_or(Error::Corrupt);
            }
            id = if last {
                *node.children.last().ok_or(Error::Corrupt)?
            } else {
                node.children[0]
            };
        }
        Err(Error::Corrupt)
    }
    fn delete(&mut self, id: u64, key: &[u8], depth: usize) -> Result<Option<Record>, Error> {
        if depth >= MAX_HEIGHT {
            return Err(Error::Corrupt);
        }
        let mut node = self.node(id)?;
        let (at, found) = self.locate(&node, key)?;
        let removed;
        if found {
            removed = node.entries[at];
            if node.children.is_empty() {
                node.entries.remove(at);
            } else if self.node(node.children[at])?.entries.len() >= DEGREE {
                let replacement = self.extreme(node.children[at], true)?;
                self.delete(node.children[at], &self.key(replacement)?, depth + 1)?
                    .ok_or(Error::Corrupt)?;
                node.entries[at] = replacement;
            } else if self.node(node.children[at + 1])?.entries.len() >= DEGREE {
                let replacement = self.extreme(node.children[at + 1], false)?;
                self.delete(node.children[at + 1], &self.key(replacement)?, depth + 1)?
                    .ok_or(Error::Corrupt)?;
                node.entries[at] = replacement;
            } else {
                let child = self.merge(&mut node, at)?;
                self.delete(child, key, depth + 1)?.ok_or(Error::Corrupt)?;
            }
        } else {
            if node.children.is_empty() {
                return Ok(None);
            }
            let child = self.prepare_child(&mut node, at)?;
            let Some(record) = self.delete(child, key, depth + 1)? else {
                return Ok(None);
            };
            removed = record;
        }
        self.put_node(id, &node)?;
        Ok(Some(removed))
    }
    pub fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let Some(removed) = self.delete(self.header.root, key, 0)? else {
            return Ok(None);
        };
        self.free_blob(removed.key, removed.key_len, None)?;
        let len = usize::try_from(removed.value_len).map_err(|_| Error::Corrupt)?;
        let mut previous = Vec::with_capacity(len);
        if removed.value == 0 {
            previous.extend_from_slice(&removed.value_inline[..len]);
        } else {
            self.free_blob(removed.value, removed.value_len, Some(&mut previous))?;
        }
        let root = self.node(self.header.root)?;
        if root.entries.is_empty() && !root.children.is_empty() {
            self.free(self.header.root)?;
            self.header.root = root.children[0];
        }
        Ok(Some(previous))
    }
    pub fn commit(self) -> Result<Header, Error> {
        let bytes = self
            .header
            .next
            .checked_mul(SIZE as u64)
            .ok_or(Error::Capacity)?;
        let required = bytes.div_ceil(65_536);
        let current = self.tree.memory.size();
        if required > current && self.tree.memory.grow(required - current) < 0 {
            return Err(Error::Capacity);
        }
        // No fallible application operation follows the first write. A backing
        // trap must roll the entire message back; filesystem crash atomicity is
        // intentionally not claimed by this storage contract.
        for (id, page) in self.pages {
            self.tree.memory.write(id * SIZE as u64, page.as_slice());
        }
        if self.header != self.tree.header {
            self.tree.memory.write(0, &self.header.encode());
        }
        Ok(self.header)
    }
}
