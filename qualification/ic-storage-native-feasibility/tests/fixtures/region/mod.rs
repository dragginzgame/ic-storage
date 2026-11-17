//! One transient allocator over exclusively loaned backing. See provider-region.md.
//! Uses core/alloc so the same implementation compiles without std on wasm32.

extern crate alloc;

use alloc::vec::Vec;
use core::{
    alloc::{AllocError, Allocator, Layout},
    cell::Cell,
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::NonNull,
};

const MAX_ELEMENTS: usize = 512;

pub(super) mod sealed {
    pub trait Sealed {}
    impl Sealed for u64 {}
}

// This closed fixture subset excludes owning elements and application Drop.
// Eligibility is not allocation, schema or initialized-prefix authority.
pub(super) trait Element: Copy + sealed::Sealed {}
impl Element for u64 {}
impl sealed::Sealed for u8 {}
impl Element for u8 {}

// The installed fixture keeps its existing concrete u64 controller interface.
pub(super) type Region<'backing, T = u64> = Controller<'backing, T>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Descriptor {
    pub(super) length: usize,
    pub(super) capacity: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Geometry,
    Root,
    State,
    Allocation,
}

#[derive(Clone, Copy)]
enum State {
    Reserved(Descriptor),
    Free,
    Live(usize),
    Sealed,
}

pub(super) struct Controller<'backing, T: Element> {
    base: NonNull<u8>,
    bytes: usize,
    state: Cell<State>,
    issuing: Cell<bool>,
    backing: PhantomData<&'backing mut [MaybeUninit<T>]>,
}

impl<'backing, T: Element> Controller<'backing, T> {
    /// Numeric checks do not supply provenance, custody or initialization.
    ///
    /// # Safety
    /// The provider must loan one live, mutable allocation covering `elements`
    /// properly aligned T slots for 'backing, exclusively to this controller.
    /// Its first root.length slots must be valid initialized T values under the
    /// admitted schema (including any application invariants) when root is structurally
    /// valid. All prior owners must have ended; no provider access or settlement
    /// is allowed until this controller and all its clients have ended.
    #[expect(
        unsafe_code,
        reason = "explicit provider loan and initialization boundary"
    )]
    pub(super) unsafe fn claim(
        base: NonNull<u8>,
        elements: usize,
        root: Descriptor,
    ) -> Result<Self, Error> {
        if elements == 0
            || elements > MAX_ELEMENTS
            || size_of::<T>() == 0
            || !base.as_ptr().addr().is_multiple_of(align_of::<T>())
        {
            return Err(Error::Geometry);
        }
        if root.length > root.capacity || root.capacity > elements {
            return Err(Error::Root);
        }
        Ok(Self {
            base,
            bytes: elements
                .checked_mul(size_of::<T>())
                .ok_or(Error::Geometry)?,
            state: Cell::new(State::Reserved(root)),
            issuing: Cell::new(false),
            backing: PhantomData,
        })
    }

    pub(super) fn restore(&self) -> Result<Vec<T, &Self>, Error> {
        let State::Reserved(root) = self.state.get() else {
            return Err(Error::State);
        };
        if root.capacity == 0 {
            self.state.set(State::Free);
            return Ok(Vec::new_in(self));
        }
        let layout = Layout::array::<T>(root.capacity).map_err(|_| Error::Allocation)?;
        self.issuing.set(true);
        let allocated = self.allocate(layout);
        self.issuing.set(false);
        let pointer = allocated.map_err(|_| Error::Allocation)?.cast::<T>();
        // SAFETY: ordinary allocate just issued this exact reserved block from
        // the loaned allocation, without writing payload. claim's caller owns
        // initialization/custody; structural checks bound length and capacity.
        // No fallible operation or callback occurs between issuance and Vec.
        // Borrowing this controller keeps the provider loan live; no old Vec
        // remains and state prevents a second reconstruction.
        #[expect(
            unsafe_code,
            reason = "freshly issued block with admitted initialized prefix"
        )]
        Ok(unsafe { Vec::from_raw_parts_in(pointer.as_ptr(), root.length, root.capacity, self) })
    }

    pub(super) fn release<'region>(
        &self,
        values: Vec<T, &'region Self>,
    ) -> Result<Descriptor, (Error, Vec<T, &'region Self>)> {
        let root = Descriptor {
            length: values.len(),
            capacity: values.capacity(),
        };
        let owns_block = match self.state.get() {
            State::Free => root.capacity == 0,
            State::Live(bytes) => {
                bytes == root.capacity * size_of::<T>()
                    && values.as_ptr().cast::<u8>() == self.base.as_ptr()
            }
            _ => false,
        };
        if !core::ptr::eq(*values.allocator(), self) || !owns_block {
            return Err((Error::State, values));
        }
        // Consume the sole owning header without destruction or deallocation.
        // Neither pointer nor allocator reference becomes part of the descriptor.
        let _ = values.into_raw_parts_with_allocator();
        self.state.set(State::Sealed);
        Ok(root)
    }

    fn supported(&self, layout: Layout) -> bool {
        layout.align() == align_of::<T>()
            && layout.size() > 0
            && layout.size() <= self.bytes
            && layout.size().is_multiple_of(size_of::<T>())
    }

    fn current(&self, pointer: NonNull<u8>, layout: Layout) -> bool {
        pointer == self.base
            && self.supported(layout)
            && matches!(self.state.get(), State::Live(bytes) if bytes == layout.size())
    }

    fn resize(
        &self,
        pointer: NonNull<u8>,
        old: Layout,
        new: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        if !self.current(pointer, old) {
            invalid();
        }
        if !self.supported(new) {
            return Err(AllocError);
        }
        // Only one block exists; the whole backing extent remains exclusively
        // loaned. Changing its issued prefix preserves old initialized bytes.
        self.state.set(State::Live(new.size()));
        Ok(NonNull::slice_from_raw_parts(self.base, new.size()))
    }
}

// SAFETY: claim supplies one exclusive stable allocation for the borrow lifetime.
// Only one exact-sized T-aligned prefix can be issued at once. Reservation is
// accessible only during restore; allocation never writes its initialized bytes.
// Cell metadata is transient, not Sync, and never allocates or invokes callbacks.
// In-place resizing stays inside the backing; failure preserves the old block.
// No Clone equivalence or automatic release of the provider loan is implemented.
#[expect(
    unsafe_code,
    reason = "bounded single-block provider allocator with explicit loan contract"
)]
unsafe impl<T: Element> Allocator for Controller<'_, T> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if !self.supported(layout) {
            return Err(AllocError);
        }
        match self.state.get() {
            State::Free => {}
            State::Reserved(root)
                if self.issuing.get() && root.capacity * size_of::<T>() == layout.size() => {}
            _ => return Err(AllocError),
        }
        self.state.set(State::Live(layout.size()));
        Ok(NonNull::slice_from_raw_parts(self.base, layout.size()))
    }

    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        if !self.current(pointer, layout) {
            invalid();
        }
        self.state.set(State::Free);
    }

    unsafe fn grow(
        &self,
        pointer: NonNull<u8>,
        old: Layout,
        new: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        self.resize(pointer, old, new)
    }

    unsafe fn shrink(
        &self,
        pointer: NonNull<u8>,
        old: Layout,
        new: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        self.resize(pointer, old, new)
    }
}

fn invalid() -> ! {
    // Allocator contract violations never unwind. The Wasm diagnostic uses no std.
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    std::process::abort();
}

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "provider-pool"))]
pub(super) mod pool;
