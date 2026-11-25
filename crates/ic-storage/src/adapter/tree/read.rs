//! Checked read views retain one record buffer.
use super::{
    Entry, Error, INLINE, MAX_ENTRIES, NODE, Node, Pointer, Reader, Result, Storable, Value,
    ensure, record,
};
use std::{borrow::Cow, cmp::Ordering, ops::Range};

struct ReadEntry {
    key: Range<usize>,
    value: ReadValue,
}
enum ReadValue {
    Inline(Range<usize>),
    External(Pointer),
}
pub(super) struct ReadNode {
    bytes: Vec<u8>,
    entries: Vec<ReadEntry>,
    pub children: Vec<Pointer>,
}
impl ReadNode {
    pub fn decode<K: Storable, V: Storable>(
        bytes: Vec<u8>,
        cursor: u64,
        unit: u16,
    ) -> Result<Self> {
        let payload = record::decode_payload(NODE, &bytes)?;
        let mut r = Reader(payload);
        let count = usize::from(r.byte()?);
        let branch = r.byte()?;
        ensure(
            count <= MAX_ENTRIES && branch <= 1,
            Error::Corrupt("B-tree node header"),
        )?;
        let mut children = Vec::with_capacity(if branch == 1 { count + 1 } else { 0 });
        if branch == 1 {
            for _ in 0..=count {
                children.push(r.pointer(cursor, unit)?);
            }
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let len = u32::from_le_bytes(r.take(4)?.try_into().unwrap()) as usize;
            let start = bytes.len() - r.0.len();
            K::BOUND.check(r.take(len)?);
            let key = start..start + len;
            let size = r.byte()?;
            let value = if size == 255 {
                ReadValue::External(r.pointer(cursor, unit)?)
            } else {
                ensure(
                    usize::from(size) <= INLINE,
                    Error::Corrupt("B-tree inline value"),
                )?;
                let start = bytes.len() - r.0.len();
                V::BOUND.check(r.take(usize::from(size))?);
                ReadValue::Inline(start..start + usize::from(size))
            };
            entries.push(ReadEntry { key, value });
        }
        ensure(r.0.is_empty(), Error::Corrupt("B-tree node trailing bytes"))?;
        Ok(Self {
            bytes,
            entries,
            children,
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn search<K: Storable + Ord>(&self, key: &K) -> Result<std::result::Result<usize, usize>> {
        let (mut low, mut high) = (0, self.len());
        while low < high {
            let mid = low + (high - low) / 2;
            let stored = &self.bytes[self.entries[mid].key.clone()];
            let order = match K::CMP_ENCODED {
                Some(compare) => compare(key, stored),
                None => key.cmp(&K::from_bytes(Cow::Borrowed(stored))),
            };
            match order {
                Ordering::Less => high = mid,
                Ordering::Greater => low = mid + 1,
                Ordering::Equal => return Ok(Ok(mid)),
            }
        }
        Ok(Err(low))
    }
    pub fn inline_value(&self, index: usize) -> Option<&[u8]> {
        match &self.entries[index].value {
            ReadValue::Inline(range) => Some(&self.bytes[range.clone()]),
            ReadValue::External(_) => None,
        }
    }
    pub fn key_bytes(&self, index: usize) -> &[u8] {
        &self.bytes[self.entries[index].key.clone()]
    }
    // Replace only an admitted inline span. Copy unchanged encodings into one
    // final frame instead of materializing every key/value, and seal it through
    // the same record writer as ordinary node encoding.
    pub fn replace_inline(&self, index: usize, value: &[u8]) -> Result<Vec<u8>> {
        ensure(value.len() <= INLINE, Error::Budget)?;
        let ReadValue::Inline(range) = &self.entries[index].value else {
            return Err(Error::Corrupt("expected inline replacement"));
        };
        let length = self.bytes.len() - record::PREFIX - range.len() + value.len();
        record::encode_payload_with(NODE, length, |bytes| {
            bytes.extend_from_slice(&self.bytes[record::PREFIX..range.start - 1]);
            bytes.push(u8::try_from(value.len()).unwrap());
            bytes.extend_from_slice(value);
            bytes.extend_from_slice(&self.bytes[range.end..]);
            Ok(())
        })
    }
    pub fn value(&self, index: usize) -> Value {
        match &self.entries[index].value {
            ReadValue::Inline(range) => Value::Inline(self.bytes[range.clone()].to_vec()),
            ReadValue::External(pointer) => Value::External(*pointer),
        }
    }
    pub fn entry(&self, index: usize) -> Entry {
        Entry {
            key: self.bytes[self.entries[index].key.clone()].to_vec(),
            value: self.value(index),
        }
    }
    pub fn owned(&self) -> Node {
        Node {
            entries: (0..self.len()).map(|i| self.entry(i)).collect(),
            children: self.children.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single_byte_pivot(byte: u8) -> ReadNode {
        ReadNode {
            bytes: vec![byte],
            entries: vec![ReadEntry {
                key: 0..1,
                value: ReadValue::Inline(1..1),
            }],
            children: vec![],
        }
    }

    #[test]
    #[should_panic(expected = "invalid boolean encoding")]
    fn search_rejects_an_invalid_boolean_pivot() {
        let _ = single_byte_pivot(2).search(&true);
    }

    #[test]
    #[should_panic(expected = "invalid UTF-8 record")]
    fn search_rejects_an_invalid_string_pivot() {
        let _ = single_byte_pivot(0xff).search(&String::new());
    }
}
