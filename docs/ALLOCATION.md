# Named allocation and host integration

A host constructs one `MemoryRuntime` over an identified `MemoryBacking`.
`Memory` is the upstream four-method backing interface: `size`, `grow`, `read`
and `write`. Growth appends zeroed Wasm pages or returns -1 without mutation.
`VectorMemory` is the native shared-byte backing. On Wasm, `DefaultMemoryImpl`
uses IC Platform's owned stable-memory calls. Collection engines come directly from `ic-stable-structures`; its fixed-ID
memory manager is not used.

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
and trusts custody. Complete metadata checks occur at construction, bootstrap,
lease opening and growth. A backing panic or failed checked-boundary validation
poisons the shared owner.

## Limits and persistence

`StorageConfig` selects allocation count, extent count, physical page and snapshot
byte limits. Defaults are 4,096 allocations, 16,384 extents, 1,048,576 physical
pages, and 4 MiB per directory snapshot.
The u64 identity width is not a promise of unbounded allocation count. Reopening
revalidates the directory under the supplied limits.

The first page contains the protected `ICROOTS2` root. Each directory commit
writes a bounded `ICDIRS02` binary snapshot into an inactive reserved arena,
then the root, and only then publishes its in-memory state. The active arena is
never overwritten during preparation/publication. New arenas grow geometrically
only when no inactive arena can hold the snapshot; all arenas remain excluded
from application extents. Growth refusal
preserves the committed state. Recovery never guesses a fallback root or resets
foreign/corrupt backing. Native partial writes can prevent reopening; Wasm traps
use the host's rollback contract. Checksums are for accidental corruption.

After the eight-byte directory magic, the layout contains generation, next ID,
next page, a counted metadata extent list and a counted entry list. Each entry
contains its ID, a length-prefixed canonical ASCII name and a counted extent
list. Each extent stores its start and page count. All numbers, including
counts and byte lengths, are unsigned 64-bit little-endian integers.

Physical extents may be discontiguous; reads and writes translate logical pages
through the allocation's extent list. Failed native commits can leave an
abandoned tail, which subsequent allocations skip so stale metadata is never
exposed as zeroed application pages. Inactive snapshot arenas are reused even after reopen; old inactive bytes are
never treated as a fallback generation. Adjacent growth of the same allocation
coalesces extents. Application extents and abandoned tails are not reclaimed,
and full snapshots are still encoded/written per directory mutation. Physical
page budgets include all reserved arenas and abandoned tails.

This manager is a storage-format and public-API hard cut. Use fresh/reset whole
backing, rebuild hosts against `StorageConfig`, `AllocationGrants` and
`allocation_id`, and update application admission for the upstream collection
formats. No legacy manager/ledger decoder remains. Publication and downstream
adoption are separate actions.
