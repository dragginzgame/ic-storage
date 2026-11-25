//! Owned degree-six B-tree. The multi-entry search, split/borrow/merge design and
//! selective payload access follow the established B-tree techniques used by
//! ic-stable-structures 0.7.2 (see the historical provenance in SOURCES.md). This is a new checked
//! format, not an implementation of its durable layout or compatibility reader.
//! Routing records contain small inline values; larger values have separate
//! checksummed records. Mutations stage bytes and reserve before publication.
use super::{
    BTreeMapOptions, Memory, Storable,
    error::{CollectionError as Error, Result, ensure},
    record,
};

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::BTreeMap,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
    rc::Rc,
};
mod framing;
use framing::{decode_link, link_bytes};
mod read;
use read::ReadNode;

pub const MAGIC: [u8; 9] = *b"ICPMAP001";
const NODE: [u8; 8] = *b"ICBTND01";
const VALUE: [u8; 8] = *b"ICBTVAL1";
const FREE: [u8; 8] = *b"ICFREE01";
const BINS: usize = 32;
const MAX_DEPTH: usize = 32;
const DEGREE: usize = 6;
const MAX_ENTRIES: usize = DEGREE * 2 - 1;
const MIN_ENTRIES: usize = DEGREE - 1;
const INLINE: usize = 32;
// Order eight is the smallest block in the original pointer format.
const FIRST_ORDER: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pointer {
    offset: u64,
    order: u8,
    // Inherited from the header when decoding a pointer; never encoded per link.
    unit: u16,
}
impl Pointer {
    fn capacity(self) -> u64 {
        u64::from(self.unit) << (usize::from(self.order) - FIRST_ORDER)
    }
    fn check(self, cursor: u64) -> Result<()> {
        ensure(
            (8..32).contains(&self.order),
            Error::Corrupt("tree block order"),
        )?;
        ensure(
            self.offset >= record::HEADER_BYTES
                && self
                    .offset
                    .checked_add(self.capacity())
                    .is_some_and(|end| end <= cursor),
            Error::Corrupt("tree block bounds"),
        )
    }
    fn put(self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.offset.to_le_bytes());
        bytes.push(self.order);
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Root {
    options: BTreeMapOptions,
    top: Option<Pointer>,
    length: u64,
    cursor: u64,
    free: Vec<Option<Pointer>>,
    key_bound: Option<(u32, bool)>,
    value_bound: Option<(u32, bool)>,
}
#[derive(Clone)]
enum Value {
    Inline(Vec<u8>),
    External(Pointer),
}
#[derive(Clone)]
struct Entry {
    key: Vec<u8>,
    value: Value,
}
#[derive(Clone, Default)]
struct Node {
    entries: Vec<Entry>,
    children: Vec<Pointer>,
}
impl Node {
    fn encode(&self) -> Result<Vec<u8>> {
        ensure(
            self.entries.len() <= MAX_ENTRIES,
            Error::Corrupt("B-tree node capacity"),
        )?;
        ensure(
            self.children.is_empty() || self.children.len() == self.entries.len() + 1,
            Error::Corrupt("B-tree child count"),
        )?;
        let capacity =
            self.entries
                .iter()
                .try_fold(2 + self.children.len() * 9, |size, entry| {
                    let value_size = match &entry.value {
                        Value::Inline(v) => v.len(),
                        Value::External(_) => 9,
                    };
                    size.checked_add(5)
                        .and_then(|n| n.checked_add(entry.key.len()))
                        .and_then(|n| n.checked_add(value_size))
                        .ok_or(Error::Budget)
                })?;
        record::encode_payload_with(NODE, capacity, |bytes| {
            bytes.extend_from_slice(&[
                u8::try_from(self.entries.len()).unwrap(),
                u8::from(!self.children.is_empty()),
            ]);
            for p in &self.children {
                p.put(bytes);
            }
            for entry in &self.entries {
                bytes.extend_from_slice(
                    &u32::try_from(entry.key.len())
                        .map_err(|_| Error::Budget)?
                        .to_le_bytes(),
                );
                bytes.extend_from_slice(&entry.key);
                match &entry.value {
                    Value::Inline(value) => {
                        bytes.push(u8::try_from(value.len()).unwrap());
                        bytes.extend_from_slice(value);
                    }
                    Value::External(p) => {
                        bytes.push(255);
                        p.put(bytes);
                    }
                }
            }
            Ok(())
        })
    }
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        ensure(n <= self.0.len(), Error::Corrupt("truncated B-tree node"))?;
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn pointer(&mut self, cursor: u64, unit: u16) -> Result<Pointer> {
        let offset = u64::from_le_bytes(self.take(8)?.try_into().unwrap());
        let p = Pointer {
            offset,
            order: self.byte()?,
            unit,
        };
        p.check(cursor)?;
        Ok(p)
    }
}

pub struct Tree<K, V, M: Memory> {
    pub memory: M,
    root: RefCell<Root>,
    poisoned: Cell<bool>,
    types: PhantomData<(K, V)>,
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> Tree<K, V, M> {
    pub fn reset(memory: M, options: BTreeMapOptions) -> Result<Self> {
        ensure(options.valid(), Error::Bounds)?;
        let root = Root {
            options,
            top: None,
            length: 0,
            cursor: record::HEADER_BYTES,
            free: vec![None; BINS],
            key_bound: record::bound::<K>(),
            value_bound: record::bound::<V>(),
        };
        let bytes = root.encode()?;
        ensure(bytes.len() as u64 <= record::HEADER_BYTES, Error::Budget)?;
        record::reserve(&memory, record::HEADER_BYTES)?;
        memory.write(0, &bytes);
        Ok(Self {
            memory,
            root: RefCell::new(root),
            poisoned: Cell::new(false),
            types: PhantomData,
        })
    }
    pub fn load(memory: M) -> Result<Self> {
        let root: Root = Root::decode(&record::read_map_header(&memory, &MAGIC)?)?;
        ensure(
            root.key_bound == record::bound::<K>() && root.value_bound == record::bound::<V>(),
            Error::Corrupt("map codec bound changed"),
        )?;
        ensure(
            root.free.len() == BINS
                && root.cursor >= record::HEADER_BYTES
                && root.cursor
                    <= memory
                        .size()
                        .checked_mul(crate::WASM_PAGE_SIZE_BYTES)
                        .ok_or(Error::Budget)?,
            Error::Corrupt("map allocator bounds"),
        )?;
        ensure(
            root.top.is_none() == (root.length == 0),
            Error::Corrupt("map root/count disagree"),
        )?;
        if let Some(p) = root.top {
            p.check(root.cursor)?;
        }
        for (order, p) in root.free.iter().enumerate() {
            if let Some(p) = p {
                p.check(root.cursor)?;
                ensure(
                    usize::from(p.order) == order,
                    Error::Corrupt("free block order"),
                )?;
            }
        }
        Ok(Self {
            memory,
            root: RefCell::new(root),
            poisoned: Cell::new(false),
            types: PhantomData,
        })
    }
    fn healthy(&self) -> Result<()> {
        ensure(!self.poisoned.get(), Error::Poisoned)?;
        // A different allocation can poison the shared governed backing. This
        // also covers iterator entries already held in a routing frame.
        let capacity = self
            .memory
            .size()
            .checked_mul(crate::WASM_PAGE_SIZE_BYTES)
            .ok_or(Error::Budget)?;
        ensure(
            self.root.borrow().cursor <= capacity,
            Error::Corrupt("map allocator bounds"),
        )
    }
    pub fn options(&self) -> BTreeMapOptions {
        self.root.borrow().options
    }
    pub fn len(&self) -> u64 {
        self.healthy().expect("map owner is poisoned");
        self.root.borrow().length
    }
    fn read_node(&self, p: Pointer, depth: usize) -> Result<Rc<ReadNode>> {
        let cursor = self.root.borrow().cursor;
        p.check(cursor)?;
        ensure(depth < MAX_DEPTH, Error::Corrupt("B-tree depth"))?;
        let node = Rc::new(ReadNode::decode::<K, V>(
            record::read_bytes(&self.memory, p.offset, p.capacity(), NODE)?,
            cursor,
            p.unit,
        )?);
        Ok(node)
    }
    fn value(&self, v: Value) -> Result<Vec<u8>> {
        let bytes = match v {
            Value::Inline(bytes) => bytes,
            Value::External(p) => {
                p.check(self.root.borrow().cursor)?;
                record::read_payload(&self.memory, p.offset, p.capacity(), VALUE)?
            }
        };
        V::BOUND.check(&bytes);
        Ok(bytes)
    }
    fn find(&self, key: &K) -> Result<Option<(Rc<ReadNode>, usize)>> {
        self.healthy()?;
        let mut p = self.root.borrow().top;
        for depth in 0..MAX_DEPTH {
            let Some(current) = p else { return Ok(None) };
            let node = self.read_node(current, depth)?;
            match node.search(key)? {
                Ok(i) => return Ok(Some((node, i))),
                Err(i) => p = node.children.get(i).copied(),
            }
        }
        Err(Error::Corrupt("map path exceeds height bound"))
    }
    pub fn get(&self, key: &K) -> Result<Option<V>> {
        let Some((node, index)) = self.find(key)? else {
            return Ok(None);
        };
        self.decode_hit(node, index).map(Some)
    }
    fn decode_hit(&self, node: Rc<ReadNode>, index: usize) -> Result<V> {
        if let Some(bytes) = node.inline_value(index) {
            V::BOUND.check(bytes);
            Ok(V::from_bytes(Cow::Borrowed(bytes)))
        } else {
            let value = node.value(index);
            drop(node);
            decode(self.value(value)?)
        }
    }

    pub fn visit_sorted_keys<E>(
        &self,
        keys: &[K],
        mut visit: impl FnMut(&K, Option<V>) -> std::result::Result<(), E>,
    ) -> Result<std::result::Result<(), E>> {
        if let [key] = keys {
            return Ok(visit(key, self.get(key)?));
        }
        self.healthy()?;
        self.visit_subtree(self.root.borrow().top, 0, keys, &mut visit)
    }

    fn visit_subtree<E>(
        &self,
        pointer: Option<Pointer>,
        depth: usize,
        mut keys: &[K],
        visit: &mut impl FnMut(&K, Option<V>) -> std::result::Result<(), E>,
    ) -> Result<std::result::Result<(), E>> {
        if keys.is_empty() {
            return Ok(Ok(()));
        }
        let Some(pointer) = pointer else {
            for key in keys {
                if let Err(error) = visit(key, None) {
                    return Ok(Err(error));
                }
            }
            return Ok(Ok(()));
        };
        let node = self.read_node(pointer, depth)?;
        // Partition at separators in ascending order. Start at the first requested
        // slot so even a pair of distant probes only descends into relevant children.
        let first = node.search(&keys[0])?.unwrap_or_else(|slot| slot);
        for slot in first..node.len() {
            let separator = K::from_bytes(Cow::Borrowed(node.key_bytes(slot)));
            let split = keys.partition_point(|key| key < &separator);
            if let Err(error) = self.visit_subtree(
                node.children.get(slot).copied(),
                depth + 1,
                &keys[..split],
                visit,
            )? {
                return Ok(Err(error));
            }
            keys = &keys[split..];
            if keys.first() == Some(&separator) {
                if let Err(error) = visit(&keys[0], Some(self.decode_hit(Rc::clone(&node), slot)?))
                {
                    return Ok(Err(error));
                }
                keys = &keys[1..];
            }
            if keys.is_empty() {
                return Ok(Ok(()));
            }
        }
        self.visit_subtree(node.children.last().copied(), depth + 1, keys, visit)
    }
    pub fn contains_key(&self, key: &K) -> Result<bool> {
        Ok(self.find(key)?.is_some())
    }
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>> {
        self.healthy()?;
        let key_bytes = key.to_bytes().into_owned();
        K::BOUND.check(&key_bytes);
        let bytes = value.into_bytes();
        V::BOUND.check(&bytes);
        let mut tx = Transaction::new(self);
        let (top, split, old) = tx.insert(tx.root.top, &key, key_bytes, bytes, 0)?;
        tx.root.top = Some(if let Some((entry, right)) = split {
            tx.save(
                None,
                &Node {
                    entries: vec![entry],
                    children: vec![top, right],
                },
            )?
        } else {
            top
        });
        drop(key);
        let old = old.map(decode).transpose()?;
        tx.commit()?;
        Ok(old)
    }
    pub fn remove(&mut self, key: &K) -> Result<Option<V>> {
        self.healthy()?;
        let mut tx = Transaction::new(self);
        let old = tx.remove_key(key)?;
        let value = old.map(decode).transpose()?;
        tx.commit()?;
        Ok(value)
    }
    pub fn remove_many(&mut self, keys: std::collections::BTreeSet<K>) -> Result<u64> {
        self.healthy()?;
        let mut removed = 0;
        let mut keys = keys.into_iter().peekable();
        while keys.peek().is_some() {
            let mut tx = Transaction::new(self);
            for key in keys.by_ref().take(32) {
                removed += u64::from(tx.remove_key(&key)?.is_some());
            }
            tx.commit()?;
        }
        Ok(removed)
    }
    pub fn clear(&mut self) -> Result<()> {
        self.healthy()?;
        let mut tx = Transaction::new(self);
        tx.root.top = None;
        tx.root.length = 0;
        tx.root.cursor = record::HEADER_BYTES;
        tx.root.free.fill(None);
        tx.commit()
    }
    fn seek_path(&self, bound: &Bound<K>, forward: bool) -> Result<Vec<Frame>> {
        self.healthy()?;
        let mut path = Vec::new();
        let mut pointer = self.root.borrow().top;
        while let Some(p) = pointer {
            ensure(
                path.len() < MAX_DEPTH,
                Error::Corrupt("map cursor exceeds height bound"),
            )?;
            let node = self.read_node(p, path.len())?;
            let index = match bound {
                Bound::Unbounded => {
                    if forward {
                        0
                    } else {
                        node.len()
                    }
                }
                Bound::Included(k) => match node.search(k)? {
                    Ok(i) => {
                        if forward {
                            i
                        } else {
                            i + 1
                        }
                    }
                    Err(i) => i,
                },
                Bound::Excluded(k) => match node.search(k)? {
                    Ok(i) => {
                        if forward {
                            i + 1
                        } else {
                            i
                        }
                    }
                    Err(i) => i,
                },
            };
            pointer = node.children.get(index).copied();
            path.push(Frame {
                node,
                index,
                descend: false,
            });
        }
        Ok(path)
    }
    fn advance_path(
        &self,
        path: &mut Vec<Frame>,
        forward: bool,
        keys_only: bool,
    ) -> Result<Option<(K, Option<Value>)>> {
        self.healthy()?;
        while let Some(frame) = path.last_mut() {
            if frame.descend {
                frame.descend = false;
                let mut child = frame.node.children.get(frame.index).copied();
                while let Some(p) = child {
                    ensure(
                        path.len() < MAX_DEPTH,
                        Error::Corrupt("map cursor exceeds height bound"),
                    )?;
                    let node = self.read_node(p, path.len())?;
                    let index = if forward { 0 } else { node.len() };
                    child = node.children.get(index).copied();
                    path.push(Frame {
                        node,
                        index,
                        descend: false,
                    });
                }
                continue;
            }
            let index = if forward {
                frame.index
            } else {
                if frame.index == 0 {
                    path.pop();
                    continue;
                }
                frame.index - 1
            };
            if index >= frame.node.len() {
                path.pop();
                continue;
            }
            let bytes = frame.node.key_bytes(index);
            K::BOUND.check(bytes);
            let key = K::from_bytes(Cow::Borrowed(bytes));
            let value = (!keys_only).then(|| frame.node.value(index));
            frame.index = if forward { index + 1 } else { index };
            // A caller may stop at this entry. Read the following subtree only
            // when another step in this direction is actually requested.
            frame.descend = !frame.node.children.is_empty();
            return Ok(Some((key, value)));
        }
        Ok(None)
    }
    pub fn range(&self, bounds: impl RangeBounds<K>) -> TreeIter<'_, K, V, M> {
        let lower = bounds.start_bound().cloned();
        let upper = bounds.end_bound().cloned();
        let invalid = match (&lower, &upper) {
            (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b)) => {
                a > b
                    || (a == b
                        && matches!((&lower, &upper), (Bound::Excluded(_), Bound::Excluded(_))))
            }
            _ => false,
        };
        TreeIter {
            tree: self,
            lower,
            upper,
            front: None,
            back: None,
            exhausted: false,
            invalid,
            keys_only: false,
        }
    }
    pub fn keys_range(
        &self,
        bounds: impl RangeBounds<K>,
    ) -> impl DoubleEndedIterator<Item = K> + '_ {
        let mut iter = self.range(bounds);
        iter.keys_only = true;
        iter.map(MapEntry::into_key)
    }
}
struct Frame {
    node: Rc<ReadNode>,
    index: usize,
    descend: bool,
}
fn decode<T: Storable>(bytes: Vec<u8>) -> Result<T> {
    T::BOUND.check(&bytes);
    Ok(T::from_bytes(Cow::Owned(bytes)))
}

