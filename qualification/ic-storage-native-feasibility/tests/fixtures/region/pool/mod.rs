//! Extension of the foreign loan contract to disjoint runs, shared with Wasm.
//! Metadata is transient; only explicitly released root spans may be retained.

extern crate alloc;

use super::{Element, Error, invalid};
use alloc::vec::Vec;
use core::{
    alloc::{AllocError, Allocator, Layout},
    cell::Cell,
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::NonNull,
};

const RUNS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Root {
    pub(in super::super) start: usize,
    pub(in super::super) length: usize,
    pub(in super::super) capacity: usize,
}

#[derive(Clone, Copy)]
enum State {
    Free,
    Reserved(Root),
    Live(Root),
    Sealed(Root),
}

impl State {
    fn span(self) -> Option<Root> {
        match self {
            Self::Free => None,
            Self::Reserved(root) | Self::Live(root) | Self::Sealed(root) => Some(root),
        }
    }
}

pub(in super::super) struct Pool<'backing, T: Element> {
    base: NonNull<u8>,
    elements: usize,
    runs: Cell<[State; RUNS]>,
    issuing: Cell<Option<usize>>,
    finished: Cell<bool>,
    // Host evidence only; neither this field nor observation code enters Wasm.
    #[cfg(test)]
    peak_elements: Cell<usize>,
    backing: PhantomData<&'backing mut [MaybeUninit<T>]>,
}

impl<'backing, T: Element> Pool<'backing, T> {
    /// # Safety
    /// Loan one exclusive live aligned allocation of `elements` T slots for
    /// 'backing. For every structurally valid, disjoint root, its length prefix
    /// must contain valid initialized T values. Prior owners must have ended.
    /// Provider access/reissuance is forbidden until all clients end and finish
    /// succeeds. Numeric descriptors alone do not supply any of these facts.
    #[expect(
        unsafe_code,
        reason = "exclusive provider loan and initialized-root boundary"
    )]
    pub(in super::super) unsafe fn claim(
        base: NonNull<u8>,
        elements: usize,
        roots: &[Root],
    ) -> Result<Self, Error> {
        if elements == 0
            || size_of::<T>() == 0
            || Layout::array::<T>(elements).is_err()
            || !base.as_ptr().addr().is_multiple_of(align_of::<T>())
        {
            return Err(Error::Geometry);
        }
        if roots.len() > RUNS {
            return Err(Error::Root);
        }
        let mut runs = [State::Free; RUNS];
        for (index, root) in roots.iter().copied().enumerate() {
            let end = root.start.checked_add(root.capacity).ok_or(Error::Root)?;
            if root.capacity == 0 || root.length > root.capacity || end > elements {
                return Err(Error::Root);
            }
            if roots[..index]
                .iter()
                .any(|other| root.start < other.start + other.capacity && other.start < end)
            {
                return Err(Error::Root);
            }
            runs[index] = State::Reserved(root);
        }
        Ok(Self {
            base,
            elements,
            runs: Cell::new(runs),
            issuing: Cell::new(None),
            finished: Cell::new(false),
            #[cfg(test)]
            peak_elements: Cell::new(roots.iter().map(|root| root.capacity).sum()),
            backing: PhantomData,
        })
    }

    pub(in super::super) fn restore(&self, index: usize) -> Result<Vec<T, &Self>, Error> {
        if self.finished.get() {
            return Err(Error::State);
        }
        let Some(State::Reserved(root)) = self.runs.get().get(index).copied() else {
            return Err(Error::State);
        };
        let layout = Layout::array::<T>(root.capacity).map_err(|_| Error::Allocation)?;
        self.issuing.set(Some(index));
        let issued = self.allocate(layout);
        self.issuing.set(None);
        let pointer = issued.map_err(|_| Error::Allocation)?.cast::<T>();
        // SAFETY: allocate freshly issued this exact reserved run without writes.
        // claim supplies initialization/provenance, the run has no other owner,
        // and this borrow keeps the controller and backing loan live.
        #[expect(unsafe_code, reason = "fresh issuance of a reserved initialized run")]
        Ok(unsafe { Vec::from_raw_parts_in(pointer.as_ptr(), root.length, root.capacity, self) })
    }

    pub(in super::super) fn release<'pool>(
        &self,
        values: Vec<T, &'pool Self>,
    ) -> Result<Option<Root>, (Error, Vec<T, &'pool Self>)> {
        if self.finished.get() || !core::ptr::eq(*values.allocator(), self) {
            return Err((Error::State, values));
        }
        if values.capacity() == 0 {
            return Ok(None);
        }
        let mut runs = self.runs.get();
        let Some(index) = runs.iter().position(|state| {
            matches!(state, State::Live(root) if root.capacity == values.capacity()
                && self.pointer(root.start).as_ptr().cast_const() == values.as_ptr().cast::<u8>())
        }) else {
            return Err((Error::State, values));
        };
        let State::Live(mut root) = runs[index] else {
            invalid()
        };
        root.length = values.len();
        let _ = values.into_raw_parts_with_allocator();
        runs[index] = State::Sealed(root);
        self.runs.set(runs);
        Ok(Some(root))
    }

    /// A forgotten or unopened owner prevents settlement; never reclaim it.
    pub(in super::super) fn finish(&self) -> Result<(), Error> {
        if self.finished.get()
            || self
                .runs
                .get()
                .iter()
                .any(|state| matches!(state, State::Live(_) | State::Reserved(_)))
        {
            return Err(Error::State);
        }
        self.finished.set(true);
        Ok(())
    }

    #[cfg(test)]
    fn reusable_elements(&self) -> usize {
        self.elements
            - self
                .runs
                .get()
                .iter()
                .filter_map(|state| state.span())
                .map(|root| root.capacity)
                .sum::<usize>()
    }

    fn supported(&self, layout: Layout) -> bool {
        layout.align() == align_of::<T>()
            && layout.size() != 0
            && layout.size().is_multiple_of(size_of::<T>())
            && layout.size() / size_of::<T>() <= self.elements
    }

    fn pointer(&self, start: usize) -> NonNull<u8> {
        // SAFETY: only admitted nonempty runs call this. Their element offsets
        // are inside the original exclusive allocation, with no integer-to-
        // pointer reconstruction or buffer-wide reference over live clients.
        #[expect(
            unsafe_code,
            reason = "derive a disjoint run pointer from its live loan"
        )]
        unsafe {
            self.base.add(start * size_of::<T>())
        }
    }
}

