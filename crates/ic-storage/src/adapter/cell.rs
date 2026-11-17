//! Governed cell collection implementation.
use super::{Bound, Memory, Storable};
use crate::RuntimeMemory;
use crate::{DefaultMemoryImpl, substrate};
use std::borrow::Cow;

///
/// `StableCell`
///
/// Governed upstream `Cell` with explicit initialization and read-only loading.
///
/// Values and stored bytes retain the upstream codec and layout contracts.
///
pub struct StableCell<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::Cell<CellValue<T>, RuntimeMemory<M>>,
}

// Upstream has no load constructor. An absent value is an internal loading
// marker, never a default T or a persisted record. Serialization refuses it
// before upstream can grow/write on its initialization fallback. Successful
// construction always holds Some(T), decoded exactly once by upstream.
struct CellValue<T>(Option<T>);

impl<T: Storable> Storable for CellValue<T> {
    const BOUND: Bound = T::BOUND;

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0
            .as_ref()
            .expect("cannot initialize a loading cell")
            .to_bytes_checked()
    }

    fn into_bytes(self) -> Vec<u8> {
        self.0
            .expect("cannot initialize a loading cell")
            .into_bytes_checked()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        T::check_bounds(&bytes);
        Self(Some(T::from_bytes(bytes)))
    }
}

impl<T: Storable, M: Memory> StableCell<T, M> {
    /// Create a cell only in zero-page memory, enforcing declared record bounds.
    ///
    /// # Panics
    /// Refuses nonempty memory or a value outside its declared bound before
    /// writing. Growth and codec failures retain upstream behavior.
    pub fn create(memory: RuntimeMemory<M>, value: T) -> Self {
        assert_eq!(memory.size(), 0, "cell memory is not empty");
        Self::reset(memory, value)
    }

    /// Deliberately replace any contents, enforcing the record's declared bound.
    ///
    /// # Panics
    /// Refuses oversized/fixed-width mismatches before writing. Growth and codec
    /// failures retain upstream behavior. Unbounded record types remain allowed.
    pub fn reset(memory: RuntimeMemory<M>, value: T) -> Self {
        Self {
            inner: substrate::Cell::new(memory, CellValue(Some(value))),
        }
    }
    /// Load an existing cell without initializing, writing or growing memory.
    ///
    /// Requires no default value or `Default`/`Clone` bound. The retained value
    /// is decoded by upstream using `T::from_bytes`. A successful load retains
    /// the allocation's exclusive lease until this cell is dropped.
    ///
    /// # Errors
    /// Returns [`crate::StableCellPayloadError`] for empty/foreign memory,
    /// unsupported layout versions, a payload beyond memory or the declared maximum. The
    /// consumed lease is released on failure; no bytes are changed.
    ///
    /// # Panics
    /// Codec and backing failures propagate as upstream panics. Loading does
    /// not write or grow even on these failures. Ordinary unwinding releases
    /// the consumed lease; Wasm traps retain the host's rollback semantics.
    /// This validates the envelope, not the application's schema or recovery policy.
    pub fn load(memory: RuntimeMemory<M>) -> Result<Self, crate::StableCellPayloadError> {
        let maximum = match T::BOUND {
            Bound::Bounded { max_size, .. } => max_size as usize,
            Bound::Unbounded => usize::MAX,
        };
        crate::stable_cell_payload_length(&memory, maximum)?;
        Ok(Self {
            inner: substrate::Cell::init(memory, CellValue(None)),
        })
    }
    /// Delegates to upstream `Cell::get` with the governed allocation.
    #[expect(
        clippy::missing_panics_doc,
        reason = "constructors always establish a cached value"
    )]
    pub fn get(&self) -> &T {
        self.inner.get().0.as_ref().expect("loaded cell value")
    }
    /// Delegates to upstream `Cell::set` with the governed allocation.
    ///
    /// # Panics
    /// Panics on insufficient memory or codec/backing failure, as upstream.
    pub fn set(&mut self, value: T) -> T {
        self.inner
            .set(CellValue(Some(value)))
            .0
            .expect("loaded cell value")
    }
}
