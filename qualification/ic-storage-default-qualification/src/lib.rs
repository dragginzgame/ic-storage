//! External default-feature Storage consumer, compiled without runtime fixtures.
use ic_storage::{DefaultMemoryImpl, Memory};

/// Compile all backing operations without opting into any Storage features.
///
/// # Panics
///
/// Panics if the host refuses growth or backing IO does not roundtrip.
pub fn backing_roundtrip() {
    let memory = DefaultMemoryImpl::default();
    let previous = memory.size();
    assert_eq!(memory.grow(1), i64::try_from(previous).unwrap());
    let offset = previous * ic_storage::WASM_PAGE_SIZE_BYTES;
    let mut bytes = [0; 3];
    memory.read(offset, &mut bytes);
    assert_eq!(bytes, [0; 3]);
    memory.write(offset, &[1, 2, 3]);
    memory.read(offset, &mut bytes);
    assert_eq!(bytes, [1, 2, 3]);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
