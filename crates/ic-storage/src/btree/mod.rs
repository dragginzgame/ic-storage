//! Fixed-size tree pages with separately allocated key/value chains.

mod audit;
mod cursor;
mod page;
mod search;
#[cfg(test)]
mod tests;
mod transaction;

pub use cursor::{Entry, Range};
use page::{Header, Node, Record};
use std::{cmp::Ordering, fmt, ops::Bound};
use transaction::Transaction;

use crate::Memory;

/// A rejected operation. Returned errors never publish a partial mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Creation requires a fresh, zero-sized memory.
    NotEmpty,
    /// Memory does not contain this tree's format.
    Format,
    /// Stored configuration or comparator identity differs from the caller.
    Configuration,
    /// Invalid page, pointer, length, checksum, or tree geometry.
    Corrupt,
    /// An input exceeds the configured key or value bound.
    TooLarge,
    /// The backing memory refused growth or arithmetic capacity was exhausted.
    Capacity,
    /// The comparator rejected malformed input.
    InvalidKey,
    /// The lower bound follows the upper bound.
    InvalidRange,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stable B-tree error: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Immutable, bounded decoding limits stored in the tree header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    /// Largest encoded key accepted by this tree, in bytes.
    pub max_key_bytes: u32,
    /// Largest encoded value accepted by this tree, in bytes.
    pub max_value_bytes: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_key_bytes: 128 * 1024,
            max_value_bytes: 4 * 1024 * 1024,
        }
    }
}
impl Config {
    fn check(self) -> Result<(), Error> {
        if self.max_key_bytes == 0
            || self.max_key_bytes > 1024 * 1024
            || self.max_value_bytes == 0
            || self.max_value_bytes > 64 * 1024 * 1024
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
}

/// A deterministic total order over encoded keys.
///
/// `IDENTITY` must change when ordering or accepted key encoding changes.
/// Equal keys must have identical encodings. The comparator must be immutable
/// and return `InvalidKey` for malformed keys, including `compare(key, key)`.
/// Persisted comparator code itself remains an application responsibility.
pub trait Comparator {
    /// Stable identity of the encoding and ordering contract.
    const IDENTITY: [u8; 16];
    /// Compare two encoded keys, rejecting invalid encodings.
    ///
    /// # Errors
    /// Returns [`Error::InvalidKey`] when an encoded key is invalid.
    fn compare(&self, left: &[u8], right: &[u8]) -> Result<Ordering, Error>;
}

/// Ordinary unsigned bytewise ordering for already order-preserving encodings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Lexicographic;
impl Comparator for Lexicographic {
    const IDENTITY: [u8; 16] = *b"ic-storage-lex01";
    fn compare(&self, left: &[u8], right: &[u8]) -> Result<Ordering, Error> {
        Ok(left.cmp(right))
    }
}

/// A stable-memory B-tree, independent of DFINITY's collection formats.
///
/// Exactly one live tree handle may own a memory region. The memory must not be
/// mutated through aliases. Mutations stage touched pages, reserve capacity,
/// then write them. Backing writes must either complete or trap in an atomic IC
/// message; this is not a crash-safe filesystem database. See the package guide.
pub struct BTreeMap<M, C = Lexicographic> {
    memory: M,
    order: C,
    header: Header,
}

impl<M: Memory, C: Comparator> BTreeMap<M, C> {
    /// Create a tree in an empty memory without overwriting an existing format.
    ///
    /// # Errors
    /// Rejects nonempty memory, invalid limits, and refused growth.
    pub fn create(memory: M, config: Config, order: C) -> Result<Self, Error> {
        config.check()?;
        if memory.size() != 0 {
            return Err(Error::NotEmpty);
        }
        let mut tree = Self {
            memory,
            order,
            header: Header::new(config, C::IDENTITY),
        };
        let mut tx = Transaction::new(&tree);
        let root = tx.allocate()?;
        tx.put_node(root, &Node::leaf())?;
        tx.header.root = root;
        tree.header = tx.commit()?;
        Ok(tree)
    }

    /// Open a tree with its exact configuration and comparator identity.
    ///
    /// Opening checks the header and root, not every page. Reads validate pages
    /// as visited; [`Self::validate`] performs a complete structural audit.
    /// # Errors
    /// Rejects unknown formats, incompatible limits/order, and observed corruption.
    pub fn open(memory: M, config: Config, order: C) -> Result<Self, Error> {
        config.check()?;
        let header = Header::read(&memory)?;
        if header.config != config || header.identity != C::IDENTITY {
            return Err(Error::Configuration);
        }
        let tree = Self {
            memory,
            order,
            header,
        };
        tree.node(tree.header.root)?;
        Ok(tree)
    }

