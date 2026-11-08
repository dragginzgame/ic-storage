// Adapted from ic-stable-structures 0.7.2, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See the package LICENSE-APACHE and docs/storage/icss-source.json.
use crate::engine::{
    Memory,
    btreemap::Allocator,
    read_address_vec, read_struct, read_to_vec, read_u32,
    storable::Storable,
    types::{Address, Bytes},
    write,
};
use std::borrow::{Borrow, Cow};
use std::cell::OnceCell;

mod io;

mod layout;

use io::NodeReader;

// The minimum degree to use in the btree.
// This constant is taken from Rust's std implementation of BTreeMap.
const B: usize = 6;
// The maximum number of entries per node.
const CAPACITY: usize = 2 * B - 1;
const LAYOUT_VERSION: u8 = 2;
const MAGIC: &[u8; 3] = b"BTN";
const LEAF_NODE_TYPE: u8 = 0;
const INTERNAL_NODE_TYPE: u8 = 1;
// The size of u32 in bytes.
const U32_SIZE: Bytes = Bytes::new(4);

#[derive(Debug, PartialEq, Copy, Clone, Eq)]

pub enum NodeType {
    Leaf,
    Internal,
}

pub type Entry<K> = (K, Vec<u8>);
pub type EntryRef<'a, K> = (&'a K, &'a [u8]);

type LazyEntry<K> = (LazyObject<K>, LazyObject<Vec<u8>>);
#[derive(Debug)]
#[expect(
    clippy::struct_field_names,
    reason = "Retain upstream node terminology for source comparison"
)]
pub struct Node<K: Storable + Ord + Clone> {
    address: Address,
    // List of tuples consisting of a key and the encoded value.
    // INVARIANT: the list is sorted by key.
    entries: Vec<LazyEntry<K>>,
    // For the key at position I, children[I] points to the left
    // child of this key and children[I + 1] points to the right child.
    children: Vec<Address>,
    node_type: NodeType,
    page_size: PageSize,

    // The address of the overflow page.
    // A node can span multiple allocator pages when its encoded entries require them.
    overflows: Vec<Address>,
}

