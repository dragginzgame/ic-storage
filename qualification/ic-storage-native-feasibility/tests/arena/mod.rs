//! Private, u64-only live allocator experiment. See allocation-boundary.md.

use std::{
    alloc::{AllocError, Allocator, Layout},
    cell::{Cell, UnsafeCell},
    mem::{MaybeUninit, align_of, size_of},
    ptr::NonNull,
};

const WORDS: usize = 64;
const WORD_BYTES: usize = size_of::<u64>();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Block {
    start: usize,
    words: usize,
    generation: u64,
    parked_len: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ledger {
    occupied: [bool; WORDS],
    blocks: [Option<Block>; WORDS],
    next_generation: u64,
}

impl Ledger {
    // Structural admission only: the original live arena and unique ownership
    // token still supply custody and initialization evidence. Consistent forged
    // metadata cannot establish those facts or authorize an IC reopen.
    fn validate(&self) -> Result<(), Refusal> {
        if self.next_generation == 0 {
            return Err(Refusal::MalformedLedger);
        }
        let mut occupied = [false; WORDS];
        for (index, entry) in self.blocks.iter().enumerate() {
            let Some(block) = entry else { continue };
            let end = block
                .start
                .checked_add(block.words)
                .filter(|end| *end <= WORDS)
                .ok_or(Refusal::MalformedLedger)?;
            if block.words == 0
                || block.generation == 0
                || block.generation >= self.next_generation
                || block.parked_len.is_some_and(|length| length > block.words)
                || self.blocks[..index]
                    .iter()
                    .flatten()
                    .any(|other| other.generation == block.generation)
            {
                return Err(Refusal::MalformedLedger);
            }
            let run = &mut occupied[block.start..end];
            if run.iter().any(|used| *used) {
                return Err(Refusal::MalformedLedger);
            }
            run.fill(true);
        }
        if occupied != self.occupied {
            return Err(Refusal::MalformedLedger);
        }
        Ok(())
    }
}

struct Arena {
    backing: Box<UnsafeCell<[MaybeUninit<u64>; WORDS]>>,
    ledger: Cell<Ledger>,
}

impl Arena {
    fn new() -> Self {
        Self {
            backing: Box::new(UnsafeCell::new([MaybeUninit::uninit(); WORDS])),
            ledger: Cell::new(Ledger {
                occupied: [false; WORDS],
                blocks: [None; WORDS],
                next_generation: 1,
            }),
        }
    }

    fn base(&self) -> *mut u8 {
        // get() retains the original allocation's provenance; no buffer-wide
        // reference is made. UnsafeCell permits mutation behind this &Arena.
        self.backing.get().cast::<u8>()
    }

    fn reusable_bytes(&self) -> usize {
        self.ledger
            .get()
            .occupied
            .iter()
            .filter(|used| !**used)
            .count()
            * WORD_BYTES
    }

    fn park(values: Vec<u64, &Self>) -> Parked<'_> {
        let (pointer, length, capacity, arena) = values.into_raw_parts_with_allocator();
        if capacity == 0 {
            return Parked {
                arena,
                descriptor: None,
            };
        }
        let mut ledger = arena.ledger.get();
        let start_bytes = pointer
            .addr()
            .checked_sub(arena.base().addr())
            .unwrap_or_else(|| invalid());
        let index = ledger
            .blocks
            .iter()
            .position(|entry| {
                entry.is_some_and(|block| {
                    block.start * WORD_BYTES == start_bytes
                        && block.words == capacity
                        && block.parked_len.is_none()
                })
            })
            .unwrap_or_else(|| invalid());
        let mut block = ledger.blocks[index].unwrap_or_else(|| invalid());
        if length > capacity {
            invalid();
        }
        block.parked_len = Some(length);
        ledger.blocks[index] = Some(block);
        arena.ledger.set(ledger);
        Parked {
            arena,
            descriptor: Some(Descriptor {
                start: block.start,
                capacity,
                length,
                generation: block.generation,
            }),
        }
    }
}

