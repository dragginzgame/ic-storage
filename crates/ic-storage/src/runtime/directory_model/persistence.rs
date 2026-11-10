// Adapted from ic-stable-structures 0.7.2 memory_manager.rs, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See LICENSE-APACHE and docs/storage/icss-source.json.
//! ICSS bucket allocation and translation behind Storage's named custody boundary.
//! Fixed-width controls replace whole-directory snapshot publication. Unknown or
//! interrupted images are rejected, never initialized or guessed back into use.
use super::{AllocationId, Error, StableKey, StorageConfig};
use crate::{Memory, WASM_PAGE_SIZE_BYTES as PAGE, hash};
use std::{cell::Cell, collections::BTreeSet};

const MAGIC: &[u8; 8] = b"ICBUCK01";
const ROOT_BYTES: usize = 64;
const ENTRY_SIZE: usize = 160;
const ENTRY_BYTES: u64 = ENTRY_SIZE as u64;
const OWNER_SIZE: usize = 16;
const OWNER_BYTES: u64 = OWNER_SIZE as u64;
const BUCKET_PAGES: u64 = 8;
const BUCKET_BYTES: u64 = BUCKET_PAGES * PAGE;

// A bounded control buffer shared by admission and verification. Its capacity
// is a whole number of both row widths; partial tails read only active rows.
const CONTROL_CHUNK_BYTES: usize = 2560;
type RowVisitor<'a> = dyn FnMut(u64, &[u8]) -> Result<(), Error> + 'a;
fn read_rows<M: Memory>(
    memory: &M,
    mut offset: u64,
    count: u64,
    width: usize,
    visit: &mut RowVisitor<'_>,
) -> Result<(), Error> {
    if count == 0 {
        return Ok(());
    }
    let mut buffer = [0; CONTROL_CHUNK_BYTES];
    let mut index = 0;
    while index < count {
        let rows = usize::try_from((count - index).min((buffer.len() / width) as u64))
            .expect("bounded control chunk");
        let bytes = rows * width;
        memory.read(offset, &mut buffer[..bytes]);
        for row in buffer[..bytes].chunks_exact(width) {
            visit(index, row)?;
            index += 1;
        }
        offset += bytes as u64;
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> u64 {
    hash::fnv64(hash::FNV_OFFSET, bytes)
}
fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn put(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Root {
    generation: u64,
    allocations: u64,
    buckets: u64,
    count: u64,
    used: u64,
}
impl Root {
    fn encode(self, dirty: bool) -> [u8; ROOT_BYTES] {
        let mut bytes = [0; ROOT_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        for (index, value) in [
            self.generation,
            self.allocations,
            self.buckets,
            self.count,
            self.used,
            u64::from(dirty),
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut bytes, 8 + index * 8, value);
        }
        let checksum = digest(&bytes[..56]);
        put(&mut bytes, 56, checksum);
        bytes
    }
    fn decode(bytes: &[u8; ROOT_BYTES]) -> Result<Self, Error> {
        if &bytes[..8] != MAGIC || word(bytes, 48) != 0 || word(bytes, 56) != digest(&bytes[..56]) {
            return Err(Error::Invalid("bucket root or interrupted publication"));
        }
        let root = Self {
            generation: word(bytes, 8),
            allocations: word(bytes, 16),
            buckets: word(bytes, 24),
            count: word(bytes, 32),
            used: word(bytes, 40),
        };
        if root.count > root.allocations || root.used > root.buckets {
            return Err(Error::Invalid("bucket table bounds"));
        }
        root.end()?;
        Ok(root)
    }
    fn owners(self) -> Result<u64, Error> {
        self.allocations
            .checked_mul(ENTRY_BYTES)
            .and_then(|bytes| bytes.checked_add(ROOT_BYTES as u64))
            .ok_or(Error::Budget)
    }
    fn data_start(self) -> Result<u64, Error> {
        self.buckets
            .checked_mul(OWNER_BYTES)
            .and_then(|bytes| bytes.checked_add(self.owners().ok()?))
            .map(|bytes| bytes.div_ceil(PAGE))
            .ok_or(Error::Budget)
    }
    fn end(self) -> Result<u64, Error> {
        self.used
            .checked_mul(BUCKET_PAGES)
            .and_then(|pages| pages.checked_add(self.data_start().ok()?))
            .filter(|pages| pages.checked_mul(PAGE).is_some())
            .ok_or(Error::Budget)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub key: StableKey,
    pub pages: u64,
    buckets: Vec<u64>,
}
impl Entry {
    fn size_bytes(pages: u64) -> [u8; 16] {
        let mut bytes = [0; 16];
        put(&mut bytes, 0, pages);
        let checksum = digest(&bytes[..8]);
        put(&mut bytes, 8, checksum);
        bytes
    }
    fn encode(&self) -> [u8; ENTRY_SIZE] {
        let mut bytes = [0; ENTRY_SIZE];
        bytes[..16].copy_from_slice(&Self::size_bytes(self.pages));
        let key = self.key.as_str().as_bytes();
        put(&mut bytes, 16, key.len() as u64);
        bytes[24..24 + key.len()].copy_from_slice(key);
        let checksum = digest(&bytes[16..152]);
        put(&mut bytes, 152, checksum);
        bytes
    }
    fn decode(bytes: &[u8; ENTRY_SIZE]) -> Result<Self, Error> {
        let length = usize::try_from(word(bytes, 16)).map_err(|_| Error::Invalid("name length"))?;
        if length > 128
            || word(bytes, 8) != digest(&bytes[..8])
            || word(bytes, 152) != digest(&bytes[16..152])
            || bytes[24 + length..152].iter().any(|&byte| byte != 0)
        {
            return Err(Error::Invalid("allocation row"));
        }
        let text = std::str::from_utf8(&bytes[24..24 + length])
            .map_err(|_| Error::Invalid("name text"))?;
        let key = StableKey::parse(text).map_err(|_| Error::Invalid("name grammar"))?;
        Ok(Self {
            key,
            pages: word(bytes, 0),
            buckets: Vec::new(),
        })
    }
}

// Keep indexed name loads and bounds checks out of each sorting-loop branch.
#[inline(never)]
fn compare_keys(entries: &[Entry], a: usize, b: usize) -> std::cmp::Ordering {
    entries[a].key.cmp(&entries[b].key)
}

pub struct Store<M: Memory> {
    memory: M,
    root: Root,
    budget: StorageConfig,
    pub(super) entries: Vec<Entry>,
    key_order: Vec<usize>,
    poisoned: Cell<bool>,
    // ICSS's last-bucket translation cache; additionally bounds it by logical
    // capacity so spare bytes in the final bucket never become accessible.
    last_span: Cell<Option<Span>>,
}
#[derive(Clone, Copy)]
struct Span {
    id: AllocationId,
    start: u64,
    end: u64,
    physical: u64,
}

impl<M: Memory> Store<M> {
    pub(super) fn create(memory: M, budget: StorageConfig) -> Result<Self, Error> {
        if memory.size() != 0 {
            return Err(Error::Invalid("create requires empty backing"));
        }
        let root = Root {
            generation: 0,
            allocations: budget.allocations as u64,
            buckets: budget.extents as u64,
            count: 0,
            used: 0,
        };
        let pages = root.end()?;
        if pages > budget.pages {
            return Err(Error::Budget);
        }
        if memory.grow(pages) == -1 {
            return Err(Error::Growth);
        }
        memory.write(0, &root.encode(false));
        Ok(Self {
            memory,
            root,
            budget,
            entries: Vec::new(),
            key_order: Vec::new(),
            poisoned: Cell::new(false),
            last_span: Cell::new(None),
        })
    }
    pub(super) fn open(memory: M, budget: StorageConfig) -> Result<Self, Error> {
        let physical = memory.size();
        if physical == 0 || physical > budget.pages {
            return Err(Error::Budget);
        }
        let mut bytes = [0; ROOT_BYTES];
        memory.read(0, &mut bytes);
        let root = Root::decode(&bytes)?;
        if root.end()? != physical
            || root.count > budget.allocations as u64
            || root.used > budget.extents as u64
        {
            return Err(Error::Invalid("allocation capacity or backing extent"));
        }
        // The validated root already gives the exact live row count. Avoid
        // repeatedly moving admitted entries while reconstructing the directory.
        let mut entries =
            Vec::with_capacity(usize::try_from(root.count).map_err(|_| Error::Budget)?);
        read_rows(
            &memory,
            ROOT_BYTES as u64,
            root.count,
            ENTRY_SIZE,
            &mut |_, bytes| {
                let entry = Entry::decode(bytes.try_into().unwrap())?;
                if entry.pages > budget.pages {
                    return Err(Error::Invalid("logical size"));
                }
                entries.push(entry);
                Ok(())
            },
        )?;
        // Entries retain append-order IDs; this index owns no duplicate names.
        let mut key_order: Vec<_> = (0..entries.len()).collect();
        key_order.sort_unstable_by(|&a, &b| compare_keys(&entries, a, b));
        if key_order
            .windows(2)
            .any(|pair| entries[pair[0]].key == entries[pair[1]].key)
        {
            return Err(Error::Invalid("duplicate name"));
        }
        // ICSS rebuilds each memory's ordered bucket vector from physical owner
        // rows. Read only the committed rows, not the reserved table capacity.
        read_rows(
            &memory,
            root.owners()?,
            root.used,
            OWNER_SIZE,
            &mut |bucket, bytes| {
                let owner = word(bytes, 0);
                if owner == 0 || owner > root.count || word(bytes, 8) != digest(&bytes[..8]) {
                    return Err(Error::Invalid("bucket owner"));
                }
                entries[usize::try_from(owner - 1).map_err(|_| Error::Budget)?]
                    .buckets
                    .push(bucket);
                Ok(())
            },
        )?;
        if entries
            .iter()
            .any(|entry| entry.pages.div_ceil(BUCKET_PAGES) != entry.buckets.len() as u64)
        {
            return Err(Error::Invalid("bucket count does not cover logical size"));
        }
        Ok(Self {
            memory,
            root,
            budget,
            entries,
            key_order,
            poisoned: Cell::new(false),
            last_span: Cell::new(None),
        })
    }
    pub(super) fn allocation_id(&self, key: &StableKey) -> Option<AllocationId> {
        self.key_order
            .binary_search_by(|&index| self.entries[index].key.cmp(key))
            .ok()
            .map(|position| AllocationId(self.key_order[position] as u64 + 1))
    }
    pub(super) const fn healthy(&self) -> Result<(), Error> {
        if self.poisoned.get() {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poison(&self) {
        self.poisoned.set(true);
    }
    pub(super) const fn generation(&self) -> u64 {
        self.root.generation
    }
    pub(super) fn verify(&self) -> Result<(), Error> {
        self.healthy()?;
        self.poisoned.set(true);
        // Full integrity checks remain at admission and explicit diagnostics.
        // Lease opening, growth and ordinary IO trust continuous custody.
        let changed = || Error::Invalid("backing changed outside its owner");
        if self.memory.size() != self.root.end()? {
            return Err(changed());
        }
        let mut root = [0; ROOT_BYTES];
        self.memory.read(0, &mut root);
        if root != self.root.encode(false) {
            return Err(changed());
        }
        // Admitted rows have one canonical encoding. Comparing it also checks
        // checksums, padding and names, without allocating another name index.
        read_rows(
            &self.memory,
            ROOT_BYTES as u64,
            self.root.count,
            ENTRY_SIZE,
            &mut |index, bytes| {
                if bytes
                    != self.entries[usize::try_from(index).expect("admitted allocation index")]
                        .encode()
                {
                    return Err(changed());
                }
                Ok(())
            },
        )?;
        read_rows(
            &self.memory,
            self.root.owners()?,
            self.root.used,
            OWNER_SIZE,
            &mut |bucket, bytes| {
                let owner = word(bytes, 0);
                if owner == 0 || owner > self.root.count || word(bytes, 8) != digest(&bytes[..8]) {
                    return Err(changed());
                }
                // Growth appends physical bucket IDs in increasing order. Checking
                // every committed row against that admitted vector proves ownership
                // without constructing a second vector or reverse ownership index.
                if self.entries[usize::try_from(owner - 1).map_err(|_| Error::Budget)?]
                    .buckets
                    .binary_search(&bucket)
                    .is_err()
                {
                    return Err(changed());
                }
                Ok(())
            },
        )?;
        self.poisoned.set(false);
        Ok(())
    }
    pub(super) fn bind(&mut self, keys: &BTreeSet<StableKey>) -> Result<(), Error> {
        self.healthy()?;
        let added: Vec<_> = keys
            .iter()
            .filter(|key| self.allocation_id(key).is_none())
            .cloned()
            .collect();
        if added.is_empty() {
            return Ok(());
        }
        let count = self
            .entries
            .len()
            .checked_add(added.len())
            .ok_or(Error::Budget)?;
        if count > self.budget.allocations || count as u64 > self.root.allocations {
            return Err(Error::Budget);
        }
        self.verify()?;
        let mut next = self.root;
        next.generation = next.generation.checked_add(1).ok_or(Error::Budget)?;
        next.count = count as u64;
        self.entries.reserve(added.len());
        self.poisoned.set(true);
        #[cfg(not(target_arch = "wasm32"))]
        self.memory.write(0, &self.root.encode(true));
        for (index, key) in added.iter().enumerate() {
            let entry = Entry {
                key: key.clone(),
                pages: 0,
                buckets: Vec::new(),
            };
            self.memory.write(
                ROOT_BYTES as u64 + (self.root.count + index as u64) * ENTRY_BYTES,
                &entry.encode(),
            );
        }
        self.memory.write(0, &next.encode(false));
        for key in added {
            let position = self
                .key_order
                .partition_point(|&index| self.entries[index].key < key);
            self.key_order.insert(position, self.entries.len());
            self.entries.push(Entry {
                key,
                pages: 0,
                buckets: Vec::new(),
            });
        }
        self.root = next;
        self.poisoned.set(false);
        Ok(())
    }
    pub(super) fn grow(&mut self, index: usize, pages: u64) -> Result<u64, Error> {
        self.healthy()?;
        let entry = self.entries.get(index).ok_or(Error::Missing)?;
        let old_size = entry.pages;
        let new_size = old_size
            .checked_add(pages)
            .ok_or(Error::ArithmeticOverflow)?;
        if pages == 0 {
            return Ok(old_size);
        }
        // ICSS growth: allocate only buckets not covered by the current reserve.
        let current_buckets = old_size.div_ceil(BUCKET_PAGES);
        let required_buckets = new_size.div_ceil(BUCKET_PAGES);
        let new_buckets_needed = required_buckets - current_buckets;
        let mut next = self.root;
        next.used = next
            .used
            .checked_add(new_buckets_needed)
            .ok_or(Error::Budget)?;
        next.generation = next.generation.checked_add(1).ok_or(Error::Budget)?;
        if next.used > self.root.buckets
            || next.used > self.budget.extents as u64
            || next.end()? > self.budget.pages
        {
            return Err(Error::Budget);
        }
        self.entries[index]
            .buckets
            .reserve(usize::try_from(new_buckets_needed).map_err(|_| Error::Budget)?);
        let physical = self.root.end()?;
        let end = next.end()?;
        self.poisoned.set(true);
        // Refusal happens before any owner/size/header write. Never publish an
        // ICSS-style partially updated in-memory bucket vector on this error.
        if end > physical && self.memory.grow(end - physical) == -1 {
            self.poisoned.set(false);
            return Err(Error::Growth);
        }
        // A native interruption is fail-closed, not a second transaction log.
        // IC executions roll these writes and physical growth back on a trap.
        #[cfg(not(target_arch = "wasm32"))]
        self.memory.write(0, &self.root.encode(true));
        let mut owner = [0; OWNER_SIZE];
        put(&mut owner, 0, index as u64 + 1);
        let checksum = digest(&owner[..8]);
        put(&mut owner, 8, checksum);
        for bucket in self.root.used..next.used {
            self.memory
                .write(self.root.owners()? + bucket * OWNER_BYTES, &owner);
        }
        // Names are immutable after admission. Growing a memory updates only
        // its independently checked size, without touching name bytes in heap
        // or stable memory.
        self.memory.write(
            ROOT_BYTES as u64 + index as u64 * ENTRY_BYTES,
            &Entry::size_bytes(new_size),
        );
        self.memory.write(0, &next.encode(false));
        self.entries[index]
            .buckets
            .extend(self.root.used..next.used);
        self.entries[index].pages = new_size;
        self.root = next;
        self.last_span.set(None);
        self.poisoned.set(false);
        Ok(old_size)
    }
    #[inline]
    pub(super) fn pages(&self, index: usize) -> Result<u64, Error> {
        self.healthy()?;
        Ok(self.entries.get(index).ok_or(Error::Missing)?.pages)
    }
    pub(super) fn diagnostics(&self) -> Result<crate::StorageDiagnostics, Error> {
        self.verify()?;
        let physical_pages = self.memory.size();
        let data_pages = self.entries.iter().map(|entry| entry.pages).sum::<u64>();
        let metadata_pages = self.root.data_start()? - 1;
        Ok(crate::StorageDiagnostics {
            generation: self.root.generation,
            allocations: self.entries.len(),
            admitted_allocations: 0,
            live_leases: 0,
            extents: usize::try_from(self.root.used).expect("admitted bucket count"),
            data_pages,
            metadata_pages,
            directory_bytes: ROOT_BYTES as u64
                + self.root.count * ENTRY_BYTES
                + self.root.used * OWNER_BYTES,
            physical_pages,
            unassigned_pages: physical_pages - 1 - metadata_pages - data_pages,
        })
    }
    #[inline]
    fn access(
        &self,
        id: AllocationId,
        offset: u64,
        length: usize,
        mut io: impl FnMut(u64, std::ops::Range<usize>),
    ) -> Result<(), Error> {
        self.healthy()?;
        let end = offset.checked_add(length as u64).ok_or(Error::Bounds)?;
        if let Some(span) = self.last_span.get()
            && span.id == id
            && offset >= span.start
            && end <= span.end
        {
            self.poisoned.set(true);
            if length != 0 {
                io(span.physical + offset - span.start, 0..length);
            }
            self.poisoned.set(false);
            return Ok(());
        }
        self.access_uncached(id, offset, length, end, io)
    }
    #[inline(never)]
    fn access_uncached(
        &self,
        id: AllocationId,
        offset: u64,
        length: usize,
        end: u64,
        mut io: impl FnMut(u64, std::ops::Range<usize>),
    ) -> Result<(), Error> {
        let index = usize::try_from(id.0.checked_sub(1).ok_or(Error::Missing)?)
            .map_err(|_| Error::Missing)?;
        let entry = self.entries.get(index).ok_or(Error::Missing)?;
        let capacity = entry.pages.checked_mul(PAGE).ok_or(Error::Bounds)?;
        if end > capacity {
            return Err(Error::Bounds);
        }
        self.poisoned.set(true);
        // Adapted from ICSS MemoryManagerInner::for_each_bucket: split a
        // virtual segment at bucket boundaries and translate each bucket ID.
        let buckets = entry.buckets.as_slice();
        let mut remaining = length;
        let mut bucket_idx = usize::try_from(offset / BUCKET_BYTES).expect("admitted bucket index");
        let mut start_offset_in_bucket = offset % BUCKET_BYTES;
        let data_start = self.root.data_start()? * PAGE;
        while remaining > 0 {
            let bucket_address = data_start + buckets[bucket_idx] * BUCKET_BYTES;
            let bucket_start = bucket_idx as u64 * BUCKET_BYTES;
            let mut run_end = bucket_idx;
            let mut segment_len = (BUCKET_BYTES - start_offset_in_bucket).min(remaining as u64);
            // Coalesce only buckets actually requested by this IO. No scan of
            // cold population, no second extent index and no repair on growth.
            while segment_len < remaining as u64
                && buckets.get(run_end + 1) == Some(&(buckets[run_end] + 1))
            {
                run_end += 1;
                segment_len += BUCKET_BYTES.min(remaining as u64 - segment_len);
            }
            let segment_len = usize::try_from(segment_len).expect("bounded chunk");
            self.last_span.set(Some(Span {
                id,
                start: bucket_start,
                end: ((run_end as u64 + 1) * BUCKET_BYTES).min(capacity),
                physical: bucket_address,
            }));
            let done = length - remaining;
            io(
                bucket_address + start_offset_in_bucket,
                done..done + segment_len,
            );
            remaining -= segment_len;
            bucket_idx = run_end + 1;
            start_offset_in_bucket = 0;
        }
        self.poisoned.set(false);
        Ok(())
    }
    #[inline]
    pub(super) fn read_data(
        &self,
        id: AllocationId,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.access(id, offset, bytes.len(), |physical, range| {
            self.memory.read(physical, &mut bytes[range]);
        })
    }
    #[inline]
    #[expect(
        unsafe_code,
        reason = "Forward completely preflighted chunks to the backing's initialized-read contract"
    )]
    pub(super) unsafe fn read_data_unsafe(
        &self,
        id: AllocationId,
        offset: u64,
        dst: *mut u8,
        count: usize,
    ) -> Result<(), Error> {
        self.access(id, offset, count, |physical, range| {
            // SAFETY: ranges partition the admitted caller-owned destination.
            unsafe {
                self.memory
                    .read_unsafe(physical, dst.add(range.start), range.len());
            }
        })
    }
    #[inline]
    pub(super) fn write_data(
        &self,
        id: AllocationId,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.access(id, offset, bytes.len(), |physical, range| {
            self.memory.write(physical, &bytes[range]);
        })
    }
}

#[cfg(test)]
mod tests;
