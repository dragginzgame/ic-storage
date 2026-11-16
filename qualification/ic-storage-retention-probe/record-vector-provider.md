# Record-vector element admission

The closed native profile now executes the Rust V3 vector across installed
retained upgrades under the [allocation contract below](#closed-native-allocation-and-linked-entrypoint-contract).
The earlier WAT-only and compiler-only results remain separate evidence. The
[portable recovery profile](record-vector-recovery.md) now also exercises export,
heap replacement and reconstruction in another native allocation domain.

This installed WAT experiment extends the [vector custody protocol](vector-provider.md)
to a closed sixteen-element V3 record profile. It reuses claim, settlement and
upgrade ordering through checked source substitutions, adding schema admission
and field operations in [the record fixture](tests/provider_custody/vector/records/mod.rs).
It executes no Rust allocator or Vec. The [fresh host controllers](../ic-storage-native-feasibility/provider-region.md#fresh-record-controllers-on-the-host)
remain separate evidence for Rust ownership over an original live allocation.

## Retained descriptor and fields

Memory remains three fixed wasm32 pages, without data segments, a start function,
tables or growth. Metadata begins at 65536; the payload is `[131072, 131328)`.
The first seven little-endian u64 words retain the vector protocol's roles:
marker, format, base, extent, initialized count, capacity and settlement. This
profile uses marker `0x4e5650524f563033`, format 1, extent 256 and
`length <= capacity <= 16`. Old u64-vector and record-vector images reject
each other; there is no implicit conversion.

Eleven additional little-endian u64 words describe the element schema:

| Offset | Meaning | Required value |
| --- | --- | --- |
| 65592 | Logical schema ID, eight literal bytes | `RECU64V1` |
| 65600 | Application schema version | 3 |
| 65608 | Element stride | 16 |
| 65616 | Alignment | 8 |
| 65624 | Field count | 2 |
| 65632 / 65640 / 65648 | Counter field ID / offset / scalar width | 1 / 0 / 4 |
| 65656 / 65664 / 65672 | Ceiling field ID / offset / scalar width | 2 / 8 / 8 |

Scalar widths are explicit fixture tags, not Rust enum discriminants. The counter
is a little-endian u32 and ceiling a little-endian u64. Bytes 4..8 in each element
are padding: never interpreted or required to be zero. Matching outer sizes
cannot substitute for schema/version/field checks. These constants specify the
WAT profile; linking Rust still requires checking the concrete compiled V3
representation and allocation contract against them.

## Admission and mutation ordering

The provider checks the full-width header, exact schema and settled custody,
then scans only the initialized prefix for `counter <= ceiling`. No length is
narrowed or multiplied before the bounded shape check. Empty roots still require
complete metadata. Only afterward may claim clear settlement and return the
payload base. Settlement checks schema, shape and resulting initialized records
before publishing length/capacity and availability. The trusted client must have
ended its payload accesses before settlement scans.

Fresh installation writes initialized fields before publishing the header. Spare
slots deliberately contain invalid records, while padding contains nonzero bytes.
Successful admission therefore exercises both the prefix bound and exclusion of
padding. Append writes both fields before extending length. Overlapping removal
copies whole sixteen-byte slots; trim and clear only change the descriptor.
Reuse writes new fields into the claimed slot. No operation reclaims IC pages.

The invariant scan is an O(length) bounded qualification mechanism, not a
production-scale admission policy. It does not authenticate source history or
prove initialization from arbitrary metadata: closed source initialization and
writers supply that history. A corrupted shorter length could still describe a
valid prefix; numeric checks do not detect every change to application state.
A future Rust integration must supply allocation/pointer validity before
constructing references and bind its compiled accesses to this profile.

## Installed evidence and remaining gate

On 16 November 2026, both new installed cases passed on x86_64 Linux with Rust
1.100.0 and the prepared PocketIC 16.1.0 server. Strict retention Clippy passed.
The existing vector descriptor and checked-access cases also passed after their
owning Rust module moved to `vector/mod.rs`; the u64 WAT source is unchanged.
`make retention-test` selects these cases in the existing normal/CI runner;
the focused filter is `provider_custody::vector::records` within `--test retention`.

Coverage includes A→B→A retaining upgrades, empty/spare/full roots, ordered values
and continued append/removal/trim/clear/reuse, full-capacity refusal, duplicate
claims, invalid settlement, invalid records at settlement and traps after writes
or publication. Failed upgrades preserve the installed module hash, all eighteen
header words and all 256 payload bytes; update traps preserve the same observed
state. This is not a comparison of all transient memory.

Twenty-six committed metadata/field faults include every schema word, u64
length/capacity overflow, mismatched scalar width, and invalid first/last records.
Their sources omit pre-upgrade guards, independently exercising destination
refusal. Empty-root schema corruption, abandoned claims and both directions of
old-format rejection are also covered. None silently resets data.

Completed evidence is under
`target/evidence/record-vector-metadata/attempt.1QVRHj/`, including the test log,
compiler identity, source copies and digest-named Wasm artifacts.

This protocol-only result leaves Rust allocation/initialization authority,
private entrypoint and checked memory accesses to the later native contract below.
The existing host record envelope supplies portable-codec evidence only; installed
logical export/import after discarding the old heap remains outstanding. This WAT
result alone does not qualify native record-vector upgrades or a production provider.

## Rust compiler and access rewrite

The subsequent [Rust entrypoint](../ic-storage-native-feasibility/tests/fixtures/provider_record_vector.rs)
now compiles the same V3 declaration and Store derive used in the host tests.
The feasibility library builds without std on wasm32; no duplicate record type
or Rust source-text injection is used. Const checks bind the schema ID/version,
field IDs/offsets, sixteen-byte stride, eight-byte alignment and twelve-byte
logical payload to this compiler build. Its source supplies the scalar kinds.
The shared controller owns allocation issuance/release, while a shared fixture
runtime refuses Global allocation and traps rather than unwinding.

The private entrypoint accepts only the trusted descriptor and scalar operation
arguments. Eight closed operations exercise mutation, reserve/append, removal,
shrink, clear, destruction/reuse, a business error after mutation/try-append,
and a forgotten scalar view. Checked V3 updates preserve the ceiling invariant;
unexpected input/allocation failures trap. Operation 6 finalizes preceding writes
even after a normal error, including full-capacity reserve refusal. Only packed
length/capacity leaves the entrypoint; no owning header or pointer is retained.

This raw record fixture uses amortized `try_reserve`, whereas the scalar core
uses exact reservation. Under its pinned compiler, a vector at length/capacity
nine requests eighteen slots when appending and therefore refuses the bounded
sixteen-slot region. Physical unused slots alone do not guarantee successful
reservation. The [mixed record-job histories](../ic-storage-native-feasibility/migration-job.md#mixed-record-job-histories)
cover this early refusal, preserved preceding writes on a business error, and
whole-message rollback on traps. This growth policy is a fixture/compiler
observation, not a promised general application allocation policy.
This is source-level lifecycle logic, not installed execution evidence.

The raw artifact has two defined functions, no data bytes and one unresolved
foreign claim import. Its 64-byte peak stack frame stays within `[32704,32768)`;
the existing stack checker establishes balanced restoration and an acyclic call
graph with two direct edges. It grants no payload allocation authority.

The access rewriter has a separate closed record profile; the scalar and u64
profiles refuse the record artifact. It rewrites all 58 memory sites through
the existing seven helper signatures:

| Operation | Sites |
| --- | --- |
| i32 / byte / i64 loads | 22 / 1 / 3 |
| i32 / byte / i64 stores | 26 / 2 / 3 |
| Overlapping copy | 1 |

The newly admitted numeric operations are unsigned i32 less-than, i64
less-than/greater-than/greater-or-equal and i64-to-i32 wrapping. They introduce
no memory or control effects. Counterexample fixtures verify their preservation
and continued refusal of growth, loops, unhandled memory operations and data
initialization. Independent source/rewrite inventories check every guarded site.
The standalone rewrite's helper imports are deliberately unresolved: **the
existing u64 payload windows must not be used for sixteen-byte records**. The
subsequent linked profile below supplies record-specific windows, trusted
entrypoint order and the foreign allocation contract. Installed portable recovery
remains separate from both compiler and direct-reopening evidence.

On 16 November 2026, `make native-feasibility-test native-provider-probe` passed,
including strict Clippy, 24 provider-region tests, 76 live-arena tests and sixteen
artifact checks. The existing compiler-layout input test remains deliberately
ignored by the host suite. Evidence is in
`target/evidence/provider-link/attempt.duptsY/` after formatting the raw fixtures
(the Wasm digests are unchanged from `attempt.1rcimo/`). The collector includes both raw
entrypoints, shared runtime, controller, actual record/derive sources, Cargo's
dependency-artifact output and the operator inventory. Cargo's successful JSON
artifact messages supply compiler search paths, including its per-package build
directories; checks neither fetch dependencies nor install tools.

The 2024-byte raw record Wasm has SHA-256
`9dbb2bf69a2e1241198dab473ef3c5c893ba4ab3ba915a73a259ed0329d12e59`;
the rewritten artifact has SHA-256
`8d92d3ef6701a4e3262e6371fc8a7e42f00dffc9f62ff5d9aa1308dae65b96dd`.
Neither artifact was linked to an IC provider or executed by this diagnostic.

The directly affected installed regression gates also passed: three provider,
four scalar-record, four checkpoint and seven migration-job cases, plus nine
host job tests. Their completed cohorts under `target/evidence/provider-link/`
are `attempt.PegqE9/`, `attempt.EWn3k8/`, `attempt.3yNr82/` and `attempt.ihlgEa/`,
respectively, each with sixteen artifact checks. These exercise the existing
installed u64 vector and scalar records, not the new record-vector entrypoint.
The shared library's no_std build and runtime extraction introduce no new
installed record-vector claim. The raw Rust fixture formatting check now belongs
to `make native-feasibility-lint`, since these freestanding entries are outside
Cargo's ordinary formatting target discovery.

## Closed native allocation and linked entrypoint contract

The concrete V2 extension, its separate compiler profile, full-width recovery and
retained conversion to V3 are specified in the
[record-vector recovery contract](record-vector-recovery.md#retained-v2-to-v3-conversion).
The original V3 evidence below retains its recorded scope and identities.

The linked experiment is limited to the reviewed Rust V3 entrypoint, shared
controller/runtime, record provider and deterministic access rewrite on Rust
1.100.0 / wasm32-unknown-unknown. It extends the
[u64 foreign-allocation argument](../ic-storage-native-feasibility/vector-provider-contract.md)
only to sixteen Copy scalar records; it admits no arbitrary Rust collection.
Its protocol source is now owned by `src/vector_provider`, shared by WAT-only
and native tests rather than maintained in two implementations.

For each instance, the external WAT provider establishes one live allocation
over `[131072,131328)`, outside LLVM and disjoint from the native transient stack
`[32704,32768)`. The opaque foreign claim returns its pointer after exact header,
schema, settlement and initialized-field admission. Retained numeric addresses
do not create Rust pointers or preserve source-instance allocation identity.
Unshared fixed memory and the closed, non-reentrant helper graph prevent remapping
or concurrent access. Provider helpers service only the current native client;
the provider's own field scan runs before claim and after the Rust owner ends.

The shared V3 declaration supplies public repr(C) scalar fields, Copy destruction
and generated logical codecs. Its actual wasm32 layout is checked during
compilation. `length <= capacity <= 16` bounds the initialized prefix independently
of padding and spare bytes. Source initialization and the admitted writers supply
initialization history; metadata and invariant scanning alone cannot do so.
The unsafe claim loans this exclusive allocation to a fresh controller, which
issues the exact `16 * capacity` bytes through Allocator before reconstructing
the Vec. Empty roots use a non-dereferenced dangling header. No old Vec header,
allocator, reference or executable pointer is reopened.

Growth/shrink retain the same backing allocation. Push initializes both fields
before increasing length; removal may copy padding as uninterpreted bytes while
preserving surviving fields. Destruction runs no application Drop. Deallocation
permits controller reuse, not platform-memory reclamation. Release consumes the
sole owner and seals its controller; only packed length/capacity leaves Rust.
The fixed entrypoint allows no owner, reference, iterator or future to escape.
Forgetting its scalar view leaves the actual owner available for finalization.

All 58 compiled memory sites pass through seven resolved helpers. Stack access
is limited to the checked 64-byte frame. Payload reads are limited to the original
initialized prefix; shrink/clear/reuse cannot read payload. Writes use the current
prefix except append/business-error may initialize one extra slot (bounded at 16),
reuse may write only the first slot, and shrink/clear may not write payload.
Only removal can call the copy helper, with both ranges entirely in the initialized
payload. Full unsigned effective-address arithmetic rejects overflow, metadata
access and out-of-window spans. These machine checks cannot repair invalid Rust
allocation/provenance; the source-level argument above must hold first.

The wrapper checks schema and full-width counts before narrowing, selects the
operation, calls the private Rust entrypoint, then publishes the returned descriptor
through settlement and resets the selector. Final admission compares the complete
linked artifact against deterministic composition from the admitted raw input and
explicit profile. An independent instruction check verifies native-return-before-
settlement order and absence of raw/helper exports or unresolved foreign imports.
Only IC reply imports remain. Test fault injection is a closed profile option,
not authority to admit arbitrary WAT or executable replacements.

Ordinary operation-6 errors commit preceding writes and finalize custody. Traps
abort the complete IC transaction or upgrade, including writes made by an earlier
successfully settled operation in that same message. No unwinding, destructor
cleanup or local poison reset is attempted. Failed publication and failed upgrades
must preserve all observed metadata/payload bytes and the old module where
applicable. Successful native assignments need preserve logical fields, not a
canonical padding pattern. Installed portable record-vector recovery and larger
state remain separate gates; this experiment is not a production cutover.

## Installed native evidence

On 16 November 2026, `make native-provider-test` passed seventeen artifact checks
and six installed cases, including three new record cases. The completed cohort
is `target/evidence/provider-link/attempt.GRhIUa/`. It captures the actual raw and
linked Wasm, source/lock/compiler inputs, the shared provider builder, access
helpers and this allocation contract. The A profile's 4374-byte linked Wasm has
SHA-256 `a2edff217054b8251dab52e08e3111ed26500a478156d0f2bcc653ff52837b8b`.

The native lifecycle case executes all eight Rust operations across empty,
spare-capacity, ordinary and full roots. It checks exact logical values, reuse
after clear/destruction, capacity after shrink, normal errors retaining writes,
forgotten scalar views, full-capacity refusal and A→B→A reopening with continued
allocation. Traps after mutation, after native owner release and after a prior
settled operation roll back every observed header/payload byte. A destination
trap after reopening and mutation also restores the old module hash and state.

Twenty-seven committed header/schema/field faults use sources without exported
pre-upgrade guards, independently exercising destination rejection before native
access. They cover every schema word, incompatible scalar width, full-width count
overflow, altered geometry/custody and invalid first/last records. A count that
extends into deliberately invalid spare storage also refuses. Empty-root schema
corruption, abandoned custody and both directions of u64/record-format confusion
are tested separately. These cases do not claim detection of arbitrary corruption
that still describes valid values and a valid initialized prefix.

The guard case executes three positive WAT probes and eighteen rejection probes
against the same helper source used in native linkage. It checks exact stack
bounds, last-record field access, overlapping removal, unsigned offset overflow,
metadata access, unclaimed/empty/spare reads, append/full-region writes, invalid
operations and copying from stack or beyond the initialized prefix. Failed probes
preserve every observed header/payload byte. This helper test is deliberately
separate from the Rust lifecycle test.

`make retention-test` passed all seven WAT/retention cases after provider-source
consolidation. Strict Clippy for the shared library and both test runners,
strict Rustdoc and formatting passed. Directly affected checkpoint callers also
passed: four scalar-record cases in `attempt.VpGK2T/`, four native-checkpoint
cases in `attempt.xX9TJQ/`, and nine host plus seven installed migration-job cases
in `attempt.PlAXKJ/`, each with seventeen artifact checks. All named cohorts are
under `target/evidence/provider-link/` and have completed receipts.

These are bounded local PocketIC results on x86_64 Linux, not production-subnet
or native macOS execution evidence. The next implementation gate is installed
record-vector logical export/import independent of the discarded native heap;
that subsequent gate is now covered by the [recovery profile](record-vector-recovery.md).