// SAFETY: one stable Box allocation, exact-sized aligned disjoint runs, and
// non-Sync metadata. Shared allocator operations never move/free backing or
// touch live payloads. Borrowing &Arena keeps the domain live. No method uses
// fallible metadata allocation, callbacks or unwinding. See the boundary note.
#[expect(
    unsafe_code,
    reason = "private live-arena Allocator boundary with per-operation proofs"
)]
unsafe impl Allocator for Arena {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.align() != align_of::<u64>()
            || layout.size() == 0
            || !layout.size().is_multiple_of(WORD_BYTES)
        {
            return Err(AllocError);
        }
        let words = layout.size() / WORD_BYTES;
        if words > WORDS {
            return Err(AllocError);
        }
        let mut ledger = self.ledger.get();
        let next_generation = ledger.next_generation.checked_add(1).ok_or(AllocError)?;
        let index = ledger
            .blocks
            .iter()
            .position(Option::is_none)
            .ok_or(AllocError)?;
        let start = (0..=WORDS - words)
            .find(|start| {
                ledger.occupied[*start..*start + words]
                    .iter()
                    .all(|used| !used)
            })
            .ok_or(AllocError)?;
        // SAFETY: start is in a bounded nonempty run of this original live Box;
        // the byte offset remains within its single allocation and u64 aligned.
        let pointer = unsafe { self.base().add(start * WORD_BYTES) };
        let pointer = NonNull::new(pointer).ok_or(AllocError)?;
        ledger.occupied[start..start + words].fill(true);
        ledger.blocks[index] = Some(Block {
            start,
            words,
            generation: ledger.next_generation,
            parked_len: None,
        });
        ledger.next_generation = next_generation;
        self.ledger.set(ledger);
        Ok(NonNull::slice_from_raw_parts(pointer, layout.size()))
    }

    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        let mut ledger = self.ledger.get();
        let start_bytes = pointer
            .as_ptr()
            .addr()
            .checked_sub(self.base().addr())
            .unwrap_or_else(|| invalid());
        let index = ledger
            .blocks
            .iter()
            .position(|entry| {
                entry.is_some_and(|block| {
                    block.start * WORD_BYTES == start_bytes
                        && block.words * WORD_BYTES == layout.size()
                })
            })
            .unwrap_or_else(|| invalid());
        if layout.align() != align_of::<u64>() {
            invalid();
        }
        let block = ledger.blocks[index].unwrap_or_else(|| invalid());
        ledger.occupied[block.start..block.start + block.words].fill(false);
        ledger.blocks[index] = None;
        self.ledger.set(ledger);
    }
}

fn invalid() -> ! {
    // Allocator implementations must never unwind, including on internal bugs.
    std::process::abort()
}

#[derive(Clone, Copy)]
struct Descriptor {
    start: usize,
    length: usize,
    capacity: usize,
    generation: u64,
}

#[must_use = "reopen the vector or leave its allocation quarantined"]
struct Parked<'arena> {
    arena: &'arena Arena,
    descriptor: Option<Descriptor>,
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    MalformedLedger,
    UnknownAllocation,
    Extent,
    Initialization,
    NotParked,
}

impl<'arena> Parked<'arena> {
    fn reopen(self) -> Result<Vec<u64, &'arena Arena>, Refusal> {
        let mut ledger = self.arena.ledger.get();
        ledger.validate()?;
        let Some(descriptor) = self.descriptor else {
            return Ok(Vec::new_in(self.arena));
        };
        let index = ledger
            .blocks
            .iter()
            .position(|entry| {
                entry.is_some_and(|block| {
                    block.start == descriptor.start && block.generation == descriptor.generation
                })
            })
            .ok_or(Refusal::UnknownAllocation)?;
        let mut block = ledger.blocks[index].ok_or(Refusal::UnknownAllocation)?;
        if descriptor.capacity != block.words {
            return Err(Refusal::Extent);
        }
        let initialized = block.parked_len.ok_or(Refusal::NotParked)?;
        if descriptor.length != initialized || descriptor.length > descriptor.capacity {
            return Err(Refusal::Initialization);
        }
        // SAFETY: the validated block remains allocated by this same borrowed
        // arena, in its original Box. Pointer arithmetic stays inside the block's
        // domain. The consumed unique token is the only parked owner. No old Vec
        // or element borrow survives parking; exactly length u64s are initialized.
        // Capacity/alignment match the allocator's exact returned layout. Nothing
        // here restores an allocation or pointer across executable instantiation.
        #[expect(
            unsafe_code,
            reason = "same-live-domain reconstruction after ledger admission"
        )]
        let values = unsafe {
            let pointer = self
                .arena
                .base()
                .add(block.start * WORD_BYTES)
                .cast::<u64>();
            Vec::from_raw_parts_in(pointer, descriptor.length, descriptor.capacity, self.arena)
        };
        block.parked_len = None;
        ledger.blocks[index] = Some(block);
        self.arena.ledger.set(ledger);
        Ok(values)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod checkpoint_tests;

#[cfg(test)]
mod ledger_tests;
