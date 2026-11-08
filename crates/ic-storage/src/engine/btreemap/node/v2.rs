// Adapted from ic-stable-structures 0.7.2, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See the package LICENSE-APACHE and docs/storage/icss-source.json.
//! Node V2
//!
//! A v2 node is an iteration on a v1 node. Compared to a v1 node, it has a
//! smaller memory footprint and support for unbounded types.
//!
//! # Memory Layout
//!
//! To support unbounded types, a v2 node is variable in size and can span multiple
//! pages of memory if needed [^note]. There are two types of pages:
//!
//! 1. Initial Page (the first page of the node)
//! 2. Overflow page (subsequent pages of the node)
//!
//! ## Initial Page Memory Layout
//!
//! ```text
//! ---------------------------------------- <-- Header
//! Magic "BTN"             ↕ 3 bytes
//! ----------------------------------------
//! Layout version (2)      ↕ 1 byte
//! ----------------------------------------
//! Node type               ↕ 1 byte
//! ----------------------------------------
//! # Entries (k)           ↕ 2 bytes
//! ----------------------------------------
//! Overflow address        ↕ 8 bytes
//! ---------------------------------------- <-- Children (Address 15)
//! Child(0) address        ↕ 8 bytes
//! ----------------------------------------
//! ...
//! ----------------------------------------
//! Child(k + 1) address    ↕ 8 bytes
//! ---------------------------------------- <-- Keys
//! Key(0)
//! ----------------------------------------
//! ...
//! ----------------------------------------
//! Key(k)
//! ---------------------------------------- <-- Values
//! Value(0)
//! ----------------------------------------
//! ...
//! ----------------------------------------
//! Value(k)
//! ----------------------------------------
//! ```
//!
//! ## Overflow Page Memory Layout
//!
//! If the data to be stored in the initial page layout is larger than the page size,
//! then overflow pages are used.
//!
//! ```text
//! ----------------------------------------
//! Magic "NOF"             ↕ 3 bytes
//! ----------------------------------------
//! Next address            ↕ 8 byte
//! ----------------------------------------
//! Data
//! ----------------------------------------
//! ```
//!
//! ## Keys and Values
//! Keys and values are both encoded in memory as blobs.
//!
//! If they are variable in size (i.e. their `IS_FIXED` attribute is set to false),
//! then the size of the blob is encoded before the blob itself. Otherwise, no size
//! information is stored.
//!
//! [^note]: The page here refers to a fixed-size chunk of memory that is provided to the
//! node by the `BTreeMap` Allocator, and has no connection with OS memory pages or
//! Wasm pages.

use super::{
    Address, Borrow, Bytes, Cow, INTERNAL_NODE_TYPE, LAYOUT_VERSION_2, LEAF_NODE_TYPE, LazyObject,
    MAGIC, Memory, Node, NodeHeader, NodeReader, NodeType, PageSize, Storable, U32_SIZE,
    read_address_vec, read_struct, read_to_vec, read_u32, read_u64,
};
use crate::engine::btreemap::Allocator;
use crate::engine::{btreemap::node::io::NodeWriter, types::NULL};

// Initial page
const EAGER_LOAD_KEY_SIZE_THRESHOLD: u32 = 16;
pub(super) const OVERFLOW_ADDRESS_OFFSET: Bytes = Bytes::new(7);
const ENTRIES_OFFSET: Bytes = Bytes::new(15);

// Overflow page
pub(super) const OVERFLOW_MAGIC: &[u8; 3] = b"NOF";
pub(super) const PAGE_OVERFLOW_NEXT_OFFSET: Bytes = Bytes::new(3);
pub(super) const PAGE_OVERFLOW_DATA_OFFSET: Bytes = Bytes::new(11);

// The minimum size a page can have.
// Rationale: a page size needs to at least store the header (15 bytes) + all the children
// addresses (88 bytes). We round that up to 128 to get a nice binary number.
const MINIMUM_PAGE_SIZE: u32 = 128;

