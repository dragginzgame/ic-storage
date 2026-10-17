//! Read-only structural audit of the persisted tree and free-list ownership.
use super::{
    BTreeMap, Comparator, Error, Memory,
    page::{self, DEGREE, MAX_HEIGHT, PAYLOAD},
};
use std::{cmp::Ordering, collections::BTreeSet};

struct Audit<'a, M, C> {
    tree: &'a BTreeMap<M, C>,
    seen: BTreeSet<u64>,
    previous: Option<Vec<u8>>,
    count: u64,
    leaf_depth: Option<usize>,
}

pub(super) fn validate<M: Memory, C: Comparator>(tree: &BTreeMap<M, C>) -> Result<(), Error> {
    let mut audit = Audit {
        tree,
        seen: BTreeSet::new(),
        previous: None,
        count: 0,
        leaf_depth: None,
    };
    audit.audit_node(tree.header.root, 0)?;
    if audit.count != tree.header.len {
        return Err(Error::Corrupt);
    }
    let mut free = tree.header.free;
    while free != 0 {
        if !audit.seen.insert(free) {
            return Err(Error::Corrupt);
        }
        let page = page::read(&tree.memory, tree.header.next, free)?;
        if page[0] != page::FREE {
            return Err(Error::Corrupt);
        }
        free = page::number(&page, 8);
    }
    if audit.seen.len() as u64 != tree.header.next - 1 {
        return Err(Error::Corrupt);
    }
    Ok(())
}

impl<M: Memory, C: Comparator> Audit<'_, M, C> {
    fn audit_node(&mut self, id: u64, depth: usize) -> Result<(), Error> {
        if depth >= MAX_HEIGHT || !self.seen.insert(id) {
            return Err(Error::Corrupt);
        }
        let node = self.tree.node(id)?;
        if depth > 0 && node.entries.len() < DEGREE - 1 {
            return Err(Error::Corrupt);
        }
        if node.entries.is_empty() && !node.children.is_empty() {
            return Err(Error::Corrupt);
        }
        if node.children.is_empty() {
            if self.leaf_depth.is_some_and(|expected| expected != depth) {
                return Err(Error::Corrupt);
            }
            self.leaf_depth = Some(depth);
        }
        for (i, record) in node.entries.iter().enumerate() {
            if !node.children.is_empty() {
                self.audit_node(node.children[i], depth + 1)?;
            }
            let key = if record.key == 0 {
                self.tree.key(*record)?
            } else {
                page::read_blob_with(record.key, record.key_len, |id, output| {
                    if !self.seen.insert(id) {
                        return Err(Error::Corrupt);
                    }
                    page::read_into(&self.tree.memory, self.tree.header.next, id, output)
                })?
            };
            self.audit_blob(record.value, record.value_len)?;
            self.tree.check_key(&key)?;
            if let Some(previous) = self.previous.as_ref()
                && self.tree.order.compare(previous, &key)? != Ordering::Less
            {
                return Err(Error::Corrupt);
            }
            self.previous = Some(key);
            self.count = self.count.checked_add(1).ok_or(Error::Corrupt)?;
        }
        if let Some(last) = node.children.last() {
            self.audit_node(*last, depth + 1)?;
        }
        Ok(())
    }
    fn audit_blob(&mut self, mut id: u64, mut len: u64) -> Result<(), Error> {
        if id == 0 {
            return Ok(());
        }
        while len != 0 {
            if !self.seen.insert(id) {
                return Err(Error::Corrupt);
            }
            let page = page::read(&self.tree.memory, self.tree.header.next, id)?;
            if page[0] != page::BLOB {
                return Err(Error::Corrupt);
            }
            id = page::number(&page, 8);
            len = len.saturating_sub(PAYLOAD as u64);
        }
        if id != 0 {
            return Err(Error::Corrupt);
        }
        Ok(())
    }
}
