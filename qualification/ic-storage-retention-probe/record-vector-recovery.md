# Portable record-vector recovery

This closed profile extends the [native record-vector proof](record-vector-provider.md)
with installed export/import independent of the discarded native heap. It is
qualification code, limited to sixteen V3 records under Rust 1.100.0 on wasm32,
with an explicit portable V2→V3 collection-conversion path.
The incompatible target changes the allocation domain; this does not claim an
actual compiler upgrade, arbitrary Rust graph recovery or a production API.

## One logical codec and governed reader

The host arena and installed bridge now share
[`checkpoint::record_vector`](../ic-storage-native-feasibility/src/checkpoint/record_vector/mod.rs).
The existing `RECVEC01` format is unchanged: a 32-byte header identifies V3,
count and expected source epoch, followed by ordered twelve-byte generated field
payloads and a 32-byte SHA-256 digest. The V3 maximum is 256 bytes. The explicit V2
source uses schema version 2, sixteen-byte payloads and a 320-byte maximum; the
governed reader's bound accommodates both. Ordinary V3 decoding still rejects V2.
Capacity,
native padding, pointers and allocator state are excluded. Every record's
`counter <= ceiling` invariant is checked before target initialization. The
decoder returns bounded transient scalar records; no old native owner is decoded.
The digest detects byte changes, not malicious authorship or replay.

The [checkpoint adapter](../ic-storage-checkpoint-provider-probe/README.md) owns
key `native.checkpoint.record_vector.v1`, owner `native-checkpoint` and authority
prefix `native.checkpoint.`. It uses published Storage 0.74.1's governed
`StableCell<Vec<u8>>` reader and existing directory/cell formats. A fresh transient
runtime opens the owned locator for every operation. Import requires that locator
and a complete bounded envelope; it never creates a missing checkpoint or repairs
unknown bytes. This is a separate fixture key, not a second provider or changes
to the frozen sibling repository.

## Native and checkpoint domains

The native provider has a distinct marker `0x4e5650524f563034`, rejecting direct
interchange with the standalone record profile. Its full V3 schema, initialized
prefix and exclusive claim/settlement contract remain unchanged. Domain A places
the 256-byte allocation at 131072; domain B places it at 147456. The provider,
claim result, initialized-field scan and every native access guard relocate
together. Raw Rust obtains its address from the opaque claim and is unchanged.
Matching element layout does not make those allocation domains interchangeable:
ordinary retained reopening rejects a mismatched base.

The shared bridge composer admits memory with 32 initial pages and a 256-page
maximum. The native stack is `[32704,32768)`; metadata is `[65536,65680)`, and both
payload domains stay below 196608. The checkpoint Rust runtime uses the lower
64 KiB stack and data/heap from 196608 upward. Stack use overlaps temporally:
native Rust and checkpoint Rust must never be live together. Six scalar callbacks
(`freeze`, `record_value`, `fresh`, `begin`, `record_put`, `finish`) stay in trusted
WAT. The combiner checks their closed call graph cannot enter either Rust domain
or call host functions. No owner, pointer or reference crosses this interface.
Only the private native entry is allowed to reconstruct a Vec after checkpoint
Rust returns. Native code cannot grow memory; checkpoint allocations may grow
the independent transient domain up to the admitted maximum.

Compatible reopening first admits the native provider and ends its transient
Rust operation, then rebuilds the checkpoint runtime through the existing reset
plan. Unknown retained data refuses before reset. Explicit replacement instead
discards the entire heap, applies the target's own initialization and enters
recovery; no source allocation or allocator metadata is consulted or dropped.

## Publication and failure ordering

Export checks settled custody, the complete schema and initialized fields before
freezing native writes. Scalar callbacks read only initialized logical fields.
The bridge encodes and writes one complete governed checkpoint in the same IC
message. Normal native mutation refuses while frozen. A trap after the stable
write rolls back both the checkpoint and the freeze; no `Err`-based undo or guard
destructor is assumed. Re-export from the same intact frozen source is allowed.

Recovery requires all eighteen metadata words to be zero and the new instance's
phase to be initial before entering checkpoint Rust. The reader verifies framing,
schema, count, epoch, digest and every record before `begin` initializes a fresh
provider. `record_put` writes checked scalar fields in strict sequential order,
bounded by sixteen records; those writes are not a published root. `finish`
rechecks the initialized fields and publishes length/capacity with settled custody.
Only after the bridge returns does native post-upgrade construct its first Vec.

Faults after a stable write, during partial construction or after root publication
must abort the complete upgrade. Tests compare the old module hash, all observed
native metadata/payload bytes and complete stable memory. Successful recovery
preserves logical fields and the checkpoint; capacity and padding need not match.
Queries of the discarded domain prove the old payload is zero, not the source of
the recovered values. Native allocation, mutation, shrink and reuse then continue.

An already exported checkpoint also permits explicit recovery when the old schema
header is corrupt. The ordinary source hook refuses; the test deliberately skips
that hook with the management replacement operation. This is test authority, not
a production recovery policy or an automatic fallback on admission failure.

## Evidence and limits

Run `make native-record-vector-test` with the existing prepared compiler, caches
and local test server. It builds the selected bridge, performs strict Clippy and
collects source, lockfiles, compiler identity and digest-named artifacts. Existing
raw native diagnostics still run. CI invokes the same target.

