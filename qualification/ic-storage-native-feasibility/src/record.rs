//! Concrete scalar record versions for the first application conversion.
//! Only the repr(C) scalar payloads describe native layout; Record is a transient
//! schema selection, never a retained enum or permission to cast stored bytes.

/// Existing logical schema identity; versioning is separate from this name.
pub const SCHEMA_ID: &[u8; 8] = b"RECU64V1";

/// Original scalar payload, with counter at byte offset zero.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V1 {
    /// Current counter, with every u64 value supported.
    pub counter: u64,
}

/// Revised payload: counter at offset zero, ceiling at offset eight.
/// Both fields are initialized scalars; no pointers or runtime headers persist.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V2 {
    counter: u64,
    ceiling: u64,
}

/// Refusal before changing the existing record.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The counter would exceed its explicit application ceiling.
    Ceiling,
    /// Addition would exceed the u64 range.
    Overflow,
}

impl V1 {
    /// Explicit application conversion; the caller chooses the new field's value.
    pub fn upgrade(self, ceiling: u64) -> Result<V2, Error> {
        V2::new(self.counter, ceiling)
    }
}

impl V2 {
    /// Validate the application invariant before constructing the new record.
    pub fn new(counter: u64, ceiling: u64) -> Result<Self, Error> {
        if counter > ceiling {
            return Err(Error::Ceiling);
        }
        Ok(Self { counter, ceiling })
    }

    /// Current counter value.
    #[must_use]
    pub fn counter(self) -> u64 {
        self.counter
    }

    /// Explicit maximum counter value, preserved by portable recovery.
    #[must_use]
    pub fn ceiling(self) -> u64 {
        self.ceiling
    }

    /// Checked application mutation; refusal leaves the complete record unchanged.
    pub fn add(&mut self, delta: u64) -> Result<(), Error> {
        let next = self.counter.checked_add(delta).ok_or(Error::Overflow)?;
        *self = Self::new(next, self.ceiling)?;
        Ok(())
    }
}

/// Transient, explicit schema selection used by the codec and host fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    /// Original counter-only schema.
    V1(V1),
    /// Counter with an application ceiling.
    V2(V2),
}

impl Record {
    /// Counter shared by the two explicitly supported schemas.
    #[must_use]
    pub fn counter(self) -> u64 {
        match self {
            Self::V1(record) => record.counter,
            Self::V2(record) => record.counter(),
        }
    }
}

#[cfg(test)]
mod tests;
