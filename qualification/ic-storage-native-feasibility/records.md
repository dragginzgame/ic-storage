# Controlled records and conversion

This experiment exercises a concrete application-schema conversion under the
supported runtime policy, both on the host and across installed IC upgrades. It
uses the existing region controller and scoped vector core. The scalar types
live in [record declarations](src/record/mod.rs), with [host integration](tests/fixtures/region/tests/host/records.rs)
and an [installed record profile](../ic-storage-retention-probe/tests/provider_link/migration/records.rs).
The scalar-only Store derive now generates their declarations, codecs and scoped
getters. This remains a qualification fixture, not a general Store runtime.

V1/V2/V3 are test application schemas, not maintained product releases. The
repository develops one current pre-1.0 implementation without a compatibility
promise for older experimental releases. Multiple declarations and envelope
versions here exist to exercise concrete upgrade, refusal and recovery cases;
they do not establish a product support matrix or indefinite format maintenance.

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
| 65592 | Schema version, exactly 1, 2 or 3 for the selected executable |
| 65600 | u64 counter in V1/V2; u32 counter in V3 |
| 65604 | Upper counter bytes in V1/V2; canonical zero reserved word in V3 |
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

The record adapter exports checkpoint v4 for V3, v3 for V2 and v2 for V1 through the same
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

## Scoped record updates (0.1.13)

The generated `V2::store_with` API lends a restricted `V2View` with `counter` and
`ceiling` getters, plus a hand-written checked `add` operation. Reads return scalar copies; no mutable record
reference, owning collection or arbitrary field setter is exposed. Higher-ranked
scoping rejects returning a view, retaining it in an outer capture or capturing
it in an escaping future. The payload remains the same repr(C) pair of u64s;
the view is transient. The V2 declaration is qualification code exercising the generated surface,
not a new retained representation or public Store runtime.

The host's `update_record` shares poison state with vector access. A normal
returned error preserves earlier successful writes and leaves the host usable.
An unwind leaves it poisoned even if the view was forgotten. Vector access,
record access and export then refuse until explicit checkpoint replacement;
failed reconstruction preserves poison and the modified record. Finalization
belongs to the host rather than the view's destructor. Tests exercise unchanged
vector storage, business errors, forgotten views and failed/successful recovery.

The installed test endpoint `record_add` takes two little-endian u64 arguments:
delta and fault selector. It admits a ready, settled V2 or V3 record with no active
vector operation. The provider copies the counter and ceiling into Rust; a
checked mutation runs through the matching generated view and publishes the resulting
counter through a scalar-only provider callback. That callback rechecks custody
and the counter ≤ ceiling invariant. No retained reference enters Rust, no vector
loan is needed and no checkpoint is written during an ordinary update.

The reply is one little-endian u64: 0 for success, 1 for ceiling refusal, 2 for
overflow, and 3 for a V3 delta outside u32 range. A refused addition leaves the
record intact. Fault selector 1 traps
after successful publication to test IC message rollback; selector 0 is normal.
Malformed arguments, unsupported selectors, schema 1 and frozen state trap.
This endpoint performs one checked addition, not an arbitrary multi-write
transaction. Its failure controls remain unauthenticated qualification endpoints.

`make native-feasibility-test` passed with 18 provider-region tests and 15
doctests, including three new view-escape compile failures. On 16 November 2026,
`make native-record-test` passed strict Clippy, 14 shared artifact checks and
three record checks in `target/evidence/provider-link/attempt.SiJmtO/`. The new
installed update matrix covers empty, spare-capacity and full vectors, successful
and refused mutations, exact post-publication trap diagnostics, frozen-state
refusal, compatible reopening, checkpoint recovery in both allocation domains,
continued vector reuse and the valid u64::MAX counter with overflow refusal.
Both native format 3 and checkpoint envelope v3 are unchanged.

