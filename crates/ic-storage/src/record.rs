//! Description types for the experimental scalar-only Store derive.
//! These describe concrete public layouts and logical fields, not allocator or
//! pointer validity. No marker trait grants permission to reopen retained bytes.

/// Supported unsigned scalar kind; this metadata enum is not a retained ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    /// A four-byte unsigned integer.
    U32,
    /// An eight-byte unsigned integer.
    U64,
}

/// One explicitly identified field in a repr(C) scalar declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// Owner-assigned field identity, stable across declaration reordering.
    pub id: u32,
    /// Compiler-established byte offset in the concrete repr(C) payload.
    pub offset: usize,
    /// Concrete scalar kind; equal offsets or record sizes do not imply compatibility.
    pub scalar: Scalar,
}

/// Refusal by the generated bounded logical codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Input length differs from the exact declared scalar payload size.
    Length,
    /// Application validation rejected the scalar values.
    Invariant,
}