type Split = Option<(Entry, Pointer)>;
struct Transaction<'a, K, V, M: Memory> {
    tree: &'a Tree<K, V, M>,
    root: Root,
    writes: BTreeMap<u64, Vec<u8>>,
}
impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> Transaction<'a, K, V, M> {
    fn new(tree: &'a Tree<K, V, M>) -> Self {
        Self {
            tree,
            root: tree.root.borrow().clone(),
            writes: BTreeMap::new(),
        }
    }
    fn remove_key(&mut self, key: &K) -> Result<Option<Vec<u8>>> {
        let Some(top) = self.root.top else {
            return Ok(None);
        };
        let (changed, old) = self.remove(top, key, 0)?;
        let Some(old) = old else { return Ok(None) };

        if let Some(node) = changed {
            self.root.top = if node.entries.is_empty() {
                self.free(top)?;
                node.children.first().copied()
            } else {
                Some(self.save(Some(top), &node)?)
            };
        }
        self.root.length -= 1;
        Ok(Some(old))
    }
    fn bytes(&self, p: Pointer, magic: [u8; 8]) -> Result<Vec<u8>> {
        p.check(self.root.cursor)?;
        if let Some(bytes) = self.writes.get(&p.offset) {
            Ok(bytes.clone())
        } else {
            record::read_bytes(&self.tree.memory, p.offset, p.capacity(), magic)
        }
    }
    fn node(&self, p: Pointer, depth: usize) -> Result<Node> {
        Ok(self.read_node(p, depth)?.owned())
    }
    fn read_node(&self, p: Pointer, depth: usize) -> Result<Rc<ReadNode>> {
        if let Some(bytes) = self.writes.get(&p.offset) {
            Ok(Rc::new(ReadNode::decode::<K, V>(
                bytes.clone(),
                self.root.cursor,
                p.unit,
            )?))
        } else {
            self.tree.read_node(p, depth)
        }
    }
    fn value(&self, v: &Value) -> Result<Vec<u8>> {
        let bytes = match v {
            Value::Inline(b) => b.clone(),
            Value::External(p) => {
                p.check(self.root.cursor)?;
                if let Some(bytes) = self.writes.get(&p.offset) {
                    record::decode_payload(VALUE, bytes)?.to_vec()
                } else {
                    record::read_payload(&self.tree.memory, p.offset, p.capacity(), VALUE)?
                }
            }
        };
        V::BOUND.check(&bytes);
        Ok(bytes)
    }
    fn free(&mut self, p: Pointer) -> Result<()> {
        let index = usize::from(p.order);
        self.writes.insert(
            p.offset,
            record::encode_payload(FREE, &link_bytes(self.root.free[index]))?,
        );
        self.root.free[index] = Some(p);
        Ok(())
    }
    fn allocate(&mut self, order: usize) -> Result<Pointer> {
        ensure(order < BINS, Error::Budget)?;
        for bin in order..BINS {
            if let Some(mut p) = self.root.free[bin] {
                let next: Option<Pointer> = decode_link(
                    record::decode_payload(FREE, &self.bytes(p, FREE)?)?,
                    self.root.cursor,
                    p.unit,
                )?;
                if let Some(next) = next {
                    next.check(self.root.cursor)?;
                    ensure(
                        next.order == p.order && next != p,
                        Error::Corrupt("map free-list link"),
                    )?;
                }
                self.root.free[bin] = next;
                // Return only the requested capacity. Each unused half remains
                // an ordinary free-list block, staged with the same mutation.
                while usize::from(p.order) > order {
                    p.order -= 1;
                    let sibling = Pointer {
                        offset: p.offset + p.capacity(),
                        ..p
                    };
                    self.free(sibling)?;
                }
                return Ok(p);
            }
        }
        let p = Pointer {
            offset: self.root.cursor,
            order: u8::try_from(order).unwrap(),
            unit: self.root.options.allocation_unit_bytes,
        };
        self.root.cursor = self
            .root
            .cursor
            .checked_add(p.capacity())
            .ok_or(Error::Budget)?;
        Ok(p)
    }
    fn save_bytes(
        &mut self,
        previous: Option<Pointer>,
        bytes: Vec<u8>,
        minimum: usize,
    ) -> Result<Pointer> {
        let allocation_len = bytes.len().max(minimum);
        let units = allocation_len.div_ceil(usize::from(self.root.options.allocation_unit_bytes));
        let order = FIRST_ORDER + units.next_power_of_two().trailing_zeros() as usize;
        let p = if let Some(p) = previous.filter(|p| p.capacity() >= allocation_len as u64) {
            // Reclaim only into already reusable smaller capacity. Splitting the
            // live block itself loses its large size class and causes unbounded
            // growth under repeated large/small replacements without coalescing.
            let smaller_available =
                (order..usize::from(p.order)).any(|order| self.root.free[order].is_some());
            if usize::from(p.order) >= order + 2 && smaller_available {
                let smaller = self.allocate(order)?;
                self.free(p)?;
                smaller
            } else {
                p
            }
        } else {
            if let Some(p) = previous {
                self.free(p)?;
            }
            self.allocate(order)?
        };
        self.writes.insert(p.offset, bytes);
        Ok(p)
    }
    fn save(&mut self, p: Option<Pointer>, node: &Node) -> Result<Pointer> {
        self.save_node_bytes(p, node.encode()?)
    }
    fn save_node_bytes(&mut self, p: Option<Pointer>, bytes: Vec<u8>) -> Result<Pointer> {
        let target = self.root.options.node_target_bytes;
        let minimum = match (record::bound::<K>(), record::bound::<V>()) {
            (Some((key, _)), Some((value, fixed))) => {
                // Upstream targets 3/4 of a full bounded node. Size our own frame:
                // header, child pointers, then length-prefixed keys and tagged values.
                let value_bytes = if fixed && value > INLINE as u32 {
                    9 // external pointer
                } else {
                    u64::from(value).min(INLINE as u64)
                };
                let full_node = record::PREFIX as u64
                    + 2
                    + (MAX_ENTRIES as u64 + 1) * 9
                    + MAX_ENTRIES as u64 * (5 + u64::from(key) + value_bytes);
                // Wide codec declarations alone should never force a large reservation.
                (full_node * 3 / 4).min(u64::from(target)) as usize
            }
            _ => usize::from(target),
        };
        self.save_bytes(p, bytes, minimum)
    }
    fn save_value(&mut self, previous: Option<&Value>, bytes: Vec<u8>) -> Result<Value> {
        let old = match previous {
            Some(Value::External(p)) => Some(*p),
            _ => None,
        };
        if bytes.len() <= INLINE {
            if let Some(p) = old {
                self.free(p)?;
            }
            Ok(Value::Inline(bytes))
        } else {
            Ok(Value::External(self.save_bytes(
                old,
                record::encode_payload(VALUE, &bytes)?,
                0,
            )?))
        }
    }
    fn insert(
        &mut self,
        p: Option<Pointer>,
        key: &K,
        key_bytes: Vec<u8>,
        bytes: Vec<u8>,
        depth: usize,
    ) -> Result<(Pointer, Split, Option<Vec<u8>>)> {
        ensure(
            depth < MAX_DEPTH,
            Error::Corrupt("map path exceeds height bound"),
        )?;
        let read = p.map(|p| self.read_node(p, depth)).transpose()?;
        let search = read.as_ref().map_or(Ok(Err(0)), |node| node.search(key))?;
        let (mut node, old) = match search {
            Ok(i) => {
                let read = read.as_ref().unwrap();
                let previous = read.value(i);
                let old = self.value(&previous)?;
                let value = self.save_value(Some(&previous), bytes)?;
                let same_pointer =
                    matches!((&previous,&value),(Value::External(a),Value::External(b)) if a==b);
                if same_pointer {
                    return Ok((p.unwrap(), None, Some(old)));
                }
                if let (Value::Inline(_), Value::Inline(bytes)) = (&previous, &value) {
                    let next = self.save_node_bytes(p, read.replace_inline(i, bytes)?)?;
                    return Ok((next, None, Some(old)));
                }
                let mut node = read.owned();
                node.entries[i].value = value;
                (node, Some(old))
            }
            Err(i) => {
                if let Some(child) = read.as_ref().and_then(|node| node.children.get(i)).copied() {
                    let (next, split, old) =
                        self.insert(Some(child), key, key_bytes, bytes, depth + 1)?;
                    if split.is_none() && next == child {
                        return Ok((p.unwrap(), None, old));
                    }
                    let mut node = read.as_ref().unwrap().owned();
                    node.children[i] = next;
                    if let Some((entry, right)) = split {
                        node.entries.insert(i, entry);
                        node.children.insert(i + 1, right);
                    }
                    (node, old)
                } else {
                    let value = self.save_value(None, bytes)?;
                    let mut node = read
                        .as_ref()
                        .map_or_else(Node::default, |node| node.owned());
                    node.entries.insert(
                        i,
                        Entry {
                            key: key_bytes,
                            value,
                        },
                    );
                    self.root.length = self.root.length.checked_add(1).ok_or(Error::Budget)?;
                    (node, None)
                }
            }
        };
        let split = if node.entries.len() > MAX_ENTRIES {
            let right_entries = node.entries.split_off(DEGREE + 1);
            let median = node.entries.pop().unwrap();
            let children = if node.children.is_empty() {
                vec![]
            } else {
                node.children.split_off(DEGREE + 1)
            };
            let right = self.save(
                None,
                &Node {
                    entries: right_entries,
                    children,
                },
            )?;
            Some((median, right))
        } else {
            None
        };
        Ok((self.save(p, &node)?, split, old))
    }
    // Return a modified node without encoding it: its parent may still repair
    // an underflow. None means its persisted routing record remains unchanged.
    fn remove(
        &mut self,
        p: Pointer,
        key: &K,
        depth: usize,
    ) -> Result<(Option<Node>, Option<Vec<u8>>)> {
        ensure(
            depth < MAX_DEPTH,
            Error::Corrupt("map path exceeds height bound"),
        )?;
        let read = self.read_node(p, depth)?;
        match read.search(key)? {
            Ok(i) => {
                let mut node = read.owned();
                let old = self.value(&node.entries[i].value)?;
                if let Value::External(p) = node.entries[i].value {
                    self.free(p)?;
                }
                if node.children.is_empty() {
                    node.entries.remove(i);
                } else {
                    let child_pointer = node.children[i + 1];
                    let (child, entry) = self.extract_min(child_pointer, depth + 1)?;
                    node.entries[i] = entry;
                    if let Some(child) = child {
                        self.finish_child(&mut node, i + 1, child, depth)?;
                    }
                }
                Ok((Some(node), Some(old)))
            }
            Err(i) => {
                let Some(child_pointer) = read.children.get(i).copied() else {
                    return Ok((None, None));
                };
                let (child, old) = self.remove(child_pointer, key, depth + 1)?;
                let node = child
                    .map(|child| self.changed_parent(&read, i, child, depth))
                    .transpose()?
                    .flatten();
                Ok((node, old))
            }
        }
    }
    fn extract_min(&mut self, p: Pointer, depth: usize) -> Result<(Option<Node>, Entry)> {
        ensure(
            depth < MAX_DEPTH,
            Error::Corrupt("map path exceeds height bound"),
        )?;
        let read = self.read_node(p, depth)?;
        ensure(read.len() != 0, Error::Corrupt("empty successor node"))?;
        if read.children.is_empty() {
            let mut node = read.owned();
            let entry = node.entries.remove(0);
            Ok((Some(node), entry))
        } else {
            let (child, entry) = self.extract_min(read.children[0], depth + 1)?;
            let node = child
                .map(|child| self.changed_parent(&read, 0, child, depth))
                .transpose()?
                .flatten();
            Ok((node, entry))
        }
    }
    // Keep unchanged ancestors borrowed, including when the child was rewritten
    // in place. Only a moved child or an underflow requires owning the parent.
    fn changed_parent(
        &mut self,
        read: &ReadNode,
        i: usize,
        child: Node,
        depth: usize,
    ) -> Result<Option<Node>> {
        if child.entries.len() >= MIN_ENTRIES {
            let previous = read.children[i];
            let next = self.save(Some(previous), &child)?;
            if next == previous {
                return Ok(None);
            }
            let mut parent = read.owned();
            parent.children[i] = next;
            Ok(Some(parent))
        } else {
            let mut parent = read.owned();
            self.rebalance(&mut parent, i, child, depth)?;
            Ok(Some(parent))
        }
    }
    fn finish_child(
        &mut self,
        parent: &mut Node,
        i: usize,
        child: Node,
        depth: usize,
    ) -> Result<()> {
        if child.entries.len() >= MIN_ENTRIES {
            parent.children[i] = self.save(Some(parent.children[i]), &child)?;
            Ok(())
        } else {
            self.rebalance(parent, i, child, depth)
        }
    }
    fn rebalance(
        &mut self,
        parent: &mut Node,
        i: usize,
        mut child: Node,
        parent_depth: usize,
    ) -> Result<()> {
        let depth = parent_depth + 1;
        let p = parent.children[i];
        let mut left = if i > 0 {
            Some(self.node(parent.children[i - 1], depth)?)
        } else {
            None
        };
        if let Some(left) = &mut left {
            ensure(
                left.children.is_empty() == child.children.is_empty(),
                Error::Corrupt("B-tree sibling levels"),
            )?;
            if left.entries.len() > MIN_ENTRIES {
                child.entries.insert(
                    0,
                    std::mem::replace(&mut parent.entries[i - 1], left.entries.pop().unwrap()),
                );
                if let Some(p) = left.children.pop() {
                    child.children.insert(0, p);
                }
                parent.children[i - 1] = self.save(Some(parent.children[i - 1]), left)?;
                parent.children[i] = self.save(Some(p), &child)?;
                return Ok(());
            }
        }
        let mut right = if i + 1 < parent.children.len() {
            Some(self.node(parent.children[i + 1], depth)?)
        } else {
            None
        };
        if let Some(right) = &mut right {
            ensure(
                right.children.is_empty() == child.children.is_empty(),
                Error::Corrupt("B-tree sibling levels"),
            )?;
            if right.entries.len() > MIN_ENTRIES {
                child.entries.push(std::mem::replace(
                    &mut parent.entries[i],
                    right.entries.remove(0),
                ));
                if !right.children.is_empty() {
                    child.children.push(right.children.remove(0));
                }
                parent.children[i + 1] = self.save(Some(parent.children[i + 1]), right)?;
                parent.children[i] = self.save(Some(p), &child)?;
                return Ok(());
            }
        }
        let (left_index, mut left, right) = if let Some(left) = left {
            (i - 1, left, child)
        } else {
            (
                i,
                child,
                right.ok_or(Error::Corrupt("B-tree missing separator"))?,
            )
        };
        ensure(
            left_index < parent.entries.len(),
            Error::Corrupt("B-tree missing separator"),
        )?;
        let left_p = parent.children[left_index];
        let right_p = parent.children[left_index + 1];
        left.entries.push(parent.entries.remove(left_index));
        left.entries.extend(right.entries);
        left.children.extend(right.children);
        self.free(right_p)?;
        parent.children[left_index] = self.save(Some(left_p), &left)?;
        parent.children.remove(left_index + 1);
        Ok(())
    }
    fn commit(self) -> Result<()> {
        let changed = self.root != *self.tree.root.borrow();
        let header = if changed {
            Some(self.root.encode()?)
        } else {
            None
        };
        if let Some(h) = &header {
            ensure(h.len() as u64 <= record::HEADER_BYTES, Error::Budget)?;
        }
        record::reserve(&self.tree.memory, self.root.cursor)?;
        self.tree.poisoned.set(true);

        for (offset, bytes) in self.writes {
            self.tree.memory.write(offset, &bytes);
        }
        if let Some(h) = header {
            self.tree.memory.write(0, &h);
        }
        *self.tree.root.borrow_mut() = self.root;
        self.tree.poisoned.set(false);
        Ok(())
    }
}
///
/// `MapEntry`
///
/// An owned key and lazily decoded value from an ordered scan.
///
pub struct MapEntry<K, V> {
    key: K,
    bytes: Vec<u8>,
    value: PhantomData<V>,
}
impl<K, V: Storable> MapEntry<K, V> {
    /// Borrow the decoded key.
    pub const fn key(&self) -> &K {
        &self.key
    }
    /// Decode the value using its declared codec.
    ///
    /// # Panics
    /// Panics if the stored encoding violates the value codec or its bound.
    pub fn value(&self) -> V {
        V::BOUND.check(&self.bytes);
        V::from_bytes(Cow::Borrowed(&self.bytes))
    }
    /// Consume the entry and decode its value.
    ///
    /// # Panics
    /// Panics if the stored encoding violates the value codec or its bound.
    pub fn into_pair(self) -> (K, V) {
        (self.key, decode(self.bytes).expect("invalid map value"))
    }
    /// Consume the entry without decoding its value.
    pub fn into_key(self) -> K {
        self.key
    }
    /// Consume the entry and decode only its value.
    ///
    /// # Panics
    /// Panics if the stored encoding violates the value codec or its bound.
    pub fn into_value(self) -> V {
        decode(self.bytes).expect("invalid map value")
    }
}

