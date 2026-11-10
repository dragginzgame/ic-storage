# Named bucket allocation

The named allocation, admission and exclusive-lease interfaces remain, while
STORNAM1 version 1 replaces all preceding backing images. The maintainer authorized
fresh/reset backing on 2026-11-10. The canonical host/geometry contract is
[ALLOCATION.md](../../crates/ic-storage/ALLOCATION.md).

## Ownership and implementation

[Persistence](../../crates/ic-storage/src/runtime/directory_model/persistence.rs)
was independently implemented from a restricted behavioral handoff. The
[implementation record](backing-replacement.md) identifies inputs and qualification.
There is one production backing implementation, no public memory-manager
constructor and no compatibility decoder. The separate Memory contract was also
reimplemented through its [restricted handoff](memory-replacement.md).

Names map to opaque u64 identities through creation-sized controls. Storage keeps
custody, namespace admission, resource ceilings, full-range bounds, structural
admission and shared native poisoning. Explicit row encodings avoid packed struct
IO. Each affected row is persisted directly; no snapshot arenas, candidate image
cloning, full Wire encoding or recovery search remains. Names are owned once,
with a sorted index; data IO indexes admitted physical units directly.

## Admission and publication

Open validates the root before using saved capacities or allocating row vectors.
Active counts are bounded by host budgets; calculated table and bucket extents
must equal physical backing size. Names are checked for grammar and uniqueness.
Each physical bucket has exactly one owner; reconstructing bucket vectors in
physical order preserves logical order. Logical sizes must require exactly the
recorded number of buckets. Root and row checksums detect accidental corruption,
not malicious rewrites with recomputed checksums.

Explicit verification and diagnostics revalidate saved controls against admitted
heap state. Lease opening uses the admitted state without backing IO. Ordinary data IO and growth trust exclusive custody;
raw aliased writes remain forbidden. Growth preflights overflow, bucket capacity
and physical capacity, then requests physical growth before any control writes.
Refusal leaves existing bytes and logical sizes unchanged. On native hosts the root is marked
interrupted before owner/size mutations; its final clean publication precedes
heap publication. No second transaction log is introduced. Wasm omits the dirty-root write because IC traps roll back
execution; native interruption is fail-closed and may require explicit host
retirement. No abandoned-tail recovery or old-image reader remains.

## Tradeoffs and qualification

Eight-page buckets trade physical reservation for fewer allocation events.
Creation-sized controls trade up-front reserved capacity for direct updates.
Both are real costs, recorded separately from logical payload and Wasm module
size. Reopening reads only active rows, not reserved empty slots. Table capacity
cannot increase within an image. Default counts reserve 18 control pages and
permit at most 8 GiB of bucket data; hosts can choose larger creation ceilings.

Tests cover more than 255 identities, omission/rebinding, cross-bucket IO,
logical slack exclusion, zero-filled growth, physical refusal, read-only reopen,
control corruption, native partial publication, detached custody and poisoning.
Installed IO/upgrade/rollback qualification and matched ICSS benchmarks are
required before claiming the replacement qualified. Source tests alone do not
establish performance, a production upgrade migration or release readiness.
