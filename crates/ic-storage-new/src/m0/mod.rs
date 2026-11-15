//! Admission and controlled mutation of M0 v1 memory images.
//!
//! All bytes are decoded explicitly. Inspection uses constant scratch space,
//! scans at most 1024 objects and follows no recursive graph paths. An immutable
//! slice represents one coherent image; obtaining that image from a live canister
//! and enforcing its custody are separate integration responsibilities.
//! [`image`] provides exclusive native-buffer custody and scoped graph mutation.

pub mod image;
mod layout;
#[cfg(test)]
mod tests;

use layout::{
    ARENA_BASE, CAPACITY, COUNTER, DESCRIPTORS, DOMAIN, INDEX_BASE, INDEX_END, INDEX_STRIDE,
    MEMORY_LIMIT, META_BASE, NODE, PAGE,
};
use sha2::{Digest, Sha256};

/// Why an image failed admission. Offsets identify bytes in the complete image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    /// Physical memory is outside 129–256 pages or is not page-aligned.
    MemorySize,
    /// A required byte range is absent or its arithmetic overflowed.
    Bounds,
    /// An immutable header value or root identity does not match M0 v1.
    Format {
        /// Absolute byte offset of the mismatched field.
        at: usize,
    },
    /// The metadata digest does not match the exact page and domain.
    Integrity,
    /// A descriptor differs from the complete expected descriptor catalog.
    Descriptors,
    /// Required zero bytes are nonzero.
    Reserved {
        /// Absolute byte offset of the first nonzero reserved byte.
        at: usize,
    },
    /// Retained extent, object count, cursor or next identity is inconsistent.
    Accounting,
    /// An allocation-index entry has an invalid identity, offset or span.
    Index {
        /// Zero-based allocation-index entry.
        entry: u32,
    },
    /// An indexed object's header, schema or payload size is invalid.
    Object {
        /// Zero-based allocation-index entry.
        entry: u32,
    },
    /// A non-null reference does not identify an indexed object of the right schema.
    Reference {
        /// Absolute byte offset of the reference field.
        at: usize,
    },
}

impl core::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "M0 admission refused: {self:?}")
    }
}

impl core::error::Error for AdmissionError {}

/// A decoded identity value. This is not an access capability or a Rust pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectReference {
    /// Offset of the object header relative to the arena base.
    pub offset: u64,
    /// Nonzero, non-reused allocation identity in the inspected image.
    pub identity: u64,
}

/// Accounting from an image that passed complete M0 admission.
///
/// This owned summary confers no authority over another or subsequently changed
/// image. It does not authenticate bytes, enforce business invariants or prove an
/// installed upgrade safe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Summary {
    /// Number of completely validated indexed objects, including unreachable ones.
    pub object_count: u32,
    /// Physically committed arena bytes.
    pub committed_bytes: u64,
    /// End of the contiguous indexed object prefix.
    pub used_bytes: u64,
    /// Next append-only allocation identity.
    pub next_identity: u64,
    /// Validated CounterState root, or no published root during construction.
    pub root: Option<ObjectReference>,
}

fn bytes<const N: usize>(source: &[u8], offset: usize) -> Result<[u8; N], AdmissionError> {
    let end = offset.checked_add(N).ok_or(AdmissionError::Bounds)?;
    source
        .get(offset..end)
        .ok_or(AdmissionError::Bounds)?
        .try_into()
        .map_err(|_| AdmissionError::Bounds)
}

fn u32_at(source: &[u8], offset: usize) -> Result<u32, AdmissionError> {
    Ok(u32::from_le_bytes(bytes(source, offset)?))
}

fn u64_at(source: &[u8], offset: usize) -> Result<u64, AdmissionError> {
    Ok(u64::from_le_bytes(bytes(source, offset)?))
}

fn expect_bytes(meta: &[u8], offset: usize, expected: &[u8]) -> Result<(), AdmissionError> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(AdmissionError::Bounds)?;
    if meta.get(offset..end) != Some(expected) {
        return Err(AdmissionError::Format {
            at: META_BASE + offset,
        });
    }
    Ok(())
}