impl<K: Storable + Ord + Clone> Node<K> {
    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn node_type(&self) -> NodeType {
        self.node_type
    }
    pub fn get_max<M: Memory>(&self, memory: &M) -> Entry<K> {
        match self.node_type {
            NodeType::Leaf => {
                let last_entry = self.entries.last().expect("A node can never be empty");
                (
                    self.get_key(last_entry, memory).clone(),
                    self.get_value(last_entry, memory).to_vec(),
                )
            }
            NodeType::Internal => {
                let last_child = Self::load(
                    *self
                        .children
                        .last()
                        .expect("An internal node must have children."),
                    self.page_size,
                    memory,
                );
                last_child.get_max(memory)
            }
        }
    }
    pub fn get_min<M: Memory>(&self, memory: &M) -> Entry<K> {
        match self.node_type {
            NodeType::Leaf => {
                // NOTE: a node can never be empty, so this access is safe.
                let entry = self.entry(0, memory);
                (entry.0.clone(), entry.1.to_vec())
            }
            NodeType::Internal => {
                let first_child = Self::load(
                    // NOTE: an internal node must have children, so this access is safe.
                    self.children[0],
                    self.page_size,
                    memory,
                );
                first_child.get_min(memory)
            }
        }
    }
    pub const fn is_full(&self) -> bool {
        self.entries.len() >= CAPACITY
    }
    pub fn swap_value<M: Memory>(&mut self, idx: usize, new: Vec<u8>, memory: &M) -> Vec<u8> {
        let old = core::mem::replace(&mut self.entries[idx].1, LazyObject::by_value(new));
        self.extract_value(old, memory)
    }
    pub fn swap_entry<M: Memory>(
        &mut self,
        idx: usize,
        (key, value): Entry<K>,
        memory: &M,
    ) -> Entry<K> {
        let (old_key, old_value) = core::mem::replace(
            &mut self.entries[idx],
            (LazyObject::by_value(key), LazyObject::by_value(value)),
        );
        (
            self.extract_key(old_key, memory),
            self.extract_value(old_value, memory),
        )
    }
    #[inline(always)]
    pub fn entry<M: Memory>(&self, idx: usize, memory: &M) -> EntryRef<'_, K> {
        (self.key(idx, memory), self.value(idx, memory))
    }

    #[inline(always)]
    fn get_key<'a, M: Memory>(&'a self, (k, _): &'a LazyEntry<K>, memory: &M) -> &'a K {
        k.get_or_load(|offset, size| self.load_key_from_memory(offset, size, memory))
    }
    #[inline(always)]
    fn get_value<'a, M: Memory>(&'a self, (_, v): &'a LazyEntry<K>, memory: &M) -> &'a [u8] {
        v.get_or_load(|offset, size| self.load_value_from_memory(offset, size, memory))
    }
    #[inline(always)]
    pub fn key<M: Memory>(&self, idx: usize, memory: &M) -> &K {
        self.get_key(&self.entries[idx], memory)
    }
    #[inline(always)]
    pub fn value<M: Memory>(&self, idx: usize, memory: &M) -> &[u8] {
        self.get_value(&self.entries[idx], memory)
    }
    fn extract_key<M: Memory>(&self, key: LazyObject<K>, memory: &M) -> K {
        key.take_or_load(|offset, size| self.load_key_from_memory(offset, size, memory))
    }
    fn extract_value<M: Memory>(&self, value: LazyObject<Vec<u8>>, memory: &M) -> Vec<u8> {
        value.take_or_load(|offset, size| self.load_value_from_memory(offset, size, memory))
    }
    fn load_key_from_memory<M: Memory>(
        &self,
        mut offset: Bytes,
        loaded_size: u32,
        memory: &M,
    ) -> K {
        let reader = NodeReader {
            address: self.address,
            overflows: &self.overflows,
            page_size: self.page_size(),
            memory,
        };

        let size = if K::BOUND.is_fixed_size() {
            K::BOUND.max_size()
        } else {
            offset += U32_SIZE;
            loaded_size
        } as usize;

        let mut bytes = Vec::with_capacity(size);
        read_to_vec(&reader, Address::from(offset.get()), &mut bytes, size);

        // This buffer belongs only to this key. Let owning codecs reuse it
        // instead of copying it while decoding String or Vec<u8> keys. Keep
        // non-owning types on the cheaper borrowed path; this folds per type.
        if std::mem::needs_drop::<K>() {
            K::from_bytes(Cow::Owned(bytes))
        } else {
            K::from_bytes(Cow::Borrowed(&bytes))
        }
    }
    fn load_value_from_memory<M: Memory>(
        &self,
        offset: Bytes,
        loaded_size: u32,
        memory: &M,
    ) -> Vec<u8> {
        // The stored length was already read during node admission. Empty
        // values (including Set entries) need no payload reader or backing IO.
        if loaded_size == 0 {
            return Vec::new();
        }
        let reader = NodeReader {
            address: self.address,
            overflows: &self.overflows,
            page_size: self.page_size(),
            memory,
        };

        let size = loaded_size as usize;
        let mut bytes = Vec::with_capacity(size);
        read_to_vec(
            &reader,
            Address::from((offset + U32_SIZE).get()),
            &mut bytes,
            size,
        );

        bytes
    }

    const fn page_size(&self) -> PageSize {
        self.page_size
    }
    pub fn child(&self, idx: usize) -> Address {
        self.children[idx]
    }
    pub fn insert_child(&mut self, idx: usize, address: Address) {
        self.children.insert(idx, address);
    }
    pub fn push_child(&mut self, address: Address) {
        self.children.push(address);
    }
    pub fn remove_child(&mut self, idx: usize) -> Address {
        self.children.remove(idx)
    }
    pub const fn children_len(&self) -> usize {
        self.children.len()
    }
    pub fn pop_child(&mut self) -> Option<Address> {
        self.children.pop()
    }
    pub fn insert_entry(&mut self, idx: usize, (key, value): Entry<K>) {
        self.entries.insert(
            idx,
            (LazyObject::by_value(key), LazyObject::by_value(value)),
        );
    }
    pub fn extract_entry_at<M: Memory>(&mut self, idx: usize, memory: &M) -> Entry<K> {
        let (key, value) = self.entries.swap_remove(idx);
        (
            self.extract_key(key, memory),
            self.extract_value(value, memory),
        )
    }
    pub fn remove_entry<M: Memory>(&mut self, idx: usize, memory: &M) -> Entry<K> {
        let (key, value) = self.entries.remove(idx);
        (
            self.extract_key(key, memory),
            self.extract_value(value, memory),
        )
    }
    pub fn push_entry(&mut self, (key, value): Entry<K>) {
        self.entries
            .push((LazyObject::by_value(key), LazyObject::by_value(value)));
    }
    pub fn pop_entry<M: Memory>(&mut self, memory: &M) -> Option<Entry<K>> {
        let len = self.entries_len();
        if len == 0 {
            return None;
        }

        let (key, value) = self.entries.pop().expect("node must not be empty");

        Some((
            self.extract_key(key, memory),
            self.extract_value(value, memory),
        ))
    }
    pub fn merge<M: Memory>(
        &mut self,
        mut source: Self,
        median: Entry<K>,
        allocator: &mut Allocator<M>,
    ) {
        // Load all the entries from the source node first, as they will be moved out.
        for i in 0..source.entries_len() {
            source.entry(i, allocator.memory());
        }

        if source.key(0, allocator.memory()) > self.key(0, allocator.memory()) {
            // The source node has keys that are greater than self.
            // Append the source node into self.
            Self::append(self, &mut source, median, allocator.memory());
        } else {
            // self has keys that are greater than the source node.
            // Append self into the source node (which more efficient).
            Self::append(&mut source, self, median, allocator.memory());

            // Move the entries and children into self.
            self.entries = core::mem::take(&mut source.entries);
            self.children = core::mem::take(&mut source.children);
        }

        self.save(allocator);
        source.deallocate(allocator);
    }
    fn append<M: Memory>(a: &mut Self, b: &mut Self, median: Entry<K>, memory: &M) {
        // Assert preconditions.
        let a_len = a.entries_len();
        assert_eq!(a.node_type(), b.node_type());
        assert!(b.entries_len() > 0);
        assert!(a_len > 0);
        assert!(a.key(a_len - 1, memory) < &median.0);
        assert!(&median.0 < b.key(0, memory));

        a.push_entry(median);

        a.entries.append(&mut b.entries);

        // Move the children (if any exist).
        a.children.append(&mut b.children);

        // Assert postconditions.
        assert_eq!(b.entries.len(), 0);
        assert_eq!(b.children.len(), 0);
    }

    pub const fn entries_len(&self) -> usize {
        self.entries.len()
    }
    pub fn search<M: Memory>(&self, key: &K, memory: &M) -> Result<usize, usize> {
        self.entries
            .binary_search_by_key(&key, |entry| self.get_key(entry, memory))
    }
    pub fn max_size(max_key_size: u32, max_value_size: u32) -> Bytes {
        NodeHeader::size()
            + Bytes::from(CAPACITY as u64)
                * (U32_SIZE + Bytes::from(max_key_size) + Bytes::from(max_value_size) + U32_SIZE)
            + Bytes::from((CAPACITY + 1) as u64) * Address::size()
    }
    pub const fn at_minimum(&self) -> bool {
        self.entries.len() < B
    }
    pub const fn can_remove_entry_without_merging(&self) -> bool {
        !self.at_minimum()
    }
    pub fn split<M: Memory>(&mut self, sibling: &mut Self, memory: &M) -> Entry<K> {
        debug_assert!(self.is_full());

        // Load the entries that will be moved out of the node and into the new sibling.
        for idx in B..self.entries_len() {
            self.entry(idx, memory);
        }

        // Move the entries and children above the median into the new sibling.
        sibling.entries = self.entries.split_off(B);
        if self.node_type == NodeType::Internal {
            sibling.children = self.children.split_off(B);
        }

        // Return the median entry.
        self.pop_entry(memory)
            .expect("An initially full node cannot be empty")
    }
    pub fn deallocate<M: Memory>(self, allocator: &mut Allocator<M>) {
        for overflow in self.overflows {
            allocator.deallocate(overflow);
        }

        allocator.deallocate(self.address);
    }
}

