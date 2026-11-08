//! Fixture backing-memory IO counters; workloads and failure injection stay local.

use crate::{BackingIdentity, Memory, MemoryBacking, VectorMemory};
use std::{cell::Cell, rc::Rc};

///
/// `MemoryIoCounts`
///
/// Reads and writes count attempts before delegation. Growth counts
/// completed calls, with requested pages counted only when growth succeeds.
/// Size queries do not count as IO. Counters saturate at `u64::MAX` and set a
/// sticky overflow flag on actual overflow. Measurement callers must reject
/// overflowed counts rather than reporting clipped values as exact totals.
///

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MemoryIoCounts {
    /// Read attempts.
    pub reads: u64,
    /// Requested bytes across all read attempts.
    pub read_bytes: u64,
    /// Write attempts.
    pub writes: u64,
    /// Requested bytes across write attempts.
    pub write_bytes: u64,
    /// Completed growth attempts, including zero-page and refused requests.
    pub grows: u64,
    /// Requested pages of successful growth calls.
    pub grown_pages: u64,
    /// At least one counter overflowed since the last reset.
    pub overflowed: bool,
}

#[expect(
    clippy::empty_line_after_doc_comments,
    reason = "Repository rustdoc style separates item docs from attributes."
)]
///
/// `MeteredMemory`
///
/// Shared fixture memory and IO counters. Clones observe the same backing and
/// counters; resetting any clone resets the whole shared observation window.
/// Backing identity delegates to the underlying host memory without IO.
/// It does not provide transactions, failure injection or IC instruction counts.
/// Direct backing access remains subject to the host's exclusive-custody contract.
///

pub struct MeteredMemory<M: Memory = VectorMemory> {
    state: Rc<MemoryState<M>>,
}

struct MemoryState<M> {
    memory: M,
    counts: Cell<MemoryIoCounts>,
}

impl<M: Memory> MeteredMemory<M> {
    /// Wrap the selected backing with zeroed counters.
    #[must_use]
    pub fn new(memory: M) -> Self {
        Self {
            state: Rc::new(MemoryState {
                memory,
                counts: Cell::default(),
            }),
        }
    }

    /// Inspect the backing directly without accounting; useful for fixture checks.
    #[must_use]
    pub fn backing(&self) -> &M {
        &self.state.memory
    }

    /// Clear the counters shared by all clones without changing stored bytes.
    pub fn reset(&self) {
        self.state.counts.set(MemoryIoCounts::default());
    }

    /// Snapshot the shared counters.
    #[must_use]
    pub fn counts(&self) -> MemoryIoCounts {
        self.state.counts.get()
    }

    fn record_read(&self, bytes: usize) {
        let mut counts = self.state.counts.get();
        add_count(&mut counts.reads, 1, &mut counts.overflowed);
        add_count(&mut counts.read_bytes, bytes as u64, &mut counts.overflowed);
        self.state.counts.set(counts);
    }
}

impl<M: Memory> Clone for MeteredMemory<M> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<M: Memory + Default> Default for MeteredMemory<M> {
    fn default() -> Self {
        Self::new(M::default())
    }
}

impl<M: Memory> Memory for MeteredMemory<M> {
    fn size(&self) -> u64 {
        self.state.memory.size()
    }

    fn grow(&self, pages: u64) -> i64 {
        let result = self.state.memory.grow(pages);
        let mut counts = self.state.counts.get();
        add_count(&mut counts.grows, 1, &mut counts.overflowed);
        if result >= 0 {
            add_count(&mut counts.grown_pages, pages, &mut counts.overflowed);
        }
        self.state.counts.set(counts);
        result
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.record_read(dst.len());
        self.state.memory.read(offset, dst);
    }

    fn write(&self, offset: u64, src: &[u8]) {
        let mut counts = self.state.counts.get();
        add_count(&mut counts.writes, 1, &mut counts.overflowed);
        add_count(
            &mut counts.write_bytes,
            src.len() as u64,
            &mut counts.overflowed,
        );
        self.state.counts.set(counts);
        self.state.memory.write(offset, src);
    }
}

impl<M: MemoryBacking> MemoryBacking for MeteredMemory<M> {
    fn backing_identity(&self) -> BackingIdentity {
        self.state.memory.backing_identity()
    }
}

const fn add_count(value: &mut u64, increment: u64, overflowed: &mut bool) {
    let (sum, overflow) = value.overflowing_add(increment);
    *value = if overflow { u64::MAX } else { sum };
    *overflowed |= overflow;
}

#[cfg(test)]
mod tests;
