//! Directory v2. Scalars and counts are little-endian u64s. Names are
//! length-prefixed ASCII. Extents are (start, pages) pairs in logical order.
use super::{AllocationId, Entry, Error, Extent, FRAME, Image, StableKey, StorageConfig};

pub(super) fn encode(image: &Image, limit: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    let mut append = |part: &[u8]| {
        if part.len() > limit.saturating_sub(bytes.len()) {
            return Err(Error::Budget);
        }
        bytes.extend_from_slice(part);
        Ok(())
    };
    append(FRAME)?;
    for n in [
        image.generation,
        image.next_id,
        image.next_page,
        image.metadata.len() as u64,
    ] {
        append(&n.to_le_bytes())?;
    }
    for extent in &image.metadata {
        append(&extent.start.to_le_bytes())?;
        append(&extent.pages.to_le_bytes())?;
    }
    append(&(image.entries.len() as u64).to_le_bytes())?;
    for entry in &image.entries {
        let name = entry.key.as_str().as_bytes();
        append(&entry.id.0.to_le_bytes())?;
        append(&(name.len() as u64).to_le_bytes())?;
        append(name)?;
        append(&(entry.extents.len() as u64).to_le_bytes())?;
        for extent in &entry.extents {
            append(&extent.start.to_le_bytes())?;
            append(&extent.pages.to_le_bytes())?;
        }
    }
    Ok(bytes)
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        self.0
            .split_off(..length)
            .ok_or(Error::Invalid("truncated directory"))
    }

    fn number(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }

    fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let count = usize::try_from(self.number()?).map_err(|_| Error::Budget)?;
        if count > limit {
            return Err(Error::Budget);
        }
        Ok(count)
    }

    fn extents(&mut self, limit: usize) -> Result<Vec<Extent>, Error> {
        let count = self.count(limit)?;
        let mut extents = Vec::new();
        for _ in 0..count {
            extents.push(Extent {
                start: self.number()?,
                pages: self.number()?,
            });
        }
        Ok(extents)
    }
}

pub(super) fn decode(payload: &[u8], budget: StorageConfig) -> Result<Image, Error> {
    let mut reader = Reader(payload);
    let generation = reader.number()?;
    let next_id = reader.number()?;
    let next_page = reader.number()?;
    let metadata = reader.extents(128)?;
    let count = reader.count(budget.allocations)?;
    let mut entries = Vec::new();
    let mut remaining = budget.extents;
    for _ in 0..count {
        let id = AllocationId(reader.number()?);
        let length = reader.count(128)?;
        let name = std::str::from_utf8(reader.take(length)?)
            .map_err(|_| Error::Invalid("key encoding"))?;
        let key = StableKey::parse(name).map_err(|_| Error::Invalid("key grammar"))?;
        let extents = reader.extents(remaining)?;
        remaining -= extents.len();
        entries.push(Entry { id, key, extents });
    }
    if !reader.0.is_empty() {
        return Err(Error::Invalid("trailing directory bytes"));
    }
    Ok(Image {
        generation,
        next_id,
        next_page,
        metadata,
        entries,
    })
}
