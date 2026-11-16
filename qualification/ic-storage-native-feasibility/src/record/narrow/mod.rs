//! Checked scalar-width conversion. The installed V3 profile and checkpoint v4
//! use the same explicit range and application-invariant checks.
//! A matching outer size does not make V2's u64 counter a V3 u32 counter.
//!
//! A scalar-name alias cannot conceal an unsupported width:
//! ```compile_fail
//! use ic_storage_native::Store;
//! type u32 = u64;
//! #[repr(C)]
//! #[derive(Clone, Copy, Store)]
//! #[store(id = "BADWIDTH", version = 1)]
//! struct Bad { #[store(field = 1)] value: u32 }
//! ```

use super::V2;

/// Logical schema 3: a narrowed counter with the existing u64 ceiling.
/// The provider separately admits its scalar fields; this value holds no saved reference.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, ic_storage_native::Store)]
#[store(id = "RECU64V1", version = 3, validate = Self::valid)]
pub struct V3 {
    #[store(field = 1)]
    counter: u32,
    #[store(field = 2)]
    ceiling: u64,
}

/// Refusal before changing or publishing a narrowed record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The source counter cannot be represented as u32.
    Range,
    /// The counter would exceed its explicit application ceiling.
    Ceiling,
    /// Addition would exceed the u32 range.
    Overflow,
}

impl V3 {
    /// Construct a fresh record after checking its application invariant.
    pub fn new(counter: u32, ceiling: u64) -> Result<Self, Error> {
        let candidate = Self { counter, ceiling };
        if !candidate.valid() {
            return Err(Error::Ceiling);
        }
        Ok(candidate)
    }

    fn valid(&self) -> bool {
        u64::from(self.counter) <= self.ceiling
    }

    /// Copy the narrowed counter.
    #[must_use]
    pub fn counter(self) -> u32 {
        self.counter
    }

    /// Copy the unchanged application ceiling.
    #[must_use]
    pub fn ceiling(self) -> u64 {
        self.ceiling
    }

    /// Checked mutation; a range or ceiling refusal preserves the record.
    pub fn add(&mut self, delta: u32) -> Result<(), Error> {
        let next = self.counter.checked_add(delta).ok_or(Error::Overflow)?;
        *self = Self::new(next, self.ceiling)?;
        Ok(())
    }
}

impl TryFrom<V2> for V3 {
    type Error = Error;

    fn try_from(source: V2) -> Result<Self, Self::Error> {
        let counter = u32::try_from(source.counter()).map_err(|_| Error::Range)?;
        Self::new(counter, source.ceiling())
    }
}

impl From<V3> for V2 {
    fn from(source: V3) -> Self {
        // Every public V3 constructor/decoder maintains counter <= ceiling.
        // Widening is exact and changes neither that invariant nor its meaning.
        Self {
            counter: u64::from(source.counter),
            ceiling: source.ceiling,
        }
    }
}

impl V3View<'_> {
    /// Add through the restricted view, retaining the width and ceiling checks.
    pub fn add(&mut self, delta: u32) -> Result<(), Error> {
        self.record.add(delta)
    }
}

#[cfg(test)]
mod tests;