fn zeros(source: &[u8], absolute: usize) -> Result<(), AdmissionError> {
    if let Some(i) = source.iter().position(|&byte| byte != 0) {
        return Err(AdmissionError::Reserved { at: absolute + i });
    }
    Ok(())
}

fn metadata(meta: &[u8]) -> Result<(), AdmissionError> {
    expect_bytes(meta, 0, b"ICOPM0\0\0")?;
    expect_bytes(meta, 16, b"op.m0.layout.v01")?;
    for (offset, value) in [
        (8, 1u32),
        (12, 256),
        (92, 1024),
        (96, 256),
        (100, 2),
        (104, 128),
        (108, 512),
        (112, 1),
        (116, 64),
        (120, 1024),
        (124, 32),
        (128, 1),
        (132, 0),
    ] {
        expect_bytes(meta, offset, &value.to_le_bytes())?;
    }
    for (offset, value) in [
        (32, META_BASE as u64),
        (40, PAGE as u64),
        (48, ARENA_BASE as u64),
        (56, (MEMORY_LIMIT - ARENA_BASE) as u64),
    ] {
        expect_bytes(meta, offset, &value.to_le_bytes())?;
    }
    for (start, end) in [
        (136, 160),
        (192, 256),
        (560, 576),
        (576, INDEX_BASE),
        (INDEX_END, PAGE),
    ] {
        zeros(&meta[start..end], META_BASE + start)?;
    }
    let actual = metadata_digest(meta);
    if actual != bytes::<32>(meta, 160)? {
        return Err(AdmissionError::Integrity);
    }
    if meta[256..512] != DESCRIPTORS {
        return Err(AdmissionError::Descriptors);
    }
    expect_bytes(meta, 512, b"op.m0.root.v1\0\0\0")?;
    expect_bytes(meta, 528, COUNTER)?;
    Ok(())
}

// The caller supplies the already bounded, complete metadata page. Shared by
// read-only admission and the image writer so mutation has identical coverage.
fn metadata_digest(meta: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(&meta[..160]);
    digest.update([0; 32]);
    digest.update(&meta[192..]);
    digest.finalize().into()
}

fn reference(
    memory: &[u8],
    meta: &[u8],
    count: u32,
    at: usize,
    target: &[u8; 16],
) -> Result<Option<ObjectReference>, AdmissionError> {
    let offset = u64_at(memory, at)?;
    let identity = u64_at(memory, at + 8)?;
    if offset == 0 && identity == 0 {
        return Ok(None);
    }
    let invalid = AdmissionError::Reference { at: META_BASE + at };
    if identity == 0 || identity > u64::from(count) {
        return Err(invalid);
    }
    // Sequential IDs give a bounded O(1) index lookup. The first pass admitted
    // every index/header span; identity is bounded before converting to usize.
    let index = INDEX_BASE + (identity as usize - 1) * INDEX_STRIDE;
    let indexed_offset = u64_at(meta, index)?;
    if offset != indexed_offset {
        return Err(invalid);
    }
    let absolute = PAGE
        .checked_add(usize::try_from(offset).map_err(|_| invalid)?)
        .ok_or(invalid)?;
    if bytes::<16>(memory, absolute + 8)? != *target {
        return Err(invalid);
    }
    Ok(Some(ObjectReference { offset, identity }))
}

/// Validate a complete immutable wasm32 linear-memory image against M0 v1.
///
/// Requires 129–256 complete 64 KiB pages. The low 8 MiB is transient and ignored;
/// every metadata byte and every indexed header/reference is checked. Scalar
/// payload values may be any u64. Unused arena bytes are not interpreted.
///
/// No input is changed and zero/missing metadata is never initialized. This is
/// snapshot validation, not a live-memory handle or an IC post-upgrade hook.
///
/// # Errors
/// Returns a typed refusal for incompatible, corrupt or truncated representation.
/// The metadata digest is checked before interpreting its index. Semantic checks
/// still apply when an attacker has recomputed a matching digest.
pub fn inspect(memory: &[u8]) -> Result<Summary, AdmissionError> {
    if !(ARENA_BASE..=MEMORY_LIMIT).contains(&memory.len()) || !memory.len().is_multiple_of(PAGE) {
        return Err(AdmissionError::MemorySize);
    }
    inspect_retained(&memory[META_BASE..])
}

