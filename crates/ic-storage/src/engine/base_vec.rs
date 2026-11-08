// Adapted from ic-stable-structures 0.7.2, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See the package LICENSE-APACHE and docs/storage/icss-source.json.
//! The common code for all Vec-like data structures.
//!
//! # V1 layout
//!
//! ```text
//! ---------------------------------------- <- Address 0
//! Magic                  ↕ 3 bytes
//! ----------------------------------------
//! Layout version         ↕ 1 byte
//! ----------------------------------------
//! Number of entries = L  ↕ 8 bytes
//! ----------------------------------------
//! Max entry size         ↕ 4 bytes
//! ----------------------------------------
//! Fixed size flag        ↕ 1 byte
//! ----------------------------------------
//! Reserved space         ↕ 47 bytes
//! ---------------------------------------- <- Address 64
//! E_0                    ↕ SLOT_SIZE bytes
//! ----------------------------------------
//! E_1                    ↕ SLOT_SIZE bytes
//! ----------------------------------------
//! ...
//! ----------------------------------------
//! E_(L-1)                ↕ SLOT_SIZE bytes
//! ----------------------------------------
//! Unallocated space
//! ```
//!
//! The `SLOT_SIZE` constant depends on the item type. If the item
//! type is fixed in size, the `SLOT_SIZE` is equal to the max size.
//! Otherwise, the `SLOT_SIZE` is the max size plus the number of
//! bytes required to represent integers up to that max size.
use crate::engine::storable::{bounds, bytes_to_store_size_bounded};
use crate::engine::{
    Address, GrowFailed, Memory, Storable, read_to_vec, read_u32, read_u64, safe_write, write,
    write_u64,
};
use std::borrow::{Borrow, Cow};
use std::cmp::min;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

const LAYOUT_VERSION: u8 = 1;
const DATA_OFFSET: u64 = 64;
const LEN_OFFSET: u64 = 4;

#[derive(Debug)]
struct HeaderV1 {
    magic: [u8; 3],
    version: u8,
    len: u64,
    max_size: u32,
    is_fixed_size: bool,
}

impl HeaderV1 {
    // Keep the active header in one IO range without touching reserved bytes.
    fn write<M: Memory>(&self, memory: &M) -> Result<(), GrowFailed> {
        let mut bytes = [0; 17];
        bytes[..3].copy_from_slice(&self.magic);
        bytes[3] = self.version;
        bytes[4..12].copy_from_slice(&self.len.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.max_size.to_le_bytes());
        bytes[16] = u8::from(self.is_fixed_size);
        safe_write(memory, 0, &bytes)
    }

    fn read<M: Memory>(memory: &M) -> Self {
        let mut bytes = [0; 17];
        memory.read(0, &mut bytes);
        Self {
            magic: bytes[..3].try_into().unwrap(),
            version: bytes[3],
            len: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            max_size: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            is_fixed_size: bytes[16] != 0,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum InitError {
    BadMagic { actual: [u8; 3], expected: [u8; 3] },
    IncompatibleVersion(u8),
    IncompatibleElementType,
}

impl fmt::Display for InitError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic { actual, expected } => {
                write!(fmt, "bad magic number {actual:?}, expected {expected:?}")
            }
            Self::IncompatibleVersion(version) => write!(
                fmt,
                "unsupported layout version {version}; supported version numbers are 1..={LAYOUT_VERSION}"
            ),
            Self::IncompatibleElementType => write!(
                fmt,
                "the bounds (either max_size or is_fixed_size) of the element type do not match the persisted vector attributes"
            ),
        }
    }
}

impl std::error::Error for InitError {}

pub struct BaseVec<T: Storable, M: Memory> {
    memory: M,
    _marker: PhantomData<T>,
}

