//! Checked blocks and persistent size-class reuse for the ordered tree.

use crate::Memory;

use super::EncodedValue;

const HEADER_SIZE: usize = 1024;
const PAGE_SIZE: u64 = 65_536;
const MAGIC: &[u8; 8] = b"STORMAP1";
const VERSION: u64 = 1;
const MIN_CLASS: usize = 5;
const CLASS_COUNT: usize = 59;
const BLOCK_HEADER: u64 = 16;
const FREE: u64 = 0x4652_4545;
const BLOB: u64 = 0x424c_4f42;
const NODE: u64 = 0x4e4f_4445;
const NODE_CLASS: usize = 7;
const NODE_BYTES: u64 = 128;
const NODE_ROUTING_BYTES: usize = 104;
const VALUE_OFFSET: u64 = NODE_ROUTING_BYTES as u64;
pub(super) const INLINE_KEY: usize = 16;
pub(super) const INLINE_VALUE: usize = 24;
pub(super) const MAX_HEIGHT: u64 = 128;

#[derive(Clone, Copy, Default)]
pub(super) struct Link {
    pub address: u64,
    pub height: u64,
    pub count: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Node {
    pub address: u64,
    pub left: Link,
    pub right: Link,
    pub key: u64,
    pub key_len: u64,
    pub value: u64,
    pub value_len: u64,
    pub inline_key: [u8; INLINE_KEY],
}

impl Node {
    pub fn link(self) -> Link {
        Link {
            address: self.address,
            height: self.left.height.max(self.right.height) + 1,
            count: self
                .left
                .count
                .checked_add(self.right.count)
                .and_then(|n| n.checked_add(1))
                .expect("ordered-map subtree count overflow"),
        }
    }
}

pub(super) struct Store<M> {
    pub memory: M,
    pub root: u64,
    pub len: u64,
    end: u64,
}

impl<M: Memory> Store<M> {
    pub fn new(memory: M) -> Self {
        ensure_capacity(&memory, HEADER_SIZE as u64);
        let mut bytes = [0; HEADER_SIZE];
        bytes[..8].copy_from_slice(MAGIC);
        put(&mut bytes, 8, VERSION);
        put(&mut bytes, 32, HEADER_SIZE as u64);
        memory.write(0, &bytes);
        Self {
            memory,
            root: 0,
            len: 0,
            end: HEADER_SIZE as u64,
        }
    }

    pub fn load(memory: M) -> Option<Self> {
        let capacity = memory.size().checked_mul(PAGE_SIZE)?;
        if capacity < HEADER_SIZE as u64 {
            return None;
        }
        // Allocation bins are admitted individually when used, not on read-only opens.
        let mut bytes = [0; 64];
        memory.read(0, &mut bytes);
        if &bytes[..8] != MAGIC || word(&bytes, 8) != VERSION {
            return None;
        }
        let root = word(&bytes, 16);
        let len = word(&bytes, 24);
        let end = word(&bytes, 32);
        if end < HEADER_SIZE as u64
            || end > capacity
            || !end.is_multiple_of(32)
            || (root == 0) != (len == 0)
            || len > (end - HEADER_SIZE as u64) / NODE_BYTES
            || bytes[40..64].iter().any(|byte| *byte != 0)
        {
            return None;
        }
        let store = Self {
            memory,
            root,
            len,
            end,
        };
        if root != 0 && store.read_node(root)?.link().count != len {
            return None;
        }
        Some(store)
    }

    pub fn commit(&self) {
        let mut bytes = [0; 16];
        put(&mut bytes, 0, self.root);
        put(&mut bytes, 8, self.len);
        self.memory.write(16, &bytes);
    }

    pub fn clear(&mut self) {
        // Reset the allocation frontier; stale capacity is never interpreted as metadata.
        self.root = 0;
        self.len = 0;
        self.end = HEADER_SIZE as u64;
        let mut bytes = [0; HEADER_SIZE - 16];
        put(&mut bytes, 16, self.end);
        self.memory.write(16, &bytes);
    }

    pub fn node(&self, address: u64) -> Node {
        self.read_node(address).expect("invalid ordered-map node")
    }

    pub fn root_node(&self) -> Option<Node> {
        if self.root == 0 {
            assert_eq!(self.len, 0, "invalid ordered-map root count");
            return None;
        }
        let node = self.node(self.root);
        assert_eq!(
            node.link().count,
            self.len,
            "invalid ordered-map root count"
        );
        Some(node)
    }

