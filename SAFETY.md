# Safety invariants

Storage owns allocation governance and collections. These invariants protect
allocation and collection integrity; they do not establish application schema
compatibility, controller authorization or data semantics.

## Allocation identity and admission

- A committed stable key retains its opaque allocation ID permanently. IDs and
  extents are never reassigned to another key. Omission does not reclaim storage.
- Only current namespace grants and successful host policy admission permit an
  allocation to open. Passive IDs and diagnostic rows grant no access.
- Directory validation checks unique names and IDs, disjoint nonempty extents,
  metadata separation, generation, checksums and configured resource bounds.
- Old manager, ledger and collection images are retired by an explicit hard cut.
  Invalid or foreign nonempty backing refuses; it is never silently initialized.

## Runtime and lease custody

One runtime claims each backing before reading or writing it. The claim survives
until the runtime and every detached lease are dropped. Each allocation permits
one live non-cloneable lease. Collections consume that lease; region views share
one retained lease and reload their tree root for each operation.

Hosts must provide truthful `MemoryBacking` identities and exclude raw aliases
and independent managers. Cooperating runtimes share the custody registry, but
Rust types cannot detect arbitrary raw accesses or a lying custom backing.
Construction, initial bootstrap, new binding commitment and explicit diagnostics
validate the directory. Lease opening and growth check admitted owner state;
ordinary IO trusts exclusive custody. Detected divergence or a backing panic
poisons all handles sharing the owner.

## Runtime IO bounds

The complete logical span is checked before backing IO. Reads and writes translate
only admitted data extents; directory metadata is never exposed as application
memory. Empty IO is permitted at the end of an allocation, but not beyond it.
Overflow and out-of-range spans fail before partial access. New data extents use
fresh zero-filled backing pages; abandoned native tails are skipped.

## Persistence and failure

Directory changes are prepared under budgets before growth or writes. New name
or ownership rows and the affected allocation row precede the complete protected
root; cached authority is published last. Native publication marks the root
interrupted before changing controls. Wasm relies on IC execution rollback.
Rejected admission or physical growth refusal does not publish a candidate.
New bindings and data growth advance generation; unchanged reopening does not.
Torn publication and unexplained physical tails refuse reopening. A fully
persisted final root remains valid even if the native caller then panics.
There is no snapshot journal, fallback generation or automatic repair.

Collections use independently implemented engines through governed adapters.
Their writes are not native transactions. Mutation/codec/allocation failures
can trap after partial writes. A native mutation unwind poisons the shared owner;
discard the runtime and leases before explicit backing recovery/replacement.
IC message rollback supplies atomicity for trapped updates and upgrades. No
second journal is added. Cached Map lengths and Cell values are not recovery APIs.

Directory checksums remain; collection checksums do not. Collection admission
checks current framing, not every node, free-list link or payload. Valid-looking
corruption may be undetected. Engine and application-codec failures trap instead
of providing typed mutation errors. Application schema and collection framing
admission remain host obligations. Cell loading admits a caller-selected payload
ceiling before allocation; Vector checks finite slot lengths before payload
allocation. Persisted tags and integer fields are validated explicitly; no copied
packed-header or buffer helper remains. Backing access retains runtime bounds
and custody checks.

## Resource limits and host policy

`StorageConfig` bounds allocations, extents, physical pages and Wire decoding.
Physical limits include reserved directory arenas and abandoned tails.
Collections enforce declared codec bounds and their own record limits. Hosts own
workload budgets, checkpoint cadence, retention and explicit fresh/reset installation.
Never clear governance alone while keeping application bytes to reinterpret as
managed storage. See [the allocation contract](crates/ic-storage/ALLOCATION.md)
and [collection limits](crates/ic-storage/README.md#current-storage-contract).
