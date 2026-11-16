# Controlled record conversion

This experiment exercises a concrete application-schema conversion under the
supported runtime policy, both on the host and across installed IC upgrades. It
uses the existing region controller and scoped vector core. The scalar types
live in [record.rs](src/record.rs), with [host integration](tests/fixtures/region/tests/host/records.rs)
and an [installed record profile](../ic-storage-retention-probe/tests/provider_link/migration/records.rs).
It does not introduce a Store API or derive macro.

## Representation and ownership

The logical identity remains `RECU64V1`; that opaque identity is separate from
the schema version. Both payload declarations use repr(C) and only u64 fields:

| Schema | Fields and offsets | Size / alignment on the tested host |
| --- | --- | --- |
| V1 | counter at 0 | 8 / 8 |
| V2 | counter at 0; ceiling at 8 | 16 / 8 |

V2 requires counter ≤ ceiling. Construction, conversion and checked addition
enforce this invariant before changing the record. Every stored field is a
scalar; neither declaration owns a vector, reference, allocator or destructor.
The host keeps the existing vector descriptor and backing separately. Its Record
enum selects a live schema transiently; neither that enum nor ProviderHost's
private layout is a retained ABI. No cast from V1 bytes to a larger V2 is used.
Host offset tests do not establish cross-instance pointer validity. The installed
profile below uses scalar metadata and the existing vector custody contract; it
does not retain Rust references to these record declarations.

## Conversion and recovery

V1→V2 requires an explicit ceiling chosen by the caller. A ceiling below the
existing counter refuses. The host constructs a scalar candidate and publishes
it with one infallible assignment, leaving the vector's descriptor, allocation
address and every backing word unchanged. A pre-publication failpoint also
preserves the old record and availability. Repeated conversion and poisoned-host
conversion refuse. This fixed scalar converter invokes no application callback,
allocator or destructor; arbitrary fallible converters need a separate protocol.

The [checkpoint codec](checkpoint-format.md#record-schema-2-envelope-version-3)
exports the new field explicitly. Recovery validates the record and reconstructs
a fresh vector; spare capacity and addresses are not portable state. Failed
candidate reconstruction leaves the current record/vector intact. Successful
recovery after a caught unwind restores both fields and vector values without
reopening the poisoned state. Explicit replacement may discard newer writes;
it is not host rollback or an implicit downgrade mechanism.

`make native-feasibility-test` covers empty, ordinary and full host vectors,
conversion refusals, unchanged vector storage, checked counter mutations,
old-reader rejection, source-scope teardown and fresh recovery, poison and failed
replacement. Codec tests independently assemble v3 bytes, preserve old golden
bytes, and reject invalid schema/invariant, corrupt, truncated, oversized and
wrong-epoch inputs. The separate installed checkpoint/job suites remain
legacy-schema regression checks.

On 16 November 2026, Rust 1.100.0 on x86_64 Linux passed the native feasibility
gate, including 30 library tests and 16 provider-region tests, plus strict Clippy
and Rustdoc. Legacy installed regression cohorts are
`target/evidence/provider-link/attempt.0NTLfK/` (14 artifact and four checkpoint
checks) and `attempt.YC0Yjp/` (14 artifact, nine host job and seven job checks).
The collector includes the record module alongside the codec inputs. Earlier
passing cohorts `attempt.1HSfOt` and `attempt.zbUv5C` omitted that module from
their source copies and are not the complete retained regression evidence.

## Installed record profile

`make native-record-test` builds the checkpoint adapter with `native-record` and
composes it with the existing guarded vector and provider. Native provider format
3 reserves these little-endian u64 words outside both Rust transient domains:

| Offset | Meaning |
| --- | --- |
| 65592 | Schema version, exactly 1 or 2 for the selected executable |
| 65600 | Counter |
| 65608 | Ceiling; zero in V1, at least counter in V2 |

The vector header and 4096-byte allocation retain their existing contract.
Format-2 executables cannot directly reopen this profile. V2-compatible reopening
requires schema 2; a V1 executable refuses V2. Only the explicit converter admits
schema 1 during cold upgrade startup. Admission checks the full-width schema and
invariant before native access. Fresh recovery checks the expanded metadata span
is zero before creating the target. A newly initialized V2 fixture uses u64::MAX
as its ceiling; this is test initialization, not an implicit migration default.

Conversion executes in this order: WAT source admission, checkpoint transient
runtime reset, Rust V1→V2 scalar conversion, then native post-upgrade admission.
The WAT callback graph cannot re-enter Rust or call the host. Only scalar copies
cross into Rust; WAT publishes the ceiling followed by the schema. No record
reference, saved enum layout or owning vector crosses that boundary. The final
native operation admits V2 after entering the ready phase. No vector loan,
allocation or destruction occurs during conversion itself.

Upgrade arguments are two little-endian u64 words: chosen ceiling and test fault
selector (0 normally, 1 before publication, 2 after publication). Conversion is
private to the upgrade hook. Validation errors and failpoints trap; installed IC
upgrade rollback restores the old module, record, vector region and stable bytes.
This does not change the rule that an ordinary application Err leaves preceding
writes intact, or make host unwinding transactional.

The record adapter exports checkpoint v3 for V2 and v2 for V1 through the same
pinned governed provider. Explicit heap replacement reconstructs the destination
record and vector from logical values. The destination must match the checkpoint
schema; recovery does not silently convert or discard a field. The checkpoint is
independently readable after discarding source linear memory. This bounded
single-message profile has test endpoints and no production migration authority;
it does not extend the separate governed-job schema or prove arbitrary graphs.

Installed cases cover empty, spare-capacity and full vectors; invalid ceilings,
malformed arguments, traps before/after publication, repeated conversion and
schema downgrade refusal; compatible V2 reopening; wrong epoch, failed stable
write and partial reconstruction rollback; relocated recovery and return to the
original domain with continued allocation. Full vector bytes are unchanged by
successful conversion. Recovery deliberately preserves logical values rather
than source addresses or spare capacity.

On 16 November 2026, `make native-record-test` passed strict Clippy, 14 shared
artifact checks and two record checks (artifact ordering and the installed
lifecycle matrix), including corrupted checkpoint refusal and exact failpoint
diagnostics. Complete inputs and results are in
`target/evidence/provider-link/attempt.dHjY9d/`. The final legacy checkpoint
regression passed 14 artifact and four installed checks in `attempt.mPIxxL/`;
the job regression passed nine host job, 14 artifact and seven job checks in
`attempt.Vhoeza/`. These are local prepared-PocketIC results, not production or
actual cross-compiler qualification.
