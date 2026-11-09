# Named bucket allocation

The maintainer authorized replacing snapshot-directory persistence with an
ICSS-derived bucket backing on 2026-11-09. The named allocation, admission and
exclusive-lease interfaces remain; persisted snapshot images are retired.
The canonical host/geometry contract is [ALLOCATION.md](../../crates/ic-storage/ALLOCATION.md).

## Ownership and implementation

[Persistence](../../crates/ic-storage/src/runtime/directory_model/persistence.rs)
adapts published ICSS 0.7.2 `memory_manager.rs` growth and bucket-segment
translation. [Provenance](../storage/icss-source.json) records its source hash.
There is one production backing implementation, no ICSS production dependency,
no public memory-manager constructor and no compatibility decoder.

ICSS's fixed 255-memory domain is replaced by creation-sized tables and opaque
u64 identities assigned to checked names. Storage retains custody, namespace
admission, checked resource ceilings, full-range bounds checks, structural
admission and shared native poisoning. Explicit row encodings avoid packed
struct IO. Each affected row is persisted directly; no snapshot arenas, candidate
image cloning, full Wire encoding or recovery search remains.

## Admission and publication

Open validates the root before using saved capacities or allocating row vectors.
Active counts are bounded by host budgets; calculated table and bucket extents
must equal physical backing size. Names are checked for grammar and uniqueness.
Each physical bucket has exactly one owner; reconstructing bucket vectors in
physical order preserves logical order. Logical sizes must require exactly the
recorded number of buckets. Root and row checksums detect accidental corruption,
not malicious rewrites with recomputed checksums.

Lease opening, explicit verification and diagnostics revalidate saved controls
against admitted heap state. Ordinary data IO and growth trust exclusive custody;
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
cannot increase within an image. Default counts reserve 15 control pages and
permit at most 8 GiB of bucket data; hosts can choose larger creation ceilings.

Tests cover more than 255 identities, omission/rebinding, cross-bucket IO,
logical slack exclusion, zero-filled growth, physical refusal, read-only reopen,
control corruption, native partial publication, detached custody and poisoning.
Installed IO/upgrade/rollback qualification and matched ICSS benchmarks are
required before claiming the replacement qualified. Source tests alone do not
establish performance, a production upgrade migration or release readiness.
