// Derived from DFINITY ic-stable-structures 0.7.2 (Apache-2.0). See SOURCES.md.
use crate::map_engine::{
    Memory,
    types::{Address, Bytes, NULL},
};

const ALLOCATOR_LAYOUT_VERSION: u8 = 1;
const CHUNK_LAYOUT_VERSION: u8 = 1;

const ALLOCATOR_MAGIC: &[u8; 3] = b"BTA"; // btree allocator
const CHUNK_MAGIC: &[u8; 3] = b"CHK"; // btree allocator

/// A free list constant-size chunk allocator.
///
/// The allocator allocates chunks of size `allocation_size` from the given `memory`.
///
/// # Properties
///
/// * The allocator tries to minimize its memory footprint, growing the memory in
///   size only when all the available memory is allocated.
///
/// * The allocator makes no assumptions on the size of the memory and will
///   continue growing so long as the provided `memory` allows it.
///
/// The allocator divides the memory into "chunks" of equal size. Each chunk contains:
///     a) A `ChunkHeader` with metadata about the chunk.
///     b) A blob of length `allocation_size` that can be used freely by the user.
///
/// # Assumptions:
///
/// * The given memory is not being used by any other data structure.
pub struct Allocator<M: Memory> {
    // The address in memory where the `AllocatorHeader` is stored.
    header_addr: Address,

    // The size of the chunk to allocate in bytes.
    allocation_size: Bytes,

    // The number of chunks currently allocated.
    num_allocated_chunks: u64,

    // A linked list of unallocated chunks.
    free_list_head: Address,

    memory: M,
}

#[repr(C, packed)]
#[derive(PartialEq, Debug)]
struct AllocatorHeader {
    magic: [u8; 3],
    version: u8,
    // Empty space to memory-align the following fields.
    _alignment: [u8; 4],
    allocation_size: Bytes,
    num_allocated_chunks: u64,
    free_list_head: Address,
    // Additional space reserved to add new fields without breaking backward-compatibility.
    _buffer: [u8; 16],
}

impl AllocatorHeader {
    fn size() -> Bytes {
        Bytes::from(core::mem::size_of::<Self>() as u64)
    }
}

impl<M: Memory> Allocator<M> {
    /// Initialize an allocator and store it in address `addr`.
    ///
    /// The allocator assumes that all memory from `addr` onwards is free.
    ///
    /// When initialized, the allocator has the following memory layout:
    ///
    ///    ```text
    /// [   AllocatorHeader       | ChunkHeader ]
    ///      ..   free_list_head  ↑      next
    ///                |__________|       |____ NULL
    ///    ```
    ///
    pub fn new(memory: M, addr: Address, allocation_size: Bytes) -> Self {
        let mut allocator = Self {
            header_addr: addr,
            allocation_size,
            num_allocated_chunks: 0,
            free_list_head: NULL, // Will be set in `allocator.clear()` below.
            memory,
        };

        allocator.clear();
        allocator
    }

    /// Deallocate all allocated chunks.
    pub fn clear(&mut self) {
        // Create the initial memory chunk and save it directly after the allocator's header.
        self.free_list_head = self.header_addr + AllocatorHeader::size();
        let chunk = ChunkHeader::null();
        chunk.save(self.free_list_head, &self.memory);

        self.num_allocated_chunks = 0;

        self.save()
    }

    /// Load an allocator from memory at the given `addr`.
    pub fn load(memory: M, addr: Address) -> Self {
        let header = AllocatorHeader::read(&memory, addr);
        assert_eq!(&header.magic, ALLOCATOR_MAGIC, "Bad magic.");
        assert_eq!(
            header.version, ALLOCATOR_LAYOUT_VERSION,
            "Unsupported version."
        );

        Self {
            header_addr: addr,
            allocation_size: header.allocation_size,
            num_allocated_chunks: header.num_allocated_chunks,
            free_list_head: header.free_list_head,
            memory,
        }
    }

    /// Allocates a new chunk from memory with size `allocation_size`.
    ///
    /// Internally, there are two cases:
    ///
    /// 1) The list of free chunks (`free_list_head`) has only one element.
    ///    This case happens when we initialize a new allocator, or when
    ///    all of the previously allocated chunks are still in use.
    ///
    ///    Example memory layout:
    ///
    ///    ```text
    ///    [   AllocatorHeader       | ChunkHeader ]
    ///         ..   free_list_head  ↑      next
    ///                   |__________↑       |____ NULL
    ///    ```
    ///
    ///    In this case, the chunk in the free list is allocated to the user
    ///    and a new `ChunkHeader` is appended to the allocator's memory,
    ///    growing the memory if necessary.
    ///
    ///    ```text
    ///    [   AllocatorHeader       | ChunkHeader | ... | ChunkHeader2 ]
    ///         ..   free_list_head      (allocated)     ↑      next
    ///                   |______________________________↑       |____ NULL
    ///    ```
    ///
    /// 2) The list of free chunks (`free_list_head`) has more than one element.
    ///
    ///    Example memory layout:
    ///
    ///    ```text
    ///    [   AllocatorHeader       | ChunkHeader1 | ... | ChunkHeader2 ]
    ///         ..   free_list_head  ↑       next         ↑       next
    ///                   |__________↑        |___________↑         |____ NULL
    ///    ```
    ///
    ///    In this case, the first chunk in the free list is allocated to the
    ///    user, and the head of the list is updated to point to the next free
    ///    block.
    ///
    ///    ```text
    ///    [   AllocatorHeader       | ChunkHeader1 | ... | ChunkHeader2 ]
    ///         ..   free_list_head      (allocated)      ↑       next
    ///                   |_______________________________↑         |____ NULL
    ///    ```
    ///
    pub fn allocate(&mut self) -> Address {
        // Get the next available chunk.
        let chunk_addr = self.free_list_head;
        let mut chunk = ChunkHeader::load(chunk_addr, &self.memory);

        // The available chunk must not be allocated.
        assert!(
            !chunk.allocated,
            "Attempting to allocate an already allocated chunk."
        );

        // Allocate the chunk.
        chunk.allocated = true;
        chunk.save(chunk_addr, &self.memory);

        // Update the head of the free list.
        if chunk.next != NULL {
            // The next chunk becomes the new head of the list.
            self.free_list_head = chunk.next;
        } else {
            // There is no next chunk. Shift everything by chunk size.
            self.free_list_head += self.chunk_size();

            // Write new chunk to that location.
            ChunkHeader::null().save(self.free_list_head, &self.memory);
        }

        self.num_allocated_chunks += 1;
        self.save();

        // Return the chunk's address offset by the chunk's header.
        chunk_addr + ChunkHeader::size()
    }