    pub fn child_node(&self, expected: Link) -> Option<Node> {
        if expected.address == 0 {
            assert!(
                expected.height == 0 && expected.count == 0,
                "invalid empty ordered-map link"
            );
            return None;
        }
        let node = self.node(expected.address);
        let actual = node.link();
        // The parent cache is routing authority only after agreement with the
        // child at its already-required read; no extra subtree read is needed.
        assert!(
            actual.height == expected.height && actual.count == expected.count,
            "inconsistent ordered-map child metadata"
        );
        Some(node)
    }

    fn read_node(&self, address: u64) -> Option<Node> {
        if !valid_pointer(address, NODE_BYTES, self.end) {
            return None;
        }
        // The inline value tail is deliberately excluded from routing and key scans.
        let mut bytes = [0; NODE_ROUTING_BYTES];
        self.memory.read(address, &mut bytes);
        if word(&bytes, 0) != NODE || word(&bytes, 8) != NODE_CLASS as u64 {
            return None;
        }
        let heights = word(&bytes, 32);
        if heights >> 16 != 0 {
            return None;
        }
        let left = Link {
            address: word(&bytes, 16),
            height: heights & 255,
            count: word(&bytes, 72),
        };
        let right = Link {
            address: word(&bytes, 24),
            height: heights >> 8,
            count: word(&bytes, 80),
        };
        for child in [left, right] {
            if child.address == 0 {
                if child.height != 0 || child.count != 0 {
                    return None;
                }
            } else if child.address == address
                || !valid_pointer(child.address, NODE_BYTES, self.end)
                || !(1..MAX_HEIGHT).contains(&child.height)
                || child.count == 0
                || child.count > (self.end - HEADER_SIZE as u64) / NODE_BYTES
            {
                return None;
            }
        }
        if left.address != 0 && left.address == right.address {
            return None;
        }
        let count = left.count.checked_add(right.count)?.checked_add(1)?;
        if count > (self.end - HEADER_SIZE as u64) / NODE_BYTES {
            return None;
        }
        let mut inline_key = [0; INLINE_KEY];
        inline_key.copy_from_slice(&bytes[88..104]);
        let node = Node {
            address,
            left,
            right,
            key: word(&bytes, 40),
            key_len: word(&bytes, 48),
            value: word(&bytes, 56),
            value_len: word(&bytes, 64),
            inline_key,
        };
        if !self.valid_blob_reference(node.key, node.key_len, INLINE_KEY)
            || !self.valid_blob_reference(node.value, node.value_len, INLINE_VALUE)
        {
            return None;
        }
        Some(node)
    }

    fn check_node_write(&self, node: Node) {
        // Callers hold a previously admitted node or a newly allocated node block.
        assert!(
            valid_pointer(node.address, NODE_BYTES, self.end),
            "invalid ordered-map node extent"
        );
        assert!(
            node.link().height <= MAX_HEIGHT,
            "ordered-map height overflow"
        );
    }

    pub fn write_node(&self, node: Node) {
        self.check_node_write(node);
        let mut bytes = [0; NODE_ROUTING_BYTES];
        for (index, value) in [
            NODE,
            NODE_CLASS as u64,
            node.left.address,
            node.right.address,
            node.left.height | (node.right.height << 8),
            node.key,
            node.key_len,
            node.value,
            node.value_len,
            node.left.count,
            node.right.count,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut bytes, index * 8, value);
        }
        bytes[88..104].copy_from_slice(&node.inline_key);
        self.memory.write(node.address, &bytes);
    }

    pub fn new_node(&mut self, key: &[u8], value: &[u8]) -> Node {
        // Keep routing and overflow key blocks adjacent on fresh allocation.
        // Large values must not separate them into different cold memory pages.
        // Reuse, pointer validation and the STORMAP1 layout remain unchanged.
        let address = self.allocate(NODE_CLASS, NODE);
        let key_address = if key.len() > INLINE_KEY {
            self.new_blob(key)
        } else {
            0
        };
        let value_address = if value.len() > INLINE_VALUE {
            self.new_blob(value)
        } else {
            0
        };
        let mut inline_key = [0; INLINE_KEY];
        if key.len() <= INLINE_KEY {
            inline_key[..key.len()].copy_from_slice(key);
        }
        let node = Node {
            address,
            left: Link::default(),
            right: Link::default(),
            key: key_address,
            key_len: key.len() as u64,
            value: value_address,
            value_len: value.len() as u64,
            inline_key,
        };
        self.write_node(node);
        if value_address == 0 {
            self.memory.write(address + VALUE_OFFSET, value);
        }
        node
    }