impl<T: Storable, M: Memory> BaseVec<T, M> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) const fn memory(&self) -> &M {
        &self.memory
    }
    pub fn new(memory: M, magic: [u8; 3]) -> Result<Self, GrowFailed> {
        let t_bounds = bounds::<T>();

        let header = HeaderV1 {
            magic,
            version: LAYOUT_VERSION,
            len: 0,
            max_size: t_bounds.max_size,
            is_fixed_size: t_bounds.is_fixed_size,
        };
        header.write(&memory)?;
        Ok(Self {
            memory,
            _marker: PhantomData,
        })
    }
    pub fn init(memory: M, magic: [u8; 3]) -> Result<Self, InitError> {
        assert!(memory.size() != 0, "Cannot load empty vector memory");
        let header = HeaderV1::read(&memory);
        if header.magic != magic {
            return Err(InitError::BadMagic {
                actual: header.magic,
                expected: magic,
            });
        }
        if header.version != LAYOUT_VERSION {
            return Err(InitError::IncompatibleVersion(header.version));
        }
        let t_bounds = bounds::<T>();
        if header.max_size != t_bounds.max_size || header.is_fixed_size != t_bounds.is_fixed_size {
            return Err(InitError::IncompatibleElementType);
        }

        Ok(Self {
            memory,
            _marker: PhantomData,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn len(&self) -> u64 {
        read_u64(&self.memory, Address::from(LEN_OFFSET))
    }
    pub fn set(&self, index: u64, item: &T) {
        assert!(index < self.len());

        let offset = DATA_OFFSET + u64::from(slot_size::<T>()) * index;
        let bytes = item.to_bytes_checked();
        let data_offset = self
            .write_entry_size(
                offset,
                u32::try_from(bytes.len()).expect("vector encoded length"),
            )
            .expect("unreachable: cannot fail to write to pre-allocated area");
        write(&self.memory, data_offset, bytes.borrow());
    }
    pub fn get(&self, index: u64) -> Option<T> {
        if index < self.len() {
            Some(self.read_entry(index))
        } else {
            None
        }
    }
    pub fn push(&self, item: &T) -> Result<(), GrowFailed> {
        let index = self.len();
        let offset = DATA_OFFSET + u64::from(slot_size::<T>()) * index;
        let bytes = item.to_bytes_checked();
        let data_offset = self.write_entry_size(
            offset,
            u32::try_from(bytes.len()).expect("vector encoded length"),
        )?;
        safe_write(&self.memory, data_offset, bytes.borrow())?;
        // NB. We update the size only after we ensure that the data
        // write succeeded.
        self.set_len(index + 1);
        Ok(())
    }
    pub fn pop(&self) -> Option<T> {
        let len = self.len();
        if len == 0 {
            return None;
        }
        let value = self.read_entry(len - 1);
        self.set_len(len - 1);
        Some(value)
    }

    pub fn iter(&self) -> Iter<'_, T, M> {
        Iter {
            vec: self,
            buf: vec![],
            range: Range {
                start: 0,
                end: self.len(),
            },
        }
    }
    fn read_entry(&self, index: u64) -> T {
        // Common fixed-width scalars and digests need no temporary heap buffer.
        // Drop-bearing values keep owned bytes so their decoder can reuse them.
        let entry_bounds = bounds::<T>();
        if entry_bounds.is_fixed_size && entry_bounds.max_size <= 32 && !std::mem::needs_drop::<T>()
        {
            return self.read_small_entry(index);
        }
        let mut data = std::vec::Vec::new();
        self.read_entry_to(index, &mut data);
        T::from_bytes(Cow::Owned(data))
    }
    // Keep stack preparation out of the caller's out-of-range path.
    #[inline(never)]
    fn read_small_entry(&self, index: u64) -> T {
        let mut bytes = [0; 32];
        let bytes = &mut bytes[..bounds::<T>().max_size as usize];
        let offset = DATA_OFFSET + u64::from(slot_size::<T>()) * index;
        self.memory.read(offset, bytes);
        T::from_bytes(Cow::Borrowed(bytes))
    }
    fn read_entry_to(&self, index: u64, buf: &mut Vec<u8>) {
        let offset = DATA_OFFSET + u64::from(slot_size::<T>()) * index;
        let (data_offset, data_size) = self.read_entry_size(offset);
        assert!(
            data_size <= bounds::<T>().max_size as usize,
            "Vector slot length exceeds bound"
        );
        read_to_vec(&self.memory, data_offset.into(), buf, data_size);
    }
    fn set_len(&self, new_len: u64) {
        write_u64(&self.memory, Address::from(LEN_OFFSET), new_len);
    }
    fn write_entry_size(&self, offset: u64, size: u32) -> Result<u64, GrowFailed> {
        let t_bounds = bounds::<T>();
        debug_assert!(size <= t_bounds.max_size);

        if t_bounds.is_fixed_size {
            Ok(offset)
        } else if u8::try_from(t_bounds.max_size).is_ok() {
            safe_write(
                &self.memory,
                offset,
                &[u8::try_from(size).expect("vector one-byte length"); 1],
            )?;
            Ok(offset + 1)
        } else if u16::try_from(t_bounds.max_size).is_ok() {
            safe_write(
                &self.memory,
                offset,
                &(u16::try_from(size).expect("vector two-byte length")).to_le_bytes(),
            )?;
            Ok(offset + 2)
        } else {
            safe_write(&self.memory, offset, &size.to_le_bytes())?;
            Ok(offset + 4)
        }
    }
    fn read_entry_size(&self, offset: u64) -> (u64, usize) {
        let t_bounds = bounds::<T>();
        if t_bounds.is_fixed_size {
            (offset, t_bounds.max_size as usize)
        } else if u8::try_from(t_bounds.max_size).is_ok() {
            let mut size = [0u8; 1];
            self.memory.read(offset, &mut size);
            (offset + 1, size[0] as usize)
        } else if u16::try_from(t_bounds.max_size).is_ok() {
            let mut size = [0u8; 2];
            self.memory.read(offset, &mut size);
            (offset + 2, u16::from_le_bytes(size) as usize)
        } else {
            let size = read_u32(&self.memory, Address::from(offset));
            (offset + 4, size as usize)
        }
    }
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().collect()
    }
}

