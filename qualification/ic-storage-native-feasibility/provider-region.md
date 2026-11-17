# Transient allocator over a provider loan

This private single-vector experiment bridges the existing live-Box transfer
tests and the fixed-scalar foreign provider. It does not extend the installed
scalar admission profile. Installed vector use has its own narrow
[foreign-allocation contract](vector-provider-contract.md).

## Allocation and ownership argument

The controller accepts one exclusive, live backing allocation through an unsafe
constructor. A pointer, numeric extent and descriptor are insufficient: the
caller must establish provenance, initialization and custody for the entire loan.
Host tests supply those facts through an exclusive borrow of initialized storage.
The Wasm fixture now has an admitted provider and bounded startup/artifact
contract extending [the scalar argument](scalar-provider-contract.md).

The controller does not own a Box, Global allocation or retained Rust reference.
Its pointer, lifetime marker and Cell bookkeeping are transient. The loan is
bounded to 512 elements, with checked element alignment, length and capacity. The
closed element set is u64 plus the V3 Copy scalar record; the installed
`Region` name still selects u64. This is not blanket admission of Copy types.
It reserves
the root's exact capacity, blocks ordinary allocation, and issues that prefix
through Allocator::allocate only during restoration. Issuance performs no payload
writes. Vec::from_raw_parts_in then receives the newly allocated pointer, exact
capacity, initialized length and this allocator; no fallible operation or callback
intervenes. No private Vec representation is retained or decoded.