The shared-composer regressions also passed: 14 artifact and four installed
checkpoint checks in `attempt.FVxzuw/`, plus nine host job, 14 artifact and seven
installed job checks in `attempt.3jtMiK/`. Formatting, dependency boundary/pin
checks and strict Rustdoc passed with the workspace prepared as 0.1.13. No release
or general retained-type admission is implied by this qualification.

## Derived scalar declarations (0.1.13)

V1 and V2 now use the experimental [`Store` derive](../../crates/ic-storage-native-derive/README.md).
Both explicitly declare `RECU64V1`, their respective version and field IDs;
V2 binds the existing ceiling validator. Generation replaces the hand-written
scope/getters and supplies the scalar payload codec used by checkpoint export
and import. V2's constructor, checked addition and conversion policy remain
hand-written, sharing the same validator. The generated scope is named
`store_with`; no compatibility wrapper preserves the earlier prototype spelling.

The generated descriptor lists logical field IDs and compiler-established public
offsets, plus size/alignment. Tests bind those values to the concrete V1/V2
contract. A reordered fixture demonstrates that portable order follows field IDs
while native offsets change; this does not authorize reopening changed layouts.
The fixed checkpoint envelope still owns schema selection, epoch, integrity,
limits and publication. Its original golden bytes and v3 framing remain unchanged.

The initial V1/V2 subset requires Copy and explicit u64 fields. No derive marker
can authorize arbitrary retained bytes: the provider still owns scalar metadata
and vector custody. Unsupported owning/borrowed fields, enums, generics, packing,
duplicate IDs, skipped fields and ambiguous attributes refuse. Compiler checks
also exclude application Drop and misleading scalar aliases. Generated decoding
checks exact length and application validation before returning a fresh record;
it does not reconstruct saved pointers or private runtime headers.

`make native-feasibility-test` now includes three macro tests (including the
declaration-refusal matrix), 32 library tests, 18 provider-region tests and 19
doctests. The existing view-escape cases exercise generated views, and four new
compile-fail cases cover owners, borrows, Drop and scalar-name shadowing.
Generated codec tests use independent bytes and check invalid length/invariants,
native metadata, reordered field encoding and continued checked mutation.
Native-core tests, all-feature workspace Clippy, dependency checks and strict
Rustdoc also passed. The collector now retains the full feasibility sources and
manifest, native support crate and proc-macro sources with the compiled bridge.

Final installed evidence on 16 November 2026 is retained under
`target/evidence/provider-link/`: `attempt.ZLbeAW` passed 14 artifact and three
record checks, `attempt.hWvl1N` passed 14 artifact and four checkpoint checks, and
`attempt.kywkpT` passed 14 artifact and seven job checks (plus nine host job tests
in the gate). These cohorts use the complete source capture described above;
earlier passing derive runs `9TUtpF`, `Kqi7xc` and `yhxZGN` used the older partial
feasibility source capture. All packages remain unpublished and experimental.

## Checked scalar-width conversion

The derive now accepts u32 as well as u64 fields, still with a maximum of sixteen
fields. Its metadata identifies each field's scalar kind as well as ID and native
offset. Generated getters return the declared primitive type. The logical codec
concatenates four or eight little-endian bytes in field-ID order, never native
padding; total payload size remains bounded by 128 bytes. Other types, including
signed integers, pointer-sized integers and bool, remain outside the subset.

The [narrowing probe](src/record/narrow/mod.rs) introduces a logical V3 declaration:
counter is u32, ceiling remains u64, and counter ≤ ceiling still applies. On the
tested host, V2 and V3 both occupy 16 bytes aligned to eight, with offsets zero
and eight. Their scalar descriptors differ, and V3's logical payload is 12 bytes
instead of 16. This illustrates why matching record sizes/offsets alone cannot
admit a changed representation.

`TryFrom<V2> for V3` performs checked u64→u32 conversion and refuses counters
above u32::MAX before constructing a target. `From<V3> for V2` widens exactly;
neither conversion casts native storage or changes the ceiling. V3's checked
addition distinguishes u32 overflow from the application ceiling and preserves
the record on refusal. Its generated codec validates the same invariant.