On 16 November 2026 the installed proof passed on x86_64 Linux with PocketIC
16.1.0: seventeen raw/link artifact checks, one recovery artifact check and two
installed recovery cases. The lifecycle case covers empty, spare, ordinary and
full roots; compatible reopening; incompatible retained refusal; missing complete
exports; wrong epochs; frozen mutation; failure injection; A→B→A replacement;
old-domain clearing; and continued native operations. The other installed case
rejects truncated, digest-damaged and unknown-schema checkpoints without reset,
then repairs the checkpoint using the intact frozen source and retries recovery.
The shared codec retains fifteen host record-element tests, including independent
expected bytes and resealed invalid records rejected before arena allocation.

The completed initial cohort is
`target/evidence/provider-link/attempt.MH28FP/`. The relocated recovery artifact
is 123711 bytes, SHA-256
`c62e2e9576bca7f104d70b0eb99d697705d51af809d4ac0c906eb95eb2a7d6bd`.
Failed development cohorts remain retained and are not passing evidence.
The final contract-capturing cohort `attempt.ztTJ1o/` passes the same checks,
includes this recovery contract and the fresh empty-root restriction, and produces
the same Wasm digest.

The directly affected regression targets also passed after the shared composer
change: four scalar/vector checkpoint cases (`attempt.Pgrjxu/`), four scalar-record
cases (`attempt.UtMgDO/`), and nine host plus seven installed migration-job cases
(`attempt.n1Osdt/`). Each installed cohort includes seventeen native artifact
checks and a completion receipt under the same provider-link evidence directory.

This bounded, single-message profile does not implement durable checkpoint
consumption, controller authorization, resumable batches or replay
prevention. Those existing scalar/u64 migration-job proofs do not automatically
apply to record vectors. Retained collection conversion, job integration and
larger workloads remain required before consumer cutover. No production subnet,
native macOS or cross-compiler transition is claimed.

## Explicit portable collection narrowing

The `native-record-vector-source` fixture owns an ordinary Global `Vec<V2>` using
the actual Store declaration and generated field codec. It has no EOP marker and
makes no retained-V2-layout claim. Source export writes the complete V2 checkpoint
and freezes mutation in one message. Its pre-upgrade hook requires that frozen
source. A failed export restores both the prior checkpoint and the mutable source.

The explicit `NarrowVector` destination follows the existing replacement order:
WAT fresh-header gate → private Rust `checkpoint_vector_narrow` → native post-upgrade.
V3 recovery and V2 conversion have distinct private entrypoints; neither silently
accepts the other's schema, including empty collections. Source and target are
qualification schemas, not maintained package-release versions.

The shared decoder validates the complete V2 envelope and every source invariant
before calling `V3::try_from` in collection order. Checked narrowing preserves
each ceiling and rejects counters greater than `u32::MAX`; the host error records
the failing index. All conversion results remain transient until the full batch
succeeds. Only then do the scalar callbacks initialize and publish the target.
There is no cast between equal-sized native records and no partially published
prefix on range failure. Source pointers, headers and allocations are discarded.

The ordinary source is linked with its data starting at 65536. Artifact admission
checks nonzero source initialization within the target's 144-byte metadata gate,
and installed tests verify retained-memory narrowing refuses. Explicit heap
replacement removes those bytes and all source allocations before reconstruction.
This evidence applies to this concrete source artifact; an arbitrary ordinary
Rust module cannot be assumed to supply the same fresh-memory discriminator.

Range failures leave the original executable, logical source values, frozen state
and complete stable memory unchanged. The test source offers explicit
`cancel_export`: it clears the obsolete checkpoint before unfreezing application
writes. A subsequent repair and export at a new epoch permits retry; the old epoch
refuses. This source-local repair control is not the durable migration job's
authorization, consumption or cancellation protocol.

On 16 November 2026, `make native-record-vector-test` passed three shared codec
tests, seventeen native artifact checks, two recovery/conversion artifact checks
and four installed cases. New cases cover empty, singleton and full collections;
the accepted `u32::MAX` boundary; rejection at the first, middle and last record;
`u64::MAX`; wrong epochs; unselected conversion; failure before/during/after
publication; source repair and retry; and native V3 reuse, append, mutation and
subsequent V3 checkpoint recovery. Fifteen host record-element cases also pass.
The codec tests reject resealed invalid records before conversion begins, including
a late invalid record after an earlier valid-but-unrepresentable value.

Evidence is in `target/evidence/provider-link/attempt.Xvbcip/`, with both source
and destination compiler outputs captured. The ordinary source is 147810 bytes,
SHA-256 `0de065dc1d9f7568498d82c74a835aa244a7218506856e39ef14a3e2251cfe86`.
The relocated narrowing destination is 126313 bytes, SHA-256
`48a06355f208dcfb8ac165627bc2c2fe1a02608889e9e72d7a04c54078874958`.
The earlier recovery digests remain historical evidence for their recorded inputs.
The shared-linker regression targets also passed: four scalar/vector checkpoint
cases in `attempt.lfM8gn/` and four scalar-record cases in `attempt.wpBkd2/`, each
with seventeen native artifact checks. Strict Clippy, Rustdoc, formatting and
local documentation links passed after the conversion change.

This proves schema conversion together with portable reconstruction. It does not
yet convert a retained native V2 record vector, preserve its allocation in place,
or implement general collection converters or durable migration authority.
