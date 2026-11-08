///
/// `CollectionError`
///
/// Collection admission or region access failed. Codec and engine failures
/// trap and follow the host's failure contract.
///

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CollectionError {
    /// The bytes belong to another format.
    #[error("unrecognized collection format")]
    Format,
    /// Capacity could not be reserved.
    #[error("collection growth refused")]
    Growth,
    /// Creation was attempted in nonempty memory.
    #[error("collection memory is not empty")]
    NotEmpty,
    /// Cell envelope admission failed before reading its payload.
    #[error("{0}")]
    Cell(#[from] crate::StableCellPayloadError),
}

pub(super) type Result<T> = std::result::Result<T, CollectionError>;

pub(super) const fn ensure(condition: bool, error: CollectionError) -> Result<()> {
    if condition { Ok(()) } else { Err(error) }
}
