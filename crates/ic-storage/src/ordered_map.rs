//! Independently authored persistent ordered tree over a caller-governed memory.
//!
//! STORMAP1 version 1 uses a 1 KiB allocation header and 128-byte AVL nodes.
//! All integers are little endian. The header stores root, entry count, allocation
//! frontier and 59 size-class free-list heads. Each block starts with kind and
//! size exponent. Node routing data stores child addresses, cached child heights
//! and counts, and key/value addresses and lengths. Keys up to 16 bytes and values
//! up to 24 bytes live in the node; larger payloads use separate reusable blocks.
//! Routing reads exclude the inline value tail. Reopening admits the small fixed
//! header and root; free-list heads are validated individually when used.
//!
//! Search and mutation use bounded AVL paths. Mutation unwind carries child
//! metadata rather than rereading siblings. Bidirectional path cursors give
//! amortized O(1) node reads per scan item, O(log n) cursor space and rank-based
//! nth skipping without value IO. Free blocks are reused within their power-of-two
//! class; classes neither split nor coalesce. Clear resets the frontier without
//! growing memory. Retained capacity is the sum of per-class high-water marks.
//!
//! No compatibility readers, journal, schema admission or native panic recovery
//! live here. The governed caller owns those authority and failure boundaries.
//! Implementation inputs: the ordered-map replacement contract, AGENTS.md,
//! TOKO-TOOLING.md, its pinned Rust hygiene/engineering policies and manifests.
//! Refinement uses black-box instruction/size results for this implementation.
//! No predecessor source, tests, layout or algorithm text was consulted.

mod iter;
mod storage;

#[cfg(test)]
mod tests;

use std::{
    borrow::Cow,
    cmp::Ordering,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
};

use crate::{
    Memory,
    storable::{Bound as CodecBound, Storable},
};
pub use iter::{Entries, Keys, Values};
use storage::{INLINE_KEY, Link, MAX_HEIGHT, Node, Store};