///
/// `TreeIter`
///
/// Lazy bidirectional traversal with bounded routing-node stacks. External values
/// are read only for in-range entries and decoded when requested by the caller.
///
pub struct TreeIter<'a, K, V, M: Memory> {
    tree: &'a Tree<K, V, M>,
    lower: Bound<K>,
    upper: Bound<K>,
    front: Option<Vec<Frame>>,
    back: Option<Vec<Frame>>,
    exhausted: bool,
    invalid: bool,
    keys_only: bool,
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> TreeIter<'_, K, V, M> {
    /// Consume the next key at the front, skipping its external value.
    /// Subsequent entry or key reads continue from the updated range.
    ///
    /// # Panics
    /// Panics on invalid bounds, backing, routing record integrity or key codec failure.
    pub fn next_key(&mut self) -> Option<K> {
        self.advance(true, true).map(MapEntry::into_key)
    }

    /// Consume the next key at the back, skipping its external value.
    ///
    /// # Panics
    /// Panics on invalid bounds, backing, routing record integrity or key codec failure.
    pub fn next_back_key(&mut self) -> Option<K> {
        self.advance(false, true).map(MapEntry::into_key)
    }

    fn step(&mut self, forward: bool, keys_only: bool) -> Result<Option<MapEntry<K, V>>> {
        ensure(!self.invalid, Error::Bounds)?;
        self.tree.healthy()?;
        let empty = match (&self.lower, &self.upper) {
            (Bound::Included(a), Bound::Included(b)) => a > b,
            (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b)) => {
                a >= b
            }
            _ => false,
        };
        if empty {
            return Ok(None);
        }
        let (path, bound) = if forward {
            (&mut self.front, &self.lower)
        } else {
            (&mut self.back, &self.upper)
        };
        if path.is_none() {
            *path = Some(self.tree.seek_path(bound, forward)?);
        }
        let entry = self
            .tree
            .advance_path(path.as_mut().unwrap(), forward, keys_only)?;
        if let Some((key, value)) = entry {
            let after = match &self.lower {
                Bound::Unbounded => true,
                Bound::Included(k) => key >= *k,
                Bound::Excluded(k) => key > *k,
            };
            let before = match &self.upper {
                Bound::Unbounded => true,
                Bound::Included(k) => key <= *k,
                Bound::Excluded(k) => key < *k,
            };
            ensure(
                if forward { after } else { before },
                Error::Corrupt("map cursor key order"),
            )?;
            if !(after && before) {
                return Ok(None);
            }
            if forward {
                self.lower = Bound::Excluded(key.clone());
            } else {
                self.upper = Bound::Excluded(key.clone());
            }
            return Ok(Some(MapEntry {
                key,
                bytes: value
                    .map(|value| self.tree.value(value))
                    .transpose()?
                    .unwrap_or_default(),
                value: PhantomData,
            }));
        }
        Ok(None)
    }
    fn advance(&mut self, forward: bool, keys_only: bool) -> Option<MapEntry<K, V>> {
        if self.exhausted {
            return None;
        }
        match self.step(forward, keys_only) {
            Ok(Some(entry)) => Some(entry),
            Ok(None) => {
                self.exhausted = true;
                None
            }
            Err(error) => {
                self.exhausted = true;
                panic!("invalid map entry: {error:?}")
            }
        }
    }
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> Iterator for TreeIter<'_, K, V, M> {
    type Item = MapEntry<K, V>;
    fn next(&mut self) -> Option<Self::Item> {
        self.advance(true, self.keys_only)
    }
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> DoubleEndedIterator
    for TreeIter<'_, K, V, M>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.advance(false, self.keys_only)
    }
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> std::iter::FusedIterator
    for TreeIter<'_, K, V, M>
{
}
