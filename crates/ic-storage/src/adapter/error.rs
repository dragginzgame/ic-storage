///
/// `CollectionError`
///
/// A rejected collection operation. Encoding, validation and capacity errors
/// precede writes; backing panics still follow the host's failure contract.
///

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CollectionError {
    /// The persisted record or collection structure is invalid.
    #[error("invalid collection: {0}")]
    Corrupt(&'static str),
    /// The bytes belong to another format.
    #[error("unrecognized collection format")]
    Format,
    /// Record integrity verification failed.
    #[error("collection checksum mismatch")]
    Checksum,
    /// A record, arithmetic operation or allocation exceeds its resource budget.
    #[error("collection resource budget exceeded")]
    Budget,
    /// Capacity could not be reserved.
    #[error("collection growth refused")]
    Growth,
    /// A prior backing failure prevents further access through this owner.
    #[error("collection owner is poisoned")]
    Poisoned,
    /// An index or range is invalid.
    #[error("invalid collection index or range")]
    Bounds,
}

pub(super) type Result<T> = std::result::Result<T, CollectionError>;

pub(super) fn ensure(condition: bool, error: CollectionError) -> Result<()> {
    if condition { Ok(()) } else { Err(error) }
}