///
/// `OrderedMap`
///
/// Persistent ordered entries over one exclusively governed memory view.
pub struct OrderedMap<K: Storable + Ord + Clone, V: Storable, M: Memory> {
    store: Store<M>,
    types: PhantomData<fn() -> (K, V)>,
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> OrderedMap<K, V, M> {
    pub(crate) fn new(memory: M) -> Self {
        Self {
            store: Store::new(memory),
            types: PhantomData,
        }
    }

    pub(crate) fn load(memory: M) -> Option<Self> {
        Some(Self {
            store: Store::load(memory)?,
            types: PhantomData,
        })
    }

    pub(crate) const fn memory(&self) -> &M {
        &self.store.memory
    }
    pub(crate) const fn len(&self) -> u64 {
        self.store.len
    }
    pub(crate) const fn is_empty(&self) -> bool {
        self.store.len == 0
    }
    pub(crate) fn clear(&mut self) {
        self.store.clear();
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "The governed insertion interface transfers ownership of the key."
    )]
    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        let key_bytes = encode(&key);
        let value_bytes = value
            .into_bytes()
            .unwrap_or_else(|error| panic!("ordered-map encoding failed: {error:?}"));
        V::BOUND
            .check(&value_bytes)
            .expect("ordered-map codec bound mismatch");
        let (root, previous) =
            self.insert_at(self.store.root_node(), &key, &key_bytes, &value_bytes, 0);
        if previous.is_none() {
            self.store.root = root.address;
            self.store.len = self
                .store
                .len
                .checked_add(1)
                .expect("ordered-map length overflow");
            self.store.commit();
        }
        previous
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<V> {
        let (root, previous) = self.remove_at(self.store.root_node(), key, 0);
        if previous.is_some() {
            self.store.root = root.address;
            self.store.len = self
                .store
                .len
                .checked_sub(1)
                .expect("invalid ordered-map length");
            self.store.commit();
        }
        previous
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        self.find(key).map(|node| self.value(node))
    }
    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.find(key).is_some()
    }

    pub(crate) fn first_key_value(&self) -> Option<(K, V)> {
        self.pair_at(0)
    }
    pub(crate) fn last_key_value(&self) -> Option<(K, V)> {
        self.len()
            .checked_sub(1)
            .and_then(|rank| self.pair_at(rank))
    }

    pub(crate) const fn iter(&self) -> Entries<'_, K, V, M> {
        Entries::new(self, 0, self.len())
    }
    pub(crate) fn range(&self, range: impl RangeBounds<K>) -> Entries<'_, K, V, M> {
        let (front, back) = self.range_ranks(&range);
        Entries::new(self, front, back)
    }
    pub(crate) const fn keys(&self) -> Keys<'_, K, V, M> {
        Keys::new(self, 0, self.len())
    }
    pub(crate) fn keys_range(&self, range: impl RangeBounds<K>) -> Keys<'_, K, V, M> {
        let (front, back) = self.range_ranks(&range);
        Keys::new(self, front, back)
    }
    pub(crate) const fn values(&self) -> Values<'_, K, V, M> {
        Values::new(self, 0, self.len())
    }

    fn key(&self, node: Node) -> K {
        check_length::<K>(node.key_len);
        if node.key == 0 {
            let length = usize::try_from(node.key_len).expect("invalid inline key length");
            assert!(length <= INLINE_KEY, "invalid inline key length");
            decode(Cow::Borrowed(&node.inline_key[..length]))
        } else {
            decode(Cow::Owned(self.store.blob(node.key, node.key_len)))
        }
    }

    fn value(&self, node: Node) -> V {
        decode(Cow::Owned(self.value_bytes(node)))
    }

    fn value_bytes(&self, node: Node) -> Vec<u8> {
        check_length::<V>(node.value_len);
        let bytes = self.store.value_bytes(node);
        V::BOUND
            .check(&bytes)
            .expect("ordered-map stored codec bound mismatch");
        bytes
    }

    fn find(&self, key: &K) -> Option<Node> {
        let mut current = self.store.root_node();
        let mut depth = 0;
        while let Some(node) = current {
            check_depth(depth);
            match key.cmp(&self.key(node)) {
                Ordering::Equal => return Some(node),
                Ordering::Less => current = self.store.child_node(node.left),
                Ordering::Greater => current = self.store.child_node(node.right),
            }
            depth += 1;
        }
        None
    }

    fn pair_at(&self, rank: u64) -> Option<(K, V)> {
        if rank >= self.len() {
            return None;
        }
        let node = self.select(rank);
        Some((self.key(node), self.value(node)))
    }

    fn select(&self, mut rank: u64) -> Node {
        let mut current = self.store.root_node();
        for _ in 0..MAX_HEIGHT {
            let node = current.expect("invalid ordered-map subtree count");
            assert!(rank < node.link().count, "invalid ordered-map rank");
            match rank.cmp(&node.left.count) {
                Ordering::Equal => return node,
                Ordering::Less => current = self.store.child_node(node.left),
                Ordering::Greater => {
                    rank -= node.left.count + 1;
                    current = self.store.child_node(node.right);
                }
            }
        }
        panic!("ordered-map traversal exceeds AVL height bound");
    }

    fn rank(&self, key: &K, after: bool) -> u64 {
        let mut current = self.store.root_node();
        let mut result = 0_u64;
        let mut depth = 0;
        while let Some(node) = current {
            check_depth(depth);
            let ordering = self.key(node).cmp(key);
            if ordering == Ordering::Less || (after && ordering == Ordering::Equal) {
                result = result
                    .checked_add(node.left.count)
                    .and_then(|n| n.checked_add(1))
                    .expect("ordered-map rank overflow");
                current = self.store.child_node(node.right);
            } else {
                current = self.store.child_node(node.left);
            }
            depth += 1;
        }
        assert!(result <= self.len(), "invalid ordered-map rank");
        result
    }

    fn range_ranks(&self, range: &impl RangeBounds<K>) -> (u64, u64) {
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
        let front = match start {
            Bound::Unbounded => 0,
            Bound::Included(key) => self.rank(key, false),
            Bound::Excluded(key) => self.rank(key, true),
        };
        let back = match end {
            Bound::Unbounded => self.len(),
            Bound::Included(key) => self.rank(key, true),
            Bound::Excluded(key) => self.rank(key, false),
        };
        (front.min(back), back)
    }

    fn balance(&self, mut node: Node) -> Link {
        if node.left.height > node.right.height + 1 {
            let left = self
                .store
                .child_node(node.left)
                .expect("missing rotation child");
            if left.right.height > left.left.height {
                node.left = self.rotate_left(left).link();
            }
            self.rotate_right(node).link()
        } else if node.right.height > node.left.height + 1 {
            let right = self
                .store
                .child_node(node.right)
                .expect("missing rotation child");
            if right.left.height > right.right.height {
                node.right = self.rotate_right(right).link();
            }
            self.rotate_left(node).link()
        } else {
            self.store.write_node(node);
            node.link()
        }
    }

    fn rotate_left(&self, mut node: Node) -> Node {
        let mut next = self
            .store
            .child_node(node.right)
            .expect("missing rotation child");
        node.right = next.left;
        self.store.write_node(node);
        next.left = node.link();
        self.store.write_node(next);
        next
    }

    fn rotate_right(&self, mut node: Node) -> Node {
        let mut next = self
            .store
            .child_node(node.left)
            .expect("missing rotation child");
        node.left = next.right;
        self.store.write_node(node);
        next.right = node.link();
        self.store.write_node(next);
        next
    }

    fn insert_at(
        &mut self,
        root: Option<Node>,
        key: &K,
        key_bytes: &[u8],
        value: &[u8],
        depth: u64,
    ) -> (Link, Option<V>) {
        check_depth(depth);
        let Some(mut node) = root else {
            return (self.store.new_node(key_bytes, value).link(), None);
        };
        let (updated, previous) = match key.cmp(&self.key(node)) {
            Ordering::Less => {
                let result = self.insert_at(
                    self.store.child_node(node.left),
                    key,
                    key_bytes,
                    value,
                    depth + 1,
                );
                node.left = result.0;
                result
            }
            Ordering::Greater => {
                let result = self.insert_at(
                    self.store.child_node(node.right),
                    key,
                    key_bytes,
                    value,
                    depth + 1,
                );
                node.right = result.0;
                result
            }
            Ordering::Equal => {
                let previous = self.value(node);
                self.store.replace_value(&mut node, value);
                return (node.link(), Some(previous));
            }
        };
        let _ = updated;
        if previous.is_some() {
            (node.link(), previous)
        } else {
            (self.balance(node), None)
        }
    }

    fn remove_at(&mut self, root: Option<Node>, key: &K, depth: u64) -> (Link, Option<V>) {
        check_depth(depth);
        let Some(mut node) = root else {
            return (Link::default(), None);
        };
        let previous = match key.cmp(&self.key(node)) {
            Ordering::Less => {
                let (left, previous) =
                    self.remove_at(self.store.child_node(node.left), key, depth + 1);
                node.left = left;
                previous
            }
            Ordering::Greater => {
                let (right, previous) =
                    self.remove_at(self.store.child_node(node.right), key, depth + 1);
                node.right = right;
                previous
            }
            Ordering::Equal => {
                let previous = self.value(node);
                self.store.free_blob(node.key);
                self.store.free_blob(node.value);
                if node.left.address == 0 || node.right.address == 0 {
                    self.store.free_node(node);
                    return (
                        if node.left.address == 0 {
                            node.right
                        } else {
                            node.left
                        },
                        Some(previous),
                    );
                }
                let (right, successor) = self.detach_first(
                    self.store
                        .child_node(node.right)
                        .expect("missing successor subtree"),
                    depth + 1,
                );
                node.right = right;
                self.store.move_payload(&mut node, successor);
                Some(previous)
            }
        };
        if previous.is_none() {
            (node.link(), None)
        } else {
            (self.balance(node), previous)
        }
    }

    fn detach_first(&mut self, mut node: Node, depth: u64) -> (Link, Node) {
        check_depth(depth);
        if node.left.address == 0 {
            self.store.free_node(node);
            (node.right, node)
        } else {
            let (left, removed) = self.detach_first(
                self.store
                    .child_node(node.left)
                    .expect("missing successor child"),
                depth + 1,
            );
            node.left = left;
            (self.balance(node), removed)
        }
    }
}

pub fn decode<T: Storable>(bytes: Cow<'_, [u8]>) -> T {
    T::BOUND
        .check(&bytes)
        .expect("ordered-map codec bound mismatch");
    T::from_bytes(bytes).unwrap_or_else(|error| panic!("ordered-map decoding failed: {error:?}"))
}

fn encode<T: Storable>(value: &T) -> Cow<'_, [u8]> {
    let bytes = value
        .to_bytes()
        .unwrap_or_else(|error| panic!("ordered-map encoding failed: {error:?}"));
    T::BOUND
        .check(&bytes)
        .expect("ordered-map codec bound mismatch");
    bytes
}

fn check_length<T: Storable>(length: u64) {
    if let CodecBound::Bounded {
        max_size,
        is_fixed_size,
    } = T::BOUND
    {
        assert!(
            length <= u64::from(max_size) && (!is_fixed_size || length == u64::from(max_size)),
            "ordered-map stored codec width mismatch"
        );
    }
}

fn check_depth(depth: u64) {
    assert!(
        depth < MAX_HEIGHT,
        "ordered-map traversal exceeds AVL height bound"
    );
}