impl<K: Storable + Ord + Clone> Node<K> {
    pub fn new_v2(address: Address, node_type: NodeType, page_size: PageSize) -> Self {
        assert!(page_size.get() >= MINIMUM_PAGE_SIZE);

        Self {
            address,
            node_type,
            page_size,
            entries: vec![],
            children: vec![],
            overflows: Vec::new(),
        }
    }
    pub(super) fn load_v2<M: Memory>(
        address: Address,
        page_size: PageSize,
        header: NodeHeader,
        memory: &M,
    ) -> Self {
        // Load the node, including any overflows, into a buffer.
        let overflows = read_overflows(address, memory);

        let reader = NodeReader {
            address,
            overflows: &overflows,
            page_size,
            memory,
        };

        let mut offset = Address::from(0);

        // Load the node type.
        let node_type = match header.node_type {
            LEAF_NODE_TYPE => NodeType::Leaf,
            INTERNAL_NODE_TYPE => NodeType::Internal,
            other => unreachable!("Unknown node type {}", other),
        };

        // Load the number of entries
        let num_entries = header.num_entries as usize;

        // Load children if this is an internal node.
        offset += ENTRIES_OFFSET;
        let children = if node_type == NodeType::Internal {
            // Empirical constant from benchmarks: individual reads are faster for ≤4 children.
            // From 5 up, batch reads consistently perform better.
            const CHILDREN_BATCH_READ_THRESHOLD: usize = 4;
            let count = num_entries + 1;
            if count <= CHILDREN_BATCH_READ_THRESHOLD {
                // For very small counts, use individual reads to avoid allocation overhead
                let mut children = Vec::with_capacity(count);
                for _ in 0..count {
                    children.push(Address::from(read_u64(&reader, offset)));
                    offset += Address::size();
                }
                children
            } else {
                // For larger counts, use the optimized batch read
                let children = read_address_vec(&reader, offset, count);
                offset += Bytes::from(count) * Address::size();
                children
            }
        } else {
            vec![]
        };

        // Load the keys (eagerly if small).
        let mut entries = Vec::with_capacity(num_entries);
        let mut buf = vec![];
        // Fixed-width keys are adjacent in the node image. Read the same eager
        // key bytes once, retaining per-key decoding and lazy larger keys.
        // NodeReader still translates any overflow-page crossings.
        let batch_key_size =
            if K::BOUND.is_fixed_size() && K::BOUND.max_size() <= EAGER_LOAD_KEY_SIZE_THRESHOLD {
                let size = K::BOUND.max_size() as usize;
                if size != 0 && num_entries != 0 {
                    read_to_vec(&reader, offset, &mut buf, num_entries * size);
                }
                Some(size)
            } else {
                None
            };

        for index in 0..num_entries {
            let key_offset = Bytes::from(offset.get());

            // Get key size.
            let key_size = if K::BOUND.is_fixed_size() {
                K::BOUND.max_size()
            } else {
                let size = read_u32(&reader, offset);
                offset += U32_SIZE;
                size
            };

            // Eager-load small keys, defer large ones.
            let key = if let Some(size) = batch_key_size {
                let start = index * size;
                LazyObject::by_value(K::from_bytes(Cow::Borrowed(&buf[start..start + size])))
            } else if key_size <= EAGER_LOAD_KEY_SIZE_THRESHOLD {
                read_to_vec(
                    &reader,
                    Address::from(offset.get()),
                    &mut buf,
                    key_size as usize,
                );
                LazyObject::by_value(K::from_bytes(Cow::Borrowed(&buf)))
            } else {
                LazyObject::by_ref(key_offset, key_size)
            };

            offset += Bytes::from(key_size);
            entries.push((key, LazyObject::by_ref(Bytes::from(0_u64), 0)));
        }

        // Load the values
        for (_key, value) in &mut entries {
            // Load the values lazily.
            let value_size = read_u32(&reader, offset);
            *value = LazyObject::by_ref(Bytes::from(offset.get()), value_size);
            offset += U32_SIZE + Bytes::from(u64::from(value_size));
        }

        Self {
            address,
            entries,
            children,
            node_type,
            page_size,
            overflows,
        }
    }

    // Saves the node to memory.
    pub(super) fn save_v2<M: Memory>(&mut self, allocator: &mut Allocator<M>) {
        let page_size = self.page_size.get();
        assert!(page_size >= MINIMUM_PAGE_SIZE);

        // Load all the entries. One pass is required to load all entries;
        // results are not stored to avoid unnecessary allocations.
        for i in 0..self.entries.len() {
            self.entry(i, allocator.memory());
        }

        // Initialize a NodeWriter. The NodeWriter takes care of allocating/deallocating
        // overflow pages as needed.
        let mut writer = NodeWriter::new(
            self.address,
            self.overflows.clone(),
            self.page_size(),
            allocator,
        );

        // Write the header of the initial page.
        let mut offset = Address::from(0);
        let header = NodeHeader {
            magic: *MAGIC,
            version: LAYOUT_VERSION_2,
            node_type: match self.node_type {
                NodeType::Leaf => LEAF_NODE_TYPE,
                NodeType::Internal => INTERNAL_NODE_TYPE,
            },
            num_entries: u16::try_from(self.entries.len()).expect("node entry count"),
        };

        writer.write_struct(&header, offset);
        offset += NodeHeader::size();

        // Add a null overflow address.
        // This might get overwritten later in case the node does overflow.
        writer.write_u64(offset, self.overflows.first().unwrap_or(&NULL).get());
        offset += Bytes::from(8u64);

        // Write the children.
        let count = self.children.len();
        let byte_len = count * usize::try_from(Address::size().get()).expect("address width");
        let mut bytes = Vec::with_capacity(byte_len);
        for child in &self.children {
            bytes.extend_from_slice(&child.get().to_le_bytes());
        }
        writer.write(offset, &bytes);
        offset += Bytes::from(byte_len);

        // Write the keys.
        for i in 0..self.entries.len() {
            let key = self.key(i, writer.memory());
            let key_bytes = key.to_bytes();

            // Write the size of the key if it isn't fixed in size.
            if !K::BOUND.is_fixed_size() {
                writer.write_u32(
                    offset,
                    u32::try_from(key_bytes.len()).expect("encoded key length"),
                );
                offset += U32_SIZE;
            }

            // Write the key.
            writer.write(offset, key_bytes.borrow());
            offset += Bytes::from(key_bytes.len());
        }

        // Write the values.
        for i in 0..self.entries.len() {
            // Write the size of the value.
            let value = self.value(i, writer.memory());
            writer.write_u32(
                offset,
                u32::try_from(value.len()).expect("encoded value length"),
            );
            offset += U32_SIZE;

            // Write the value.
            writer.write(offset, value);
            offset += Bytes::from(value.len());
        }

        self.overflows = writer.finish();
    }
}

fn read_overflows<M: Memory>(address: Address, memory: &M) -> Vec<Address> {
    #[repr(C, packed)]
    struct OverflowPageHeader {
        magic: [u8; 3],
        next: Address,
    }

    let mut overflows = vec![];
    let mut overflow = Address::from(read_u64(memory, address + OVERFLOW_ADDRESS_OFFSET));
    while overflow != NULL {
        overflows.push(overflow);

        let header: OverflowPageHeader = read_struct(overflow, memory);
        assert_eq!(&header.magic, OVERFLOW_MAGIC, "Bad overflow magic.");
        overflow = header.next;
    }

    overflows
}