Host tests cover zero and both width limits, range refusal without altering the
source, exact widening, independently assembled 12-byte payloads, source-scope
completion and fresh reconstruction, continued scoped mutation, invalid length
and invariant refusal, and reordered mixed-width fields. A compile-fail case
ensures aliasing `u32` to `u64` cannot conceal a different field representation.

This host proof is now extended by the installed V3 profile below. Envelope
version 3 continues to mean record schema 2; envelope v4 explicitly selects
schema 3. No V1/V2 wire bytes or vector element types change. The earlier host
evidence alone did not establish installed publication or rollback.

On 16 November 2026, the host gate passed three macro tests, 36 library tests,
18 provider-region tests and 20 doctests, including the new width-alias refusal.
All-feature workspace Clippy, strict Rustdoc and formatting passed. Installed
V1/V2 regressions passed in `target/evidence/provider-link/attempt.pepvUj/`
(14 artifact and three record checks), `attempt.zfyXg5/` (14 artifact and four
checkpoint checks), and `attempt.fQZJkp/` (14 artifact and seven job checks,
plus nine host job tests in the gate). Those installed results protect the
existing schemas; they do not qualify retained V3 state.

## Installed narrowing and recovery

The V3 fixture admits the four-byte counter at 65600, requires zero at 65604,
and preserves the eight-byte ceiling at 65608. The provider owns these scalar
bytes; no Rust V3 reference is reconstructed over retained memory. Rust's native
padding is never copied into this reserved word. The existing vector descriptor,
allocation and custody contract remain unchanged.

The narrowing executable admits exactly schema 2 while cold. The composer first
checks the source in WAT, then resets the transient checkpoint runtime, invokes
checked `TryFrom<V2>`, and finally admits the published schema 3 before native
access. The provider independently checks the range and unchanged ceiling before
writing the four-byte counter, canonical zero word and schema. Wrong sources,
repeated conversion and direct reopening under the other schema refuse.
Arguments are two little-endian u64 words: reserved zero and fault selector
(0 normally, 1 before publication, 2 after publication).

`record_add` also accepts ready V3 state, with replies 0 for success, 1 for ceiling
refusal, 2 for u32 overflow and 3 for a delta outside u32 range. It retains the
existing two-u64 request format. The generated view still exposes a u32 counter;
the provider validates every scalar read/write. Successful conversion and updates
leave the entire vector observation unchanged.

Checkpoint envelope v4 selects schema 3 and reconstructs a fresh record/vector
after explicit heap replacement. Installed cases cover empty, spare-capacity and
full vectors, maximum representable and oversized source counters, exact trap
failpoints, preserved application ceilings, compatible V3 reopening, destination
schema refusal, wrong epochs, failed stable writes and partial construction,
relocation and return migration with continued mutation and vector reuse.

A qualification-only `damage_record_padding` endpoint deliberately writes a
nonzero reserved word. Export and compatible upgrade then refuse. A checkpoint
created before corruption is still readable after explicit heap replacement
with the unusable old hook skipped, demonstrating recovery independent of the
discarded native metadata. This is an explicit recovery test, not silent reset.
The existing test-only authority and bounded-state limitations still apply.

On 16 November 2026, the host gate passed 37 library tests, 19 provider-region
tests and 20 doctests, plus the macro and existing ownership/layout cases.
Installed evidence under `target/evidence/provider-link/` includes
`attempt.QXwHkh` (14 artifact and four record checks, covering both conversion
entrypoint orders), `attempt.GEZ4JZ` (14 artifact and four checkpoint checks), and
`attempt.tWSKkB` (14 artifact and seven job checks, plus nine host job tests in the
gate). These are qualification results for one current implementation, not a
promise to maintain pre-1.0 release branches or formats.
