//! Fixed-width metadata for the owned B-tree. Zero link bytes denote an empty slot.
use super::{BINS, BTreeMapOptions, Error, MAGIC, Pointer, Reader, Result, Root, ensure, record};

pub(super) fn link_bytes(link: Option<Pointer>) -> [u8; 9] {
    let mut bytes = [0; 9];
    if let Some(link) = link {
        bytes[..8].copy_from_slice(&link.offset.to_le_bytes());
        bytes[8] = link.order;
    }
    bytes
}

pub(super) fn decode_link(bytes: &[u8], cursor: u64, unit: u16) -> Result<Option<Pointer>> {
    ensure(bytes.len() == 9, Error::Format)?;
    if bytes == [0; 9] {
        return Ok(None);
    }
    Reader(bytes).pointer(cursor, unit).map(Some)
}

fn write_bound(bytes: &mut Vec<u8>, bound: Option<(u32, bool)>) {
    let (width, tag) = match bound {
        None => (0, 0),
        Some((width, false)) => (width, 1),
        Some((width, true)) => (width, 2),
    };
    bytes.push(tag);
    bytes.extend_from_slice(&width.to_le_bytes());
}

fn read_bound(reader: &mut Reader<'_>) -> Result<Option<(u32, bool)>> {
    let tag = reader.byte()?;
    let width = u32::from_le_bytes(reader.take(4)?.try_into().unwrap());
    match tag {
        0 if width == 0 => Ok(None),
        1 | 2 => Ok(Some((width, tag == 2))),
        _ => Err(Error::Format),
    }
}

impl Root {
    pub(super) fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(39 + BINS * 9);
        bytes.extend_from_slice(&self.options.node_target_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.options.allocation_unit_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.length.to_le_bytes());
        bytes.extend_from_slice(&self.cursor.to_le_bytes());
        bytes.extend_from_slice(&link_bytes(self.top));
        write_bound(&mut bytes, self.key_bound);
        write_bound(&mut bytes, self.value_bound);
        for &link in &self.free {
            bytes.extend_from_slice(&link_bytes(link));
        }
        record::encode_payload(MAGIC, &bytes)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader(bytes);
        let options = BTreeMapOptions {
            node_target_bytes: u16::from_le_bytes(reader.take(2)?.try_into().unwrap()),
            allocation_unit_bytes: u16::from_le_bytes(reader.take(2)?.try_into().unwrap()),
        };
        ensure(options.valid(), Error::Format)?;
        let length = u64::from_le_bytes(reader.take(8)?.try_into().unwrap());
        let cursor = u64::from_le_bytes(reader.take(8)?.try_into().unwrap());
        let top = decode_link(reader.take(9)?, cursor, options.allocation_unit_bytes)?;
        let key_bound = read_bound(&mut reader)?;
        let value_bound = read_bound(&mut reader)?;
        let free = (0..BINS)
            .map(|_| decode_link(reader.take(9)?, cursor, options.allocation_unit_bytes))
            .collect::<Result<_>>()?;
        ensure(reader.0.is_empty(), Error::Format)?;
        Ok(Self {
            options,
            top,
            length,
            cursor,
            free,
            key_bound,
            value_bound,
        })
    }
}
