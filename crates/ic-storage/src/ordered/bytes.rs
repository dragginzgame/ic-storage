use super::Error;
use core::cmp::Ordering;

/// Borrowed logical bytes from an admitted payload arena. No owner escapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteSlice<'a>(pub(super) &'a [u8]);

impl<'a> ByteSlice<'a> {
    /// Exact logical bytes, excluding arena slack.
    #[must_use]
    pub fn as_slice(self) -> &'a [u8] {
        self.0
    }
}

/// Bounded inline binary data, with lexicographic ordering and zeroed unused tail.
/// Empty, embedded-zero and all-FF values are ordinary distinct byte strings.
/// This is not UTF-8 validation or a codec for a foreign Rust type.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Bytes<const N: usize> {
    pub(super) length: u64,
    pub(super) data: [u8; N],
}

impl<const N: usize> Bytes<N> {
    pub(super) fn copy_admitted(bytes: &[u8]) -> Self {
        Self::new(bytes).expect("admission and mutations enforce the byte bound")
    }
    /// Canonical empty value.
    pub const EMPTY: Self = Self {
        length: 0,
        data: [0; N],
    };
    /// Copy bounded bytes, initializing the complete inline representation.
    ///
    /// # Errors
    /// Refuses more than N bytes without truncation.
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > N {
            return Err(Error::Limit);
        }
        let mut value = Self::EMPTY;
        value.data[..bytes.len()].copy_from_slice(bytes);
        value.length = bytes.len() as u64;
        Ok(value)
    }
    /// Borrow the logical bytes, excluding the initialized unused tail.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.length as usize]
    }
}
impl<const N: usize> PartialEq for Bytes<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl<const N: usize> Eq for Bytes<N> {}
impl<const N: usize> PartialOrd for Bytes<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<const N: usize> Ord for Bytes<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}
