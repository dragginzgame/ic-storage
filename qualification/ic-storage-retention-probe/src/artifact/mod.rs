//! Qualification-owned core-module admission and evidence formatting.
//! Byte identity comes from Host; Wasm syntax/types come from the selected parser.

use ic_host_artifacts::{Length, copy};
use std::{fmt::Write, io};

/// Refusal before a fixture is admitted for installation or transformation.
#[derive(Debug)]
pub enum Refusal {
    /// Source or output exceeds the caller's byte allowance.
    Limit,
    /// The artifact is not a version-one core Wasm module.
    Encoding,
    /// Text parsing or encoding failed.
    Text(wat::Error),
    /// Full instruction/type validation failed.
    Validation(wasmparser::BinaryReaderError),
}

/// Hash complete original bytes through the new Host byte-admission owner.
///
/// # Panics
/// Panics if Host refuses the exact-length in-memory read into an infallible sink.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    // A borrowed slice and sink cannot fail I/O; the exact bound is its own length.
    copy(bytes, io::sink(), Length::Exactly(bytes.len() as u64))
        .expect("complete in-memory byte identity")
        .sha256
}

/// Lowercase digest text used by existing qualification evidence reports.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in sha256(bytes) {
        write!(&mut output, "{byte:02x}").expect("format into String");
    }
    output
}

/// Validate a bounded core module with the pinned parser's default proposal set.
/// Byte admission happens before parsing; this is not a CPU or peak-memory quota.
pub fn validate_wasm(bytes: &[u8], limit: usize) -> Result<(), Refusal> {
    if bytes.len() > limit {
        return Err(Refusal::Limit);
    }
    if !bytes.starts_with(b"\0asm\x01\0\0\0") {
        return Err(Refusal::Encoding);
    }
    wasmparser::Validator::new()
        .validate_all(bytes)
        .map(|_| ())
        .map_err(Refusal::Validation)
}

/// Compile bounded fixture text; callers separately validate instructions/types.
pub fn compile_wat(
    source: &str,
    source_limit: usize,
    module_limit: usize,
) -> Result<Vec<u8>, Refusal> {
    if source.len() > source_limit {
        return Err(Refusal::Limit);
    }
    let bytes = wat::parse_str(source).map_err(Refusal::Text)?;
    if bytes.len() > module_limit {
        return Err(Refusal::Limit);
    }
    if !bytes.starts_with(b"\0asm\x01\0\0\0") {
        return Err(Refusal::Encoding);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
