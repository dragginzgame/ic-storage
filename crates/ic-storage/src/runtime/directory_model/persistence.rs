//! Durable named allocations and translation of their admitted byte ranges.
//!
//! The image reserves its metadata tables once, then appends eight-page payload
//! units. Names are held once; a sorted index provides allocation lookup. Grants,
//! custody, leases, and application framing remain with the runtime.
//! Default capacities reserve seventeen metadata pages and one protected root
//! page up front. This fixed cost avoids metadata relocation during payload growth.

use std::{cell::Cell, collections::BTreeSet};

use crate::{Memory, StorageDiagnostics};

use super::{AllocationId, Error, StableKey, StorageConfig};
use framing::{ENTRY_BYTES, Layout, Limits, PAGE_BYTES, ROOT_BYTES, Root, UNIT_BYTES};

mod framing;
#[cfg(test)]
mod named_backing_tests;

///
/// Entry
///
/// An admitted name and its logical size, with a private physical-unit index.
pub(super) struct Entry {
    pub key: StableKey,
    pub pages: u64,
    units: Vec<u64>,
}

///
/// Store
///
/// Owns one admitted backing image and its validated translation metadata.
pub struct Store<M: Memory> {
    pub(super) entries: Vec<Entry>,
    memory: M,
    limits: Limits,
    root: Root,
    layout: Layout,
    names: Vec<usize>,
    poisoned: Cell<bool>,
}

impl<M: Memory> Store<M> {
    pub(super) fn create(memory: M, budget: StorageConfig) -> Result<Self, Error> {
        let limits = Limits::new(budget)?;
        let layout = Layout::new(limits)?;
        if memory.size() != 0 {
            return Err(Error::Invalid("creation requires empty backing"));
        }
        let root = Root {
            capacities: limits,
            generation: 0,
            entries: 0,
            units: 0,
            digest: 0,
        };
        let store = Self {
            entries: Vec::new(),
            memory,
            limits,
            root,
            layout,
            names: Vec::new(),
            poisoned: Cell::new(false),
        };
        let previous = store.io(|memory| memory.grow(layout.payload_page));
        if previous < 0 {
            return Err(Error::Growth);
        }
        if previous != 0 {
            store.poisoned.set(true);
            return Err(Error::Invalid("unexpected backing growth result"));
        }
        store.write_root(root);
        Ok(store)
    }

