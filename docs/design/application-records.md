# Application byte-record ownership

Status: local implementation for the pending 0.76.2 release. This is the first
Root/entry increment of
[Storage #5 — Provide retained runtime, records and composed ownership for Canic 0.76 adoption](https://github.com/dragginzgame/ic-storage/issues/5),
not completion or publication of that issue's full supplier contract.

## Scope and construction

The `records` feature provides `Plan`, `Root`, `Store`, participant grants and
owned `HostMemory`. The optional `platform` feature selects Platform Runtime
0.23.17 and supplies `StableMemory` on Wasm. All product code remains safe Rust.
This path reuses the [checkpoint backing](checkpoint-storage.md) and its records;
it adds application custody and admission rather than another allocation engine.
The retained native collection direction and its runtime work remain unchanged.

An application bootstrap declares sorted, nonzero root IDs, participant IDs,
schema identities and logical byte capacities. `Plan::new` validates every
reservation and the physical page ceiling before backing mutation. The current
directory supports at most four roots. Capacity/root-set changes require an
explicit migration; no automatic growth, root deletion or compatibility discovery
is provided. This bound is a supplier limitation, not a Canic policy constant.

Bootstrap acquires the backing and calls exactly one of:

- `Store::create(backing, &plan, initial_values)` for an explicit fresh installation.
  Initial values follow declaration order; all bounds and fallible payload-buffer
  preparation happen before backing writes. Existing allocated pages refuse.
- `Store::open(backing, &plan)` for existing state. Directory, allocated size,
  root/participant/schema identities and record seals must match. Missing, corrupt
  or unsupported state refuses without writes. There is no fresh fallback.

`AdmissionError` returns the original backing for inspection or explicit recovery.
Normal creation errors are not a disk-crash transaction. Infallible Rust allocator
failure can abort/trap; IC message rollback remains the installed failure boundary.
Applications retain Wire or other logical codecs and must validate their complete
payloads and invariants before exposing services. A seal is not authorization or
proof that an application journal is semantically valid.

The backing is consumed, not cloned. Host backing owns its byte allocation;
reopened snapshots must be owned and page-aligned. `StableMemory::take` leases the
entire stable address space once per live owner generation. Other raw/legacy
managers must not share it. Applications receive grants, not copies of this binding.
This does not make old Blob/DB `RuntimeMemory` handles compatible.

## Participant and transaction contract

`Store::take(participant)` issues that participant's declared roots exactly once.
A grant is not cloneable; dropping or forgetting it does not permit reissuance
within the generation. Grants retain the owner/backing lifetime even if bootstrap
drops its `Store`. `into_backing` succeeds only after all grants are dropped and
the owner is healthy. Forgetting a grant prevents reuse instead of duplicating custody.

Participants use scoped `read` and `update` closures. The coordinator may pass
several grants to the same Store operation for composed access; foreign, duplicate
and empty grant lists refuse. Only granted root IDs can be read or written.
Neither views nor byte references can escape a scope or cross an await. Mutable
reentry refuses; cached immutable reads do no backing IO or reinitialization.

Each successful `set` prepares its bounded sealed bytes, then writes the payload
and changed length synchronously. There is no end-of-message or pre-upgrade flush.
The record's expected capacity/allocation refusal precedes its writes, but a normal
Rust `Err` does **not** undo earlier operations in a composed scope. Applications
requiring atomic failure should prepare their logical payloads first and trap on
remaining refusal in the IC message. Host unwind poisons every participant; it
does not restore earlier bytes or permit healthy backing reclamation.

Installed IC traps roll back backing, cached values and ownership state together,
including failure after Storage returns, such as response encoding. Synchronous
intent writes and outgoing-call enqueueing share the current IC transaction;
callbacks are separate transactions. Storage provides no scheduler, effect policy,
endpoint mounting, readiness or automatic service activation.

## Representation and lifecycle

Each existing `ICREC001` record contains `APREC001`, three little-endian u64 fields
(root, participant, schema), exact application bytes and SHA-256 of the preceding
envelope bytes. This adds 64 bytes to the logical payload, plus the underlying
16-byte record header. Physical reservation is the existing 128-byte directory
plus each capacity + 80, rounded to 64-KiB pages. Changing schema/participant
identity refuses reopen even when byte lengths fit.

Reads are cached after one bounded full-record admission. Setting a record copies,
hashes and writes its logical length, not another root or the complete store.
Shrinking replaces/reclaims its transient owned buffer; backing reservation and
unused old tail bytes remain private to that same root. There is no physical
reclamation or secure-erasure promise. SHA-256 detects corruption, not hostile
replacement by the storage authority.

This first path supports ordinary heap-discarding IC upgrades. Owner/grant/cache
headers survive ordinary messages but are recreated by explicit post-upgrade open.
No pre-upgrade close/flush is required because updates already reached backing.
Query execution changes remain nondurable. Enhanced orthogonal persistence and
native retained allocator/session reconstruction require the separate R2 contract;
do not retain these private Rc/Vec/RefCell headers as a native ABI.

## Installed supplier evidence — 17 November 2026

`make application-records-test` builds ordinary A/B canisters and tests the same
public API offered to consumers. The application fixture contains no raw memory
partitioner, checkpoint region issuance or copied native provider code. Source,
lock/compiler identities and final artifacts are recorded under
`target/evidence/application-records/attempt.RdsQev/`; the passing run is
`target/application-records-installed-retry.log`.

Both artifacts are 59,379 bytes: A has SHA-256
`a2318635d5cd6b24c3110390fb4da2d2216b4797fb53ba97fba323e989eaf6f9`;
B has `4acddbf437e2b8a7909fe8eba9cc87f1393558aa84da691f183ccfb0d5318154`.
The 16,384-byte journal, 1,048,600-byte historical artifact ceiling and 128-byte
entry binding reserve 17 physical pages (1,114,112 bytes), within the supplied
Root ceiling. This tests the envelope bound; it does not put a new artifact
repository into Root or change Canic's Store-reference ownership policy.

| Operation | Measured body instructions |
| --- | ---: |
| Maximum journal/artifact write plus binding | 96,618,489 |
| First record over its bound | 338,836 |
| Later record over its bound, preserving the earlier normal write | 4,098,337 |
| Small 37/47-byte pair plus binding | 837,964 |

These include request handling, value preparation, envelope hashing/writes and
reply encoding, ending before reply transmission. They exclude complete upgrade
cost, heap high-water accounting and Canic's real codecs/effect logic.

Installed checks cover three late in-scope traps, actual reply-encoder refusal,
query rollback, normal partial-error behavior, A→B→A upgrades without a flush,
failed target writes, unsupported schemas and empty/corrupt state without reset.
Four host tests additionally cover independent/foreign grants, source-independent
reopen, exact bounds, backing preservation and whole-owner poisoning. Compile-fail
examples reject escaping views/references. Focused host/Wasm lint and Rustdoc apply.

## Remaining Storage #5 acceptance

Keep Storage #5 open. Publish the API only under a release request, then let Canic adapt
and qualify its actual Root/entry codecs, complete effect custody and lifecycle.
R2's public retained native runtime, R3's real two-root profile engine, and real
Blob/DB/Store participant adoption remain open. Their current 0.74 handles cannot
be mixed into this owner. Consumer changes, cross-supplier composition and existing
fleet transitions require their owning qualification; this fresh fixture proves
none of those migrations. See the issue for the canonical remaining acceptance.