// SAFETY: one exclusive stable allocation, exact-size disjoint T-aligned runs,
// and non-Sync transient metadata. Reserved and sealed runs remain unavailable.
// allocate changes only metadata, deallocate releases only the exact live run;
// failure changes neither metadata nor payload. &Pool keeps the loan alive.
// Default grow/shrink can allocate/copy/free distinct runs under this contract.
#[expect(unsafe_code, reason = "private provider-loan pool allocator boundary")]
unsafe impl<T: Element> Allocator for Pool<'_, T> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if self.finished.get() || !self.supported(layout) {
            return Err(AllocError);
        }
        let capacity = layout.size() / size_of::<T>();
        let mut runs = self.runs.get();
        let (index, root) = if let Some(index) = self.issuing.get() {
            let State::Reserved(root) = runs[index] else {
                return Err(AllocError);
            };
            if root.capacity != capacity {
                return Err(AllocError);
            }
            (index, root)
        } else {
            let index = runs
                .iter()
                .position(|state| matches!(state, State::Free))
                .ok_or(AllocError)?;
            let mut start = 0usize;
            loop {
                let end = start
                    .checked_add(capacity)
                    .filter(|end| *end <= self.elements)
                    .ok_or(AllocError)?;
                let overlap = runs
                    .iter()
                    .filter_map(|state| state.span())
                    .find(|root| start < root.start + root.capacity && root.start < end);
                match overlap {
                    Some(root) => start = root.start + root.capacity,
                    None => break,
                }
            }
            (
                index,
                Root {
                    start,
                    length: 0,
                    capacity,
                },
            )
        };
        let pointer = self.pointer(root.start);
        runs[index] = State::Live(root);
        self.runs.set(runs);
        // Count every unavailable span, including unopened and sealed roots.
        // This point precedes default grow/shrink retiring the old allocation.
        #[cfg(test)]
        self.peak_elements.set(
            self.peak_elements
                .get()
                .max(self.elements - self.reusable_elements()),
        );
        Ok(NonNull::slice_from_raw_parts(pointer, layout.size()))
    }

    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        if self.finished.get() || !self.supported(layout) {
            invalid();
        }
        let mut runs = self.runs.get();
        let index = runs
            .iter()
            .position(|state| {
                matches!(state, State::Live(root) if root.capacity * size_of::<T>() == layout.size()
                && self.pointer(root.start) == pointer)
            })
            .unwrap_or_else(|| invalid());
        runs[index] = State::Free;
        self.runs.set(runs);
    }
}

#[cfg(test)]
mod peak_tests;
#[cfg(test)]
mod tests;
