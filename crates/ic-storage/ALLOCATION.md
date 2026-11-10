# Named allocation and host integration

A host constructs one `MemoryRuntime` over an identified `MemoryBacking`.
`Memory` is Storage's owned four-method backing interface: `size`, `grow`, `read`
and `write`. Growth appends zeroed Wasm pages or returns -1 without mutation.
`VectorMemory` is the native shared-byte backing. On Wasm, `DefaultMemoryImpl`
uses IC Platform's owned stable-memory calls. No collection library or
memory manager is imported through Platform.

`MemoryBacking` must identify all physical aliases consistently. Storage rejects
another runtime before backing IO while any owner or detached lease retains its
claim. Custom wrappers delegate to the underlying identity. Raw aliased backing
access remains prohibited outside explicit infrastructure/fault fixtures.

## Admission

Components contribute `MemoryRequest` values with an authority label and a
checked `StableKey`. Hosts compose them into a `SealedDeclarationSnapshot` and
supply non-overlapping `AllocationGrants` made from `MemoryAuthority` namespace
prefixes. There are no numeric pools, reserved numeric slots or caller-selected
IDs. Internal IDs are opaque u64 values assigned once and never reused.

Bootstrap validates grants, runs `RuntimeBootstrapPolicy::prepare_bootstrap`,
checks each selected key, commits new bindings, then publishes current admission.
A policy can inspect retained identities and `include_historical` under current
grants. An ignored historical-selection error still rejects the whole attempt.
Omitted names retain their IDs and pages but cannot be opened until admitted by
a later host composition. The host owns application schema/migration decisions.

Warm bootstrap and `verify_bootstrap` require identical declarations, grants and
policy identity. `verify_authority` checks a component's subset without IO or
replaying policy. A cold bootstrap that adds no names performs no directory
commit. Directory generations advance for binding and extent mutations, not
merely because a host reopened the backing.

`open_memory` grants one exclusive, non-cloneable `RuntimeMemory` per admitted
key. Collections consume the lease. A lease may outlive its runtime; custody
lasts until the final owner/handle is dropped. Data IO validates logical bounds
and trusts custody. Opening checks current admission, owner health and exclusive
lease ownership without backing IO. Growth likewise trusts the admitted owner.
Complete metadata checks remain at construction, initial bootstrap, new binding
commitment and explicit `diagnostics()`/`memory_allocations()` calls. A backing
panic or failed integrity verification poisons the shared owner.

Opening a handle no longer detects raw metadata changes made outside its owner
after admission. Such writes remain prohibited by `MemoryBacking`; custody is
not isolation against arbitrary linked code or a dishonest backing. Hosts that
need an explicit integrity observation must call diagnostics. Cold reopen still
validates the entire retained directory. No automatic scan is added per call,
callback or handle, and no alternate checked/unchecked opening mode is provided.

## Limits and persistence

`MemoryRuntime::diagnostics()` verifies current backing and reports generation,
retained/admitted allocations, live leases, data capacity, directory reservations,
physical capacity and unassigned pages without writing or opening a lease.
It is available before bootstrap, with zero admitted names. The protected root
uses one page; root, data, metadata and unassigned pages sum to physical pages.
These totals are capacity rather than payload occupancy. Unassigned pages are
not automatically reusable. Corrupt/poisoned owners return an error rather than
an apparently healthy cached report. Hosts own namespace/authority presentation
and external diagnostic protocols; no retired manager recovery image is decoded.

`StorageConfig` selects allocation count, bucket count (`extents`) and physical
page ceilings. Defaults remain 4,096 allocations, 16,384 buckets and 1,048,576
physical pages. Each bucket contains eight 64-KiB Wasm pages. The bucket ceiling
therefore limits default data reservation to 8 GiB; the page ceiling also counts
controls. Counts are capacities, not a promise of live business payload size.
The obsolete `wire` snapshot budget is removed.

The image reserves fixed-width control tables from the creation-time count
ceilings: a 64-byte root, 160 bytes per possible named allocation, and 16 bytes
per possible bucket, rounded up to full pages. Default control reservation is
15 pages (960 KiB), including the root page. Only active rows are read/written;
unused capacity is not initialized by explicit writes. Reopening can impose
smaller live-usage limits but cannot enlarge the saved table capacities. Select
larger ceilings when creating a backing if needed; no numeric IDs are exposed.

The `ICBUCK01` root records table capacities, live counts and generation.
Allocation rows contain names and logical page sizes. Physical bucket-owner rows
reconstruct each allocation's ordered bucket vector on open. The root and rows
have accidental-corruption checksums. The physical bucket addresses are derived
from the tables' end and bucket index, as in ICSS's manager. Growth writes only
new ownership rows, the affected size field and the root; it never serializes
a directory snapshot or rereads all metadata. Binding writes only new name rows.

Native publication marks the root interrupted before changing controls and writes
a complete clean root last. Wasm omits the interruption write and relies on IC
execution rollback. The in-memory state is published only after successful
writes. Physical growth refusal happens before any metadata write and remains
retryable. A native backing panic poisons all leases; a torn publication or
unexplained physical tail is rejected on reopen. There is no native rollback or
fallback to an older snapshot. A fully persisted final root can reopen after a
native panic. On IC, execution rollback restores both memory and controls.

Logical growth within a bucket updates size without growing physical memory.
Bounds checks exclude unused bucket slack, including through the translation
cache. Reads/writes split at bucket boundaries, coalescing adjacent requested physical
buckets into one IO call without scanning unrelated buckets. Buckets are never reclaimed or
assigned to another allocation. Diagnostics count slack as unassigned capacity;
it is reserved to its owner and becomes usable through later logical growth.

This is a fresh-backing and public-configuration hard cut. Existing snapshot
images are rejected without rewriting them. Hosts must explicitly reset/retire
whole backing or use fresh backing, and remove `StorageConfig::wire` overrides.
Collection images and successful collection APIs are otherwise unchanged, but
old whole-store images are not attachable. Publication and sibling adoption are
separate actions and have not been performed.
