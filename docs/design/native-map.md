# Native keyed collection

The optional `map` feature in `ic-storage` implements the first reusable
keyed collection: u64 keys and u64 values under an explicit allocator. Product
code, portable encoding and ordinary tests live beside the owning library.
The [sustained workload](../../crates/ic-storage/tests/map_firehose.rs)
uses only its public API. This is an independently authored native hash table;
no upstream implementation was copied or adapted.

## Representation and ownership

The retained payload candidate is an array of `repr(C)` Slots, each containing
three initialized u64 words: occupied at offset 0, key at 8 and value at 16.
Its size is 24 bytes and alignment is `align_of::<u64>()`. Occupied is exactly
0 or 1; empty slots contain three zeros. All u64 key/value combinations are
permitted, including zero and u64::MAX. No record field owns another allocation,
borrows memory, invokes application Drop or contains an executable pointer.

The initial supported compiler remains Rust 1.100.0. This contract deliberately
controls the actual retained layout; it does not retain a HashMap or Vec's
private representation. `Map`, `View`, allocator objects and Vec headers are
transient owners and must never be retained as raw Rust structs. Host layout
assertions, wasm32 compilation and the host provider loan test establish their
respective evidence boundaries. The subsequent closed installed profile is
recorded separately below; it uses a bounded provider pool and one scalar schema.

Buckets are a power of two, at least eight, and at most 75% occupied. A fixed
64-bit avalanche mixer and linear probing determine placement. Deletion shifts
entries whose probe paths cross the removed slot, including wrapped chains;
there are no tombstones to accumulate during churn. These rules are part of the
native schema. Changing layout, mixer, probing or interpretation requires
explicit conversion or portable reconstruction. The mixer is deterministic,
not a defense against adversarial collision selection; worst-case operations
are linear and native admission can be quadratic for pathological clusters.

`Map::with` lends a higher-ranked view without mutable carrier/allocator access,
owning iterators or references to stored objects. The runtime keeps the actual
owner, even when the application forgets the view. Ordinary callback errors
preserve earlier writes. An unwind leaves the owner poisoned: reads, scopes,
checkpoint export and native release refuse. Explicit recovery constructs a
complete candidate before replacing either a healthy or poisoned owner.

## Allocation, admission and provider boundary

`Map::new_in` selects the allocator and maximum number of live entries.
Insertion updates an existing key without allocation even at the limit.
New entries grow the bucket array geometrically. Reservation and growth build
a separate candidate, rehash every existing entry and replace only on success.
Expected allocation/limit failures preserve entries and the original backing.
Peak growth memory includes both old and new allocations. The live-entry limit
is not a reservation or byte budget; consumers must account for those peaks.

`Map::rebuild_in(minimum_capacity, allocator)` builds an independent native
candidate in another allocator while leaving the source healthy and unchanged.
It copies the admitted scalar entries and recomputes hash placement directly,
without a logical checkpoint. The candidate retains the source's live-entry
limit. The runtime owns root publication and retiring the old owner; this API
does not itself publish anything. A poisoned source refuses before allocation.

The release boundary returns the actual Vec of Slots and its logical entry
count. A provider owns allocator release, descriptor/count publication and final
settlement after that call. `Map::admit` receives an already valid live Vec and
checks geometry, tags, empty slots, count, unique keys and probe reachability.
It returns rejected storage unchanged. It scans the table; it is not an O(1)
upgrade path. Ordinary access scopes do not scan or rebuild it.

Neither numeric descriptors nor `Map::admit` authorize construction of Rust
values from retained bytes. Before reconstructing the Vec, the provider must
independently establish schema/runtime compatibility, live allocation identity,
exclusive custody, initialization and exact allocator issuance. Provider root
metadata must identify this schema and its hash rules as well as count and slot
geometry. Unknown data must be refused before interpretation. The current API
does not implement that external root catalog or its atomic publication.