One block may be live at a time. Growth and shrink change its issued prefix in
place, within the same exclusively loaned backing allocation. Failed allocation
or growth leaves the current block and bytes intact. The allocator returns exact
sizes, accepts only nonzero element-sized multiples with exact element alignment,
and never unwinds. Its default
grow_zeroed cannot acquire a second block and refuses without changing the first.
Ordinary allocate_zeroed is allowed only on a free controller, never a reserved
root. These restrictions satisfy a bounded allocator, not arbitrary allocation
requests or element-type admission. [Allocator contract](https://doc.rust-lang.org/std/alloc/trait.Allocator.html).

Release consumes the vector through its public raw-parts API after checking the
issuing controller, pointer and exact block. It seals further access and returns
only length/capacity. No pointer or private header is retained. Refusal returns
the original vector; forgotten owners keep their block unavailable. Dropping a
vector deallocates its block for reuse but cannot replay the old root descriptor.
No application destructor runs because the admitted element types are Copy.

The provider must not settle or reissue the loan until the controller and all
clients have ended. This private unsafe boundary is not the safe application
scope API: source release, forgotten-loan quarantine and source/target identity
must still be enforced by the eventual provider wrapper. A copied descriptor
alone cannot authorize another claim. The host replacement test retains its
original backing allocation, so it proves no cross-instance allocation identity.
Allocator reuse does not return backing capacity or Wasm pages to the platform.

## Product keyed table on the host

The closed host test set additionally admits the product map's `repr(C)` Slot:
three initialized u64 words, with no owning fields or Drop. The
[map fixture](tests/fixtures/region/tests/map.rs) runs the public `Map` against
this same allocator, releases its slot vector, creates a fresh controller, and
readmits the table. It checks delete/reinsert after reopening and portable
reconstruction in a different backing allocation. Unlike vector in-place growth,
map rehash requires two live allocations; this one-block provider correctly
refuses growth and replacement without changing the original state.

`Map::rebuild_in` additionally builds a native candidate in a second disjoint
loan while the original map stays live. The host test checks destination
allocation refusal, simultaneous source/candidate ownership, independent
mutation, release/readmission and a portable round trip. The installed map
profile now uses the shared pool below; its [contract](native-map-provider.md)
owns two-root admission and publication within one exclusive allocation loan.

The streamed map importer also runs against this one-block controller. Read
errors at every phase, final-digest corruption and a source panic discard the
private candidate. A subsequent successful import reuses the same destination
loan, while the original source map remains unchanged. This establishes host
candidate reclamation, not durable checkpoint custody or installed recovery.

These one-block recovery cases are host-only. The [product map contract](../../docs/design/native-map.md)
owns layout, hash placement, portable encoding and sustained-load evidence.

## Multiple-root host pool

The private [pool controller](tests/fixtures/region/pool/mod.rs) extends the same
exclusive provider-loan model to disjoint allocation runs. The earlier live-Box
arena remains historical host evidence; this experiment follows the selected
foreign-loan direction used by installed collections. Product libraries still
forbid unsafe code, and the pool is not an application-facing allocator yet.

The provider loans one stable aligned allocation for a closed element type.
Admission checks the total layout, every root's initialized length/capacity and
offset, arithmetic overflow, and pairwise span disjointness before issuance.
Those checks do not supply provenance or initialization: the unsafe caller must
establish both from its actual backing and source custody. Fresh construction
supplies no roots; reopening supplies only released roots from the same live
backing after all old owners end. No unknown native state is silently reset.

There are at most 32 simultaneous nonempty runs, including candidates, reserved
roots and released roots awaiting settlement. Metadata is a transient fixed-size
array, independent of backing size; no per-slot bitmap or retained allocator
header is introduced. Allocation selects a free contiguous run, so geometric
map growth can keep the source alive while building a replacement. Deallocation
frees only the exact issuing controller's live run. Adjacent freed spans can be
reused together without relocating surviving roots. Fragmentation and run-slot
exhaustion return allocation refusal even when total free bytes remain.

An unopened root remains reserved. Restoration freshly issues its exact span
through Allocator before reconstructing the public Vec header, without writing
its initialized prefix. Release validates controller identity and exact span,
consumes the Vec, then seals that span against reuse. Settlement requires every
run to be free or sealed and blocks subsequent allocation/restoration. Forgotten
live owners and unopened reservations prevent settlement. The provider still
owns complete application-root binding, counts, loan teardown and publication;
these transient host descriptors are not a new retained metadata ABI.

Six focused host tests pass for independent map growth, out-of-order reopening,
portable reconstruction after source-backing destruction, fragmentation and
capacity refusal, failed recovery reclamation, malformed/overlapping roots,
wrong-controller release, sealed-span protection, default Vec grow/shrink,
run-budget exhaustion and forgotten-owner refusal. Strict Clippy passes for the
owning `provider_region_tests` target. Ordinary CI includes these tests through
the existing native-feasibility target.

`make native-map-pool-firehose` selects the separate ignored release-profile
case: three maps containing 1,000,000, 16,384 and 8,192 records share a 96 MiB
provider allocation. They grow through the product API, release/reopen under
fresh controllers, delete/reinsert every third key, then recover all three maps
into one fresh pool after the source backing is destroyed. Every key/value and
continued mutation is checked; final drops make the entire loan reusable.
On 16 November 2026 this 1,024,576-record lifecycle passed in 0.316382 seconds on
the existing x86_64 Linux host. This is a host lifecycle timing, not sustained
IC throughput. Portable checkpoints still occupy transient host buffers in this
pool test; the separate map firehose qualifies file-backed streamed recovery.

The same Pool implementation now also serves the installed two-map profile.
Its [Wasm contract](native-map-provider.md) qualifies foreign loan custody,
artifact/access guards, atomic multi-root publication and installed upgrades and
rollback. Its idle provider can now grow the allocation from 24 to 192 KiB,
including additional Wasm pages, before issuing a fresh loan. No Pool object or
client pointer survives that expansion. The installed workload retains 4,608
records; dynamic rehash still needs contiguous candidate space and may refuse
earlier. Pool itself never grows memory during a loan. Neither component returns
physical pages, implements compaction, admits arbitrary owning elements or
provides a production root catalog. Those remain explicit integration work.

## Fresh record controllers on the host

The private [V3 provider fixture](tests/fixtures/region/tests/records.rs) owns one
original Box of sixteen `MaybeUninit<V3>` slots. It initializes only the declared
prefix. Each operation lends this backing exclusively to a fresh typed controller,
reconstructs a Vec through allocator issuance, and consumes that owner before
publishing its new descriptor. Spare slots and native record padding need not be
initialized. The same controller implementation continues to serve the installed
u64 fixture. The later [record compiler diagnostic](../ic-storage-retention-probe/record-vector-provider.md#rust-compiler-and-access-rewrite)
also selects V3. Its subsequent [closed installed profile](../ic-storage-retention-probe/record-vector-provider.md#closed-native-allocation-and-linked-entrypoint-contract)
now admits the sixteen-element record fixture under its own allocation contract.

The provider keeps initialization authority and availability independently of
caller-supplied metadata. Exact schema ID/version, native size/alignment, generated
field metadata and length/capacity must match before the loan begins. Numeric
agreement does not establish pointer validity: the pointer comes from the original
Box under an exclusive borrow, and no previous client remains accessible.

Availability is cleared before issuance and restored only after successful owner
release and descriptor publication. Ordinary callback errors publish preceding
writes. An unwind or lost owning Vec leaves the provider unavailable; no guard
destructor can clear that state. The callback is trusted private fixture code,
not an application-facing collection API. Its higher-ranked signature prevents
returning a controller-bound owner; losing one inside the callback is tested
separately from forgetting a non-owning scalar view.

Five cases cover empty/spare/full roots, fresh controller replacement, in-place
growth/refusal, removal/shrink/reuse, schema and initialized-length refusal,
ordinary errors, forgotten views, lost owners and caught unwinds. Generated
logical field payloads reconstruct fresh storage after the old provider is
discarded, then mutation/allocation continue. This last case tests payload
reconstruction, not a new checkpoint envelope or governed provider; the existing
[record-vector envelope](records.md#fresh-record-vector-elements) owns framing,
integrity and epoch checks.

On 16 November 2026, `make native-feasibility-test` passed with 24 provider-region
tests, including these five cases, and strict Clippy. This proves fresh controller
issuance over an original live host allocation. It does not establish retained
record-vector admission or pointer validity after an IC executable replacement.

The shared-controller regression also passed `make native-provider-test
native-record-test native-checkpoint-test native-job-test`: fourteen artifact
checks per cohort, three installed provider cases, four record cases, four
checkpoint cases, and seven installed migration-job cases (plus nine host job
tests). Completed cohorts are `target/evidence/provider-link/attempt.tvXMZx/`,
`attempt.O62a8M/`, `attempt.geLbkF/` and `attempt.iCStnH/`, respectively. These
exercise the existing installed u64 vector and scalar records, not V3 vector
elements. The source collector now captures the controller at `region/mod.rs`.

## Validation and next boundary

`make native-feasibility-test` selects the host tests alongside existing ownership,
transfer and portable-checkpoint tests. They cover fresh issuance without payload
writes, controller replacement, repeated restoration, reserved/duplicate allocation,
malformed roots, empty/cleared roots, wrong-controller release, forgotten owners,
growth failure, shrink/removal, zeroing and reuse.

On 15 November 2026 the eight new host tests passed on x86_64 Linux with Rust
1.100.0. The complete native-feasibility target passed 100 tests (including
doctests), with its existing compiler-layout-input test intentionally ignored by
the ordinary test invocation. Strict Clippy passed. Spare-capacity coverage also
uses genuinely uninitialized slots and reads only the admitted initialized prefix.

`make native-provider-probe` now also compiles
[provider_vector.rs](tests/fixtures/provider_vector.rs) using the same controller.
It exercises compilation of reconstruction, mutation, append, removal, shrink,
clear and reuse through ordinary Vec operations. Linking alloc requires a Global
allocator even though the vector explicitly selects Region; the fixture supplies
one that refuses every request and traps on invalid deallocation. It grants no
default-heap allocation or growth authority.

The diagnostic reserves a 32 KiB transient stack, fixes memory at three pages,
and exports compiler boundaries for inspection. The successful raw cohort is
`target/evidence/provider-link/attempt.Y75Ei5/vector/`. Its 1628-byte Wasm has
SHA-256 `3d40321d831bd7cc82aba77a6e16e97b44399e095eeefba6dab7f9eeda7d20af`:
two defined functions, no data bytes, one unresolved diagnostic provider import,
stack/data/heap boundaries at 32768, two global writes and one memory.copy.
Optimized IR retains the foreign pointer call, transient controller/header stack
storage and the removal memmove. Empty vectors use std's non-dereferenced dangling
pointer; that is not a retained-region pointer-creation mechanism.

Four artifact diagnostic tests passed, including refusal of this vector module
by the exact scalar admission profile. The inventory checks bounds, imports,
startup/growth exclusions and selected compiler structure; it does not establish
every load/store address or stack bound on every path. **This cohort neither linked
nor executed vector Wasm.** No private layout or installed-vector qualification
follows from compilation, the reported artifact size or the host lifecycle tests.

## Host poisoning and explicit replacement

The private [provider-host adapter](tests/fixtures/region/tests/host/mod.rs) connects
the same `Region` controller and `ic-storage::with_vector` used by the
installed fixture to a native host with 16 inline u64 slots. This is separate
from the earlier Global/arena ownership model; it is not a new product API.
The host owns its backing, descriptor, scalar counter and poison bit. It derives
each loan pointer from an exclusive borrow of the real array. No pointer, std
header or controller survives a scope, so moving an idle host does not move a
live client allocation. The higher-ranked callback receives only the scoped
scalar view and may return transient copies.

The host marks itself poisoned before admission, fresh allocation issuance or
callback execution. Successful owner release supplies the next descriptor;
publication then clears poison. Ordinary callback errors are return values and
still publish completed writes, including growth and full-capacity refusal.
Forgetting the non-owning view leaves the actual Vec with the host and does not
prevent successful finalization. No guard destructor clears the poison bit.

An unwind escaping the callback leaves poison set. Catching it outside the host
does not roll back memory: tests observe earlier scalar writes and appended data
while the published descriptor remains unchanged. Subsequent updates and exports
refuse before invoking a callback or issuing another allocation. An admission
failure also leaves the host poisoned. This policy does not detect a panic caught
inside application code, recover a process abort or repair an unsafe allocator
contract violation.

Finalization failpoints after release and after descriptor publication also leave
poison set. Both run after a callback mutates/grows the vector and returns an
ordinary error, with and without forgetting its view. Before publication, the
old descriptor remains; after publication, the new descriptor remains. Neither
permits access or export. Tests observe the actual prior writes, preserve poison
through invalid/failed recovery, and resume allocation only after successful
checkpoint replacement. These failpoints return an injected lifecycle error;
they do not simulate allocator corruption or recover a released native owner.

Recovery is an explicit logical replacement using caller-held checkpoint bytes.
Decode checks the existing wire contract and expected epoch, then enforces this
host's smaller capacity. A private candidate reconstructs its vector through
fresh controller issuance. Only complete construction replaces the old host;
there is no reset-poison method and no reopening of the poisoned descriptor.
The source contains only inline u64 storage and transient metadata, so discarding
it invokes no stale native owner or application destructor. Recovery can discard
successful changes newer than the saved checkpoint; it is not automatic rollback.

Fault injection after one value, after the final value and before publication
preserves the old host's complete backing, descriptor and availability state.
Both poisoned and available sources are covered. Corrupt bytes, wrong epochs
and a valid 17-element checkpoint refuse unchanged. Successful empty/full
replacements restore the scalar counter and support subsequent native allocation,
clear, shrink and reuse. These are construction failpoints, not simulated proof
of an operating-system allocation failure.

On 16 November 2026, all fourteen provider-region tests passed on x86_64 Linux
with Rust 1.100.0, including five provider-host tests, with strict Clippy. The
native-feasibility and scoped-core gates also passed; the core retains its eight
ownership compile-fail checks. Fourteen artifact checks passed in
`target/evidence/provider-link/attempt.qAFEnf/` before adding the installed
finalization failpoint; its raw vector and linked A Wasm were byte-identical to
the 0.1.11 artifacts. The later [installed finalization evidence](../ic-storage-retention-probe/vector-link.md#finalization-failure-evidence-0112)
changes the WAT wrapper only; raw Rust remains unchanged. The host adapter is
excluded from that Wasm build. `make native-feasibility-test` selects these tests
through `provider_region_tests`. This establishes in-process unwind/replacement
behavior only, not durable checkpoint custody, host-process restart, an installed
IC upgrade, or macOS execution. The installed IC path continues to use trap
rollback under its separately recorded evidence.

## Transient stack and direct-call proof

The 0.1.7 diagnostic adds a deliberately narrow control-flow check to the actual
vector Wasm. Only the exported entry may change the stack global. Its entry
sequence reserves one positive, aligned constant frame in a protected Wasm local;
that local cannot be reassigned. The only later stack write must restore exactly
that amount from the saved local. The normal return follows restoration through
a straight-line sequence without calls, effects or control-flow entry points.
All other entry exits must trap. Local block branches may not target the implicit
function label and bypass restoration.

Helpers cannot read or change the stack global. The complete direct-call graph
must be acyclic; indirect/tail calls, loops, exceptions, growth and unrecognized
operators refuse. The foreign provider remains a conditional non-reentrant leaf:
its future linked implementation must establish that assumption. This check
starts from validated Wasm and the inspected import/export/global roster.

Nine diagnostics passed on x86_64 Linux with Rust 1.100.0 on 15 November 2026,
including five tests using valid Wasm counterexamples for stack mutation,
restoration mismatch, early returns, branch escapes, recursion and unsupported
control. The actual vector reports a 64-byte frame, stack-pointer floor 32704
from initial 32768, and two direct helper-call sites. The raw vector digest is
unchanged from the cohort above. New raw evidence and the full operator listing
are under `target/evidence/provider-link/attempt.YrVrf1/`.

This proves the admitted shape's **stack-pointer discipline**, not that every
load/store falls inside its frame or the provider payload. Helper pointer
arguments, initialized vector span, memory.copy bounds, complete retained metadata
and the final linked provider remain independent admission obligations. The
vector still cannot pass scalar admission; that cohort did not execute it.

## Installed boundary and remaining recovery

The [installed vector metadata protocol](../ic-storage-retention-probe/vector-provider.md)
now checks full-width length/capacity before narrowing, refuses invalid source
headers and publishes replacement metadata only through settlement. Its WAT-only
tests compare the entire header and payload after rollback. The accompanying
source-level access audit records the Rust candidate's initialized-span and copy
obligations; it does not yet bind all compiled accesses or execute that candidate.

Installed use now combines final vector memory-access and entrypoint checks with
the explicit allocation/initialization argument. The scalar combiner still rejects
additional code. Numeric descriptor checks alone do not establish backing custody
or initialization. The portable scalar/vector format remains a separate required
recovery contract. Its new 512-element envelope and governed provider are tested,
and the separate [native bridge](../ic-storage-retention-probe/native-checkpoint.md)
now connects retained values to fresh reconstruction in a relocated domain.
The [checked-access experiment](../ic-storage-retention-probe/vector-access.md)
now routes all 49 compiled memory operations through unresolved helper imports.
Installed WAT tests exercise the helpers' operation-specific access windows;
the intermediate rewrite remains unresolved. The subsequent
[link experiment](../ic-storage-retention-probe/vector-link.md) binds those imports
and checks trusted entrypoint ordering. Its subsequent installed lifecycle now
executes the bounded native vector under the separate allocation contract;
general runtime migration remains outside that original lifecycle fixture.