    pub fn free_node(&self, node: Node) {
        // Removal passes a node admitted during this exclusive traversal. No
        // intervening operation changes its NODE identity or fixed size class;
        // later removals must readmit storage rather than reuse a saved Node.
        assert!(
            valid_pointer(node.address, NODE_BYTES, self.end),
            "invalid ordered-map node extent"
        );
        self.release_admitted(node.address, NODE_CLASS);
    }

    pub fn value_bytes(&self, node: Node) -> EncodedValue {
        if node.value == 0 {
            let length = usize::try_from(node.value_len).expect("invalid inline value length");
            assert!(length <= INLINE_VALUE, "invalid inline value length");
            let mut bytes = [0; INLINE_VALUE];
            self.memory
                .read(node.address + VALUE_OFFSET, &mut bytes[..length]);
            EncodedValue::Inline {
                bytes,
                length: u8::try_from(length).expect("inline value length fits byte"),
            }
        } else {
            EncodedValue::Heap(self.blob(node.value, node.value_len))
        }
    }

    pub fn admit_value(&self, node: Node) {
        if node.value != 0 {
            let class = self.block(node.value, BLOB);
            assert!(
                node.value_len <= (1_u64 << class) - BLOCK_HEADER,
                "oversized ordered-map blob"
            );
        }
    }

    pub fn replace_value(&mut self, node: &mut Node, bytes: &[u8]) {
        let previous = (node.value, node.value_len);
        if bytes.len() <= INLINE_VALUE {
            self.memory.write(node.address + VALUE_OFFSET, bytes);
            self.free_blob(node.value);
            node.value = 0;
        } else {
            node.value = self.replace_blob(node.value, bytes);
        }
        node.value_len = bytes.len() as u64;
        if previous == (node.value, node.value_len) {
            // The payload write suffices when its saved routing fields agree.
            // Retain the same node checks even though no metadata is published.
            self.check_node_write(*node);
        } else {
            self.write_node(*node);
        }
    }

    pub fn move_payload(&self, target: &mut Node, source: Node) {
        // The successor's block can be freed before this transfer: its inline tail
        // is untouched by the free-list prefix, and no allocation occurs in between.
        if source.value == 0 {
            let bytes = self.value_bytes(source);
            self.memory
                .write(target.address + VALUE_OFFSET, bytes.as_slice());
        }
        target.key = source.key;
        target.key_len = source.key_len;
        target.inline_key = source.inline_key;
        target.value = source.value;
        target.value_len = source.value_len;
    }

