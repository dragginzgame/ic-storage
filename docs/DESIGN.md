# Named stable storage

Selected 25 November 2026: retain the named allocation architecture and ship two
owned collection engines, a B-tree and a Cell, in the `ic-storage` library.

Applications declare canonical `StableKey` names under host-issued namespace
grants. Bootstrap validates the declarations and durably binds names to opaque
u64 allocation IDs. Defaults admit 4,096 allocations; names are at most 128 bytes.
The host opens an exclusive, non-cloneable `RuntimeMemory` lease for each store.
The lease implements the Storage `Memory` contract, translating logical offsets
through the allocation's disjoint stable-memory extents.

The map uses the independently implemented 0.67.0 degree-six B-tree with
bottom-up splits, underflow repair, inline values and separate large-value
records. Node allocations target 1 KiB when either codec is unbounded. When
both codecs are bounded, the target is three quarters of the maximum encoded
node size, capped at 1 KiB of initial reservation. The allocator rounds up to
a power-of-two block of at least 256 bytes and grows to fit the actual frame.
Each map can override its node target and allocation unit at creation. Its
allocator doubles the unit for successive block classes and uses the same unit
for splitting and reuse. Every checksummed root starts its payload with the node target
and allocation unit. Readers validate both before admitting any pointers.
External value records are sized independently. The `ICPMAP001` header records codec bounds, root, length and free
lists. Bulk deletion stages at most 32 requested keys at a time, shares writes
to affected nodes and publishes the root after the node writes. Cell uses the
0.74.1 owned `STORCELL` envelope and caches its decoded value under the lease.
Applications implement Storage's `Storable` and own their schemas.

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

The V1 directory refuses the former ICROOTS2 / ICDIRS02 format. It uses `ICPROOT1` / `ICPDIR01` and a bounded binary layout owned by Storage.
It rejects 0.77.0 directories (`ICROOTS1` / `ICDIRS01`) before writing. It deliberately refuses earlier directories, including 0.64's
`ICROOTR1` / `ICDIRR01`. The 0.79 map and
Cell payloads are refused. Collection creation requires fresh allocations.

Source provenance and original grants are recorded in [SOURCES.md](SOURCES.md).
Only `ic-storage` is publishable. Qualification is an independent private workspace.
