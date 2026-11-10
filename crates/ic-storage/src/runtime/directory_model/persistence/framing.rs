//! Canonical controls for the STORNAM1 image. Checks detect accidental damage;
//! they do not authenticate storage supplied by an adversary.

use crate::{
    Memory,
    hash::{FNV_OFFSET, fnv64},
};

use super::{Error, StableKey, StorageConfig};

pub(super) const PAGE_BYTES: u64 = 65_536;
pub(super) const UNIT_PAGES: u64 = 8;
pub(super) const UNIT_BYTES: u64 = PAGE_BYTES * UNIT_PAGES;
pub(super) const ROOT_BYTES: usize = 96;
pub(super) const ENTRY_BYTES: usize = 168;
pub(super) const UNIT_ROW_BYTES: usize = 24;
const MAGIC: &[u8; 8] = b"STORNAM1";

///
/// Limits
///
/// Capacity bounds carried independently of the runtime's configuration type.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Limits {
    pub allocations: u64,
    pub extents: u64,
    pub pages: u64,
}

impl Limits {
    pub fn new(config: StorageConfig) -> Result<Self, Error> {
        Ok(Self {
            allocations: u64::try_from(config.allocations)
                .map_err(|_| Error::ArithmeticOverflow)?,
            extents: u64::try_from(config.extents).map_err(|_| Error::ArithmeticOverflow)?,
            pages: config.pages,
        })
    }

    pub fn intersect(self, capacity: Self) -> Self {
        Self {
            allocations: self.allocations.min(capacity.allocations),
            extents: self.extents.min(capacity.extents),
            pages: self.pages.min(capacity.pages),
        }
    }
}

///
/// Layout
///
/// Fixed table positions derived from persisted image capacities.

#[derive(Clone, Copy)]
pub(super) struct Layout {
    pub metadata_pages: u64,
    pub payload_page: u64,
    units_offset: u64,
}

impl Layout {
    pub fn new(limits: Limits) -> Result<Self, Error> {
        let entry_bytes = limits
            .allocations
            .checked_mul(ENTRY_BYTES as u64)
            .ok_or(Error::ArithmeticOverflow)?;
        let unit_bytes = limits
            .extents
            .checked_mul(UNIT_ROW_BYTES as u64)
            .ok_or(Error::ArithmeticOverflow)?;
        let table_bytes = entry_bytes
            .checked_add(unit_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        let metadata_pages = table_bytes.div_ceil(PAGE_BYTES);
        let payload_page = metadata_pages
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if payload_page > limits.pages {
            return Err(Error::Budget);
        }
        limits
            .pages
            .checked_mul(PAGE_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let units_offset = PAGE_BYTES
            .checked_add(entry_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Self {
            metadata_pages,
            payload_page,
            units_offset,
        })
    }

    pub fn entry_offset(index: usize) -> Result<u64, Error> {
        (index as u64)
            .checked_mul(ENTRY_BYTES as u64)
            .and_then(|offset| offset.checked_add(PAGE_BYTES))
            .ok_or(Error::ArithmeticOverflow)
    }

    pub fn unit_offset(self, index: u64) -> Result<u64, Error> {
        index
            .checked_mul(UNIT_ROW_BYTES as u64)
            .and_then(|offset| offset.checked_add(self.units_offset))
            .ok_or(Error::ArithmeticOverflow)
    }
}

///
/// Root
///
/// The sole publication control for the directory's complete active state.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Root {
    pub capacities: Limits,
    pub generation: u64,
    pub entries: u64,
    pub units: u64,
    pub digest: u64,
}

impl Root {
    pub fn physical_pages(self, layout: Layout) -> Result<u64, Error> {
        self.units
            .checked_mul(UNIT_PAGES)
            .and_then(|pages| pages.checked_add(layout.payload_page))
            .ok_or(Error::ArithmeticOverflow)
    }

    pub fn encode(self, layout: Layout) -> [u8; ROOT_BYTES] {
        let mut bytes = [0; ROOT_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        for (index, value) in [
            1,
            self.capacities.allocations,
            self.capacities.extents,
            self.capacities.pages,
            self.generation,
            self.entries,
            self.units,
            layout.metadata_pages,
            self.physical_pages(layout)
                .expect("preflighted physical size"),
            self.digest,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut bytes, (index + 1) * 8, value);
        }
        seal(&mut bytes);
        bytes
    }

