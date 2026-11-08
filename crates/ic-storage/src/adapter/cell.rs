//! Governed cached cell over the copied ICSS cell engine.
use super::{
    Bound, Memory, Storable,
    bridge::{self, Codec},
    error::{CollectionError as Error, Result, ensure},
};
use crate::{DefaultMemoryImpl, RuntimeMemory, engine};

///
/// `StableCell`
///
/// ICSS-baseline single cached value with explicit non-initializing admission.
/// Stored values have no checksum. Malformed application records trap.
///
pub struct StableCell<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: engine::cell::Cell<Codec<T>, RuntimeMemory<M>>,
}
impl<T: Storable, M: Memory> StableCell<T, M> {
    /// Create only in empty admitted memory.
    /// # Errors
    /// Refuses nonempty memory or initial growth refusal.
    pub fn create(memory: RuntimeMemory<M>, value: T) -> Result<Self> {
        ensure(memory.size() == 0, Error::NotEmpty)?;
        Self::reset(memory, value)
    }
    /// Explicitly replace the cell image.
    /// # Errors
    /// Refuses initial growth; codec/engine failures trap.
    pub fn reset(memory: RuntimeMemory<M>, value: T) -> Result<Self> {
        bridge::reserve(&memory)?;
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = memory.poison_on_unwind();
        Ok(Self {
            inner: engine::cell::Cell::new(memory, Codec(value)),
        })
    }
    /// Load without initializing. Admit the payload ceiling before allocation.
    /// # Errors
    /// Refuses invalid framing or an exceeded ceiling; malformed values trap.
    /// # Panics
    /// Stored codec failures propagate as traps.
    pub fn load(memory: RuntimeMemory<M>, maximum: usize) -> Result<Self> {
        let maximum = match T::BOUND {
            Bound::Bounded { max_size, .. } => maximum.min(max_size as usize),
            Bound::Unbounded => maximum,
        };
        let length = crate::stable_cell_payload_length(&memory, maximum)?;
        Ok(Self {
            inner: engine::cell::Cell::load_with_length(
                memory,
                u32::try_from(length).expect("admitted cell length"),
            ),
        })
    }
    /// Borrow the cached value without IO.
    pub const fn get(&self) -> &T {
        &self.inner.get().0
    }
    /// Persist a replacement and return the old value. Failures trap.
    pub fn set(&mut self, value: T) -> T {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.inner.memory().poison_on_unwind();
        self.inner.set(Codec(value)).0
    }
}