// A transient data structure for reading/writing metadata into/from stable memory.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct NodeHeader {
    magic: [u8; 3],
    version: u8,
    node_type: u8,
    num_entries: u16,
}

impl NodeHeader {
    fn size() -> Bytes {
        Bytes::from(core::mem::size_of::<Self>() as u64)
    }
}
#[derive(Debug)]
enum LazyObject<T> {
    ByVal(T),
    ByRef {
        offset: Bytes,
        size: u32,
        loaded: OnceCell<T>,
    },
}

impl<T> LazyObject<T> {
    #[inline(always)]
    pub const fn by_value(value: T) -> Self {
        Self::ByVal(value)
    }

    #[inline(always)]
    pub const fn by_ref(offset: Bytes, size: u32) -> Self {
        Self::ByRef {
            offset,
            size,
            loaded: OnceCell::new(),
        }
    }

    #[inline(always)]
    pub fn get_or_load(&self, load: impl FnOnce(Bytes, u32) -> T) -> &T {
        match self {
            Self::ByVal(value) => value,
            Self::ByRef {
                offset,
                size,
                loaded,
            } => loaded.get_or_init(|| load(*offset, *size)),
        }
    }

    #[inline(always)]
    pub fn take_or_load(self, load: impl FnOnce(Bytes, u32) -> T) -> T {
        match self {
            Self::ByVal(value) => value,
            Self::ByRef {
                offset,
                size,
                loaded,
            } => loaded.into_inner().unwrap_or_else(|| load(offset, size)),
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub struct PageSize(pub u32);

impl PageSize {
    pub const fn get(self) -> u32 {
        self.0
    }
}
