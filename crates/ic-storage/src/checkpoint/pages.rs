use super::{Error, Memory, Region};

/// Fixed-width checkpoint pages. Opening checks framing and count, without
/// reading pages; the logical manifest/hash chain validates each selected page.
pub struct Pages<const N: usize, M: Memory> {
    region: Region<M>,
    length: u64,
}

impl<const N: usize, M: Memory> Pages<N, M> {
    /// Create an empty page sequence in an unused region.
    ///
    /// # Errors
    /// Refuses zero width, insufficient capacity or nonzero existing framing.
    pub fn create(region: Region<M>) -> Result<Self, Error> {
        if N == 0 {
            return Err(Error::Bounds);
        }
        region.fits(24, N as u64)?;
        region.initialize(b"ICPAG001", 0)?;
        region.write(16, &(N as u64).to_le_bytes());
        Ok(Self { region, length: 0 })
    }

    /// Open an existing bounded sequence, without reading any page payload.
    ///
    /// # Errors
    /// Refuses unknown framing, zero width, overflow or count outside capacity.
    pub fn load(region: Region<M>) -> Result<Self, Error> {
        if N == 0 {
            return Err(Error::Bounds);
        }
        let length = region.header(b"ICPAG001")?;
        region.fits(24, length.checked_mul(N as u64).ok_or(Error::Bounds)?)?;
        let mut width = [0; 8];
        region.read(16, &mut width);
        if u64::from_le_bytes(width) != N as u64 {
            return Err(Error::Layout);
        }
        Ok(Self { region, length })
    }

    /// Number of published pages.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.length
    }

    /// Whether no pages have been published.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Read one page in exactly one backing read, or return `None` out of range.
    #[must_use]
    pub fn get(&self, index: u64) -> Option<[u8; N]> {
        if index >= self.length {
            return None;
        }
        let mut page = [0; N];
        self.region.read(24 + index * N as u64, &mut page);
        Some(page)
    }

    /// Append bytes before publishing their count. No general growth is attempted.
    ///
    /// # Errors
    /// Refuses capacity or arithmetic overflow without writing.
    pub fn push(&mut self, page: &[u8; N]) -> Result<(), Error> {
        let next = self.length.checked_add(1).ok_or(Error::Bounds)?;
        self.region
            .fits(24, next.checked_mul(N as u64).ok_or(Error::Bounds)?)?;
        self.region.write(24 + self.length * N as u64, page);
        self.region.write(8, &next.to_le_bytes());
        self.length = next;
        Ok(())
    }

    /// Replace one existing page.
    ///
    /// # Errors
    /// Refuses an out-of-range index without writing.
    pub fn set(&mut self, index: u64, page: &[u8; N]) -> Result<(), Error> {
        if index >= self.length {
            return Err(Error::Bounds);
        }
        self.region.write(24 + index * N as u64, page);
        Ok(())
    }

    /// Remove and return the last page, leaving the reserved capacity intact.
    pub fn pop(&mut self) -> Option<[u8; N]> {
        let index = self.length.checked_sub(1)?;
        let page = self.get(index)?;
        self.region.write(8, &index.to_le_bytes());
        self.length = index;
        Some(page)
    }

    /// Unpublish all pages without erasing bytes or releasing reserved capacity.
    pub fn clear(&mut self) {
        if self.length != 0 {
            self.region.write(8, &0u64.to_le_bytes());
            self.length = 0;
        }
    }
}