    /// Number of live entries, persisted with each completed mutation.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.header.len
    }
    /// Whether the tree contains no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.header.len == 0
    }
    /// Bytes reserved in the backing memory, including reusable free pages.
    #[must_use]
    pub fn memory_bytes(&self) -> u64 {
        self.memory.size().saturating_mul(65_536)
    }
    /// Release the sole handle and return its backing memory for later reopening.
    #[must_use]
    pub fn into_memory(self) -> M {
        self.memory
    }

    /// Read one value by key.
    /// # Errors
    /// Rejects malformed keys, oversized keys, and observed storage corruption.
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.check_key(key)?;
        self.find_validated(key)?
            .map(|(_, node, at)| self.value(node.entries[at]))
            .transpose()
    }

    /// Test key membership without reading the value payload.
    /// # Errors
    /// Rejects invalid keys and observed key or tree-page corruption.
    pub fn contains_key(&self, key: &[u8]) -> Result<bool, Error> {
        self.check_key(key)?;
        Ok(self.find_validated(key)?.is_some())
    }

    // Operation boundaries validate the input key before entering this traversal.
    fn find_validated(&self, key: &[u8]) -> Result<Option<(u64, Node, usize)>, Error> {
        let mut id = self.header.root;
        for _ in 0..page::MAX_HEIGHT {
            let node = self.node(id)?;
            let (at, found) = self.locate(&node, key)?;
            if found {
                return Ok(Some((id, node, at)));
            }
            if node.children.is_empty() {
                return Ok(None);
            }
            id = node.children[at];
        }
        Err(Error::Corrupt)
    }

    /// Insert or replace a value, returning the previous value if present.
    /// # Errors
    /// Rejects invalid keys, oversized inputs, corruption, or insufficient capacity
    /// before publishing changes. Memory traps follow the backing contract.
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.check_key(key)?;
        if value.len() > self.header.config.max_value_bytes as usize {
            return Err(Error::TooLarge);
        }
        let located = self.find_validated(key)?;
        let mut tx = Transaction::new(self);
        let previous = if let Some((id, node, at)) = located {
            Some(tx.replace(id, node, at, value)?)
        } else {
            tx.insert(key, value)?;
            tx.header.len = tx.header.len.checked_add(1).ok_or(Error::Capacity)?;
            None
        };
        self.header = tx.commit()?;
        Ok(previous)
    }

    /// Remove a key, returning its previous value.
    /// # Errors
    /// Rejects malformed keys and observed corruption without publishing changes.
    pub fn remove(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.check_key(key)?;
        let mut tx = Transaction::new(self);
        let Some(previous) = tx.remove(key)? else {
            // Any speculative balancing remains private to the discarded transaction.
            return Ok(None);
        };
        tx.header.len = tx.header.len.checked_sub(1).ok_or(Error::Corrupt)?;
        self.header = tx.commit()?;
        Ok(Some(previous))
    }

    /// Iterate a bounded range in either direction without reading value payloads.
    ///
    /// Bounds use the supplied comparator. Equal exclusive bounds yield an empty
    /// range. `rev()` and interleaved `next`/`next_back` are supported.
    /// # Errors
    /// Rejects invalid bounds; traversal errors are yielded by the iterator.
    pub fn range(
        &self,
        lower: Bound<&[u8]>,
        upper: Bound<&[u8]>,
    ) -> Result<Range<'_, M, C>, Error> {
        Range::new(self, lower, upper)
    }

    /// Traverse all entries in comparator order, with lazy value reads.
    #[must_use]
    pub fn iter(&self) -> Range<'_, M, C> {
        Range::all(self)
    }

    /// Check every reachable page, key order, depth, occupancy, entry count and
    /// free-list ownership. This explicit audit is linear in allocated storage.
    /// # Errors
    /// Returns an error on corruption, invalid encoded keys or invalid ordering.
    pub fn validate(&self) -> Result<(), Error> {
        audit::validate(self)
    }

    fn check_key(&self, key: &[u8]) -> Result<(), Error> {
        if key.len() > self.header.config.max_key_bytes as usize {
            return Err(Error::TooLarge);
        }
        if self.order.compare(key, key)? != Ordering::Equal {
            return Err(Error::InvalidKey);
        }
        Ok(())
    }
    fn node(&self, id: u64) -> Result<Node, Error> {
        let mut page = [0; page::SIZE];
        page::read_into(&self.memory, self.header.next, id, &mut page)?;
        Node::decode(&page, self.header.next, self.header.config)
    }
    fn key(&self, record: Record) -> Result<Vec<u8>, Error> {
        if record.key == 0 {
            return Ok(record.key_inline
                [..usize::try_from(record.key_len).map_err(|_| Error::Corrupt)?]
                .to_vec());
        }
        page::blob(&self.memory, self.header.next, record.key, record.key_len)
    }
    fn value(&self, record: Record) -> Result<Vec<u8>, Error> {
        if record.value == 0 {
            return Ok(record.value_inline
                [..usize::try_from(record.value_len).map_err(|_| Error::Corrupt)?]
                .to_vec());
        }
        page::blob(
            &self.memory,
            self.header.next,
            record.value,
            record.value_len,
        )
    }
    fn locate(&self, node: &Node, key: &[u8]) -> Result<(usize, bool), Error> {
        search::locate(
            node,
            key,
            &self.order,
            |id, buffer| page::read_into(&self.memory, self.header.next, id, buffer),
            |record| self.key(record),
        )
    }
}

impl<'a, M: Memory, C: Comparator> IntoIterator for &'a BTreeMap<M, C> {
    type Item = Result<Entry<'a, M, C>, Error>;
    type IntoIter = Range<'a, M, C>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