The existing [host region controller](../../qualification/ic-storage-native-feasibility/provider-region.md#product-keyed-table-on-the-host)
now exercises Slots through fresh allocation issuance, release, a replacement
controller, complete map admission and continued mutation. Portable recovery
uses separate backing. Its one-block allocator cannot support two live arrays
for growth or replacement: those operations refuse without losing existing
data. A second loan can now receive a native candidate while the first stays
live. Host tests check refusal, source preservation, candidate publication and
portable recovery after relocation. This is not a general retained allocator.

## Portable recovery

The logical envelope is `NKEYMAP1` (8 bytes), epoch (u64 little endian), entry
count (u64 little endian), count key/value pairs (two u64 little-endian words),
and SHA-256 of all preceding bytes. Entry order is unspecified. Its length is
56 + 16 × count bytes. It contains no native slots, hash positions, pointers,
capacity or compiler layout. The epoch is supplied by the trusted recovery
controller; the digest detects corruption, not malicious replacement.

Slice import checks the version, expected epoch, caller's entry bound, exact length
and digest before allocation. Duplicate keys refuse even with a valid digest.
The target creates a fresh table and recomputes placement. Invalid input or a
failed candidate leaves the original owner and poison state intact.

`Map::records_from_checkpoint` shares slice framing/epoch/count/integrity
admission and returns transient logical pairs sorted by ascending u64 key.
It allocates one pair per record, sorts in place and rejects adjacent equal keys
after sorting, including duplicates with matching values. It does not allocate
native hash buckets. The map checkpoint bridge uses these records directly for
atomic staging and durable job batches; native construction remains a separate
step after the checkpoint runtime returns. Whole-input validation and record
storage still scale with checkpoint size. A separately validated chunk format
is required before individual batches can avoid reading the complete checkpoint.
The [sequential page codec](paged-map-checkpoint.md) now supplies that portable
storage boundary with fixed-size page reads through governed host handles.
It does not yet replace the installed job's checkpoint or prefix validation.

`export` streams with constant encoder scratch space and chunks up to 32 bytes.
Its caller should coalesce I/O and publish a checkpoint only after successful
completion. A sink error may leave an incomplete sink; it cannot mutate the
source map. `checkpoint` is a convenience that allocates the whole byte vector.
`from_reader`/`from_reader_in` consume an exact-length checkpoint through a
caller-supplied consecutive reader. Read requests are at most 4096 bytes; a
4 KiB record buffer plus fixed header/digest state replaces the complete input
allocation. Header, length and entry limits are checked before allocation. The
single-pass importer constructs a private native candidate and returns it only
after entry validation and the final digest succeed. Source failures, integrity
failures and unwinding discard that candidate through its allocator. The slice
and streaming paths share envelope and record validation and use the same bytes.
The reader must fill each requested buffer or fail; the caller owns exact object
framing, custody and publication. This remains synchronous recovery, not a
durable cross-message import job.
This library introduces no direct stable-memory operations. Durable checkpoint
custody must use the design's governed transitional provider boundary when the
installed map integration is selected.

## Validation and load workload

`make native-core-test` covers the optional map/derive features, default vector
core, ownership compile failures and strict host/wasm32 Clippy. Map tests include
400,000 seeded mixed operations against BTreeMap, recovery and native admission
throughout those histories, wrapped collision deletion, limit/update behavior,
malformed native images, all checkpoint truncation positions, digest/epoch/count
failures, duplicate keys and poisoning/recovery. The provider-region test checks
fresh controller reopening and real allocator refusal.

`make native-map-firehose` runs a release-profile workload with 1,000,000 live
keys, batches of 4096 and 64 churn rounds: 1,000,000 initial inserts followed by
256,000,000 mixed reads, updates, deletes and reinserts. Every result is checked;
all records are checked after native readmission and after portable recovery.
Churn must preserve allocated capacity. The map then deletes and inserts again
after recovery to verify continued usability. It is ignored by ordinary unit
test runs and has its own offline Make target.

At this size the native table has 2,097,152 buckets, occupying 50,331,648 bytes
(48 MiB), and the logical checkpoint contains 16,000,056 bytes. These figures
exclude allocator metadata and transient candidate/checkpoint coexistence.
Timing output reports host wall-clock execution including result assertions;
it is not an IC instruction budget or a comparison against another store.

On 16 November 2026, one release-profile run on x86_64 Linux / AMD Ryzen
Threadripper 7970X, Rust 1.100.0, passed with these observed times:

| Phase | Work | Seconds |
| --- | --- | ---: |
| Ingest with growth | 1,000,000 new keys | 0.094918 |
| Mixed churn | 256,000,000 verified operations | 5.086270 |
| Native admission scan | 1,000,000 entries | 0.008949 |
| Portable checkpoint | 16,000,056 bytes | 0.018044 |
| Fresh portable reconstruction | 1,000,000 entries | 0.037051 |

The mixed phase measured about 50.3 million operations/second in this specific
workload. These single-host samples are development evidence, not latency or
throughput guarantees. The complete workload took 5.31 seconds including
additional verification. The same change passed 11 core unit tests, five derive
integration tests, 22 documentation/ownership checks, strict host/wasm32 Clippy,
and all 25 provider-region host tests. These host measurements remain separate
from the later installed map qualification.
Compiler details, source hashes and the captured load/lint/doc logs are retained
locally under `target/evidence/native-map/attempt.FTPqQG/`.

The workload now additionally calls `rebuild_in` with one million records,
verifies that source and destination allocations coexist at different addresses,
then readmits and recovers the rebuilt table. A subsequent host run on the same
compiler/machine completed that native rebuild in 0.028393 seconds, with 96 MiB
of simultaneous slot allocations. Its 256-million-operation mixed phase took
4.817048 seconds, and the complete test passed in 5.07 seconds. These remain
single-host observations; no IC throughput claim follows.

The subsequent streaming workload writes the checkpoint through a buffered file
sink, destroys the source map, then imports directly from the file with no
whole-checkpoint input buffer. On the same host, export took 0.025714 seconds
and recovery 0.039833 seconds; every recovered entry and continued mutation were
verified. The maximum read request was 4096 bytes for the 16,000,056-byte file.
The mixed phase took 4.796793 seconds and the complete test passed in 5.07 seconds.
File timings include host buffering and do not establish crash durability.
Captured output is under `target/evidence/native-map-provider/attempt.KU9EiD/`.
That cohort also passed the three map artifact checks; it did not rerun installed
lifecycle tests or exercise streamed import on IC. Focused host tests cover
chunk boundaries, short reads, framing, duplicate keys, integrity failures and
destination-loan reclamation after source errors or panic. Host/Wasm Clippy and
Rustdoc checks passed.

## Installed map qualification

`make native-map-test` compiles the actual product map into a closed foreign
provider fixture with two roots sharing one pool. Between loans, the provider
grows from 24 to 48, 96 and 192 KiB, adding Wasm pages without retaining Rust
owners or pointers across growth. Each table can grow through native rehash
while the other stays live and unchanged. The installed workload reaches 4,608
records across 4096- and 2048-slot roots and reopens both across compatible
upgrades. Contiguous candidate space is still required beside both sources;
capacity or fragmentation can refuse growth earlier. Explicit retirement makes
freed runs available for reuse without shrinking physical memory.
The fixture also qualifies joint growth and publication, lookup, deletion,
ordinary business errors, forgotten views, malformed metadata/slots, independent
source/target admission and failed upgrade rollback. Failpoints span candidate
writes, partial owner release, native return, each root write and settlement.
Separate counterexamples exercise stack/payload edges, copy bounds and custody.
Pool-growth failures also check page count, the complete physical payload,
initialization of previously unloaned space, failed-upgrade rollback and ordinary
memory.grow refusal under a restricted module maximum. The fill fixture uses
one scoped native batch; reopening both maps per record exceeded an IC message's
instruction limit at the larger sizes. Full boundary admission remains in place.

The [installed contract](../../qualification/ic-storage-native-feasibility/native-map-provider.md)
records the reviewed compiler shape, memory geometry, admission limits and
evidence. `make native-map-probe` runs artifact checks without a server and joins
CI. This is an installed map lifecycle increment, not the million-key IC test:
the native host load measurements above do not establish IC throughput.

## Next integration work

The [delivery roadmap](../DESIGN.md#delivery-roadmap) owns priority and completion
gates: larger installed recovery, reusable root custody, concrete records, then
ordered collections required by sibling consumers. The details below describe
this collection's evidence and remaining technical boundaries.

The [governed two-map checkpoint](../../qualification/ic-storage-native-feasibility/native-map-recovery.md)
now connects the product codec to the existing Storage locator and installed
fresh-heap reconstruction. It qualifies at most 64 records per map, including
full-width scalars, frozen-source custody, corruption/epoch refusal, rollback and
explicit recovery after corrupt native metadata. This separate fixed-domain
profile does not yet cover recovery of the larger growing-pool workload.
The [durable map-job profile](../../qualification/ic-storage-native-feasibility/native-map-jobs.md)
adds controller checks, sequential epochs, cancellation, frozen resumption and
atomic consumption through the shared migration state machine for that bound.
It also qualifies private native prefixes across bounded import batches,
compatible interruption and explicit restart from the independent checkpoint.

The [provider pool host experiment](../../qualification/ic-storage-native-feasibility/provider-region.md#multiple-root-host-pool)
now supports disjoint runs for multiple maps and simultaneous growth candidates.
Its opt-in `make native-map-pool-firehose` passes a three-root, 1,024,576-record
growth/reopen/mutation/recovery lifecycle within one 96 MiB loan. Six focused
host cases cover refusal, reclamation and custody boundaries. The same allocator
now serves the installed two-root contract above; no allocator object or private
collection header is retained across entrypoints.

The [paged native profile](paged-map-checkpoint.md#installed-paged-native-recovery)
now recovers the 3,072 + 1,536-record installed workload, growing the private target
from 24 to 192 KiB between ended native loans. Faults roll back growth, records
and durable progress together. The [entrypoint measurements](paged-map-checkpoint.md#one-scope-page-application-evidence)
now peak near 102 million instructions, down from 1.41 billion. Each page applies
within one admitted native scope through a checked scalar staging callback;
both-root validation and publication checks remain. Live allocator peaks,
repeated paged epochs and application service budgets remain unqualified.
The separate native growth profile adds physical Wasm pages, while the paged
profile reserves its native windows in advance. Neither establishes million-record
IC throughput or automatic elasticity. Application records and ordered traversal
need their own representation, lifecycle and portable paths under the same runtime.

These are integration gaps in one native collection direction, not reasons to
build a second M0 collection system. The package remains experimental and
unpublished; no consumer cutover is authorized by these qualification results.
