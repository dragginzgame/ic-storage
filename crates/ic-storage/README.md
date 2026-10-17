# IC Storage

An independent page-based stable-memory B-tree for Internet Computer storage.
The current implementation provides ordered byte keys, opaque variable-size
values, custom comparators and fallible operations. It does not yet replace
`IcyDB`'s deployed canonical stores.

The library requires Rust 1.91 or later.

```rust
use ic_storage::{BTreeMap, Config, Lexicographic};
use std::ops::Bound::{Included, Excluded};

# fn example(runtime: &ic_memory::MemoryRuntime<ic_memory::DefaultMemoryImpl>) -> Result<(), Box<dyn std::error::Error>> {
// The application has already admitted and committed this allocation.
let memory = runtime.open_memory("app.rows.v1")?;
let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic)?;
tree.insert(b"account:1", b"row bytes")?;
tree.insert(b"account:2", b"another row")?;
for entry in tree.range(Included(b"account:1"), Excluded(b"account:3"))? {
    let entry = entry?;
    // No value payload is read until explicitly requested.
    println!("{:?}: {} bytes", entry.key(), entry.value_len());
    let row = entry.value()?;
    assert!(!row.is_empty());
}
drop(tree);
let tree = BTreeMap::open(runtime.open_memory("app.rows.v1")?, Config::default(), Lexicographic)?;
assert_eq!(tree.len(), 2);
# Ok(())
# }
```

The dependency direction is application → IC Storage → IC Memory. Both infrastructure
crates privately depend on the upstream stable-memory substrate. The B-tree algorithm, page codec, free list, checksums,
mutation staging and cursors are implemented here. Public `BTreeMap`, `Entry`
and `Range` types fix the collection memory to `ic_memory::RuntimeMemory<M>`;
the remaining generic `M` identifies the runtime's backing, not an arbitrary
collection-memory implementation. Allocation authority and IDs belong to IC Memory.

`ic_storage::adapter` also exports our types, upstream `StableBTreeMap`,
`StableBTreeSet`, `StableCell`, `StableVec`, `StableMinHeap`, `StableLog`, their
supported iterator types, `LogWriteError`, `Storable` and `storable`. Upstream
collection wrappers require runtime handles too and preserve their existing formats and
failure contracts. They do not inherit our custom tree's staged-write behavior.
No raw `Memory` trait, backing constructors, memory manager or unrestricted
upstream module is exported. Runtime bootstrap remains the application's IC
Memory infrastructure boundary; ordinary collection code uses this adapter.

Arbitrary backing memory is rejected by both collection surfaces:

```compile_fail
use ic_memory::VectorMemory;
use ic_storage::{BTreeMap, Config, Lexicographic};
let _ = BTreeMap::create(VectorMemory::default(), Config::default(), Lexicographic);
```

```compile_fail
use ic_memory::VectorMemory;
use ic_storage::adapter::StableBTreeMap;
let _ = StableBTreeMap::<u64, u64>::new(VectorMemory::default());
```

Raw substrate access is not an application-facing storage API:

```compile_fail
use ic_storage::adapter::{Memory, MemoryManager, DefaultMemoryImpl};
```

Collection wrappers keep their inner structures private. They implement no
`Deref`, raw-memory accessor or `into_memory` conversion. Drop the collection
before reopening its allocation through IC Memory. Supported upstream iterators
remain borrowed views; their types and codecs are intentionally part of the API.

```compile_fail
fn extract(tree: ic_storage::BTreeMap) {
    let _ = tree.into_memory();
}
```

```compile_fail
fn extract(map: ic_storage::adapter::StableBTreeMap<u64, u64>) {
    let _ = map.into_memory();
}
```

```compile_fail
fn bypass(map: &ic_storage::adapter::StableBTreeMap<u64, u64>) {
    let _: &ic_stable_structures::BTreeMap<u64, u64, ic_memory::RuntimeMemory<ic_memory::DefaultMemoryImpl>> = map;
}
```

IC Memory 0.46 runtime handles are non-cloneable allocation leases. While a
collection owns a lease, another open through that runtime returns
`RuntimeOpenError::AlreadyOpen`. Dropping a collection, including a failed
fallible constructor's handle, releases its lease. A log holds two distinct
leases. Hosts must keep exclusive backing custody: do not construct another
runtime or access aliased raw backing until the runtime and all its leases
have been dropped. The runtime's lease tracking does not coordinate independent
runtime owners over the same backing.

```compile_fail
fn duplicate(memory: ic_memory::RuntimeMemory<ic_memory::DefaultMemoryImpl>) {
    let _ = memory.clone();
}
```

Keys up to 32 bytes and values up to 16 bytes are inline in tree nodes. Larger
payloads use checksummed chains. Nodes have at most 31 entries, with split,
borrow, merge and root-shrink operations. Deleted nodes and payload pages return
to a persisted free list; stable memory's physical capacity does not shrink.
Replacing an existing value preserves tree topology and reuses its overflow
pages. Shrinking frees excess pages; growing allocates only the extra pages
needed. Same-size and shrinking replacements do not require backing growth.
Replacement reuses the located node and collects the returned previous value
during its overflow-chain traversal, avoiding a second search and payload read.
Default limits accept keys up to 128 KiB and values up to 4 MiB. Callers select
and retain exact limits when creating/opening a store. Values are opaque bytes;
application codecs remain responsible for their meaning and validation.

Custom `Comparator` implementations own encoding-aware order. Their 16-byte
identity is persisted and checked on open. Ordering must remain deterministic,
immutable and total; equal keys must have identical encodings. Change the
identity whenever ordering or accepted encoding changes. Identity equality is
not proof that two binaries implement the same comparator.

Ranges accept included, excluded and unbounded endpoints, support reverse and
interleaved traversal, and retain tree paths rather than materializing a full
store. Key-only scans and `contains_key` avoid external value-payload reads;
inline value bytes share the node page. Iterator errors terminate traversal.
`validate()` explicitly audits all live/free pages, checks ordering and counts,
and rejects duplicate ownership, cycles, orphan pages and uneven tree depth.
Opening checks the header/root; it does not run that full scan.

## Persistence boundary

Each tree requires exclusive ownership of one memory region, with one live
handle. Do not mutate that region through another handle or retained memory
clone. Creation requires zero-sized memory and rejects existing data. Opening
requires the exact format, configuration and comparator identity.

Mutations stage touched pages and payloads in heap memory, then reserve backing
capacity before the first write. Returned validation/capacity errors leave the
published tree bytes unchanged. Successful growth can reserve unused capacity.
Once writes begin, the `Memory` implementation must complete them or trap within
an atomic IC message that rolls its writes back. Do not catch a native backing
panic and continue using the tree. This is **not filesystem crash recovery**:
a process crash or partial non-transactional write can leave mixed pages. The
checksums detect accidental corruption; they are not cryptographic authentication.
An application journal, transaction coordinator and multi-store atomicity remain
outside this collection.

This format is new and incompatible with DFINITY stable B-tree bytes. Adoption
must explicitly select fresh memory and resolve retained-data/rebuild obligations
under the application's recovery contract. Never reopen old bytes as this tree,
erase an allocation ledger, or infer that pre-1.0 stored data is disposable.

Native tests cover differential mutations, splits/merges, page reuse, bounded
bidirectional traversal, large rows/keys, semantic ordering, growth rejection
and corruption. Wasm compilation is checked. These checks do not establish IC
message rollback, upgrade recovery, or `IcyDB` integration/performance qualification.

Licensed under MIT; see LICENSE.
