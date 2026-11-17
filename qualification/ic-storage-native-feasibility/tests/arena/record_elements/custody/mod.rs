//! V3-only reconstruction from original live allocation authority.
//! Descriptors are transient test metadata, not a persisted or cross-instance ABI.

use super::{Arena, V3};
use crate::arena::{Block, Refusal, WORD_BYTES, invalid};
use ic_storage::record::Field;
use std::alloc::Layout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Contract {
    schema: [u8; 8],
    version: u32,
    size: usize,
    align: usize,
    fields: [Field; 2],
}

impl Contract {
    const V3: Self = Self {
        schema: *V3::STORE_ID,
        version: V3::STORE_VERSION,
        size: V3::STORE_SIZE,
        align: V3::STORE_ALIGN,
        fields: V3::STORE_FIELDS,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Descriptor {
    contract: Contract,
    start: usize,
    generation: u64,
    length: usize,
    capacity: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Schema,
    Root,
    Ledger(Refusal),
}

// A private, non-Clone witness of initialized V3 elements and exclusive ownership.
// It keeps the original allocation domain borrowed; descriptors cannot create it.
// Dropping/forgetting/refusing the token leaks its run rather than guessing cleanup.
#[must_use = "reopen the record vector or leave its allocation unavailable"]
struct Parked<'a> {
    arena: &'a Arena,
    descriptor: Descriptor,
    allocation: Option<Block>,
}

impl<'a> Parked<'a> {
    fn new(values: Vec<V3, &'a Arena>) -> Self {
        let (pointer, length, capacity, arena) = values.into_raw_parts_with_allocator();
        let mut descriptor = Descriptor {
            contract: Contract::V3,
            start: 0,
            generation: 0,
            length,
            capacity,
        };
        let allocation = if capacity == 0 {
            None
        } else {
            let layout = Layout::array::<V3>(capacity).unwrap_or_else(|_| invalid());
            let start = pointer
                .addr()
                .checked_sub(arena.base().addr())
                .unwrap_or_else(|| invalid());
            let block = arena
                .ledger
                .get()
                .blocks
                .into_iter()
                .flatten()
                .find(|block| {
                    block.start * WORD_BYTES == start
                        && block.words * WORD_BYTES == layout.size()
                        && block.parked_len.is_none()
                })
                .unwrap_or_else(|| invalid());
            descriptor.start = block.start;
            descriptor.generation = block.generation;
            Some(block)
        };
        // The existing ledger's parked_len describes u64 initialization only.
        // Do not set it for records: their sole initialization witness is this
        // typed token. The live run remains allocated and unavailable for reuse.
        Self {
            arena,
            descriptor,
            allocation,
        }
    }

    fn descriptor(&self) -> Descriptor {
        self.descriptor
    }

    fn reopen(self, descriptor: Descriptor) -> Result<Vec<V3, &'a Arena>, Error> {
        if descriptor.contract != Contract::V3 {
            return Err(Error::Schema);
        }
        if descriptor != self.descriptor {
            return Err(Error::Root);
        }
        if self.arena.suspended.get() {
            return Err(Error::Ledger(Refusal::Suspended));
        }
        let ledger = self.arena.ledger.get();
        ledger.validate().map_err(Error::Ledger)?;
        if ledger.occupied[self.arena.limit..].iter().any(|used| *used) {
            return Err(Error::Ledger(Refusal::MalformedLedger));
        }
        let Some(block) = self.allocation else {
            return Ok(Vec::new_in(self.arena));
        };
        // Compare the full original block, including its generation and absence
        // of a u64 parking claim. Consistent metadata alone grants no authority.
        if !ledger.blocks.contains(&Some(block)) {
            return Err(Error::Ledger(Refusal::UnknownAllocation));
        }
        // SAFETY: this consumed token came only from Vec<V3, &Arena>'s public
        // raw-parts API. Its unchanged length/capacity witness the initialized V3
        // prefix and exact allocation layout; no old Vec or borrow survived.
        // The original arena remains live and exclusively owns its fixed Box.
        // Full ledger admission and exact original-block matching prove this
        // bounded span remains allocated and was neither freed nor reassigned.
        // base() derives from that same live allocation, not an integer address.
        // V3 has only admitted scalar fields, no owners/Drop; padding is not read.
        // No persisted descriptor or new executable can manufacture this token.
        #[expect(
            unsafe_code,
            reason = "V3-only reconstruction with original live allocation witness"
        )]
        let values = unsafe {
            let pointer = self.arena.base().add(block.start * WORD_BYTES).cast::<V3>();
            Vec::from_raw_parts_in(pointer, descriptor.length, descriptor.capacity, self.arena)
        };
        Ok(values)
    }
}

#[cfg(test)]
mod tests;
