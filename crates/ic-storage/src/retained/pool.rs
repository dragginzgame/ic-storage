//! Private disjoint-run allocator. Only supplier backing admits initialized
//! Node/u8 spans; no arbitrary T, raw pointers or Vec owners escape the runtime.
use super::{Error, Node, backing::Backing};
use alloc::{rc::Rc, vec::Vec};
use core::{
    alloc::{AllocError, Allocator, Layout},
    cell::Cell,
    marker::PhantomData,
    ptr::NonNull,
};
const RUNS: usize = 64;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct Span {
    pub start: usize,
    pub len: usize,
    pub cap: usize,
}
#[derive(Clone, Copy)]
enum Run {
    Free,
    Reserved(Span),
    Live(Span),
    Sealed(Span),
}
impl Run {
    fn span(self) -> Option<Span> {
        match self {
            Self::Free => None,
            Self::Reserved(s) | Self::Live(s) | Self::Sealed(s) => Some(s),
        }
    }
}
mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Node {}
    impl Sealed for u8 {}
}
/// Closed element set for the runtime's allocator; applications cannot add types.
#[doc(hidden)]
pub trait Element: sealed::Sealed + Copy {
    #[doc(hidden)]
    const NODE: bool;
}
impl Element for Node {
    const NODE: bool = true;
}
impl Element for u8 {
    const NODE: bool = false;
}
/// Opaque allocator identity appearing in scoped view types. Only the runtime
/// constructs this authority; it exposes neither raw backing nor owning maps.
pub struct NativeAllocator<T: Element> {
    pool: Rc<Pool<T>>,
}
impl<T: Element> Clone for NativeAllocator<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}
struct Pool<T: Element> {
    backing: Rc<Backing>,
    runs: Cell<[Run; RUNS]>,
    marker: PhantomData<T>,
}
impl<T: Element> NativeAllocator<T> {
    pub(super) fn claim(backing: Rc<Backing>, roots: &[Span]) -> Result<Self, Error> {
        let count = if T::NODE {
            backing.geometry.nodes
        } else {
            backing.geometry.bytes
        };
        if roots.len() > RUNS {
            return Err(Error::Geometry);
        }
        let mut runs = [Run::Free; RUNS];
        for (i, span) in roots.iter().copied().enumerate() {
            let end = span.start.checked_add(span.cap).ok_or(Error::Geometry)?;
            if span.cap == 0
                || span.len > span.cap
                || end > count
                || roots[..i]
                    .iter()
                    .any(|other| span.start < other.start + other.cap && other.start < end)
            {
                return Err(Error::Geometry);
            }
            runs[i] = Run::Reserved(span);
        }
        Ok(Self {
            pool: Rc::new(Pool {
                backing,
                runs: Cell::new(runs),
                marker: PhantomData,
            }),
        })
    }
    fn capacity(&self) -> usize {
        if T::NODE {
            self.pool.backing.geometry.nodes
        } else {
            self.pool.backing.geometry.bytes
        }
    }
    #[expect(
        unsafe_code,
        reason = "derive a run pointer within its original allocation"
    )]
    fn pointer(&self, start: usize) -> NonNull<T> {
        let base = if T::NODE {
            self.pool.backing.nodes
        } else {
            self.pool.backing.bytes
        };
        // SAFETY: all callers checked a nonempty run inside capacity; the backing
        // uses exact T alignment and outlives this allocator and every client.
        unsafe { base.cast::<T>().add(start) }
    }
    #[expect(
        unsafe_code,
        reason = "single issuance of a validated initialized retained span"
    )]
    pub(super) fn restore(&self, index: usize) -> Result<Vec<T, Self>, Error> {
        let mut runs = self.pool.runs.get();
        let Some(Run::Reserved(span)) = runs.get(index).copied() else {
            return Err(Error::State);
        };
        runs[index] = Run::Live(span);
        self.pool.runs.set(runs);
        // SAFETY: closed directory and runtime custody established initialized
        // scalar T values; disjoint span is issued once. Private Vec ABI is new.
        Ok(unsafe {
            Vec::from_raw_parts_in(
                self.pointer(span.start).as_ptr(),
                span.len,
                span.cap,
                self.clone(),
            )
        })
    }
    pub(super) fn release(&self, values: Vec<T, Self>) -> Result<Option<Span>, Error> {
        if !Rc::ptr_eq(&self.pool, &values.allocator().pool) {
            return Err(Error::State);
        }
        if values.capacity() == 0 {
            return Ok(None);
        }
        let mut runs = self.pool.runs.get();
        let i = runs.iter().position(|run| matches!(run, Run::Live(span) if span.cap == values.capacity() && self.pointer(span.start).as_ptr().cast_const() == values.as_ptr())).ok_or(Error::State)?;
        let Run::Live(mut span) = runs[i] else {
            return Err(Error::State);
        };
        span.len = values.len();
        let (_, _, _, allocator) = values.into_raw_parts_with_allocator();
        drop(allocator);
        runs[i] = Run::Sealed(span);
        self.pool.runs.set(runs);
        Ok(Some(span))
    }
    pub(super) fn settled(&self) -> bool {
        !self
            .pool
            .runs
            .get()
            .iter()
            .any(|run| matches!(run, Run::Live(_) | Run::Reserved(_)))
    }
}
// SAFETY: closed Node/u8 representations, exact aligned disjoint spans, Rc-backed
// allocation lifetime and non-Sync ledger. Freeing never touches another run.
#[expect(
    unsafe_code,
    reason = "private supplier allocator with validated backing provenance"
)]
unsafe impl<T: Element> Allocator for NativeAllocator<T> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.align() != align_of::<T>()
            || layout.size() == 0
            || !layout.size().is_multiple_of(size_of::<T>())
        {
            return Err(AllocError);
        }
        let cap = layout.size() / size_of::<T>();
        let mut runs = self.pool.runs.get();
        let slot = runs
            .iter()
            .position(|run| matches!(run, Run::Free))
            .ok_or(AllocError)?;
        let mut start = 0usize;
        loop {
            let end = start
                .checked_add(cap)
                .filter(|end| *end <= self.capacity())
                .ok_or(AllocError)?;
            if let Some(other) = runs
                .iter()
                .filter_map(|run| run.span())
                .find(|other| start < other.start + other.cap && other.start < end)
            {
                start = other.start + other.cap;
            } else {
                break;
            }
        }
        runs[slot] = Run::Live(Span { start, len: 0, cap });
        self.pool.runs.set(runs);
        Ok(NonNull::slice_from_raw_parts(
            self.pointer(start).cast(),
            layout.size(),
        ))
    }
    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        let mut runs = self.pool.runs.get();
        let index = runs.iter().position(|run| matches!(run, Run::Live(span) if span.cap * size_of::<T>() == layout.size() && self.pointer(span.start).cast::<u8>() == pointer)).expect("allocator custody");
        runs[index] = Run::Free;
        self.pool.runs.set(runs);
    }
}
