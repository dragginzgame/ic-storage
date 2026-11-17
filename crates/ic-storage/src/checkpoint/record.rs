use super::{Error, Memory, Region};
use alloc::vec::Vec;

mod sealed {
    pub trait Sealed {}
    impl Sealed for alloc::vec::Vec<u8> {}
    impl<const N: usize> Sealed for [u8; N] {}
}

/// An exact array or owned byte buffer, never an arbitrary Rust representation.
pub trait Bytes: sealed::Sealed + AsRef<[u8]> {
    /// Construct an owned value from its logical bytes.
    ///
    /// # Errors
    /// Fixed arrays refuse a different length.
    fn decode(bytes: Vec<u8>) -> Result<Self, Error>
    where
        Self: Sized;
}

impl Bytes for Vec<u8> {
    fn decode(bytes: Vec<u8>) -> Result<Self, Error> {
        Ok(bytes)
    }
}

impl<const N: usize> Bytes for [u8; N] {
    fn decode(bytes: Vec<u8>) -> Result<Self, Error> {
        bytes.try_into().map_err(|_| Error::Bounds)
    }
}

/// A bounded byte record cached for its exclusive backing-ownership lifetime.
/// Logical codecs, rather than this container, validate payload integrity.
pub struct Record<T: Bytes, M: Memory> {
    region: Region<M>,
    value: T,
}

impl<T: Bytes, M: Memory> Record<T, M> {
    /// Initialize one previously unused region.
    ///
    /// # Errors
    /// Refuses nonzero framing or a value exceeding the region capacity.
    pub fn create(region: Region<M>, value: T) -> Result<Self, Error> {
        let bytes = value.as_ref();
        region.fits(16, bytes.len() as u64)?;
        region.initialize(b"ICREC001", bytes.len() as u64)?;
        region.write(16, bytes);
        Ok(Self { region, value })
    }

    /// Load a record under a caller-selected logical byte bound.
    ///
    /// # Errors
    /// Refuses unknown framing, excessive/truncated lengths or a wrong array size.
    pub fn load(region: Region<M>, limit: usize) -> Result<Self, Error> {
        let length = region.header(b"ICREC001")?;
        region.fits(16, length)?;
        let length = usize::try_from(length).map_err(|_| Error::Bounds)?;
        if length > limit {
            return Err(Error::Bounds);
        }
        let mut bytes = alloc::vec![0; length];
        region.read(16, &mut bytes);
        Ok(Self {
            region,
            value: T::decode(bytes)?,
        })
    }

    /// Read the cached admitted bytes, without additional backing IO.
    #[must_use]
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Replace the payload, then publish a changed length. Same-sized updates
    /// write only payload bytes. The enclosing transaction owns rollback.
    ///
    /// # Errors
    /// Refuses a value exceeding the reserved region without writing.
    pub fn set(&mut self, value: T) -> Result<(), Error> {
        let bytes = value.as_ref();
        self.region.fits(16, bytes.len() as u64)?;
        self.region.write(16, bytes);
        if bytes.len() != self.value.as_ref().len() {
            self.region.write(8, &(bytes.len() as u64).to_le_bytes());
        }
        self.value = value;
        Ok(())
    }
}
