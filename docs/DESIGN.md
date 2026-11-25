# Named stable storage

Selected 17 November 2026: restore the merged 0.64 allocation architecture and
use upstream `ic-stable-structures` collections. Orthogonal persistence and custom
collection engines are deferred. One maintained library is `ic-storage`.

Applications declare canonical `StableKey` names under host-issued namespace
grants. Bootstrap validates the declarations and durably binds names to opaque
u64 allocation IDs. Defaults admit 4,096 allocations; names are at most 128 bytes.
The host opens an exclusive, non-cloneable `RuntimeMemory` lease for each store.
The lease implements the upstream `Memory` contract, translating logical offsets
through the allocation's disjoint stable-memory extents.

Collection wrappers consume those leases. Upstream BTreeMap, BTreeSet, Cell, Vec,
MinHeap and Log own their algorithms and bytes; Storage owns names, grants,
backing custody, allocation limits and safe constructor selection. The map engine batches sorted exact-key deletion in a locally patched upstream
0.7.2 tree, preserving its node layout and ordinary rebalancing rules. Applications
use upstream `Storable` and own their record codecs and schema migrations.
The upstream 255-ID MemoryManager is not used; our directory owns allocation.
No retained Rust heap, custom Rust allocator or nightly features are required.

Ordinary updates operate on stable bytes. After a normal heap-replacing upgrade,
the host reconstructs the runtime, repeats declarations/admission and loads each
collection. There is no whole-table serialization in pre-upgrade. Full directory
validation happens at checked allocation boundaries, not on every collection read.
Growth commits directory metadata; 0.64's bounded directory snapshots and lack
of application-extent reclamation remain limitations to measure and improve
within this architecture. We do not rewrite collection engines to address them.

Unknown backing and foreign collections refuse without initialization. Creation
requires an empty lease; reset is an explicit destructive API. Ordinary returned
errors do not roll back prior writes. Native backing is not transactional; IC
traps roll back the complete message, including directory growth and collection
writes. Installed qualification must establish those lifecycle properties.

The replacement directory uses `ICROOTS2` / `ICDIRS02` and a bounded binary layout owned by Storage.
It rejects 0.77.0 directories (`ICROOTS1` / `ICDIRS01`) before writing. It deliberately refuses earlier directories, including 0.64's
`ICROOTR1` / `ICDIRR01`. Upstream collection payloads also differ from the former
owned AVL/B-tree formats. This repository replacement does not migrate or reset
any consumer's deployed data. Such cutovers need explicit data migration.

Source provenance and original grants are recorded in [SOURCES.md](SOURCES.md).
Only `ic-storage` is publishable. Qualification is an independent private workspace.