    pub fn decode(bytes: &[u8; ROOT_BYTES]) -> Result<Self, Error> {
        if &bytes[..8] != MAGIC || get(bytes, 8) != 1 {
            return Err(Error::Invalid("unsupported directory framing"));
        }
        check(bytes)?;
        let root = Self {
            capacities: Limits {
                allocations: get(bytes, 16),
                extents: get(bytes, 24),
                pages: get(bytes, 32),
            },
            generation: get(bytes, 40),
            entries: get(bytes, 48),
            units: get(bytes, 56),
            digest: get(bytes, 80),
        };
        let layout = Layout::new(root.capacities)?;
        if get(bytes, 64) != layout.metadata_pages
            || get(bytes, 72) != root.physical_pages(layout)?
        {
            return Err(Error::Invalid("invalid directory geometry"));
        }
        Ok(root)
    }

    pub fn admit(self, limits: Limits, layout: Layout, physical_pages: u64) -> Result<(), Error> {
        if self.entries > limits.allocations
            || self.units > limits.extents
            || physical_pages > limits.pages
        {
            return Err(Error::Budget);
        }
        if physical_pages != self.physical_pages(layout)? {
            return Err(Error::Invalid("unexplained backing size"));
        }
        Ok(())
    }
}

/// Read only active rows, using a fixed scratch buffer independent of capacity.
pub(super) fn scan<M: Memory, const N: usize>(
    memory: &M,
    start: u64,
    count: u64,
    mut visit: impl FnMut(u64, &[u8; N]) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut scratch = [0; 4032];
    let mut done = 0_u64;
    while done < count {
        let rows = (count - done).min((scratch.len() / N) as u64);
        let length = usize::try_from(rows).expect("row count is bounded by scratch capacity") * N;
        let offset = done
            .checked_mul(N as u64)
            .and_then(|offset| start.checked_add(offset))
            .ok_or(Error::ArithmeticOverflow)?;
        memory.read(offset, &mut scratch[..length]);
        for (index, row) in scratch[..length].as_chunks::<N>().0.iter().enumerate() {
            visit(done + index as u64, row)?;
        }
        done += rows;
    }
    Ok(())
}

pub(super) const fn units_for(pages: u64) -> u64 {
    pages.div_ceil(UNIT_PAGES)
}

pub(super) fn encode_entry(
    index: usize,
    key: &StableKey,
    pages: u64,
    units: u64,
) -> [u8; ENTRY_BYTES] {
    let mut row = [0; ENTRY_BYTES];
    put(&mut row, 0, index as u64 + 1);
    put(&mut row, 8, pages);
    put(&mut row, 16, units);
    let name = key.as_str().as_bytes();
    put(&mut row, 24, name.len() as u64);
    row[32..32 + name.len()].copy_from_slice(name);
    seal(&mut row);
    row
}

pub(super) fn decode_entry(
    row: &[u8; ENTRY_BYTES],
    index: usize,
) -> Result<(StableKey, u64, u64, u64), Error> {
    check(row)?;
    if get(row, 0) != index as u64 + 1 {
        return Err(Error::Invalid("invalid allocation identity"));
    }
    let length = usize::try_from(get(row, 24)).map_err(|_| Error::ArithmeticOverflow)?;
    if length > 128 || row[32 + length..160].iter().any(|byte| *byte != 0) {
        return Err(Error::Invalid("invalid allocation name encoding"));
    }
    let name = std::str::from_utf8(&row[32..32 + length])
        .map_err(|_| Error::Invalid("invalid allocation name encoding"))?;
    let key = StableKey::parse(name).map_err(|_| Error::Invalid("invalid allocation name"))?;
    Ok((key, get(row, 8), get(row, 16), checksum(row)))
}

pub(super) fn encode_unit(physical: u64, owner: u64, logical: u64) -> [u8; UNIT_ROW_BYTES] {
    let mut row = [0; UNIT_ROW_BYTES];
    put(&mut row, 0, owner);
    put(&mut row, 8, logical);
    let digest = fnv64(fnv64(FNV_OFFSET, &physical.to_le_bytes()), &row[..16]);
    put(&mut row, 16, digest);
    row
}

pub(super) fn decode_unit(
    row: &[u8; UNIT_ROW_BYTES],
    physical: u64,
) -> Result<(u64, u64, u64), Error> {
    if fnv64(fnv64(FNV_OFFSET, &physical.to_le_bytes()), &row[..16]) != checksum(row) {
        return Err(Error::Invalid("damaged ownership control"));
    }
    Ok((get(row, 0), get(row, 8), checksum(row)))
}

pub(super) fn checksum(bytes: &[u8]) -> u64 {
    get(bytes, bytes.len() - 8)
}

fn get(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed-width control"),
    )
}

fn put(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn seal(bytes: &mut [u8]) {
    let end = bytes.len() - 8;
    let digest = fnv64(FNV_OFFSET, &bytes[..end]);
    put(bytes, end, digest);
}

fn check(bytes: &[u8]) -> Result<(), Error> {
    if fnv64(FNV_OFFSET, &bytes[..bytes.len() - 8]) != checksum(bytes) {
        return Err(Error::Invalid("damaged directory control"));
    }
    Ok(())
}
