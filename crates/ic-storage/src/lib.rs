//! Scoped scalar-vector operations for an already valid live allocation.
//!
//! This crate neither opens retained bytes nor establishes allocator custody.
//! The vector caller owns admission, initialization, publication and failure policy.
//! The optional `map` feature adds an allocator-owned scalar hash table with
//! scoped access, host poisoning and portable recovery. No private Vec header
//! is admitted for persistence. The installed provider uses the vector API.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod record;

/// Native scalar hash tables, scoped mutation and portable recovery.
#[cfg(feature = "map")]
pub mod map;

/// Declare a bounded scalar record; this does not grant persistence admission.
///
/// Requires `repr(C)`, `Copy`, one to sixteen u32/u64 fields, explicit nonzero
/// field IDs, an eight-byte non-NUL ASCII schema ID and a nonzero version.
/// Primitive types may use bare names or absolute `::core::primitive` paths.
/// The optional validator checks logical encoding and decoding; `store_with`
/// assumes an already valid record. Callers own admission and poison/trap policy.
///
/// ```
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "APPVALUE", version = 1)]
/// struct Value { #[store(field = 1)] count: ::core::primitive::u32 }
/// let mut value = Value { count: 42 };
/// assert_eq!(value.store_encode().unwrap(), [42, 0, 0, 0]);
/// assert_eq!(Value::store_decode(&[42, 0, 0, 0]).unwrap().count, 42);
/// assert_eq!(value.store_with(|view| view.count()), 42);
/// ```
///
/// Borrowed fields refuse even if their record is Copy:
/// ```compile_fail
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "BADBORRO", version = 1)]
/// struct Bad { #[store(field = 1)] value: &'static u64 }
/// ```
/// Owning fields are outside the admitted subset:
/// ```compile_fail
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Store)]
/// #[store(id = "BADOWNER", version = 1)]
/// struct Bad { #[store(field = 1)] values: Vec<u64> }
/// ```
/// Application Drop cannot bypass the generated Copy requirement:
/// ```compile_fail
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Store)]
/// #[store(id = "BADDROPP", version = 1)]
/// struct Bad { #[store(field = 1)] value: u64 }
/// impl Drop for Bad { fn drop(&mut self) {} }
/// ```
/// Shadowing a scalar name cannot conceal a reference or a different width:
/// ```compile_fail
/// use ic_storage::Store;
/// type u64 = &'static str;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "BADALIAS", version = 1)]
/// struct Bad { #[store(field = 1)] value: u64 }
/// ```
/// ```compile_fail
/// use ic_storage::Store;
/// type u32 = u64;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "BADWIDTH", version = 1)]
/// struct Bad { #[store(field = 1)] value: u32 }
/// ```
///
/// Views cannot escape by return, outer capture or future:
/// ```compile_fail
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "APPVALUE", version = 1)]
/// struct Value { #[store(field = 1)] count: u32 }
/// let mut value = Value { count: 42 };
/// let escaped = value.store_with(|view| view);
/// ```
/// ```compile_fail
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "APPVALUE", version = 1)]
/// struct Value { #[store(field = 1)] count: u32 }
/// let mut value = Value { count: 42 };
/// let mut escaped = None;
/// value.store_with(|view| escaped = Some(view));
/// ```
/// ```compile_fail
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "APPVALUE", version = 1)]
/// struct Value { #[store(field = 1)] count: u32 }
/// let mut value = Value { count: 42 };
/// let future = value.store_with(|view| async move { view.count() });
/// ```
/// The view has no DerefMut route around reviewed application mutations:
/// ```compile_fail
/// use ic_storage::Store;
/// #[repr(C)]
/// #[derive(Clone, Copy, Store)]
/// #[store(id = "APPVALUE", version = 1)]
/// struct Value { #[store(field = 1)] count: u32 }
/// let mut value = Value { count: 42 };
/// value.store_with(|mut view| { let _: &mut Value = &mut *view; });
/// ```
#[cfg(feature = "derive")]
pub use ic_storage_derive::Store;

use alloc::vec::Vec;
use core::alloc::Allocator;