    /// Deallocates a previously allocated chunk.
    pub fn deallocate(&mut self, address: Address) {
        let chunk_addr = address - ChunkHeader::size();

        let mut chunk = ChunkHeader::load(chunk_addr, &self.memory);

        assert!(chunk.allocated);

        chunk.allocated = false;
        chunk.next = self.free_list_head;

        chunk.save(chunk_addr, &self.memory);

        self.free_list_head = chunk_addr;

        self.num_allocated_chunks -= 1;
        self.save();
    }

    /// Saves the allocator to memory.
    pub fn save(&self) {
        let header = AllocatorHeader {
            magic: *ALLOCATOR_MAGIC,
            version: ALLOCATOR_LAYOUT_VERSION,
            _alignment: [0; 4],
            num_allocated_chunks: self.num_allocated_chunks,
            allocation_size: self.allocation_size,
            free_list_head: self.free_list_head,
            _buffer: [0; 16],
        };

        crate::map_engine::write(&self.memory, self.header_addr.get(), &header.encode());
    }

    // The full size of a chunk, which is the size of the header + the `allocation_size` that's
    // available to the user.
    fn chunk_size(&self) -> Bytes {
        self.allocation_size + ChunkHeader::size()
    }

    /// Destroys the allocator and returns the underlying memory.
    #[inline]
    pub fn into_memory(self) -> M {
        self.memory
    }

    /// Returns a reference to the underlying memory.
    #[inline]
    pub fn memory(&self) -> &M {
        &self.memory
    }
}

#[derive(Debug)]
#[repr(C, packed)]
struct ChunkHeader {
    magic: [u8; 3],
    version: u8,
    allocated: bool,
    // Empty space to memory-align the following fields.
    _alignment: [u8; 3],
    next: Address,
}

impl ChunkHeader {
    // Initializes an unallocated chunk that doesn't point to another chunk.
    fn null() -> Self {
        Self {
            magic: *CHUNK_MAGIC,
            version: CHUNK_LAYOUT_VERSION,
            allocated: false,
            _alignment: [0; 3],
            next: NULL,
        }
    }

    fn save<M: Memory>(&self, address: Address, memory: &M) {
        crate::map_engine::write(memory, address.get(), &self.encode());
    }

    fn load<M: Memory>(address: Address, memory: &M) -> Self {
        let header = ChunkHeader::read(memory, address);
        assert_eq!(&header.magic, CHUNK_MAGIC, "Bad magic.");
        assert_eq!(header.version, CHUNK_LAYOUT_VERSION, "Unsupported version.");

        header
    }

    fn size() -> Bytes {
        Bytes::from(core::mem::size_of::<Self>() as u64)
    }
}

impl AllocatorHeader {
    fn read(memory: &impl Memory, address: Address) -> Self {
        let mut bytes = [0; 48];
        memory.read(address.get(), &mut bytes);
        Self {
            magic: bytes[0..3].try_into().unwrap(),
            version: bytes[3],
            _alignment: bytes[4..8].try_into().unwrap(),
            allocation_size: Bytes::from(u64::from_le_bytes(bytes[8..16].try_into().unwrap())),
            num_allocated_chunks: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            free_list_head: Address::from(u64::from_le_bytes(bytes[24..32].try_into().unwrap())),
            _buffer: bytes[32..48].try_into().unwrap(),
        }
    }
    fn encode(&self) -> [u8; 48] {
        let mut bytes = [0; 48];
        bytes[0..3].copy_from_slice(&self.magic);
        bytes[3] = self.version;
        bytes[4..8].copy_from_slice(&self._alignment);
        bytes[8..16].copy_from_slice(&{ self.allocation_size }.get().to_le_bytes());
        bytes[16..24].copy_from_slice(&self.num_allocated_chunks.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.free_list_head.get().to_le_bytes());
        bytes[32..48].copy_from_slice(&self._buffer);
        bytes
    }
}

impl ChunkHeader {
    fn read(memory: &impl Memory, address: Address) -> Self {
        let mut bytes = [0; 16];
        memory.read(address.get(), &mut bytes);
        Self {
            magic: bytes[0..3].try_into().unwrap(),
            version: bytes[3],
            allocated: match bytes[4] {
                0 => false,
                1 => true,
                _ => panic!("invalid chunk allocation flag"),
            },
            _alignment: bytes[5..8].try_into().unwrap(),
            next: Address::from(u64::from_le_bytes(bytes[8..16].try_into().unwrap())),
        }
    }
    fn encode(&self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[0..3].copy_from_slice(&self.magic);
        bytes[3] = self.version;
        bytes[4] = self.allocated as u8;
        bytes[5..8].copy_from_slice(&self._alignment);
        bytes[8..16].copy_from_slice(&self.next.get().to_le_bytes());
        bytes
    }
}