    pub(super) fn open(memory: M, budget: StorageConfig) -> Result<Self, Error> {
        let limits = Limits::new(budget)?;
        let physical_pages = memory.size();
        if physical_pages == 0 {
            return Err(Error::Invalid("missing directory image"));
        }
        let mut bytes = [0; ROOT_BYTES];
        memory.read(0, &mut bytes);
        let root = Root::decode(&bytes)?;
        let limits = limits.intersect(root.capacities);
        let layout = Layout::new(root.capacities)?;
        root.admit(limits, layout, physical_pages)?;
        let count = usize::try_from(root.entries).map_err(|_| Error::ArithmeticOverflow)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| Error::Budget)?;
        let mut digest = 0;
        let mut declared_units = 0_u64;
        framing::scan::<_, ENTRY_BYTES>(
            &memory,
            Layout::entry_offset(0)?,
            root.entries,
            |index, row| {
                let index = usize::try_from(index).map_err(|_| Error::ArithmeticOverflow)?;
                let (key, pages, unit_count, checksum) = framing::decode_entry(row, index)?;
                if unit_count != framing::units_for(pages) || unit_count > root.units {
                    return Err(Error::Invalid("invalid allocation coverage"));
                }
                declared_units = declared_units
                    .checked_add(unit_count)
                    .ok_or(Error::ArithmeticOverflow)?;
                if declared_units > root.units {
                    return Err(Error::Invalid("excess allocation coverage"));
                }
                let mut units = Vec::new();
                units
                    .try_reserve_exact(
                        usize::try_from(unit_count).map_err(|_| Error::ArithmeticOverflow)?,
                    )
                    .map_err(|_| Error::Budget)?;
                entries.push(Entry { key, pages, units });
                digest ^= checksum;
                Ok(())
            },
        )?;
        let mut names: Vec<_> = (0..count).collect();
        names.sort_unstable_by(|left, right| entries[*left].key.cmp(&entries[*right].key));
        if names
            .windows(2)
            .any(|pair| entries[pair[0]].key == entries[pair[1]].key)
        {
            return Err(Error::Invalid("duplicate allocation name"));
        }
        framing::scan::<_, { framing::UNIT_ROW_BYTES }>(
            &memory,
            layout.unit_offset(0)?,
            root.units,
            |unit, row| {
                let (owner, logical, checksum) = framing::decode_unit(row, unit)?;
                let index = owner
                    .checked_sub(1)
                    .ok_or(Error::Invalid("invalid unit owner"))?;
                let entry = entries
                    .get_mut(usize::try_from(index).map_err(|_| Error::ArithmeticOverflow)?)
                    .ok_or(Error::Invalid("invalid unit owner"))?;
                if logical != entry.units.len() as u64 || logical >= framing::units_for(entry.pages)
                {
                    return Err(Error::Invalid("invalid unit ownership order"));
                }
                entry.units.push(unit);
                digest ^= checksum;
                Ok(())
            },
        )?;
        if digest != root.digest
            || entries
                .iter()
                .any(|entry| entry.units.len() as u64 != framing::units_for(entry.pages))
        {
            return Err(Error::Invalid("incomplete directory controls"));
        }
        Ok(Self {
            entries,
            memory,
            limits,
            root,
            layout,
            names,
            poisoned: Cell::new(false),
        })
    }

    pub(super) fn allocation_id(&self, key: &StableKey) -> Option<AllocationId> {
        if self.healthy().is_err() {
            return None;
        }
        self.names
            .binary_search_by(|index| self.entries[*index].key.cmp(key))
            .ok()
            .map(|position| AllocationId(self.names[position] as u64 + 1))
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
        let result = self.verify_admitted();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }

    fn verify_admitted(&self) -> Result<(), Error> {
        let mut root_bytes = [0; ROOT_BYTES];
        self.io(|memory| memory.read(0, &mut root_bytes));
        if root_bytes != self.root.encode(self.layout)
            || self.io(Memory::size) != self.root.physical_pages(self.layout)?
        {
            return Err(Error::Invalid("directory changed after admission"));
        }
        let mut digest = 0;
        self.io(|memory| {
            framing::scan::<_, ENTRY_BYTES>(
                memory,
                Layout::entry_offset(0)?,
                self.root.entries,
                |index, row| {
                    let index = usize::try_from(index).map_err(|_| Error::ArithmeticOverflow)?;
                    let entry = self
                        .entries
                        .get(index)
                        .ok_or(Error::Invalid("missing admitted allocation"))?;
                    let expected = framing::encode_entry(
                        index,
                        &entry.key,
                        entry.pages,
                        entry.units.len() as u64,
                    );
                    if row != &expected {
                        return Err(Error::Invalid("allocation changed after admission"));
                    }
                    digest ^= framing::checksum(row);
                    Ok(())
                },
            )?;
            framing::scan::<_, { framing::UNIT_ROW_BYTES }>(
                memory,
                self.layout.unit_offset(0)?,
                self.root.units,
                |physical, row| {
                    let (owner, logical, checksum) = framing::decode_unit(row, physical)?;
                    let owner = owner
                        .checked_sub(1)
                        .ok_or(Error::Invalid("invalid unit owner"))?;
                    let entry = self
                        .entries
                        .get(usize::try_from(owner).map_err(|_| Error::ArithmeticOverflow)?)
                        .ok_or(Error::Invalid("invalid unit owner"))?;
                    if entry
                        .units
                        .get(usize::try_from(logical).map_err(|_| Error::ArithmeticOverflow)?)
                        != Some(&physical)
                    {
                        return Err(Error::Invalid("ownership changed after admission"));
                    }
                    digest ^= checksum;
                    Ok(())
                },
            )
        })?;
        if digest != self.root.digest {
            return Err(Error::Invalid("directory integrity mismatch"));
        }
        Ok(())
    }

    pub(super) fn bind(&mut self, keys: &BTreeSet<StableKey>) -> Result<(), Error> {
        self.healthy()?;
        let mut additions = Vec::new();
        for key in keys {
            if self.allocation_id(key).is_none() {
                if self.entries.len() as u64 + additions.len() as u64 >= self.limits.allocations {
                    return Err(Error::Budget);
                }
                additions.push(Entry {
                    key: key.clone(),
                    pages: 0,
                    units: Vec::new(),
                });
            }
        }
        if additions.is_empty() {
            return Ok(());
        }
        let count = self
            .entries
            .len()
            .checked_add(additions.len())
            .ok_or(Error::ArithmeticOverflow)?;
        if count as u64 > self.limits.allocations {
            return Err(Error::Budget);
        }
        let mut root = self.root;
        root.entries = count as u64;
        root.generation = root
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        self.entries
            .try_reserve(additions.len())
            .map_err(|_| Error::Budget)?;
        let old_count = self.entries.len();
        let mut names: Vec<_> = (0..count).collect();
        names.sort_unstable_by(|left, right| {
            let key = |index: usize| {
                if index < old_count {
                    &self.entries[index].key
                } else {
                    &additions[index - old_count].key
                }
            };
            key(*left).cmp(key(*right))
        });
        // Binding is a trust boundary: compare every retained control with the
        // admitted ownership before making new allocations durable.
        self.verify()?;
        #[cfg(not(target_arch = "wasm32"))]
        self.begin_publication();
        for (position, entry) in additions.iter().enumerate() {
            let index = old_count + position;
            let row = framing::encode_entry(index, &entry.key, 0, 0);
            root.digest ^= framing::checksum(&row);
            self.io(|memory| {
                memory.write(
                    Layout::entry_offset(index).expect("preflighted entry offset"),
                    &row,
                );
            });
        }
        self.write_root(root);
        self.entries.extend(additions);
        self.names = names;
        self.root = root;
        Ok(())
    }

    pub(super) fn grow(&mut self, index: usize, pages: u64) -> Result<u64, Error> {
        self.healthy()?;
        let entry = self.entries.get(index).ok_or(Error::Missing)?;
        let previous = entry.pages;
        if pages == 0 {
            return Ok(previous);
        }
        let next = previous
            .checked_add(pages)
            .ok_or(Error::ArithmeticOverflow)?;
        next.checked_mul(PAGE_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let old_units = entry.units.len() as u64;
        let extra = framing::units_for(next)
            .checked_sub(old_units)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut root = self.root;
        root.units = root
            .units
            .checked_add(extra)
            .ok_or(Error::ArithmeticOverflow)?;
        root.generation = root
            .generation
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let physical_pages = root.physical_pages(self.layout)?;
        if root.units > self.limits.extents || physical_pages > self.limits.pages {
            return Err(Error::Budget);
        }
        let row = framing::encode_entry(index, &entry.key, next, old_units + extra);
        root.digest ^= framing::checksum(&framing::encode_entry(
            index, &entry.key, previous, old_units,
        ));
        root.digest ^= framing::checksum(&row);
        self.entries[index]
            .units
            .try_reserve(usize::try_from(extra).map_err(|_| Error::ArithmeticOverflow)?)
            .map_err(|_| Error::Budget)?;
        let old_physical = self.root.physical_pages(self.layout)?;
        if extra != 0 {
            let grown = self.io(|memory| memory.grow(physical_pages - old_physical));
            if grown < 0 {
                return Err(Error::Growth);
            }
            if u64::try_from(grown).ok() != Some(old_physical) {
                self.poisoned.set(true);
                return Err(Error::Invalid("unexpected backing growth result"));
            }
        }
        // A native interruption after physical growth leaves an unexplained
        // tail; after this marker it leaves an invalid root. Wasm traps instead
        // roll back all effects, so the marker is unnecessary there.
        #[cfg(not(target_arch = "wasm32"))]
        self.begin_publication();
        for added in 0..extra {
            let unit_row =
                framing::encode_unit(self.root.units + added, index as u64 + 1, old_units + added);
            root.digest ^= framing::checksum(&unit_row);
            self.io(|memory| {
                memory.write(
                    self.layout
                        .unit_offset(self.root.units + added)
                        .expect("preflighted unit offset"),
                    &unit_row,
                );
            });
        }
        self.io(|memory| {
            memory.write(
                Layout::entry_offset(index).expect("admitted entry offset"),
                &row,
            );
        });
        self.write_root(root);
        self.entries[index]
            .units
            .extend(self.root.units..root.units);
        self.entries[index].pages = next;
        self.root = root;
        Ok(previous)
    }

    pub(super) fn pages(&self, index: usize) -> Result<u64, Error> {
        self.healthy()?;
        self.entries
            .get(index)
            .map(|entry| entry.pages)
            .ok_or(Error::Missing)
    }

    pub(super) fn diagnostics(&self) -> Result<StorageDiagnostics, Error> {
        self.verify()?;
        let data_pages = self.entries.iter().try_fold(0_u64, |total, entry| {
            total
                .checked_add(entry.pages)
                .ok_or(Error::ArithmeticOverflow)
        })?;
        let physical_pages = self.root.physical_pages(self.layout)?;
        Ok(StorageDiagnostics {
            generation: self.root.generation,
            allocations: self.entries.len(),
            admitted_allocations: 0,
            live_leases: 0,
            extents: usize::try_from(self.root.units).map_err(|_| Error::ArithmeticOverflow)?,
            data_pages,
            metadata_pages: self.layout.metadata_pages,
            directory_bytes: ROOT_BYTES as u64
                + self.root.entries * ENTRY_BYTES as u64
                + self.root.units * framing::UNIT_ROW_BYTES as u64,
            physical_pages,
            unassigned_pages: physical_pages - 1 - self.layout.metadata_pages - data_pages,
        })
    }

    pub(super) fn read_data(
        &self,
        id: AllocationId,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        let entry = self.range(id, offset, bytes.len())?;
        if bytes.is_empty() {
            return Ok(());
        }
        let (physical, count) = self.segment(entry, offset, bytes.len());
        if count == bytes.len() {
            self.io(|memory| memory.read(physical, bytes));
            return Ok(());
        }
        self.io(|memory| memory.read(physical, &mut bytes[..count]));
        let mut done = count;
        while done < bytes.len() {
            let (physical, count) = self.segment(entry, offset + done as u64, bytes.len() - done);
            self.io(|memory| memory.read(physical, &mut bytes[done..done + count]));
            done += count;
        }
        Ok(())
    }

    #[expect(
        unsafe_code,
        reason = "Translate a fully checked logical range into initializing reads of caller-owned memory"
    )]
    pub(super) unsafe fn read_data_unsafe(
        &self,
        id: AllocationId,
        offset: u64,
        dst: *mut u8,
        count: usize,
    ) -> Result<(), Error> {
        let entry = self.range(id, offset, count)?;
        if count == 0 {
            return Ok(());
        }
        let (physical, length) = self.segment(entry, offset, count);
        if length == count {
            // SAFETY: The admitted range is one contiguous physical span; the
            // caller supplies the entire writable, nonoverlapping destination.
            self.io(|memory| unsafe { memory.read_unsafe(physical, dst, count) });
            return Ok(());
        }
        // SAFETY: This first span is a bounded prefix of the caller's destination.
        self.io(|memory| unsafe { memory.read_unsafe(physical, dst, length) });
        let mut done = length;
        while done < count {
            let (physical, length) = self.segment(entry, offset + done as u64, count - done);
            // SAFETY: The caller provides count writable, nonoverlapping bytes.
            // The checked logical range and segment partition bound this write.
            self.io(|memory| unsafe { memory.read_unsafe(physical, dst.add(done), length) });
            done += length;
        }
        Ok(())
    }

    pub(super) fn write_data(
        &self,
        id: AllocationId,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let entry = self.range(id, offset, bytes.len())?;
        if bytes.is_empty() {
            return Ok(());
        }
        let (physical, count) = self.segment(entry, offset, bytes.len());
        if count == bytes.len() {
            self.io(|memory| memory.write(physical, bytes));
            return Ok(());
        }
        self.io(|memory| memory.write(physical, &bytes[..count]));
        let mut done = count;
        while done < bytes.len() {
            let (physical, count) = self.segment(entry, offset + done as u64, bytes.len() - done);
            self.io(|memory| memory.write(physical, &bytes[done..done + count]));
            done += count;
        }
        Ok(())
    }

    fn range(&self, id: AllocationId, offset: u64, count: usize) -> Result<&Entry, Error> {
        self.healthy()?;
        let index = id.0.checked_sub(1).ok_or(Error::Missing)?;
        let entry = self
            .entries
            .get(usize::try_from(index).map_err(|_| Error::Missing)?)
            .ok_or(Error::Missing)?;
        let end = offset
            .checked_add(u64::try_from(count).map_err(|_| Error::Bounds)?)
            .ok_or(Error::Bounds)?;
        if end > entry.pages * PAGE_BYTES {
            return Err(Error::Bounds);
        }
        Ok(entry)
    }

    fn segment(&self, entry: &Entry, offset: u64, count: usize) -> (u64, usize) {
        let logical = usize::try_from(offset / UNIT_BYTES).expect("admitted logical unit index");
        let within = offset % UNIT_BYTES;
        let physical =
            self.layout.payload_page * PAGE_BYTES + entry.units[logical] * UNIT_BYTES + within;
        let mut length = (UNIT_BYTES - within).min(count as u64);
        let mut current = logical;
        while length < count as u64
            && current + 1 < entry.units.len()
            && entry.units[current + 1] == entry.units[current] + 1
        {
            length += UNIT_BYTES.min(count as u64 - length);
            current += 1;
        }
        (
            physical,
            usize::try_from(length).expect("segment is bounded by the caller's slice length"),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn begin_publication(&self) {
        self.io(|memory| memory.write(0, &[0; 8]));
    }

    fn write_root(&self, root: Root) {
        self.io(|memory| memory.write(0, &root.encode(self.layout)));
    }

    fn io<T>(&self, operation: impl FnOnce(&M) -> T) -> T {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = PanicPoison(&self.poisoned);
        operation(&self.memory)
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct PanicPoison<'a>(&'a Cell<bool>);

#[cfg(not(target_arch = "wasm32"))]
impl Drop for PanicPoison<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.set(true);
        }
    }
}