/// Expected refusal before this operation changes the logical vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Initial length or an attempted append exceeds the caller's element limit.
    Limit,
    /// The allocator refused an append; previous values remain intact.
    Allocation,
}

/// Lend a restricted view while keeping the owning Vec with the caller.
///
/// An oversized initial vector refuses before invoking the callback. Ordinary
/// returned errors preserve preceding writes. This is not a transaction or a
/// persistence admission API. A native host must arrange its own poisoning
/// before invoking user code; IC callers must trap on unexpected failure.
/// Forgetting the view does not leak the vector or leave an ownership ticket.
///
/// ```
/// use ic_storage::with_vector;
/// let mut values = vec![17, 23];
/// let sum = with_vector(&mut values, 512, |mut view| {
///     view.push(41).unwrap();
///     view.values().iter().sum::<u64>()
/// }).unwrap();
/// assert_eq!(sum, 81);
/// ```
///
/// A view cannot escape through a return value:
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// let escaped = with_vector(&mut values, 512, |view| view);
/// ```
/// An outer capture cannot retain it either:
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// let mut escaped = None;
/// with_vector(&mut values, 512, |view| escaped = Some(view));
/// ```
/// Nor can a future carry it out of the scope:
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// let future = with_vector(&mut values, 512, |view| async move { view.values().len() });
/// ```
#[inline(always)]
pub fn with_vector<A: Allocator, R>(
    values: &mut Vec<u64, A>,
    limit: usize,
    operation: impl for<'scope> FnOnce(Vector<'scope, A>) -> R,
) -> Result<R, Refusal> {
    if values.len() > limit {
        return Err(Refusal::Limit);
    }
    Ok(operation(Vector { values, limit }))
}

/// An exclusive scalar view, with no Deref, Clone or owning iterator.
///
/// Scalar slices cannot expose the Vec carrier to mem::take or mem::replace:
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// with_vector(&mut values, 512, |mut view| {
///     let _: Vec<u64> = core::mem::take(view.values_mut());
/// });
/// ```
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// with_vector(&mut values, 512, |mut view| {
///     let _ = core::mem::replace(view.values_mut(), Vec::<u64>::new());
/// });
/// ```
/// Borrowed elements cannot escape:
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// let escaped = with_vector(&mut values, 512, |view| view.values());
/// ```
/// Consuming iteration cannot extract the carrier:
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// with_vector(&mut values, 512, |view| view.into_iter());
/// ```
/// The view cannot clone an owning carrier:
/// ```compile_fail
/// use ic_storage::with_vector;
/// let mut values = vec![17];
/// with_vector(&mut values, 512, |view| view.clone());
/// ```
pub struct Vector<'scope, A: Allocator> {
    values: &'scope mut Vec<u64, A>,
    limit: usize,
}

impl<A: Allocator> Vector<'_, A> {
    /// Borrow only initialized scalar values, never spare capacity or the owner.
    #[must_use]
    #[inline(always)]
    pub fn values(&self) -> &[u64] {
        self.values
    }

    /// Mutate non-owning scalar elements without changing vector ownership.
    #[inline(always)]
    pub fn values_mut(&mut self) -> &mut [u64] {
        self.values
    }

    /// Append one scalar after checking the logical limit and reserving space.
    #[inline(always)]
    pub fn push(&mut self, value: u64) -> Result<(), Refusal> {
        if self.values.len() == self.limit {
            return Err(Refusal::Limit);
        }
        self.values
            .try_reserve_exact(1)
            .map_err(|_| Refusal::Allocation)?;
        self.values.push(value);
        Ok(())
    }

    /// Remove the first scalar, preserving the order of remaining values.
    #[inline(always)]
    pub fn remove_first(&mut self) -> Option<u64> {
        if self.values.is_empty() {
            None
        } else {
            Some(self.values.remove(0))
        }
    }

    /// Ask the existing allocator to reduce spare capacity; no platform-page claim.
    #[inline(always)]
    pub fn shrink_to_fit(&mut self) {
        self.values.shrink_to_fit();
    }

    /// Remove all scalar elements while keeping the allocation available for reuse.
    #[inline(always)]
    pub fn clear(&mut self) {
        self.values.clear();
    }
}

#[cfg(test)]
mod tests;
