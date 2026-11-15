//! Fixed, allocation-free workload shared by both installed benchmark canisters.
#![no_std]
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
pub mod wire;

/// Semantic operations shared by a scalar cell and an ordered sequence.
pub trait Access {
    /// Backend failure, never converted into a successful measurement.
    type Error;
    /// Read the scalar.
    fn scalar(&self) -> Result<u64, Self::Error>;
    /// Persist a replacement scalar.
    fn set_scalar(&mut self, value: u64) -> Result<(), Self::Error>;
    /// Sum every sequence value in order, with wrapping arithmetic.
    fn scan(&self) -> Result<u64, Self::Error>;
    /// Replace every sequence value with `base + index`.
    fn replace(&mut self, base: u64) -> Result<(), Self::Error>;
    /// Append one value, preserving sequence order.
    fn append(&mut self, value: u64) -> Result<(), Self::Error>;
}

/// Fixed operation names, also used in the raw CSV.
pub const OPERATIONS: [&str; 6] = [
    "control",
    "scalar_read",
    "scalar_write",
    "sequence_scan",
    "sequence_replace",
    "append",
];

/// Execute one admitted workload. Setup, request decoding, replies and correctness
/// queries are outside this function. Batches and operation tags are admitted by
/// the wire parser before timing. Control uses the same arithmetic loop but no IO.
///
/// # Errors
/// Propagates any backend refusal; a failed operation never yields a timing row.
///
/// # Panics
/// Panics if an operation outside `OPERATIONS` reaches a nonempty batch.
pub fn run<A: Access>(access: &mut A, operation: usize, batch: u64) -> Result<u64, A::Error> {
    let mut result = 0u64;
    for i in 0..batch {
        let value = core::hint::black_box(10_000 + i);
        let observed = match operation {
            0 => value,
            1 => access.scalar()?,
            2 => {
                access.set_scalar(value)?;
                value
            }
            3 => access.scan()?,
            4 => {
                access.replace(value)?;
                value
            }
            5 => {
                access.append(value)?;
                value
            }
            _ => unreachable!("wire admits operation"),
        };
        result = result.wrapping_add(core::hint::black_box(observed));
    }
    Ok(result)
}