    pub fn blob(&self, address: u64, length: u64) -> Vec<u8> {
        assert!(
            address != 0 && self.valid_blob_reference(address, length, 0),
            "invalid ordered-map blob range"
        );
        let class = self.block(address, BLOB);
        assert!(
            length <= (1_u64 << class) - BLOCK_HEADER,
            "oversized ordered-map blob"
        );
        let length = usize::try_from(length).expect("ordered-map blob exceeds host address space");
        let mut bytes = Vec::with_capacity(length);
        // SAFETY: fresh capacity is exclusively writable and cannot overlap
        // backing. The checked source is read into that capacity; read_unsafe
        // initializes every byte on success. Keep length zero through the call
        // so a panic after partial initialization exposes no unread bytes.
        #[expect(
            unsafe_code,
            reason = "Memory initializes exclusive Vec capacity before its length is published."
        )]
        unsafe {
            self.memory
                .read_unsafe(address + BLOCK_HEADER, bytes.as_mut_ptr(), length);
            bytes.set_len(length);
        }
        bytes
    }

    fn new_blob(&mut self, bytes: &[u8]) -> u64 {
        let class = blob_class(bytes.len());
        let address = self.allocate(class, BLOB);
        self.memory.write(address + BLOCK_HEADER, bytes);
        address
    }

    fn replace_blob(&mut self, address: u64, bytes: &[u8]) -> u64 {
        if address != 0 && self.block(address, BLOB) == blob_class(bytes.len()) {
            self.memory.write(address + BLOCK_HEADER, bytes);
            return address;
        }
        let replacement = self.new_blob(bytes);
        self.free_blob(address);
        replacement
    }

    pub fn free_blob(&self, address: u64) {
        if address != 0 {
            self.release(address, BLOB);
        }
    }

    fn valid_blob_reference(&self, address: u64, length: u64, inline: usize) -> bool {
        if address == 0 {
            length <= inline as u64
        } else {
            length > inline as u64
                && length
                    .checked_add(BLOCK_HEADER)
                    .is_some_and(|size| valid_pointer(address, size, self.end))
        }
    }

    fn block(&self, address: u64, kind: u64) -> usize {
        assert!(
            valid_pointer(address, BLOCK_HEADER, self.end),
            "invalid ordered-map block"
        );
        let mut bytes = [0; 16];
        self.memory.read(address, &mut bytes);
        assert_eq!(word(&bytes, 0), kind, "invalid ordered-map block identity");
        let class = usize::try_from(word(&bytes, 8)).expect("invalid block class");
        assert!(
            (MIN_CLASS..MIN_CLASS + CLASS_COUNT).contains(&class),
            "invalid block class"
        );
        assert!(
            valid_pointer(address, 1_u64 << class, self.end),
            "invalid block extent"
        );
        class
    }

    fn free_head(&self, class: usize) -> u64 {
        let mut bytes = [0; 8];
        self.memory.read(free_head_offset(class), &mut bytes);
        let address = u64::from_le_bytes(bytes);
        assert!(
            address == 0 || valid_pointer(address, 1_u64 << class, self.end),
            "invalid free-list head"
        );
        address
    }

    fn allocate(&mut self, class: usize, kind: u64) -> u64 {
        let head = self.free_head(class);
        let address = if head == 0 {
            let address = self.end;
            let end = address
                .checked_add(1_u64 << class)
                .expect("ordered-map address overflow");
            ensure_capacity(&self.memory, end);
            self.end = end;
            self.memory.write(32, &end.to_le_bytes());
            address
        } else {
            let mut bytes = [0; 24];
            self.memory.read(head, &mut bytes);
            assert_eq!(word(&bytes, 0), FREE, "invalid free block identity");
            assert_eq!(word(&bytes, 8), class as u64, "invalid free block class");
            let next = word(&bytes, 16);
            assert!(
                next == 0 || (next != head && valid_pointer(next, 1_u64 << class, self.end)),
                "invalid free block link"
            );
            self.memory
                .write(free_head_offset(class), &next.to_le_bytes());
            head
        };
        // Claim every allocation immediately. A node can share its size class
        // with later overflow payloads; leaving its old FREE identity until
        // node initialization would admit it twice through a corrupt free cycle.
        let mut bytes = [0; 16];
        put(&mut bytes, 0, kind);
        put(&mut bytes, 8, class as u64);
        self.memory.write(address, &bytes);
        address
    }

    fn release(&self, address: u64, kind: u64) {
        let class = self.block(address, kind);
        self.release_admitted(address, class);
    }

    fn release_admitted(&self, address: u64, class: usize) {
        let head = self.free_head(class);
        assert_ne!(address, head, "double free of ordered-map block");
        let mut bytes = [0; 24];
        put(&mut bytes, 0, FREE);
        put(&mut bytes, 8, class as u64);
        put(&mut bytes, 16, head);
        self.memory.write(address, &bytes);
        self.memory
            .write(free_head_offset(class), &address.to_le_bytes());
    }
}

fn blob_class(length: usize) -> usize {
    let size = u64::try_from(length)
        .expect("ordered-map blob too large")
        .checked_add(BLOCK_HEADER)
        .and_then(u64::checked_next_power_of_two)
        .expect("ordered-map blob size overflow")
        .max(1 << MIN_CLASS);
    let class = usize::try_from(size.trailing_zeros()).expect("block class overflow");
    assert!(
        class < MIN_CLASS + CLASS_COUNT,
        "ordered-map blob too large"
    );
    class
}

const fn free_head_offset(class: usize) -> u64 {
    (64 + (class - MIN_CLASS) * 8) as u64
}

fn ensure_capacity(memory: &impl Memory, end: u64) {
    let pages = end.div_ceil(PAGE_SIZE);
    let current = memory.size();
    if pages > current {
        assert!(
            memory.grow(pages - current) >= 0,
            "ordered-map memory growth failed"
        );
        assert!(
            memory.size() >= pages,
            "ordered-map memory growth incomplete"
        );
    }
}

const fn valid_pointer(address: u64, size: u64, end: u64) -> bool {
    address >= HEADER_SIZE as u64
        && address.is_multiple_of(32)
        && match address.checked_add(size) {
            Some(limit) => limit <= end,
            None => false,
        }
}

fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("word width"))
}

fn put(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
