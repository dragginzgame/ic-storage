//! Concrete scalar record versions for the first application conversion.
//! Only the repr(C) scalar payloads describe native layout; Record is a transient
//! schema selection, never a retained enum or permission to cast stored bytes.
//!
//! Declaration and view-confinement compile-fail cases belong to the product
//! [`ic_storage_native::Store`] API. This fixture owns conversion, mutation,
//! retained-layout and recovery evidence for its concrete application schemas.

pub mod narrow;

/// Existing logical schema identity; versioning is separate from this name.
pub const SCHEMA_ID: &[u8; 8] = V1::STORE_ID;

/// Original scalar payload, with counter at byte offset zero.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, ic_storage_native::Store)]
#[store(id = "RECU64V1", version = 1)]
pub struct V1 {
    /// Current counter, with every u64 value supported.
    #[store(field = 1)]
    pub counter: u64,
}

/// Revised payload: counter at offset zero, ceiling at offset eight.
/// Both fields are initialized scalars; no pointers or runtime headers persist.
/// Lend a restricted view of an already valid scalar record.
///
/// Ordinary errors retain earlier writes. The caller owns poison/trap policy;
/// this scope neither admits retained bytes nor provides a transaction.
/// Forgetting the view cannot prevent finalization by its caller.
///
/// ```
/// use ic_storage_native_feasibility::record::V2;
/// let mut record = V2::new(41, 100).unwrap();
/// let counter = record.store_with(|mut view| {
///     view.add(1).unwrap();
///     view.counter()
/// });
/// assert_eq!(counter, 42);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, ic_storage_native::Store)]
#[store(id = "RECU64V1", version = 2, validate = Self::valid)]
pub struct V2 {
    #[store(field = 1)]
    counter: u64,
    #[store(field = 2)]
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
        let candidate = Self { counter, ceiling };
        if !candidate.valid() {
            return Err(Error::Ceiling);
        }
        Ok(candidate)
    }

    fn valid(&self) -> bool {
        self.counter <= self.ceiling
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

impl V2View<'_> {
    /// Checked addition; a refusal preserves the current record.
    pub fn add(&mut self, delta: u64) -> Result<(), Error> {
        self.record.add(delta)
    }
}

/// Transient, explicit schema selection used by the codec and host fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    /// Original counter-only schema.
    V1(V1),
    /// Counter with an application ceiling.
    V2(V2),
    /// Counter narrowed to u32 with the original u64 ceiling.
    V3(narrow::V3),
}

impl Record {
    /// Counter shared by the two explicitly supported schemas.
    #[must_use]
    pub fn counter(self) -> u64 {
        match self {
            Self::V1(record) => record.counter,
            Self::V2(record) => record.counter(),
            Self::V3(record) => u64::from(record.counter()),
        }
    }
}

#[cfg(test)]
mod tests;
