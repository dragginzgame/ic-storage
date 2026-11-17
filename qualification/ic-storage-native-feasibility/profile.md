# Native profile workload

Status: host qualification and Wasm compilation for the pending 0.76.2 work.
This advances R3 and
[Storage #5 — Canic retained runtime, records and composed ownership](https://github.com/dragginzgame/ic-storage/issues/5).
It does not close that issue, publish a profile engine, or migrate Canic.

## Consumer and representation

The behavior basis is the two-root profile request in Storage #5 and the
[pinned Canic state interface](https://github.com/dragginzgame/canic/blob/d652c55e4cc455a642f44645bf2e16a6d08baefc/apps/profile/canic-profile-worker/src/state/mod.rs).
The independently authored fixture models its identity, immutable owner binding,
revision, exact retry, cardinality and name limits. It is not Canic's actual engine
or Wire codec, nor a claim of source/format compatibility. Authentication and
identity namespaces remain the embedding application's responsibility.

`profile::State` holds Storage's public
[`ordered::session` owner and grant](../../docs/design/native-sessions.md) for two
native maps. Root declarations restrict rows to 8-byte big-endian ID keys and
at most 512-byte records, and the owner index to at most 29-byte keys and
8-byte little-endian IDs. Both maps use the common K=29/V=512 collection type;
per-root bounds are enforced by Storage. Both have a 16-record ceiling. Each uses the
existing native AVL nodes and byte arena; these are not serialized tables in
stable cells. Live owners survive ordinary calls without readmission, table
scans or checkpoint writes. Full bounded relationship validation occurs at
explicit application admission/recovery, not lookup.

Records contain only the fixture's logical bytes: `PROFILE1`, schema u8, ID u64,
revision u64 (Wide) or u32 (Narrow), owner length u8, name length u16, owner bytes
and UTF-8 name bytes. Integers are little-endian. Exact framing, nonzero identity
and revision, owner bound/non-anonymity and nonblank names of at most 128 bytes
are validated. Empty owner bytes are allowed, as in the reviewed consumer;
anonymous `[4]` is refused. Names are bounded by UTF-8 bytes, not character count.
Unused suffix bytes, native pointers and Rust padding are never encoded.

The maximum Wide record is 185 bytes; Narrow is 181. `Profile`, bounded `Bytes`
copies, map headers and allocator references are transient projections, not new
retained Rust layouts. This workload requires no extension of the scalar derive.

## Operations and failure boundary

Creation/update prepares bounded logical bytes before writes, checks both indexes,
then replaces the row and owner binding. An exact retry returns the existing
revision without mutation. Foreign-owner, duplicate-owner, stale-revision, invalid
input and live-capacity refusals preserve both roots. Revisions cannot wrap.
Storage-level deletion removes both entries and recycles native allocations;
Canic's current profile API does not expose deletion.

Storage's public session owns the whole-family poison boundary. The fixture has
no private poison flag or direct map access during operations. Normal application
refusals preserve earlier writes; after a storage error the application explicitly
quarantines the session because the first root may already have changed. A caught
unwind within a write scope poisons every participant automatically. No poison
reset is provided. Discard the failed host owner and reconstruct from a selected
checkpoint. An installed caller would have to trap to obtain IC rollback;
these host tests do not establish that installed behavior.

## Admission and recovery

Compatible host readmission first releases and re-admits each actual live native
allocation, then validates row IDs, decoded schemas and the owner index's complete
bijection. This proves application invariants on valid live allocations, not raw
pointer provenance or cross-executable allocator reconstruction.

Portable recovery binds both existing `ORDMAP01` artifacts in `PROFCP01` with
explicit source schema, epoch, root IDs 1/2, exact lengths and an outer SHA-256
seal. The complete artifact has a declaration-derived size ceiling before native
candidate allocation. Source state is destroyed in the recovery tests. Both
private candidates must satisfy native and application validation before return.
Digests detect damage; external custody must supply authorization and freshness.

The sole schema conversion is explicit Wide→Narrow revision conversion. It
preserves IDs, owners, names and revision values, refusing values above u32::MAX.
Neither automatic schema detection nor reverse conversion is supplied. Conversion
failure discards all candidate allocations and preserves the source artifact.
Recovery uses bounded complete buffers for this 16-profile fixture, independently
of the larger collections' paged recovery job. It is not a routine upgrade flush.

## Evidence and next boundary

Run `make native-profile-test` for six product session tests, three scope
compile-fail examples, seven profile/allocation tests and host/Wasm lint. The
native core/feasibility CI suites also select these tests. Local evidence:
`target/profile-host-tests.log`, `target/profile-allocation-tests.log`, and
`target/native-profile-test.log`.

The 16-profile sample with maximum-sized names/owners uses 14,224 requested bytes
of live native map capacity. Repeated deletion/reinsertion keeps capacity constant.
The explicit full-family conversion peaks at 30,576 requested bytes in the fault
allocator, including overlapping native candidates; it succeeds after 19 native
allocations. Every earlier allocation cutoff refuses and returns live allocation
accounting to zero. These figures exclude checkpoint buffers, stack, allocator
metadata and provider reserves. They are not installed physical-page or instruction
measurements and do not qualify Canic's 32-page budget.

Remaining gates are the public retained allocation/bootstrap integration,
installed two-root traps and late reply failure, compatible executable
upgrades and conversion/recovery lifecycle, and the real Canic engine/codecs.
Root/entry's separate installed byte-record tests do not supply those proofs.