/// Admit only the retained region, beginning at the fixed 8 MiB boundary.
///
/// The slice contains the metadata page followed by the entire physical arena.
/// It must exclude the executing stack, module data and transient allocator.
/// Error offsets remain absolute linear-memory addresses. The caller owns
/// custody and must supply a coherent slice; this function creates no raw slice.
///
/// # Errors
/// Refuses sizes outside 1–128 pages or any invalid M0 representation, without
/// modifying bytes or initializing missing state.
pub fn inspect_retained(memory: &[u8]) -> Result<Summary, AdmissionError> {
    if !(PAGE..=MEMORY_LIMIT - META_BASE).contains(&memory.len())
        || !memory.len().is_multiple_of(PAGE)
    {
        return Err(AdmissionError::MemorySize);
    }
    let meta = &memory[..PAGE];
    metadata(meta)?;
    let count = u32_at(meta, 88)?;
    let committed = u64_at(meta, 64)?;
    let cursor = u64_at(meta, 72)?;
    let next = u64_at(meta, 80)?;
    if count > CAPACITY as u32
        || committed != (memory.len() - PAGE) as u64
        || cursor > committed
        || !cursor.is_multiple_of(8)
        || next != u64::from(count) + 1
    {
        return Err(AdmissionError::Accounting);
    }
    let inactive = INDEX_BASE + count as usize * INDEX_STRIDE;
    zeros(&meta[inactive..INDEX_END], META_BASE + inactive)?;

    // First admit every span/header, even if no root reaches it. Fixed object
    // lengths and checked u64 arithmetic precede all host-width conversions.
    let mut end = 0u64;
    for entry in 0..count {
        let index = INDEX_BASE + entry as usize * INDEX_STRIDE;
        let offset = u64_at(meta, index)?;
        let identity = u64_at(meta, index + 8)?;
        let span = u64_at(meta, index + 16)?;
        let invalid = AdmissionError::Index { entry };
        if offset != end
            || identity != u64::from(entry) + 1
            || span < 48
            || !span.is_multiple_of(8)
            || u64_at(meta, index + 24)? != 0
        {
            return Err(invalid);
        }
        end = offset.checked_add(span).ok_or(invalid)?;
        if end > cursor {
            return Err(invalid);
        }
        let at = PAGE + offset as usize; // end <= cursor <= physical extent
        let object = AdmissionError::Object { entry };
        let schema = bytes::<16>(memory, at + 8)?;
        let payload = if schema == *COUNTER {
            40u64
        } else if schema == *NODE {
            24
        } else {
            return Err(object);
        };
        if u64_at(memory, at)? != identity
            || u64_at(memory, at + 24)? != payload
            || span != 48 + payload
        {
            return Err(object);
        }
        zeros(&memory[at + 32..at + 48], META_BASE + at + 32)?;
    }
    if end != cursor {
        return Err(AdmissionError::Accounting);
    }

    // Now references may use any admitted index entry, including a forward edge
    // or a cycle. No recursive traversal or graph-sized scratch is required.
    for entry in 0..count {
        let index = INDEX_BASE + entry as usize * INDEX_STRIDE;
        let at = PAGE + u64_at(meta, index)? as usize;
        let _ = reference(memory, meta, count, at + 48 + 8, NODE)?;
        if bytes::<16>(memory, at + 8)? == *COUNTER {
            let _ = reference(memory, meta, count, at + 48 + 24, NODE)?;
        }
    }
    let root = reference(memory, meta, count, 544, COUNTER)?;
    Ok(Summary {
        object_count: count,
        committed_bytes: committed,
        used_bytes: cursor,
        next_identity: next,
        root,
    })
}
