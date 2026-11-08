// Adapted from ic-stable-structures 0.7.2, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See the package LICENSE-APACHE and docs/storage/icss-source.json.
//! This module implements a growable array in stable memory.

use crate::engine::Memory;
use crate::engine::base_vec::BaseVec;
pub use crate::engine::base_vec::InitError;
use crate::engine::storable::Storable;
use std::fmt;

const MAGIC: [u8; 3] = *b"SVC"; // Short for "stable vector".
pub struct Vec<T: Storable, M: Memory>(BaseVec<T, M>);

impl<T: Storable, M: Memory> Vec<T, M> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) const fn memory(&self) -> &M {
        self.0.memory()
    }
    pub fn new(memory: M) -> Self {
        BaseVec::<T, M>::new(memory, MAGIC)
            .map(Self)
            .expect("Failed to create a new vector")
    }
    pub fn init(memory: M) -> Result<Self, InitError> {
        BaseVec::<T, M>::init(memory, MAGIC).map(Self)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> u64 {
        self.0.len()
    }
    pub fn set(&self, index: u64, item: &T) {
        self.0.set(index, item);
    }
    pub fn get(&self, index: u64) -> Option<T> {
        self.0.get(index)
    }
    pub fn push(&self, item: &T) {
        self.0
            .push(item)
            .expect("Failed to push item to the vector");
    }
    pub fn pop(&self) -> Option<T> {
        self.0.pop()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = T> + '_ {
        self.0.iter()
    }
}

impl<T: Storable + fmt::Debug, M: Memory> fmt::Debug for Vec<T, M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmt)
    }
}
