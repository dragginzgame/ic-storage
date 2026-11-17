//! Owned payload bytes with segregated free lists. No pointers or Rust headers
//! occur inside the allocation. Splitting reuses larger blocks; coalescing and
//! compaction are intentionally not implied by this bounded first allocator.
use super::{Error, Image};
use alloc::vec::Vec;
use core::alloc::Allocator;

const CLASSES: usize = 64;
pub(super) const HEADER: usize = CLASSES * 16;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Span {
    pub offset: u64,
    pub length: u64,
}
impl Span {
    pub const EMPTY: Self = Self {
        offset: 0,
        length: 0,
    };
    pub fn bytes(self, arena: &[u8]) -> &[u8] {
        &arena[self.offset as usize..self.offset as usize + self.length as usize]
    }
}

fn class(length: usize) -> Result<Option<usize>, Error> {
    if length == 0 {
        return Ok(None);
    }
    let size = length
        .max(8)
        .checked_next_power_of_two()
        .ok_or(Error::Limit)?;
    Ok(Some(size.trailing_zeros() as usize - 3))
}
fn width(class: usize) -> Result<usize, Error> {
    1usize
        .checked_shl(class as u32)
        .and_then(|n| n.checked_mul(8))
        .ok_or(Error::NativeImage)
}
fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("arena word"))
}
fn set(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// Scalar simulation of the allocator, used before any mutation. It needs only
/// the 64 class counts, never a traversal of existing records or free blocks.
pub(super) struct Plan {
    counts: [u64; CLASSES],
    end: usize,
}
impl Plan {
    pub fn new(arena: &[u8]) -> Self {
        Self {
            counts: core::array::from_fn(|i| {
                if arena.is_empty() {
                    0
                } else {
                    word(arena, i * 16 + 8)
                }
            }),
            end: arena.len(),
        }
    }
    pub fn allocate(&mut self, length: usize) -> Result<(), Error> {
        let Some(class) = class(length)? else {
            return Ok(());
        };
        if let Some(larger) = (class..CLASSES).find(|i| self.counts[*i] != 0) {
            self.counts[larger] -= 1;
            for count in &mut self.counts[class..larger] {
                *count += 1;
            }
        } else {
            self.end = self
                .end
                .max(HEADER)
                .checked_add(width(class)?)
                .ok_or(Error::Limit)?;
        }
        Ok(())
    }
    pub fn retire(&mut self, length: usize) -> Result<(), Error> {
        if let Some(class) = class(length)? {
            self.counts[class] += 1;
        }
        Ok(())
    }
    pub fn replace(&mut self, old: usize, new: usize) -> Result<(), Error> {
        if class(old)? != class(new)? {
            self.allocate(new)?;
            self.retire(old)?;
        }
        Ok(())
    }
    pub fn reserve<A: Allocator>(&self, arena: &mut Vec<u8, A>) -> Result<(), Error> {
        arena
            .try_reserve(self.end - arena.len())
            .map_err(|_| Error::Allocation)
    }
}

pub(super) fn reserve_pair<A: Allocator>(
    arena: &mut Vec<u8, A>,
    key: usize,
    value: usize,
) -> Result<(), Error> {
    let mut plan = Plan::new(arena);
    plan.allocate(key)?;
    plan.allocate(value)?;
    plan.reserve(arena)
}

fn push(arena: &mut [u8], offset: usize, class: usize) {
    let head = word(arena, class * 16);
    let count = word(arena, class * 16 + 8);
    set(arena, offset, head);
    set(arena, class * 16, offset as u64);
    set(arena, class * 16 + 8, count + 1);
}

/// Called only after complete reserve. Every newly exposed byte is initialized.
pub(super) fn allocate<A: Allocator>(arena: &mut Vec<u8, A>, bytes: &[u8]) -> Span {
    let Some(class) = class(bytes.len()).expect("reserved bound") else {
        return Span::EMPTY;
    };
    if arena.is_empty() {
        arena.resize(HEADER, 0);
    }
    let offset = if let Some(larger) = (class..CLASSES).find(|i| word(arena, i * 16) != 0) {
        let offset = word(arena, larger * 16) as usize;
        let next = word(arena, offset);
        let count = word(arena, larger * 16 + 8);
        set(arena, larger * 16, next);
        set(arena, larger * 16 + 8, count - 1);
        set(arena, offset, 0);
        for i in (class..larger).rev() {
            push(arena, offset + width(i).expect("admitted class"), i);
        }
        offset
    } else {
        let offset = arena.len();
        arena.resize(offset + width(class).expect("reserved class"), 0);
        offset
    };
    arena[offset..offset + bytes.len()].copy_from_slice(bytes);
    Span {
        offset: offset as u64,
        length: bytes.len() as u64,
    }
}

pub(super) fn retire(arena: &mut [u8], span: Span) {
    if let Some(class) = class(span.length as usize).expect("admitted length") {
        let offset = span.offset as usize;
        arena[offset..offset + width(class).expect("admitted class")].fill(0);
        push(arena, offset, class);
    }
}

pub(super) fn replace<A: Allocator>(
    arena: &mut Vec<u8, A>,
    old: Span,
    bytes: &[u8],
) -> Result<Span, Error> {
    if class(old.length as usize)? == class(bytes.len())? {
        let offset = old.offset as usize;
        arena[offset..offset + old.length as usize].fill(0);
        arena[offset..offset + bytes.len()].copy_from_slice(bytes);
        return Ok(Span {
            offset: old.offset,
            length: bytes.len() as u64,
        });
    }
    let mut plan = Plan::new(arena);
    plan.replace(old.length as usize, bytes.len())?;
    plan.reserve(arena)?;
    let replacement = allocate(arena, bytes);
    retire(arena, old);
    Ok(replacement)
}

pub(super) fn validate<const K: usize, const V: usize, A: Allocator, P: Allocator>(
    image: &Image<K, V, A, P>,
) -> Result<(), Error> {
    let arena = &image.payload;
    if !arena.is_empty() && arena.len() < HEADER {
        return Err(Error::NativeImage);
    }
    let mut intervals = Vec::new();
    let mut count = image.nodes.len().checked_mul(2).ok_or(Error::NativeImage)?;
    if !arena.is_empty() {
        for i in 0..CLASSES {
            let free = usize::try_from(word(arena, i * 16 + 8)).map_err(|_| Error::NativeImage)?;
            count = count.checked_add(free).ok_or(Error::NativeImage)?;
            if free > (arena.len() - HEADER) / 8 {
                return Err(Error::NativeImage);
            }
        }
    }
    // Bound forged aggregate counts before attempting validation scratch allocation.
    if count
        > image
            .nodes
            .len()
            .saturating_mul(2)
            .saturating_add(arena.len() / 8)
    {
        return Err(Error::NativeImage);
    }
    intervals
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    for node in &image.nodes {
        for (span, limit) in [(node.key, K), (node.value, V)] {
            let length = usize::try_from(span.length).map_err(|_| Error::NativeImage)?;
            if length > limit {
                return Err(Error::NativeImage);
            }
            let Some(class) = class(length).map_err(|_| Error::NativeImage)? else {
                if span.offset != 0 {
                    return Err(Error::NativeImage);
                }
                continue;
            };
            let (start, end) = interval(arena, span.offset, width(class)?)?;
            if arena[start + length..end].iter().any(|b| *b != 0) {
                return Err(Error::NativeImage);
            }
            intervals.push((start, end));
        }
    }
    if arena.is_empty() {
        return Ok(());
    }
    for i in 0..CLASSES {
        let mut next = word(arena, i * 16);
        for _ in 0..word(arena, i * 16 + 8) {
            let (start, end) = interval(arena, next, width(i)?)?;
            if arena[start + 8..end].iter().any(|b| *b != 0) {
                return Err(Error::NativeImage);
            }
            intervals.push((start, end));
            next = word(arena, start);
        }
        if next != 0 {
            return Err(Error::NativeImage);
        }
    }
    intervals.sort_unstable();
    let mut next = HEADER;
    for (start, end) in intervals {
        if start != next {
            return Err(Error::NativeImage);
        }
        next = end;
    }
    if next != arena.len() {
        return Err(Error::NativeImage);
    }
    Ok(())
}
fn interval(arena: &[u8], offset: u64, width: usize) -> Result<(usize, usize), Error> {
    let start = usize::try_from(offset).map_err(|_| Error::NativeImage)?;
    let end = start.checked_add(width).ok_or(Error::NativeImage)?;
    if start < HEADER || !start.is_multiple_of(8) || end > arena.len() {
        return Err(Error::NativeImage);
    }
    Ok((start, end))
}