impl<T: Storable + fmt::Debug, M: Memory> fmt::Debug for BaseVec<T, M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_vec().fmt(fmt)
    }
}

const fn slot_size<T: Storable>() -> u32 {
    let t_bounds = bounds::<T>();
    t_bounds.max_size + bytes_to_store_size_bounded(&t_bounds)
}

pub struct Iter<'a, T, M>
where
    T: Storable,
    M: Memory,
{
    vec: &'a BaseVec<T, M>,
    buf: std::vec::Vec<u8>,
    range: Range<u64>,
}

impl<T, M> Iterator for Iter<'_, T, M>
where
    T: Storable,
    M: Memory,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.range.is_empty() || self.vec.len() <= self.range.start {
            return None;
        }

        self.vec.read_entry_to(self.range.start, &mut self.buf);
        self.range.start = self.range.start.saturating_add(1);
        Some(T::from_bytes(Cow::Borrowed(&self.buf)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            usize::try_from(min(self.vec.len(), self.range.end).saturating_sub(self.range.start))
                .unwrap_or(usize::MAX),
            None,
        )
    }

    fn count(self) -> usize {
        min(self.vec.len(), self.range.end)
            .saturating_sub(self.range.start)
            .try_into()
            .expect("Cannot express count as usize")
    }

    fn last(mut self) -> Option<T> {
        if self.range.is_empty() {
            return None;
        }
        // Like repeated next(), restrict the original range to the surviving
        // prefix. next_back() alone would return None after a truncation.
        let end = min(self.vec.len(), self.range.end);
        if end <= self.range.start {
            return None;
        }
        self.vec.read_entry_to(end - 1, &mut self.buf);
        Some(T::from_bytes(Cow::Borrowed(&self.buf)))
    }

    fn nth(&mut self, n: usize) -> Option<T> {
        self.range.start = self.range.start.saturating_add(n as u64);
        self.next()
    }
}

impl<T, M> DoubleEndedIterator for Iter<'_, T, M>
where
    T: Storable,
    M: Memory,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.range.is_empty() || self.vec.len() < self.range.end {
            return None;
        }

        self.vec.read_entry_to(self.range.end - 1, &mut self.buf);
        self.range.end = self.range.end.saturating_sub(1);
        Some(T::from_bytes(Cow::Borrowed(&self.buf)))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.range.end = self.range.end.saturating_sub(n as u64);
        self.next_back()
    }
}
