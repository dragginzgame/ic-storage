# Implementation ownership

Storage owns named allocation governance, exclusive leases and all supported
collections in one crate. The private directory commits permanent key-to-ID
bindings and disjoint extents. The public runtime performs host admission before
issuing collection handles. There is no separate Memory package or upstream
stable-structures dependency.

| Capability | Owner / interface |
| --- | --- |
| Durable names and declarations | `StableKey`, `MemoryRequest`, `SealedDeclarationSnapshot` |
| Host namespaces and admission | `AllocationGrants`, `MemoryAuthority`, `RuntimeBootstrapPolicy` |
| Backing custody and leases | `MemoryBacking`, `MemoryRuntime`, `RuntimeMemory` |
| Resource ceilings and diagnostics | `StorageConfig`, `MemoryAllocation` |
| Typed collections | `ic_storage::adapter`: Cell, Map, Set, Vector and map regions |
| Primitive codecs and backing IO | Storage's owned `Storable` and `Memory` traits |
| Wasm system calls | IC Platform stable-memory API |
| Bounded record encoding | IC Wire; Storage owns format framing and invariants |
| Metrics/logging semantics | External IC Metrics and IC Logging |
| Stable adapter records | Metrics Storage and Logging Storage over governed Storage maps |
| Lifecycle, schemas, clocks, cadence and reset decisions | Integrating hosts |

One runtime owns each backing until its final lease drops. Construction, initial
bootstrap, new binding commitment and explicit diagnostics validate the directory.
Lease opening, growth and ordinary IO trust the admitted owner and exclusive
custody. Public passive diagnostics cannot grant
open authority. See [safety](../../SAFETY.md), [allocation](../../crates/ic-storage/ALLOCATION.md)
and [the composed host](../../crates/ic-storage/examples/composed_host.rs).

The owned engine replaces all retained manager and collection formats by hard
cut. Current formats cannot read old images. Both stable adapters require fresh
backing even where their application-level record schema is unchanged.

The root [catalog](../../Cargo.toml) owns the three packages and shared dependencies.
[Release policy](../../ci/cargo-release-groups.json) assigns Storage to `storage`
and each adapter to its independent group. The independent installed runner has
its own native-only Platform/PocketIC graph. Shared Tooling, IC Build, Host and
Testkit retain their existing tooling boundaries; see
[the adoption guide](../adoption/storage-tooling.md).

## Release tooling ownership

The release adapter already delegates manifest rendering and lock projection to
IC Build, durable receipt/archive writes to Host FS, digests to Host Artifacts,
and bounded capture to Host Process. Saved intent, package ordering, source-bound
receipts and retry admission remain Storage policy. Removing those checks
would remove consumer proofs rather than eliminate a duplicate implementation.

IC Build’s `qualify_package_archives` also owns packaging and runs all-target
and doctest suites. It cannot replace the current extracted-package checks and
native/MSRV/Wasm consumer runner without changing that qualification roster.
A future adoption needs a composable archive-admission boundary that retains
source/archive custody while leaving consumer commands explicit. Keep the
current gate until that boundary is available and qualified.
