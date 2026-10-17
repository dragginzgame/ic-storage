use super::{Config, Error};
use crate::Memory;

#[cfg(test)]
mod tests;

pub(super) const SIZE: usize = 4096;
pub(super) const PAYLOAD: usize = SIZE - 24;
pub(super) const DEGREE: usize = 16;
pub(super) const MAX: usize = 2 * DEGREE - 1;
pub(super) const MAX_HEIGHT: usize = 64;
pub(super) const KEY_INLINE_BYTES: usize = 32;
pub(super) const VALUE_INLINE_BYTES: usize = 16;
const RECORD_BYTES: usize = 32 + KEY_INLINE_BYTES + VALUE_INLINE_BYTES;
pub(super) const FREE: u8 = 1;
pub(super) const BLOB: u8 = 2;
const LEAF: u8 = 3;
const BRANCH: u8 = 4;
pub(super) type Page = [u8; SIZE];

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Header {
    pub root: u64,
    pub len: u64,
    pub next: u64,
    pub free: u64,
    pub config: Config,
    pub identity: [u8; 16],
}
impl Header {
    pub const fn new(config: Config, identity: [u8; 16]) -> Self {
        Self {
            root: 0,
            len: 0,
            next: 1,
            free: 0,
            config,
            identity,
        }
    }
    pub fn encode(self) -> Page {
        let mut page = [0; SIZE];
        page[..8].copy_from_slice(b"ICBTREE1");
        put(&mut page, 8, self.root);
        put(&mut page, 16, self.len);
        put(&mut page, 24, self.next);
        put(&mut page, 32, self.free);
        put(&mut page, 40, u64::from(self.config.max_key_bytes));
        put(&mut page, 48, u64::from(self.config.max_value_bytes));
        page[56..72].copy_from_slice(&self.identity);
        seal(&mut page);
        page
    }
    pub fn read(memory: &impl Memory) -> Result<Self, Error> {
        if memory.size() == 0 {
            return Err(Error::Format);
        }
        let mut page = [0; SIZE];
        memory.read(0, &mut page);
        if &page[..8] != b"ICBTREE1" {
            return Err(Error::Format);
        }
        verify(&page)?;
        let config = Config {
            max_key_bytes: u32::try_from(number(&page, 40)).map_err(|_| Error::Corrupt)?,
            max_value_bytes: u32::try_from(number(&page, 48)).map_err(|_| Error::Corrupt)?,
        };
        config.check().map_err(|_| Error::Corrupt)?;
        let mut identity = [0; 16];
        identity.copy_from_slice(&page[56..72]);
        let header = Self {
            root: number(&page, 8),
            len: number(&page, 16),
            next: number(&page, 24),
            free: number(&page, 32),
            config,
            identity,
        };
        if header.root == 0
            || header.root >= header.next
            || header.free >= header.next
            || header.next.checked_mul(SIZE as u64).ok_or(Error::Corrupt)?
                > memory.size().checked_mul(65_536).ok_or(Error::Corrupt)?
        {
            return Err(Error::Corrupt);
        }
        Ok(header)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Record {
    pub key: u64,
    pub key_len: u64,
    pub value: u64,
    pub value_len: u64,
    pub key_inline: [u8; KEY_INLINE_BYTES],
    pub value_inline: [u8; VALUE_INLINE_BYTES],
}
impl Record {
    pub fn new(key: &[u8], value: &[u8], key_page: u64, value_page: u64) -> Self {
        Self {
            key: key_page,
            key_len: key.len() as u64,
            value: value_page,
            value_len: value.len() as u64,
            key_inline: inline_bytes(key),
            value_inline: inline_bytes(value),
        }
    }
    pub fn set_value(&mut self, value: &[u8], first_page: u64) {
        self.value = first_page;
        self.value_len = value.len() as u64;
        self.value_inline = inline_bytes(value);
    }
}

fn inline_bytes<const N: usize>(bytes: &[u8]) -> [u8; N] {
    let mut inline = [0; N];
    if bytes.len() <= N {
        inline[..bytes.len()].copy_from_slice(bytes);
    }
    inline
}

#[derive(Clone)]
pub(super) struct Node {
    pub entries: Vec<Record>,
    pub children: Vec<u64>,
}
impl Node {
    pub const fn leaf() -> Self {
        Self {
            entries: Vec::new(),
            children: Vec::new(),
        }
    }
    pub fn encode(&self) -> Result<Box<Page>, Error> {
        if self.entries.len() > MAX
            || (!self.children.is_empty() && self.children.len() != self.entries.len() + 1)
        {
            return Err(Error::Corrupt);
        }
        let mut page = zeroed_page();
        page[0] = if self.children.is_empty() {
            LEAF
        } else {
            BRANCH
        };
        put(&mut page, 8, self.entries.len() as u64);
        for (i, entry) in self.entries.iter().enumerate() {
            let at = 16 + i * RECORD_BYTES;
            put(&mut page, at, entry.key);
            put(&mut page, at + 8, entry.key_len);
            put(&mut page, at + 16, entry.value);
            put(&mut page, at + 24, entry.value_len);
            page[at + 32..at + 32 + KEY_INLINE_BYTES].copy_from_slice(&entry.key_inline);
            page[at + 32 + KEY_INLINE_BYTES..at + RECORD_BYTES]
                .copy_from_slice(&entry.value_inline);
        }
        for (i, child) in self.children.iter().enumerate() {
            put(&mut page, 16 + MAX * RECORD_BYTES + i * 8, *child);
        }
        seal(&mut page);
        Ok(page)
    }
    pub fn decode(page: &Page, next: u64, config: Config) -> Result<Self, Error> {
        if page[0] != LEAF && page[0] != BRANCH {
            return Err(Error::Corrupt);
        }
        let count = usize::try_from(number(page, 8)).map_err(|_| Error::Corrupt)?;
        if count > MAX {
            return Err(Error::Corrupt);
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let at = 16 + i * RECORD_BYTES;
            let mut key_inline = [0; KEY_INLINE_BYTES];
            let mut value_inline = [0; VALUE_INLINE_BYTES];
            key_inline.copy_from_slice(&page[at + 32..at + 32 + KEY_INLINE_BYTES]);
            value_inline.copy_from_slice(&page[at + 32 + KEY_INLINE_BYTES..at + RECORD_BYTES]);
            let entry = Record {
                key_inline,
                value_inline,
                key: number(page, at),
                key_len: number(page, at + 8),
                value: number(page, at + 16),
                value_len: number(page, at + 24),
            };
            if entry.key_len > u64::from(config.max_key_bytes)
                || entry.value_len > u64::from(config.max_value_bytes)
                || entry.key >= next
                || entry.value >= next
                || (entry.key == 0) != (entry.key_len <= KEY_INLINE_BYTES as u64)
                || (entry.value == 0) != (entry.value_len <= VALUE_INLINE_BYTES as u64)
            {
                return Err(Error::Corrupt);
            }
            entries.push(entry);
        }
        let mut children = Vec::new();
        if page[0] == BRANCH {
            for i in 0..=count {
                let child = number(page, 16 + MAX * RECORD_BYTES + i * 8);
                if child == 0 || child >= next {
                    return Err(Error::Corrupt);
                }
                children.push(child);
            }
        }
        Ok(Self { entries, children })
    }
}

pub(super) fn put(page: &mut Page, at: usize, value: u64) {
    page[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
pub(super) fn number(page: &Page, at: usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&page[at..at + 8]);
    u64::from_le_bytes(bytes)
}
fn checksum(page: &Page) -> u64 {
    checksum_with_zero_tail(page, SIZE - 8)
}
// Only encoders may exclude bytes they have just initialized to zero. Reads
// always pass the entire covered region through `checksum`.
fn checksum_with_zero_tail(page: &Page, mut end: usize) -> u64 {
    const ZERO_CHUNK_FACTOR: u64 = 0x100_0000_01b3u64.wrapping_pow(8);
    let tail_chunks = u32::try_from((SIZE - 8 - end) / 8).expect("page tail length");
    let mut suffix_factor = ZERO_CHUNK_FACTOR.wrapping_pow(tail_chunks);
    // Dense payloads pay one eight-byte check. For sparse tails, inspect larger
    // zero blocks before the final boundary, still checking every covered byte.
    if page[end - 8..end] == [0; 8] {
        while end >= 32 && page[end - 32..end] == [0; 32] {
            suffix_factor = suffix_factor.wrapping_mul(ZERO_CHUNK_FACTOR.wrapping_pow(4));
            end -= 32;
        }
        while end >= 8 && page[end - 8..end] == [0; 8] {
            suffix_factor = suffix_factor.wrapping_mul(ZERO_CHUNK_FACTOR);
            end -= 8;
        }
    }
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    // The covered region is a multiple of eight. Fixed-size inner iterations
    // let Wasm eliminate per-byte loop control without changing FNV-1a order.
    for chunk in page[..end].as_chunks::<8>().0 {
        for byte in chunk {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash.wrapping_mul(suffix_factor)
}
pub(super) fn seal(page: &mut Page) {
    put(page, SIZE - 8, checksum(page));
}
pub(super) fn encode_blob(next: u64, payload: &[u8]) -> Box<Page> {
    assert!(payload.len() <= PAYLOAD);
    // Initialize the heap buffer once: payload bytes need not be zeroed first.
    let mut header = [0; 16];
    header[0] = BLOB;
    header[8..].copy_from_slice(&next.to_le_bytes());
    let mut bytes = Vec::with_capacity(SIZE);
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(payload);
    bytes.resize(SIZE, 0);
    let mut page: Box<Page> = bytes
        .into_boxed_slice()
        .try_into()
        .expect("fixed page length");
    let end = (16 + payload.len()).next_multiple_of(8);
    let hash = checksum_with_zero_tail(&page, end);
    put(&mut page, SIZE - 8, hash);
    page
}
fn zeroed_page() -> Box<Page> {
    vec![0; SIZE]
        .into_boxed_slice()
        .try_into()
        .expect("fixed page length")
}
pub(super) fn free(next: u64) -> Box<Page> {
    const TAIL_FACTOR: u64 = {
        let mut factor = 1u64;
        let mut at = 16;
        while at < SIZE - 8 {
            factor = factor.wrapping_mul(0x100_0000_01b3);
            at += 1;
        }
        factor
    };
    let mut page = zeroed_page();
    page[0] = FREE;
    put(&mut page, 8, next);
    // Only freshly constructed free pages have a known-zero tail. FNV-1a
    // over each zero byte multiplies by the prime, so fold the tail at once.
    // Reads still verify every byte, including the reserved zero padding.
    let prefix = page[..16]
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        });
    put(&mut page, SIZE - 8, prefix.wrapping_mul(TAIL_FACTOR));
    page
}
fn verify(page: &Page) -> Result<(), Error> {
    if number(page, SIZE - 8) != checksum(page) {
        return Err(Error::Corrupt);
    }
    Ok(())
}
pub(super) fn read(memory: &impl Memory, next: u64, id: u64) -> Result<Page, Error> {
    let mut page = [0; SIZE];
    read_into(memory, next, id, &mut page)?;
    Ok(page)
}
pub(super) fn read_into(
    memory: &impl Memory,
    next: u64,
    id: u64,
    page: &mut Page,
) -> Result<(), Error> {
    if id == 0 || id >= next {
        return Err(Error::Corrupt);
    }
    let offset = id.checked_mul(SIZE as u64).ok_or(Error::Corrupt)?;
    if offset.checked_add(SIZE as u64).ok_or(Error::Corrupt)?
        > memory.size().checked_mul(65_536).ok_or(Error::Corrupt)?
    {
        return Err(Error::Corrupt);
    }
    memory.read(offset, page);
    verify(page)
}
// Borrow a complete payload only after the caller has verified its page.
pub(super) fn single_blob(page: &Page, len: u64) -> Result<&[u8], Error> {
    if page[0] != BLOB || number(page, 8) != 0 || len > PAYLOAD as u64 {
        return Err(Error::Corrupt);
    }
    let len = usize::try_from(len).map_err(|_| Error::Corrupt)?;
    Ok(&page[16..16 + len])
}

pub(super) fn blob(
    memory: &impl Memory,
    next: u64,
    start: u64,
    len: u64,
) -> Result<Vec<u8>, Error> {
    read_blob_with(start, len, |id, output| read_into(memory, next, id, output))
}

// Readers validate page bounds and checksums before supplying each page. A
// transaction reader must prefer staged pages; this helper owns chain framing.
pub(super) fn read_blob_with(
    start: u64,
    len: u64,
    mut read: impl FnMut(u64, &mut Page) -> Result<(), Error>,
) -> Result<Vec<u8>, Error> {
    let len = usize::try_from(len).map_err(|_| Error::Corrupt)?;
    let mut output = Vec::with_capacity(len);
    let mut id = start;
    let mut page = [0; SIZE];
    while output.len() < len {
        read(id, &mut page)?;
        if page[0] != BLOB {
            return Err(Error::Corrupt);
        }
        let take = (len - output.len()).min(PAYLOAD);
        output.extend_from_slice(&page[16..16 + take]);
        id = number(&page, 8);
    }
    if id != 0 {
        return Err(Error::Corrupt);
    }
    Ok(output)
}
