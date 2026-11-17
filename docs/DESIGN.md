# Architecture

Status: selected research direction, revised 17 November 2026. Native retained
collections with explicit application-type conversion and a portable migration
path for incompatible runtimes. This supersedes the
[M0 research plan](design/m0-research-plan.md); it is not a production contract,
a completed implementation or authorization to migrate an existing consumer.

## 1 Decision and implementation status

Pursue Nyxus-style orthogonal persistence: application data lives in native Rust
records and collections allocated by an explicitly retained allocator. Compatible
upgrades retain those allocations without serializing the complete graph.
Selected collections should expose familiar Rust operations through scoped
access, while ordinary Global allocations and execution state remain transient.

**Upgrade flexibility is a foundational requirement.** Support three distinct
paths rather than assuming every future binary can reopen the same native heap:

| Change | Selected path |
| --- | --- |
| Application code changes; physical and interpretation contracts remain compatible | Admit and reopen retained state directly |
| Application schema changes under a compatible runtime | Explicit versioned type conversion; move compatible fields and rebuild changed representations |
| Native collection layout, allocator, compiler/SDK contract or memory target becomes incompatible | Export logical state through a compatible source executable, then reconstruct a fresh target heap from a portable checkpoint |

Native access and the portable checkpoint serve different purposes. The
checkpoint is an exceptional migration/recovery representation, not a second
authoritative copy maintained on every ordinary update. Routine compatible
upgrades must not acquire a whole-graph save/load requirement.

The initial product scope supports one explicit runtime contract. Direct reopening
is qualified within that contract; incompatible compiler, allocator or heap
changes use a planned migration procedure that may require downtime. Universal
compiler portability and automatic compatibility discovery are not prerequisites
for useful records, type declarations and application migrations. This narrows
the compatibility promise without changing memory-safety obligations.

Before 1.0, develop one current implementation; do not maintain historical product
versions or promise compatibility across experimental releases. V1/V2/V3 labels
in qualification fixtures identify application schemas used to test conversion,
refusal and recovery, not supported release lines. Keep predecessor declarations
only where an active qualification case needs them, and preserve historical
evidence. Do not add adapters solely to support earlier pre-1.0 releases.
Experimental APIs and formats may change; unknown or incompatible retained data
must still be rejected rather than silently reset or misinterpreted.

The private `ic-storage-m0` crate preserves M0, a safe, byte-defined record model
with a fixture-only live-memory kernel. Separate qualification fixtures exercise
bounded native-vector allocation, compatible upgrades and portable recovery.
They do not implement the general native runtime, collection admission or
application converters described here.
The public [`ic-storage`](../crates/ic-storage/README.md) crate
now supplies a safe, reusable scoped scalar-vector interface used by the installed
provider fixture. It accepts an already valid live Vec under the caller's
allocator authority; it does not admit retained bytes or expose an owning carrier.
Its optional `map` feature now implements a reusable allocator-owned u64-key/u64-value
hash table, scoped access, poisoning and streaming portable export/recovery.
Its synchronous reader path uses a bounded record buffer and validates a private
candidate before returning it; a million-record file-backed host round trip
avoids holding the complete checkpoint in memory. This does not implement a
durable import job. Bounded installed map recovery is now qualified separately below.
The [map contract](design/native-map.md) records its controlled slot representation,
host region-controller admission, sustained public-API workload and integration
limits. A closed installed profile now exercises two native maps within one
provider pool, independent growth, compatible upgrades, custody and atomic
multi-root publication rollback. The idle provider grows the loan from 24 to
192 KiB, adding Wasm pages only after every Rust owner has ended. Installed tests
retain 4,608 records, check rollback of page count as well as bytes, and exercise
actual memory.grow refusal. Rehash still needs contiguous candidate space beside
both source roots and can refuse earlier. The same private pool has host evidence
for several maps,
fresh-controller reopening and portable recovery of over one million records.
A separate governed checkpoint profile now freezes and exports two maps of up
to 64 records each, then reconstructs them after whole-heap replacement in another
allocation domain, including explicit recovery after corrupt native metadata.
A distinct durable map-job profile now adds controller-authorized export,
sequential epochs, cancellation, frozen resumption and single-use recovery. Bounded
map imports now span messages with prefix validation, exact retries, compatible
interruption and explicit restart of unfinished imports from the checkpoint.
The paged adapter now commits retained native writes, verifier continuations and
durable progress in one installed IC message, with compatible interruption and
explicit restart. Its current profile admits 3,072 first-root and 1,536 second-root
records in native format 11, with one page per batch, source cancellation and
sequential export/recovery epochs.
The 72-page recovery grows the private target from 24 to 192 KiB only between
ended native loans. Traps roll back growth, records and durable progress together.
Disjoint native windows preserve occupied grown tails across checkpoint resets
and upgrades. Applying each page in one admitted scope reduces the worst measured
batch from 1.41 billion to about 102 million instructions. Separate checkpoint-heap
instrumentation now measures requested live bytes, with all sampled calls returning
to zero. Native-pool rehash peaks, allocator-internal overhead, consumer-facing root
custody and general record values remain incomplete.
These are shared native collection components, not a complete Store API.
Keep the existing M0 format, tests and evidence; do not reinterpret those bytes
as native Rust values or expand M0 into a competing collection architecture.
This direction does not select an upstream dependency or adoption of cdk-rsx
code.

The [delivery roadmap](#delivery-roadmap) owns current priorities and completion
gates, including the sibling consumer requirements reviewed on 17 November.

### Successor rule

This remains the intended successor to ic-storage, aiming to subsume the
persistence capabilities our applications need, including capabilities served
by ic-stable-structures. Develop one product architecture. The sibling
ic-storage-legacy repository remains on hold and unchanged.

Replacement means qualified capabilities and migration, not API, package or
stored-format compatibility. Consumer cutovers require separate authorization,
data/lifecycle evidence and an operational recovery procedure. Existing Storage
bytes require an explicit importer through their owning format and handles.

## 2 Design basis and provenance

### Pinned cdk-rsx-store design basis

Retain [Neutrinomic/cdk-rsx at ae5d3791361ff00a609ae9adf80e064ca08b14a4](https://github.com/Neutrinomic/cdk-rsx/tree/ae5d3791361ff00a609ae9adf80e064ca08b14a4)
as the reviewed reference, beginning with its
[Store contract](https://github.com/Neutrinomic/cdk-rsx/blob/ae5d3791361ff00a609ae9adf80e064ca08b14a4/ver1/crates/cdk-rsx-store/README.md).
The subsequent source review found allocator-parameterized native collection
wrappers, scoped module access, physical/interpretation admission and explicit
owner-local conversion. These inform the selected direction, not an assumption
that the upstream runtime is already qualified for our applications.

Nyxus's application conversion assumes an admitted kernel, allocator and compiler
class. Its [upgrade boundary](https://github.com/Neutrinomic/cdk-rsx/blob/ae5d3791361ff00a609ae9adf80e064ca08b14a4/docs/upgrades.md#kernel-and-allocator-freeze-boundary)
does not supply general native-image/kernel migration. Our required portable
checkpoint and, where necessary, resumable migration protocol extend beyond that
scope. Hylix and Minima still retain native anchors; selecting stable payloads
alone would not remove the native-runtime compatibility problem.

The [recorded Motoko and cdk-rsx consultations](design/m0-research-plan.md#motoko-implementation-consultation)
remain part of the research provenance. The later LOC exercise counted selected
source in temporary storage and inspected Nyxus allocator declarations; it was
not an upstream build, audit or benchmark. No upstream implementation has been
copied or adapted into this product. Independently author from documented
behavior and interface contracts; do not claim an unexposed clean-room context.

The earlier [Storage review](design/m0-research-plan.md#2-what-the-prior-storage-review-establishes)
remains pinned to 98eb9313b7b241607a5dd81a7a4e34aa6c04f755. It supplies requirements
for governed ownership, failure handling and consumer migration, not the new
heap implementation or release machinery.

### Platform and Rust constraints

The IC's [retention and upgrade contract](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/)
is an input to qualification. Retained linear memory does not itself retain
execution state, authorize typed access or reconstruct transient services.
Unknown data must be rejected before interpretation, never silently initialized.

[Rust layout guarantees](https://doc.rust-lang.org/reference/type-layout.html)
do not freeze native records and private standard-library collection layouts
across compilations. An outer repr(C) declaration does not freeze its nested
containers. Equal size alone does not establish equal representation or meaning.
A changed compiler fingerprint is a reason to check compatibility, not proof
that every stored layout changed.

Keep Rust 1.100.0 as this repository's compiler and support floor. Its published
documentation and the installed pinned rust-src mark
[Allocator](https://doc.rust-lang.org/std/alloc/trait.Allocator.html) and ordinary
[Vec::new_in](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.new_in)
as stable since 1.100.0; const use and other allocator extensions have separate
stability boundaries. This supplies an allocation API, not persistence admission.
[Vec's guarantees](https://doc.rust-lang.org/std/vec/struct.Vec.html#guarantees)
explicitly exclude a stable ABI and field order. These sources were checked on
15 November 2026; no native-persistence proof follows from API availability.

Establish the representation contract for the subset actually retained. Record
its layout, initialized versus unused storage, ownership and interpretation
rules within the supported runtime. Controlled layouts and public reconstruction
interfaces can avoid retaining private runtime headers entirely. Private nodes
or allocator metadata that do persist still need an explicit justification;
matching sizes, source hashes or successful upgrade tests alone are insufficient.

Universal layout certification is deferred. The referenced
[representation certificates](https://github.com/Neutrinomic/cdk-rsx/blob/ae5d3791361ff00a609ae9adf80e064ca08b14a4/ver1/crates/cdk-rsx-memory-abi/README.md)
depend on compiler-derived metadata, private-layout recipes and controlled std
builds; reproducing that infrastructure is not a prerequisite for the first
record API. An ordinary derive macro does not supply those certificates either.

If the selected toolchain cannot establish a private representation, choose a
representation we control or make a separate explicit compiler-tooling decision
before retaining it. Native standard-library collection reuse remains a
feasibility decision. Do not silently weaken admission or switch to nightly.

## 3 Native storage and ownership

Persistent storage has an explicit allocator authority, declared roots,
owner-private modules and a bounded set of admitted native data types.
Ordinary Global-owned Vec, String, Box or Rc values do not become persistent
because their headers happen to lie in retained memory.

The eventual collection surface includes retained vectors, text, ordered maps
and sets, queues, unique owners and selected shared owners. Qualify a small
record/vector subset first. Reusing standard-library algorithms through an
explicit allocator is desirable where the pinned toolchain and complete
representation contract support it; it is not a promise to wrap every container.

Each admitted type needs all three:

- A recursive data/ownership contract excluding transient allocations, borrowed
  application references, code/static addresses, closures, futures, trait-object
  vtables and active runtime resources.
- A physical and interpretation contract covering the actual concrete record,
  private collection nodes, allocator metadata, initialization rules, ownership,
  drop behavior, ordering/comparison and relevant configuration.
- A versioned logical export/import contract, with bounded decoding, ownership
  and identity preservation and an exercised round trip. No type enters
  persistent application state before its portable path is available; adding
  a container also requires contracts for its concrete elements and owning edges.

A derive marker alone proves none of these contracts. Opaque third-party types
require explicit review. Stable schema identities and frozen declarations belong
to owners; compiler TypeId values and raw addresses are not durable application IDs.

### Application type declarations

A [controlled scalar-record experiment](../qualification/ic-storage-native-feasibility/records.md)
now defines concrete V1/V2 payloads and an explicit counter/ceiling conversion.
Host and installed IC tests preserve the unchanged vector, convert under explicit
scalar metadata admission, and recover both record fields through a versioned
checkpoint. The installed profile checks failed-upgrade rollback and fresh recovery
in both allocation domains. The first scalar-only Store derive now generates metadata, bounded logical
field codecs and scoped getters for V1/V2. Checked mutation, validation and
conversion remain application policy. Host and installed fixtures exercise the
generated code, including confinement, poisoning, rollback and recovery. General
record ownership remains future work. This scalar-record profile preserves its
u64 vector; separate record-element profiles follow below. See the
[derive subset](../crates/ic-storage-derive/README.md).

The derive also supports u32 fields with explicit scalar-kind metadata and packed
logical encoding. V2→V3 counter narrowing now runs in the installed fixture,
with checked range refusal, unchanged vector storage, publication rollback and
portable recovery through checkpoint envelope v4. Its unchanged outer native
size does not hide the changed field representation. Native admission checks
V3's four-byte counter and canonical reserved word. An explicit installed
[V3→V2 widening converter](../qualification/ic-storage-native-feasibility/records.md#installed-exact-widening)
now preserves vector storage and the ceiling, validates the source before Rust
entry, and exercises publication rollback and full-width portable recovery.
A separately selected portable widening entry also restores a V3 checkpoint
directly into fresh V2 state after discarding the source heap. Its explicit
V2→V3 counterpart checks range before native construction and preserves the V2
checkpoint for full-width recovery when narrowing refuses. Ordinary recovery
continues to require matching schemas.
This closed scalar conversion does not authorize automatic downgrade or a
record-vector/job schema change.

A [fresh record-vector host experiment](../qualification/ic-storage-native-feasibility/records.md#fresh-record-vector-elements)
now exercises scalar records as allocator-backed collection elements, checked
whole-collection conversion, failed-candidate reclamation and bounded logical
recovery after source teardown. A V3-only token also checks reopening in the same
live host allocation, with exact schema/layout/initialized-length admission and
refusal preserving allocation accounting. A closed V3 specialization of the
provider controller additionally checks fresh allocator issuance over exclusively
borrowed original host backing, descriptor publication, lost-owner/unwind refusal
and logical-payload reconstruction after teardown. An
[installed WAT record-element profile](../qualification/ic-storage-retention-probe/record-vector-provider.md)
now checks exact retained schema metadata, initialized-field invariants, custody
and upgrade rollback for sixteen V3 records. It executes no Rust vector. A
[separate Rust compiler diagnostic](../qualification/ic-storage-retention-probe/record-vector-provider.md#rust-compiler-and-access-rewrite)
reuses the actual V3 declaration, checks its wasm32 representation and bounded
stack, and rewrites all 58 memory sites through access guards. Its closed native
profile now links those guards to the record provider and passes installed
mutation/reclamation, A→B→A retained upgrades, full observed rollback and corrupt
source refusal under the explicit foreign-allocation contract. Its
[portable recovery profile](../qualification/ic-storage-retention-probe/record-vector-recovery.md)
now exports the generated logical record fields through governed Storage, discards
the native heap and reconstructs in a different allocation domain, with installed
rollback and continued-allocation evidence. The portable path also performs
explicit whole-collection V2→V3 narrowing from an ordinary Rust source, validating
every predecessor before publishing any target records. Installed tests preserve
the source and checkpoint on range failures and allow explicit repair/re-export
and retry. An explicit portable V3-to-V2 widening path now reconstructs full-width
records after heap replacement, preserving ordered fields and ceilings. It checks
the complete V3 checkpoint before construction, retains schema-exact ordinary
recovery, and qualifies rollback plus continued full-width updates and recovery.
A subsequent retained V2-to-V3 profile converts under keep mode,
preserving backing, capacity and logical order. It validates the entire source
and a transient candidate before replacing non-owning scalar fields and publishing
the target schema; no old Rust owner exists during replacement. V2 has its own
full-width portable recovery when narrowing refuses. Migration authority and
broader type/scale coverage remain required before a persistent collection API
or production cutover.

Use ordinary-looking Rust records with a derive-based persistence declaration
as the intended application surface. The following syntax and collection names
are illustrative beyond the implemented scalar-only subset; owning fields and
StoredVec are not implemented by the derive or a decision to retain std Vec:

```rust,ignore
#[derive(Store)]
#[store(id = "game.player", version = 1)]
struct Player {
    id: u64,
    experience: u64,
    inventory: StoredVec<Item>,
}

#[derive(Store)]
#[store(id = "game.item", version = 1)]
struct Item {
    kind: u32,
    quantity: u64,
}
```

`StoredVec` denotes an admitted persistent owner, with its allocator and concrete
element representation included in admission. Reuse of standard-library
algorithms or storage remains subject to the layout and custody feasibility gate.
Declaring a type does not allocate it, register a persistent root or make existing
Global allocations persistent; construction and root attachment occur through
the admitted runtime's scoped ownership operations.

The derive should generate or require:

- A stable owner-assigned schema identity and version, independent of Rust type
  names, module paths and compiler TypeId. Frozen versions must not be silently
  redefined; field identities and encoding rules belong to the portable contract.
- Recursive eligibility checks for each concrete field and collection element.
  Initially reject ordinary heap owners, borrowed references, executable
  pointers, futures, runtime resources and arbitrary application Drop. An
  unchecked manual marker implementation must not bypass admission.
- Ownership metadata distinguishing scalar data from admitted owning edges,
  including the operations required to move, attach, retire and export them.
- Versioned logical export/import support composed from admitted field codecs,
  with bounded decoding and exercised round trips. Generation does not waive
  identity, ownership or completeness checks, nor imply routine serialization.
- Scoped record and collection views that expose permitted scalar operations
  and tracked owner operations without unrestricted mutable access to the
  owning record. Apply the same restrictions recursively to owning elements.

Keep transient services and caches outside stored records in the first subset;
an attribute that merely skips their serialization would not make their native
representation safe to retain. Application code should not implement unsafe
layout or pointer reconstruction to use the derive.

The derive describes the contract and connects its checks; it is not allocator
authority or permission to reinterpret arbitrary bytes. Begin with records whose
retained representation the runtime controls, using the existing scalar/vector
lifecycle and portable proof as the foundation. Implement concrete declarations
and a hand-written conversion first; derive generation should remove proven
boilerplate. It need not wait for universal private-layout certificates or an
actual compiler transition. The kernel must still establish allocation and
pointer validity before generated views access each newly admitted type.

### Borrowing and ownership boundaries

The first public API prohibits untracked retained-owner extraction, including
returning owners, storing them in outer captures and carrying them in futures.
A borrow ending does not prove an owner remained in the persistent root:
[mem::take](https://doc.rust-lang.org/std/mem/fn.take.html), mem::replace, shared
owner cloning and consuming iterators can move ownership out through safe Rust.
Consequently the API must not expose unrestricted &mut access to an owning
native collection or a record containing one, nor DerefMut/Clone/iterator escape
routes to that carrier. A lifetime on a guard around an otherwise extractable
owner is insufficient.

Expose scoped collection operations, approved scalar/element views and owned
transient copies. Element views must recursively enforce the same restriction
when an element itself owns retained allocations. Move/attach/retire operations
use explicit, frame-bound ownership tickets and accounting: a detached owner
must be attached or retired before successful frame completion, and cannot be
returned, captured outside the frame or persisted as a transient ticket. Shared
owners, when admitted later, need explicit tracked sharing with the same scope
rules. Owner-local converters must obey this boundary too; accepting old data
does not authorize an untracked native owner to escape the conversion frame.

References, guards, ownership tickets, allocator/provider loans and capturing
futures cannot escape the scope or cross await. Mutations require exclusive
access; module attachment ownership and stale-binding checks remain explicit.
Native pointers are internal to their admitted heap and must not escape as RPC
identities or checkpoint references. Familiar operations are a qualified API
subset, not a promise of unrestricted standard-library access.

Safe Rust permits [mem::forget](https://doc.rust-lang.org/std/mem/fn.forget.html).
Memory safety must not depend on a guard/ticket destructor running. Outstanding
loans and detached owners are tracked independently in the frame; finalization
must detect them before enabling later access, teardown or allocator reuse.
Unresolved obligations trap the IC execution or quarantine the native instance.
Never clear a lost loan merely to open the next frame, reclaim beneath it, or
guess ownership and run destructors. Leaks or refusal are acceptable outcomes
of deliberate forgetting; invalid aliasing or allocator use is not.

Require compile-fail/runtime adversarial cases for mem::take/replace, outer
captures, shared-owner cloning, consuming iterators, ownership captured in a
future, and forgotten guards/tickets. Positive tests must also demonstrate legal
in-frame moves, attachment and retirement. No collection wrapper is admitted
solely because ordinary borrowed references cannot escape.

For the current u64-only core, `with_vector` borrows the runtime-owned vector
and lends a higher-ranked view with scalar slices and explicit push, removal,
clear and shrink operations. The view owns no allocation and needs no destructor
to settle custody. Forgetting it leaves the original owner with the runtime.
Compile-fail tests reject view/reference/future escape, take/replace of the carrier,
cloning and consuming iteration; runtime tests cover ordinary error semantics and
allocation refusal through the real bounded provider allocator. Native claim,
publication and panic/trap policy remain the runtime caller's responsibility.
The installed caller now exercises normal callback errors and forgotten scoped
views across compatible upgrades. Earlier scalar writes and a successful append
survive a normal error; full-capacity append refusal still preserves earlier
writes. The runtime releases the actual owner and settles custody even after
the view is forgotten. A later trap rolls back its whole IC message. This remains
a closed operation selector, not arbitrary application callbacks, owning
elements, native-host poisoning or record conversion.

The closed installed fixture also injects a trap after Rust releases the owner
and returns, before the wrapper publishes its descriptor and settles custody.
Full header/payload comparisons and subsequent operations/upgrades verify that
rollback restores usable custody even when the callback returned a business Err.

A separate [provider-host fixture](../qualification/ic-storage-native-feasibility/provider-region.md#host-poisoning-and-explicit-replacement)
now connects this same scoped core and region controller to native-host failure
policy. Poison is set before admission/callback execution and cleared only after
owner release and root publication. A panic escaping the callback leaves actual
partial writes and blocks subsequent access/export even when caught outside.
Injected finalization failures after release and after descriptor publication
also keep poison set. A published root alone does not mean access is safe;
availability is restored only after the entire frame completes successfully.
Explicit checkpoint replacement builds a fresh private candidate; invalid input
or failed construction leaves the prior host intact, including its poison state.
This is a 16-element host-only policy proof with caller-held checkpoint bytes,
not durable checkpoint custody, native process restart or a new product API.

Keep allocator/kernel unsafe operations private and narrowly reviewed.
The existing M0 crate continues to forbid unsafe Rust; adopting this direction
does not remove that prohibition. A native implementation needs a separately
reviewed custody boundary, not unsafe expectations spread through application
code. Its allocation/std requirements must be stated separately from M0's
current no_std, allocation-free behavior.

### Mutation, destruction and reclamation

An access scope is not a transaction. Returning an ordinary business Err does
not undo preceding writes or suppress required successful-frame publication.
Keep these outcomes distinct:

| Outcome | Required behavior |
| --- | --- |
| Expected refusal before mutation | Return an error with the refused operation's bytes/ownership unchanged; earlier completed operations still stand |
| Ordinary callback/business Err after completed operations | Preserve those operations and finalize required publication; no implicit rollback |
| Unexpected mutation, invariant, destruction or finalization failure | Trap on IC; quarantine/poison the native owner until its host restores the complete snapshot or discards it safely |
| Failed migration after persistent conversion work begins | Fail the atomic upgrade/message through a trap; never report a normal error that commits an incomplete conversion |

Native catching/unwinding does not establish rollback or permit continued use
of a poisoned allocator. Failures in a multi-message migration undo only the
failing message; earlier progress follows the recovery protocol in section 6.

For the first subset, permit compiler-generated destruction of admitted owners
and narrowly reviewed container/allocator destruction. Arbitrary application
Drop implementations and user finalizers are excluded. Destruction may operate
only on live, initialized values under the admitted source runtime; neither
spare capacity nor stale/incompatible representations may be dropped as T.
Drop behavior and helpers belong to interpretation admission, and their failure
paths must preserve the trap/poison boundary.

Qualify growth, shrink, removal, partial construction, destruction, allocation
reuse and failed conversion, including fault injection and forgotten owners.
Report allocator-reusable bytes separately from physical memory pages: freeing
an allocation does not prove that capacity was returned to the platform or
that linear memory shrank. Any platform-capacity reclamation claim needs its own
mechanism and evidence.

Reference-counted strong cycles can leak; supporting shared ownership is not
tracing GC. The earlier M0 tracing-collector proposal is superseded, not silently
implemented. Tracing, moving compaction and arbitrary finalizers are outside the
initial native scope.

## 4 Runtime admission and compatible upgrades

Native heap representations can change because of application type evolution,
compiler/std changes, private collection implementation changes, allocator or
Store metadata revisions, or a wasm32/wasm64 transition. Interpretation can
change without a physical change, for example a new map comparator.

### Supported runtime contract

Initially qualify one configuration: the pinned Rust 1.100.0 toolchain, wasm32
target, selected dependency/features and build settings, allocator/provider
contract, retained representations and startup/ownership rules. Record these as
the supported runtime contract; this is a design policy, not a new implemented
manifest or admission API. Application schema identities remain separate.

Ordinary application releases may reopen directly when their actual retained
representations and interpretation remain compatible under that contract.
Pinning a compiler or assigning a runtime version is not proof that arbitrary
Rust layouts match between builds. For the supported subset, use explicitly
specified representations or a reviewed mechanism establishing the layouts
actually retained. Do not require certificates for transient Vec headers that
are reconstructed through public interfaces and never retained.

A change outside the supported configuration defaults to planned export/import.
Direct reopening across it requires a separately reviewed compatibility extension;
it is optional, not a commitment to prove every compiler or allocator transition
compatible. Unknown contracts refuse before typed access and never trigger an
automatic empty-state reset or destructive reconstruction.

The exact compiler-body, stack and access inventories in the current fixtures
remain binding for those experiments. They are evidence for their particular
unsafe boundary, not the intended permanent application compatibility interface.
Replacing them requires a justified runtime boundary; this policy does not
disable existing guards or relabel different artifacts as equivalent.

### Allocation and pointer validity

Layout agreement and equal numerical addresses do not establish valid Rust
allocations, pointers or references after instantiation. Before producing any
typed reference, the kernel's unsafe argument must explain how the new executable
establishes the live allocation domain and valid pointer provenance for retained
objects, with alignment, complete-span bounds, initialized valid values, lifetime
and aliasing guarantees. Cover allocator metadata and internal collection
pointers as well as the root, including zero-sized values and unused capacity.
The [Rust pointer contract](https://doc.rust-lang.org/std/ptr/index.html#safety)
is an obligation, not permission to manufacture references from matching bytes.

Specify the target/compiler assumptions and the exact pointer-creation/reopening
mechanism before implementing unsafe casts. An integer-to-pointer cast, matching
hash, byte copy, or passing native/installed test is not by itself that argument.
If a complete justification cannot be established, reject the native mechanism
and choose a supported representation or explicit compiler/runtime integration.

The current [vector provider contract](../qualification/ic-storage-native-feasibility/vector-provider-contract.md)
retains controlled metadata and u64 payload while rebuilding transient Vec and
allocator headers through public reconstruction interfaces. Its foreign provider
establishes backing ownership before fresh allocator issuance; the installed
fixtures qualify only that bounded contract. General private collection nodes
and arbitrary Rust heaps need separate admission.

Earlier live-arena, ledger, suspension and backing-transfer experiments are
[listed with their evidence](#8-evidence-and-next-implementation-gates). They
establish narrower host mechanisms; none substitutes for the provider contract
or grants authority merely from a decoded address or matching descriptor.

### Lifecycle ordering

The normal lifecycle is:

1. Establish application quiescence and drain callbacks/tasks/providers.
2. Finish old-runtime teardown of explicitly transient owners.
3. Start the admitted target without overwriting retained allocations; rebuild
   transient stack/data/statics before using newly compiled metadata.
4. Validate the bounded bootstrap and accepted source catalog without opening
   native objects or using the old allocator prematurely.
5. Open the admitted allocator and root; execute any declared schema conversion.
6. Validate the resulting state and publish the current catalog/root only on
   success, then reconstruct transient services and permit application access.

Use explicit keep mode for this path. Final artifact admission covers memory,
stack/data ranges, startup, growth, tables, lifecycle exports and required
markers. M0's fixed 8 MiB boundary and deny-all allocator are fixture choices,
not the selected native layout. A new page-domain/allocator contract must prove
that transient reconstruction and persistent allocation never overlap.

Maintain invariants during normal access rather than carrying M0's whole-image
scan into every field write. The exact dynamic checks and corruption coverage
need a safety argument and tests; build-time layout compatibility is not a
whole-heap corruption detector.

Neither wasm64 nor cross-target migration is automatically qualified by the
existing wasm32 evidence. Preserve target limits and capacity headroom as
explicit admission and deployment inputs.

## 5 Application-type migration

Maintain immutable, owner-local schema versions and explicitly supported source
versions. A converter reads the actual admitted predecessor type and constructs
the target type; unknown predecessors refuse. Compiling a historical declaration
with an incompatible new compiler does not recover the historical native layout.

For derive-declared records, preserve declarations such as PlayerV1 and PlayerV2
under the same stable schema identity with distinct versions, and register an
explicit supported conversion from V1 to V2. A version attribute alone neither
defines nor authorizes that conversion. Simple declared defaults may eventually
be generated; changes in meaning, range, ownership or indexing require explicit
policy. Converters use admitted predecessor views and tracked ownership moves,
not an unrestricted mutable owning record. A runtime incompatibility selects
portable reconstruction even when the logical application schema is unchanged.

| Change | Required treatment |
| --- | --- |
| Add a field with an explicit default | Construct the new record; move unchanged compatible owners |
| Change field width, optionality, enum cases or meaning | Explicit checked conversion, including overflow/unknown-case policy |
| Change collection element representation | Convert affected elements while preserving declared identities/ownership |
| Change key ordering, equality, hashing or index semantics | Rebuild and validate the affected index |
| Remove an owning field | Explicitly move or retire it under the admitted source contract |
| Change runtime/allocator/native collection representation incompatibly | Use the portable migration path; do not cast or drop through the target type |

### Collection structure versus application records

For a future IcyDB integration, prefer separating index structure from evolving
application records where its storage and performance contracts permit it. A
B-tree whose values are stable record IDs can retain its admitted node/key/value
representation while records behind those IDs undergo explicit type conversion.
Preserve the IDs when replacing records so unchanged index entries still locate
the converted records. This is a proposed integration boundary, not a statement
about IcyDB's current implementation or authorization to migrate it.

| Change | Potential reuse and required work |
| --- | --- |
| Non-indexed record fields change; tree stores stable record IDs | Convert records and update their admitted lookup bindings; reuse the tree if its complete contract and indexed values remain compatible |
| Record values are embedded inline in tree nodes | Changed value layout can change node layout; convert or rebuild affected tree storage unless complete compatibility is established |
| Key representation, comparator, collation or index definition changes | Convert keys and rebuild affected indexes under the new interpretation contract |
| An indexed field's value or meaning changes during record conversion | Update or rebuild affected index entries even if node layout is unchanged |
| Tree implementation changes under an otherwise compatible, admitted runtime | Require a supported structural conversion or rebuild; unchanged application schema is insufficient |
| Allocator or compiler representation becomes incompatible | Use portable reconstruction rather than directly reopening the old tree, even when its logical keys and records are unchanged |

Stable IDs are logical identities, not raw addresses or permission to reopen
old pointers. Their lookup representation and ownership rules also need their
own admission and migration contracts. Separate versions for tree structure,
record schemas and index semantics make reuse explicit; they do not make those
components independently safe by assumption. Publish converted records and
affected indexes consistently, using an atomic conversion when bounded or the
qualified resumable protocol before enabling normal queries and writes.

Converters must be deterministic, owner-scoped and bounded. No external calls,
clock/random dependencies or unrelated application effects. Preserve sharing
and application identities explicitly. A field-copy loop or an ordinary serde
round trip does not by itself preserve a shared/cyclic graph.

A small conversion can run in one qualified atomic upgrade. Include source
teardown, startup, admission, conversion, target validation, retirement and
provider flush in the instruction and memory budgets. Source and target may
coexist temporarily. No claim of constant-cost schema migration follows from
constant-size root replacement.

If the complete operation cannot fit that lifecycle, require the separately
qualified resumable protocol below. Do not silently turn a failing synchronous
conversion into a partially committed upgrade or expose partially converted
records to normal endpoints.

## 6 Portable migration and recovery

Ship an export capability with the first stateful native prototype. Preserve the
compatible source executable, toolchain/locks and artifacts needed to operate it.
Do not depend on retrofitting an exporter after losing the ability to open the
source heap.

### Planned runtime replacement

Major compiler, allocator or heap-format changes may use an operator-run procedure
with application downtime. Freeze writes, export through the retained compatible
source executable, verify an independently readable complete checkpoint, then
replace the runtime and reconstruct logical state before reopening access. Keep
the source executable and its build inputs available; a dedicated exporter may
use that old toolchain, but must itself be compatible with the source heap.

The first implementation needs a tested bounded escape route, not seamless or
automatic migration across arbitrary compiler versions. Existing tests between
incompatible allocation domains supply that bounded research evidence. A real compiler
transition needs qualification when selected; it does not block application
record work within the current contract. Maintenance procedures, chunked export,
large-state orchestration and post-cutover recovery must meet a consumer's actual
needs before deployment, rather than becoming universal early API prerequisites.
No rollback to an old checkpoint may silently discard newer application writes.

### Checkpoint contract

The checkpoint represents logical owned state, not native memory dumps, pointer
values, Rust padding, allocator free lists or opaque std headers. It needs a
versioned byte-defined envelope, explicit source schema identities, root and
object identities, bounded lengths/counts, ownership/reference kinds, integrity
coverage and an explicit completeness marker. Its exact byte format and codecs
must be specified and independently tested before use.

The first [scalar/vector checkpoint fixture](../qualification/ic-storage-native-feasibility/checkpoint-format.md)
now specifies and exercises a bounded envelope with schema/root/ownership IDs,
source epoch, completeness marker and SHA-256 coverage. Host tests destroy the
source arena before reconstructing a fresh target with reordered record fields.
This establishes a small logical round trip. The separate installed provider
experiment below exercises durability without the old heap; neither experiment
completes the native incompatible-runtime migration proof. The prototype format admits
only one counter and one unique u64 vector; it cannot encode shared/cyclic graphs.

Host handoff tests now export under exclusive source access and immediately
freeze the vector, then restore from independent checkpoint bytes after losing
the source arena and resume token. Refused imports preserve the frozen source,
checkpoint and existing target for retry. Cancellation resumes the original
allocation; subsequent writes require a new export and coordinator-selected epoch.
These fixture sequences connect the existing mechanisms without implementing
durable publication, restart authority or a complete migration coordinator.

Assign checkpoint-local object IDs and preserve aliasing/cycles with an identity
map. Define weak-edge and unreachable-resource policy explicitly. Reconstruct
object identities before connecting graph edges, and validate all required
targets and ownership before publishing roots. Stable business IDs must survive;
allocator addresses need not. Arbitrary hidden pointers make a type ineligible
until it supplies a complete export/import contract.

A source-compatible executable performs export. A target importer reads the
checkpoint using only the checkpoint contract, without dereferencing or dropping
old native objects through new layouts. A bootstrap/migration entry point must
distinguish normal reopen from explicitly authorized fresh reconstruction.
Unknown or corrupt anchors never trigger that path automatically.

### Owned checkpoint backing

`ic-storage::checkpoint` owns bounded records and fixed-width pages over an
explicitly owned memory namespace. The [backing contract](design/checkpoint-storage.md)
defines its byte format, region identities, refusal rules and transaction boundary.
This replaces the initial published 0.74.1 provider; it neither reads its stored
formats nor copies its implementation. Existing legacy checkpoints need an
explicit migration before a consumer cutover; no such migration is claimed here.
The sibling repository remains frozen and qualification has no legacy dependency.

The [checkpoint provider](../qualification/ic-storage-checkpoint-provider-probe/README.md)
binds this library to Platform stable-memory IO in fixtures that own the complete
stable namespace. Production integration needs its own namespace custody and
transaction qualification. The [native bridge](../qualification/ic-storage-retention-probe/native-checkpoint.md)
connects retained scalar/vector state to independent checkpoint storage, compatible
reopening and reconstruction in a different allocation domain. Checkpoint runtime
headers remain transient; their memory is rebuilt before entering checkpoint Rust.

The [two-map bridge](../qualification/ic-storage-native-feasibility/native-map-recovery.md)
now binds two product map checkpoints to one epoch and distinct root identities
under `native.checkpoint.maps.v1`. It freezes both maps, stages at most 64 records
each through non-reentrant scalar callbacks, and constructs fresh native maps
only after checkpoint Rust returns. Installed tests cover whole-heap replacement
in both allocation directions, full-width values, failure rollback and explicit
source-hook bypass when native metadata is corrupt. Its separate
[durable map-job profile](../qualification/ic-storage-native-feasibility/native-map-jobs.md)
shares the existing job transition owner, adding controller checks, domain-bound
epochs, cancellation, frozen-source continuity and single-use consumption in the
same transaction as native reconstruction. Its staged path imports at most 64
records per message, validates both retained prefixes against the checkpoint,
survives compatible interruptions and explicitly restarts only unfinished jobs.
Completed retries preserve subsequent application writes. Total recovery remains
bounded to 64 records per map; the larger growing-pool workload is unqualified.
Scalar copying scans each map's buckets once, while every import batch still
decodes the whole checkpoint and validates the complete committed prefix.
The checkpoint decoder returns sorted logical records directly; it does not
rebuild transient hash tables merely to stage their contents. Native prefix
verification also reads scalar records directly, using sorted checkpoint lookup
and a fixed seen bitmap instead of reconstructing temporary maps. It still
scans the complete prefix and is bounded to the installed profile's limits.
Validation and target construction remain separate admission boundaries.
The map-job adapter stores metadata and checkpoint in separate governed cells,
so progress updates write only 104 metadata bytes. The shared job digest binds
both parts, and installed rollback must cover publication between the cells.
This does not yet bound checkpoint reads or validation independently of total state.
The [sequential page codec](design/paged-map-checkpoint.md) separately qualifies
a byte-defined manifest/hash chain and fixed-size page reads through governed
host handles. It is a portable storage prerequisite, not a second retained
collection architecture. The host adapter binds both paged roots to one fixed
pair manifest and uses the shared job state machine for persisted page offsets,
exact retries and explicit restart. A byte-defined verifier continuation binds
the exact job metadata to both reader positions and private native fingerprints.
Each batch reads only its new checkpoint pages while still scanning native maps
to check those fingerprints. A shared governed store now has installed
ordinary-upgrade evidence for page/manifest and continuation/metadata publication,
rollback between writes, retries and missing/corrupt-store refusal through
4,608-record pairs. That storage probe reconstructs transient maps and does not
retain native roots. A separate bounded native profile now adopts the shared
store: it admits the stored continuation against retained prefixes, reads the
next page, applies native writes, verifies the resulting records, then commits
continuation/metadata before publishing either root. Installed traps exercise
rollback across all of those boundaries, including start/restart and compatible
interruption. Completed exact retries preserve later application writes. The
adapter captures bucket-order fingerprints directly through scalar callbacks.
Its transient expected-record list is discarded before the message ends and is
never used as cross-message recovery state. The paged native profile now uses
native format 11 with limits of 3,072 and 1,536 records, one page per batch,
source cancellation and sequential epochs. It qualifies 72-page recovery with
private-target growth from 24 to 192 KiB only after native owners end. Disjoint memory reservations
isolate both pools from checkpoint reset. Instrumented entrypoint measurements
now peak near 102 million instructions after applying each page in one admitted
scope, with fixed page I/O. Four installed epochs cover cancellation, shrinking
and growing checkpoints, reverse-domain recovery and stale retries. Live
allocation peaks and larger workloads remain open.

The [governed job](../qualification/ic-storage-native-feasibility/migration-job.md)
adds controller authority, durable epochs, consumption, pre-cutover cancellation
and frozen-source resumption. The [staged import](../qualification/ic-storage-native-feasibility/staged-import.md)
adds bounded batches, exact retries, compatible interruption and explicit restart
from the independent checkpoint. Restart replaces the unfinished heap; it cannot
replay a completed job or silently discard subsequent application writes.

The scalar/job profiles cover one counter and at most 512 u64 values. A separate
[record-vector checkpoint](../qualification/ic-storage-retention-probe/record-vector-recovery.md)
covers sixteen V2/V3 records and explicit narrowing, sharing its codec with
host recovery and its governed reader with the existing checkpoint provider.
It proves full heap replacement, portable and retained collection conversion,
full-width predecessor recovery and repeated domain migrations. A separate
[V3 record job](../qualification/ic-storage-native-feasibility/migration-job.md#record-vector-job-profile)
now shares the durable job state machine and controller adapter, covering
authority, cancellation, consumption, frozen resumption and staged import/restart
for sixteen records. Its envelope, locator and native format are distinct; this
does not automatically enroll an existing checkpoint or converted heap.
These proofs do not qualify a real
compiler transition, arbitrary graphs or production recovery authority. Each
staged batch still validates the complete bounded checkpoint; whole-transient-
domain clearing is fixture machinery, not a production cost commitment. Detailed
protocols, artifact identities and historical outcomes belong to the linked
qualification records rather than this architecture.

This repository owns the checkpoint backing format, logical envelopes, adapter,
source/target conformance fixtures and matching readers. Preserve source and
reproducible artifacts for as long as those checkpoints are supported. Historical
legacy-provider evidence remains tied to its original reader; it does not imply
compatibility with the new backing. The sibling freeze does not authorize
patching that implementation. If a supported reader cannot be maintained, block
cutover instead of guessing its format.

The adapter must locate and open a checkpoint through an admitted stable-provider
format using new transient handles, with no retained Rust handle, old native
module directory, native allocator or heap pointer needed. The frozen locator identifies
the checkpoint namespace, provider/format version and complete checkpoint; its
authorization, validation and recovery rules must be independently specified.
An ordinary schema migration may reuse a provider, but incompatible native-heap
reconstruction must prove this independent opening. Run that proof with the old
native heap unavailable to the reader, not merely with its pointers unused by
convention.

Replacing this transitional provider requires explicit checkpoint/provider
conversion while its admitted reader remains available, conformance tests and
installed recovery evidence. It is not implied by replacing the application
storage API. Stable memory is not automatically independent of software formats.
External backup may be an additional operational route, but is not assumed
implemented. The installed bounded native export/import fixtures exercise this
boundary. Extend their logical contract and recovery checks alongside new
application types; a production provider cutover remains separately qualified.

### Source-to-target protocol

Freeze application writes and drain in-flight effects before establishing the
source epoch. The source remains authoritative during export. Seal and verify
the complete checkpoint before any destructive heap transition. Bind the job
to its source identity/epoch, checkpoint integrity and authorized target.

The target creates a fresh admitted allocator and imports a candidate graph.
Validate schema, counts, references, ownership and application invariants before
publishing a complete root and enabling application writes. Retain the recovery
checkpoint until the explicit recovery-retention policy permits reclamation.
Never use reinstall as an implicit migration shortcut.

The actual IC mode, startup, old-memory disposal and provider continuity for an
incompatible-runtime transition need their own installed experiment. Ordinary
keep admission deliberately rejects that runtime; bypassing its check is not a
migration implementation. Do not invoke old destructors under an incompatible
allocator. Old-region retirement/reset is permissible only through the explicit,
validated migration protocol.

### Large migrations and interruptions

A resumable migration is a durable state machine, not one transaction spanning
messages. Its declared phases cover export, checkpoint completion, target import,
validation and publication, with bounded work and recorded progress. Persist
source epoch, job identity, object mapping and restart-safe chunk acknowledgments;
retries must not duplicate objects or ownership. Corrupt/incomplete checkpoints
and mismatched resumes refuse.

Normal application writes remain blocked until target publication. Permit only
explicitly defined administrative/progress operations and any separately proven
read path. State which executable and representation own each phase, how a crash
or failed upgrade resumes, and when cancellation can return to the source.
Cancellation after a destructive transition may require checkpoint restoration;
the old native heap is not presumed still usable.

Budget export bytes, import growth, temporary coexistence, checkpoint capacity,
progress metadata, instructions per step and expected downtime. Insufficient
space or unavailable compatible source readers blocks migration before cutover.
Restoration and cross-message interruption tests are release requirements for
this path.

### Rollback and downgrade

An IC trap rolls back its message or failed upgrade within the platform contract;
it does not undo earlier committed migration chunks or external effects.
Native poisoning is not rollback.

After successful migration, old code may no longer admit the data. Downgrade
requires a supported reverse conversion or an explicit checkpoint restore.
Restoring an older checkpoint can lose later writes and must reconcile external
business effects. A saved binary alone is not a data recovery plan.

## 7 Transient state and integration ownership

Executor state, callbacks, futures, Wakers, timers, locks, active borrow flags,
service bindings, caches and entropy state are transient unless separately
designed as durable data. Reconstruct them from declared state after admission.
Do not discover arbitrary Rust statics or retain code pointers by accident.

Applications own durable business intents, effect identities and the upgrade
drain boundary. A timeout does not establish that a remote effect ended.
Scheduling rebuilds timer demand; entropy starts under its fresh-generation
contract; metrics/logging require explicit continuity decisions. Snapshot restore
has its own external-effect and lifecycle obligations.

| Owner | Responsibility |
| --- | --- |
| Persistence runtime | Native allocation custody, roots/modules, ownership, dynamic access checks and runtime admission |
| Type/contract tooling | Supported-type closure, actual layouts, interpretation contracts and versioned conversion declarations |
| Build integration | Controlled startup, final-artifact admission, reproducible manifests and reports |
| Platform Runtime | IC bindings, message context and transient execution integration |
| Migration tooling | Portable format, locator/provider adapter, maintained qualified reader dependencies, durable progress and explicit publication/recovery |
| Application | Schema meaning, conversion policy, business invariants and external-effect quiescence |
| Host/qualification | Exact source/target artifacts, authorized mode, installed outcomes and retained evidence |

Keep product code under crates/ and probes in qualification/. No second executor,
automatic application retry system, copied Storage release machinery or parallel
Hylix-like product is selected. Tooling additions need concrete uses; do not add
empty qualification packages or speculative collection APIs.

## 8 Evidence and next implementation gates

Repository evidence covers M0, native ownership models and installed bounded
scalar/vector lifecycle and migration fixtures. Each linked record owns its
artifact identities, observed outcomes and limits; historical results do not
automatically qualify a later source revision:

| Evidence | What it establishes |
| --- | --- |
| [WAT retention probe](../qualification/README.md) | Selected IC retention and rollback behavior on local PocketIC |
| [WAT provider custody](../qualification/ic-storage-retention-probe/provider-custody.md) | Exclusive claim/settlement, destination refusal and rollback across installed upgrades; no Rust pointer validity |
| [WAT vector descriptor custody](../qualification/ic-storage-retention-probe/vector-provider.md) | Full-width length/capacity admission, descriptor publication and full provider-payload rollback across upgrades; no installed Rust vector |
| [Installed Rust scalar provider](../qualification/ic-storage-native-feasibility/provider-boundary.md#installed-scalar-lifecycle) | Explicit fixed foreign-allocation model, admitted Rust access/settlement, compatible upgrades and refusal/rollback; no native vector or allocator issuance |
| [Provider-region controller](../qualification/ic-storage-native-feasibility/provider-region.md) | Host fresh vector issuance over exclusive loaned backing, growth/shrink/reuse and refusal |
| [Installed native vector](../qualification/ic-storage-retention-probe/vector-link.md#installed-vector-lifecycle) | Bounded foreign-allocation contract, transient std headers, A→B→A operations, reclamation, descriptor refusal and rollback; no incompatible-runtime native-vector recovery or general heap admission |
| [Connected native checkpoint](../qualification/ic-storage-retention-probe/native-checkpoint.md) | Governed export and relocated recovery; explicit compatible reopening by rebuilding transient memory, continued native allocation and rollback; no actual compiler transition or production migration authority |
| [Governed migration job](../qualification/ic-storage-native-feasibility/migration-job.md) | Controller-authorized export, sequential durable epochs, allocation-domain binding, consumed-job refusal and atomic rollback; no executable authentication or protection against restoring an entire old canister snapshot |
| [Rust startup/growth probe](../qualification/ic-storage-rust-startup-probe/README.md) | Controlled transient initialization and bounded growth for the admitted wasm32 artifacts |
| [M0 graph probe](../qualification/ic-storage-rust-startup-probe/managed-graph.md) | Byte-defined sharing/cycles, allocator continuation and selected installed rollback cases |
| [Historical M0 vs Storage benchmarks](../qualification/ic-storage-simple-benchmarks/README.md#historical-m0-comparison-retired) | Archived implementation costs, including session read reuse; benchmark code retired, current comparisons use ic-stable-structures only |
| [Native ownership experiment](../qualification/ic-storage-native-feasibility/README.md) | Host-only scalar/vector confinement, tracked moves, forgotten-ticket quarantine and ordinary Err/panic behavior |
| [Compiler layout diagnostics](../qualification/ic-storage-native-feasibility/README.md#compiler-layout-diagnostics) | Stable-compiler debug metadata exposes selected private Vec header fields on host/wasm32; equal-sized reordered records are distinguished |
| [Live allocation custody](../qualification/ic-storage-native-feasibility/allocation-boundary.md) | A private host arena supports real std vectors, same-domain parking/reopening, run reuse and selected failure/forgetting cases |
| [Scoped arena ownership](../qualification/ic-storage-native-feasibility/scoped-ownership.md) | Live host allocation confinement, tracked attachment/retirement and quarantine; no retained std header |
| [Allocation ledger](../qualification/ic-storage-native-feasibility/ledger-format.md) | Versioned host allocation metadata and structural refusal; decoding grants no allocation authority |
| [Single-root suspension](../qualification/ic-storage-native-feasibility/suspension.md) | Host allocation accounting and one-use source authority; no IC allocation-domain proof |
| [Backing transfer](../qualification/ic-storage-native-feasibility/transfer.md) | Fresh controller issuance over the original live host allocation, with refusal and reclamation checks |
| [Scalar/vector checkpoint](../qualification/ic-storage-native-feasibility/checkpoint-format.md) | Bounded logical bytes, refusal cases and reconstruction after source-arena teardown; candidate failure preserves the existing host root |
| [Installed checkpoint provider](../qualification/ic-storage-checkpoint-provider-probe/README.md) | Fresh governed handles reconstruct scalar/vector state after ordinary upgrades discard Wasm memory; selected failed imports preserve the previous module, heap observations and stable bytes |
| [Native map workloads](design/native-map.md) | Million-key host operations/recovery and a separate 4,608-record installed two-root growth workload; host throughput does not establish IC throughput |
| [Paged native recovery](design/paged-map-checkpoint.md#installed-paged-native-recovery) | Installed recovery of 3,072 + 1,536 records, private 24–192 KiB growth, combined native/progress rollback, one-scope page application, cancellation and sequential epochs; live allocation peaks remain open |

These do not qualify retained private allocator metadata or standard-library
headers, arbitrary owning-element destruction, old Rust pointer retention,
arbitrary application schema conversion or an actual compiler/SDK transition.
The native portable proof uses two incompatible allocation domains within one toolchain.
M0 remains a regression/evidence baseline; it is not the new persistent ABI.
The [frozen M0 format](design/m0-layout.md) remains unchanged.

The native ownership experiment exercises the same safe frame interface with
Global and explicitly supplied live arena allocations. Its
[scoped arena tests](../qualification/ic-storage-native-feasibility/scoped-ownership.md)
cover actual run custody, quarantine, reclamation and entry after fresh native
issuance. Its std Vec is not retained or certified.
The separate live-arena and checkpoint experiments establish same-domain
reopening and a host logical round trip. The later provider fixture supplies a
separate, narrowly reviewed foreign-allocation contract and installed bounded
checkpoint recovery. These establish a scalar/vector foundation, not general
cross-instance Rust heap validity. Shared-owner cloning, escaping provider loans
and owning elements remain excluded rather than being inferred safe from those
cases. Their unresolved general admission does not block controlled scalar
records under the supported runtime contract.

The compiler diagnostic probe supplies a candidate measurement source for private
headers without a nightly compiler. Its recursive metadata observations are not
representation certificates: allocation/initialization and validity semantics,
debug-metadata fidelity, controlled std provenance and binding to the actual
optimized executable remain unresolved. Do not use a diagnostic report or its
successful collection marker to authorize retained interpretation.

### Delivery roadmap

Updated 17 November 2026 against Storage development baseline
[`9f27fcd`](https://github.com/dragginzgame/ic-storage/commit/9f27fcd741b77ff45d35a4466de2d7bf4d5ba9aa)
and the format-11 qualification recorded below. This is the single ordering of
upcoming work; collection/provider documents own their detailed contracts and
evidence. Update a gate here when its qualifying
implementation lands, linking the results and recording any remaining limit.
The gates are evidence requirements, not calendar or release promises.

The former first steps now have bounded evidence: controlled scalar declarations,
explicit field addition and width conversion, scoped vector/map operations,
compatible installed upgrades and portable reconstruction. They do not establish
general owning records or a reusable persistence runtime. The current paged
profile has sequential epochs, one 64-record page per batch and limits of 3,072
first-root and 1,536 second-root records. Private imports now expand their loan
from 24 to 192 KiB between ended native owners, within the batch transaction.

#### Consumer requirements

The following source review informs priorities; it is not consumer qualification.
The linked files were checked against the pinned local HEAD revisions. Sibling
worktrees also contain unrelated work, which this review does not certify or
modify. The inspected Canic, DB, Wasm Store and Blob Storage lockfiles selected
published legacy `ic-storage 0.74.1`; no inspected sibling uses the new native
crate. Auth's reference to `ic-storage-new` is the former name of this experiment.

| Consumer and reviewed source | Required storage behavior | Roadmap consequence |
| --- | --- | --- |
| [Canic profiles](https://github.com/dragginzgame/canic/blob/632e900485722bf87f5ca4c9464d47b985ad3339/apps/profile/canic-profile-worker/src/state/mod.rs) | ID→profile and principal→ID maps, bounded names, revisions, consistent two-root mutations | First consumer-shaped qualification in R3; no Canic cutover yet |
| [IC DB](https://github.com/dragginzgame/ic-db/blob/ab36e93fd918f550c8c2c0d112f5cd01ec98fd91/crates/ic-db/src/store/mod.rs) | Encoded records, composite keys, secondary indexes, ordered range/prefix scans, pagination and atomic batches | R4 needs ordered traversal; a larger scalar hash map alone cannot satisfy DB |
| [Auth receiver](https://github.com/dragginzgame/ic-auth/blob/a05b5ddd4099a3f64b95deb173a0c16746d98ac5/crates/ic-auth/src/receiver_memory/mod.rs) | Principal and digest keys, session/replay records, expiry indexes and coordinated accounting | R3/R4 type and index coverage, followed by preservation of Auth's no-reset and security-state contracts |
| [Wasm Store](https://github.com/dragginzgame/ic-wasm-store/blob/0701c5c8b13284d2f9c13efee6142e626a454a99/crates/ic-wasm-store/src/state/mod.rs) | Release catalog, hash-keyed artifacts/chunks, settings and coordinated reference/quota accounting | R4 ordered catalog traversal and R5 variable-length payloads/multiple-root updates |
| [Blob Storage control state](https://github.com/dragginzgame/ic-blob-storage/blob/6a369f850e5853ad3e0c41ed55c56cd904b65d31/crates/ic-blob-storage/src/local/persistence/mod.rs) | Upload/reference records and a separate callback receipt allocation; current bounded cells rewrite complete records | R3/R5 durable records and transaction composition; this is not evidence for native bulk-blob storage |
| [Scheduling checkpoints](https://github.com/dragginzgame/ic-scheduling/blob/d7f8a69caef0af274e802382d9a510999fd18a0a/crates/ic-jobs/src/checkpoint/mod.rs) | Job progress atomically bound to job identity, schedule and execution policy | R5 record adapters; timer capabilities and futures remain transient |
| [Logging history](https://github.com/dragginzgame/ic-observability/blob/ba4e4f19c046dff092f9f27abe2a180b50d86880/crates/ic-logging/src/history/mod.rs) and [Metrics restoration](https://github.com/dragginzgame/ic-observability/blob/ba4e4f19c046dff092f9f27abe2a180b50d86880/crates/ic-metrics/src/persistence/mod.rs) | Bounded append/eviction, cursor continuity and validated metric state | R5 retention/record adapters; existing heap types are not automatically retained layouts |

#### R1 — Larger installed recovery (next)

Build on the current paged job and disjoint windows. Extend the existing
qualification rather than adding another recovery owner.

Completion gates:

- Recover the existing 4,608-record two-root installed workload, including its
  asymmetric distribution, with private-target pool expansion only between
  ended native loans. Keep incomplete roots unavailable to application access.
- Exercise mixed lookup/write/delete traffic before export and after recovery;
  allocation/fragmentation refusal; traps around growth, rehash, continuation,
  progress and publication; compatible interruption, restart, corruption and
  exact retries. Failed batches preserve both native and governed state.
- Measure complete-call instructions, transient/retained memory and backing
  reads/writes for export, batches, admission and upgrades. State tested size
  and operational limits. Fixed page reads alone do not bound full-prefix scans
  or export sorting; redesign work that cannot stay within the selected budget
  before increasing the supported size.
- Use the installed native-map versus ic-stable-structures workload to guide
  admission and batching work. Keep the legacy/M0 comparison historical; do not
  add legacy Storage to this benchmark. Qualify any optimized ownership boundary
  with the existing corruption, publication and rollback checks.
- Extend the paged lifecycle to cancellation and repeated export/recovery epochs
  through the shared job state machine. Demonstrate at least two cycles with
  intervening application writes, stale-job refusal and completed retries that
  cannot overwrite newer state.

The first slice now passes: private-target growth and installed 4,608-record
recovery, including rollback, interruption, corrupt-prefix restart and newer
writes surviving consumed retries. See the
[format-11 evidence](design/paged-map-checkpoint.md#format-11-large-recovery-evidence).
The [entrypoint cost evidence](design/paged-map-checkpoint.md#complete-entrypoint-cost-evidence)
now covers export, all batches, sampled retries, native admission and post-upgrade
hooks through the large pair. [One-scope page application](design/paged-map-checkpoint.md#one-scope-page-application-evidence)
reduces the worst batch from 1.41 billion to 101,728,323 instructions and the
72-batch total from 51.07 billion to 3.57 billion. Full validation, transactional
publication, 6,656-byte reads and 616-byte writes remain.
[Cancellation and repeated epochs](design/paged-map-checkpoint.md#cancellation-and-repeated-epoch-evidence)
now pass across four installed epochs, including three recoveries, intervening
writes, reverse-domain reconstruction, stale-job refusal and replacement rollback.
Next qualify installed native-pool rehash peaks and allocator-internal overhead before
increasing size.
The fixture gates sampled calls at 150 million instructions, 2 MiB Wasm memory and
50 stable pages. [Repeated lifecycle cost evidence](design/paged-map-checkpoint.md#repeated-lifecycle-cost-evidence)
now covers cancellation, replacement, reverse recovery and checkpoint trimming
and regrowth across five epochs per shape. The largest replacement is about
50 million instructions, cancellation about 23 million, and reverse batches
peak near 102 million; all pass the same fixture budgets without stable growth.
[Checkpoint allocation evidence](design/paged-map-checkpoint.md#checkpoint-allocation-evidence)
now covers 119 installed samples: the largest requested-byte peak is 197,841,
and every sampled call returns to zero live checkpoint allocations. A separate
realloc-overlap bound keeps those measurements honest about System internals.
[Host native-pool measurements](design/paged-map-checkpoint.md#host-native-pool-peak-evidence)
now include old/new rehash coexistence: the large pair peaks at 172,032 occupied
bytes versus 147,456 retained bytes in its 196,608-byte pool. Reset/regrowth
reproduces the peak. These observations use the shared allocator in host tests;
installed native-pool peaks, System metadata/rounding/fragmentation,
failed-call/whole management-upgrade costs and application service budgets remain open.
The earlier standalone growth profile owns fragmentation/refusal evidence.
Larger workloads follow measured limits; a host million-key result is not an
installed acceptance substitute.

The [two-way map benchmark](design/native-map-comparison.md) supplies 324
state-checked installed samples and now 54 isolated admission samples. The first
cohort exposed repeated provider checks on each access and owner reconstruction
for each operation. One admitted loan and one pair of owners now serve a whole
batch: at 1,536 records, 64-record replacement/deletion/reinsertion costs less
than StableBTreeMap; lookup costs 1.115× and growing insertion 1.256×. Single
lookup still costs 4.05×, almost entirely owner admission. These measure the
guarded fixture, not intrinsic memory-access speeds. Next qualify reusable
ownership between ordinary messages, with explicit close/revalidation boundaries,
alongside the remaining R1 memory measurements before increasing size.

#### R2 — Reusable application ownership and lifecycle (started)

The benchmark makes owner lifetime a concrete priority: retain admitted owners
across ordinary messages only under an explicit runtime custody contract. Keep
private Rust headers transient across upgrades; close or invalidate ownership
before incompatible reopening, provider-controlled relocation or external writes,
and qualify fresh admission at those boundaries. Do not replace table validation
with a caller-supplied trusted flag or an unchecked application constructor.

The first [host session qualification](../qualification/ic-storage-native-feasibility/native-map-provider.md#host-owner-session-lifetime)
now holds two actual map owners and their pool loan across separate calls. It
closes both owners before moving backing, readmits on reopening, and quarantines
the entire session on a caught unwind. A separate
[installed service qualification](../qualification/ic-storage-native-feasibility/native-map-provider.md#installed-owner-session-lifetime)
now keeps owners alive between messages in a controlled transient interval. It
closes both before upgrade and readmits after startup clears that interval, with
query-write isolation and failed source/target lifecycle rollback. It removes
per-message admission in this trusted-source fixture; the original guarded
benchmark and paged recovery adapters still use scoped loans. Next move the
qualified service responsibilities into the reusable runtime, connect recovery
and pool growth at explicit close boundaries. The
[on-demand matched matrix](design/native-map-matrix.md) now compares this service
with ic-stable-structures across 972 validated samples. Native wins 142 of 144
non-control cells; single insertion across the largest tested rehash boundary
remains slower. Ordered ranges, larger values/populations and lifecycle costs
remain outside that matrix.

The [ordered owner session](design/ordered-storage.md#owner-session-evidence--17-november-2026)
now qualifies the same generation boundary for compact tree nodes and a separate
payload allocation. Ordinary messages reuse one owner; source close ends both
loans before publication, and shared startup clears private headers before fresh
target admission. Query isolation, omitted/failed close, failed target startup
and checkpoint recovery are exercised. These remain closed service fixtures;
general application runtime integration is still an R2 completion gate.

The first [public application-record increment](design/application-records.md)
addresses Canic Root/entry from
[Storage #5 — Canic retained runtime, records and composed ownership](https://github.com/dragginzgame/ic-storage/issues/5).
It reuses the existing checkpoint backing for bounded byte records, adds exclusive
backing custody and participant grants, and qualifies ordinary heap-discarding
upgrades through the public API. It does not promote the private native allocator
or add a stable-memory table architecture. Native collection custody, the profile
engine and real supplier composition remain the R2/R3/R5 completion work.

The [public native generation owner](design/native-sessions.md) now moves named
map custody, participant grants, cross-root scopes and shared poisoning into the
product library. The profile workload consumes it instead of a private ownership
wrapper. A closed installed ordered binding now gives its metadata a distinct
generation-long allocation and qualifies close/reopen, allocation refusal, late
reply rollback and compatible upgrades through public grants. Its four-root
extension now qualifies simultaneous main/source/target/job grants, complete
family recovery and target/job cleanup with source preservation. It still accepts
valid live maps under fixture-owned allocators; reusable retained bootstrap and
consumer startup integration are required before replacing the private runtime.

The [public retained runtime](design/retained-runtime.md) now owns the ordered
allocator, sealed root directory, generation grants and mandatory artifact
startup transform. A separate installed application imports this surface without
copying a kernel. It qualifies four simultaneous roots, compatible upgrades,
source/target rollback and complete portable recovery in both banks. This supplies
Storage #4's reusable bootstrap boundary. Real DB/Canic integration, broader
native types and durable job integration remain completion work. The supported
contract also documents ordinary heap growth across compatible generations and
explicit portable replacement before its fixed memory cap is exhausted.

Use the existing map/vector callers to move qualified responsibilities into
product libraries under `crates/`, with tests beside their owner.

Completion gates:

- Applications can declare named roots and allocation budgets, obtain scoped
  access, and distinguish fresh creation, compatible reopen, schema conversion
  and authorized recovery. Missing/incompatible retained state refuses.
- One runtime owns allocation custody and publication. Tests cover multiple
  roots, reclamation, forbidden escaping owners/references, ordinary errors,
  forgotten views, host poisoning and IC trap rollback.
- Existing installed fixtures consume that surface. Admission, transient service
  reconstruction and mutation/publication occur in the documented order; no
  provider loan, allocator object or executable pointer crosses its lifetime.

Do not require every future collection or a general plugin framework before the
first caller. R3's concrete requirements should guide the surface.

#### R3 — Real records and a profile qualification (started)

Select the Canic profile workload as the first concrete application-shaped case
inside this repository's qualification boundary. This does not authorize editing
or migrating Canic.

The [native profile workload](../qualification/ic-storage-native-feasibility/profile.md)
now exercises two actual ordered byte maps with IDs, revisions, bounded owner
bytes and UTF-8 display names. Host checks cover retries, ownership restrictions,
deletion/reuse, both-root application admission, explicit revision-width conversion,
source-independent portable recovery and whole-family quarantine after partial
failure. It now uses R2's public generation-local session and grants. The same
safe workload compiles for Wasm, but its retained allocation/bootstrap authority
and installed profile upgrades/rollback remain completion gates.

Completion gates:

- Admit a concrete record family covering IDs, revisions, principal/byte keys
  and bounded display names. Define representation, initialization, ownership,
  reclamation and portable codecs for every newly retained field. Existing std
  String/Vec or Principal layouts are not admitted merely by accepting the type.
- Maintain ID→profile and owner→ID roots through the consumer's create, read,
  name update, revision checks and exact retries, preserving immutable ownership.
  Duplicate-owner and foreign-owner refusals preserve invariants; installed traps
  cannot publish only one side of the relationship. Add storage-level deletion
  and reuse cases for both roots; the current profile API has no delete operation.
- Demonstrate compatible upgrades, explicit versioned record conversion,
  source-version and invalid-value refusal, allocation failure and whole-heap
  portable reconstruction of both roots, followed by further writes/deletes.
- Use the R2 public surface with representative sizes and measured budgets.
  Extend derive support only for the demonstrated field contracts; keep semantic
  conversion policy explicit.

#### R4 — Ordered collections and indexes (in progress)

Add ordered storage within the same allocation, ownership and recovery contract.
DB range traversal and Auth expiry indexes supply concrete consumers.

The first [ordered byte-storage slice](design/ordered-storage.md) now supplies
bounded keys/values in a separately owned byte arena, compact AVL nodes,
forward/reverse ranges,
prefixes, exclusive-key continuation, preflighted physical batches and portable
recovery. Host tests cover DB's 512-byte keys, 4-KiB rows, 16-KiB aggregate-job
frames and expanded row/index batches. Existing provider-pool tests cover native
settlement/readmission, allocator refusal and source-independent reconstruction.
The [closed installed profile](design/ordered-storage.md#installed-ordered-root-lifecycle)
now qualifies 64 full-size rows plus secondary entries through compatible upgrades,
row/index and descriptor rollback, and two-domain portable recovery across two
epochs. Full-width cursor keys and forward/reverse pagination are exercised.
Compact nodes and a splitting/reusing payload allocator now pass the same
installed lifecycle, with independent node/byte custody and allocation refusal.
Initialized storage for 64 full rows plus indexes falls from 594,944 to 273,408
bytes. The installed generation-owned session now removes per-message admission
and preserves both allocator owners across calls, with explicit close/reopen,
query isolation, shared controlled startup and recovery qualification.
The [paged ordered path](design/ordered-recovery.md) now adds 32-KiB bounded
pages and verifier continuations without rereading prior pages. Its host profile
reuses the shared migration state machine for restart and exact retries. The
closed installed four-root binding now verifies one page per call and commits
native writes, cursor and job progress together. It qualifies private candidates,
compatible interruption, explicit whole-heap restart, cancellation and exact
retries through two opposite-domain epochs. Export remains synchronous; the
installed adapter and allocation policy are not a reusable application runtime.
The DB operation surface now also includes lazy key/value-length preflight,
membership without payload access and discard deletion, exposed through native
session grants together with prefixes and preflighted batches. Four host cases
exercise unconsumed-row retries and main/source/target/job roles with distinct
row/job bounds. The [four-root installed profile](design/ordered-storage.md#four-root-installed-evidence--pending-0762)
now retains seventeen 16-KiB jobs with 512-byte keys and 4-KiB main/source/target
rows. It covers composed write/reply rollback, sealed family recovery in both
allocation domains, declaration/nested-checkpoint refusal and bounded target/job
cleanup followed by slot reuse. This advances Storage #4's collection and custody
evidence. Reusable retained startup is now supplied by the public runtime;
the DB engine's adoption witnesses remain open.
Fragmentation/coalescing policy, larger workloads, general application runtime
integration, incremental export and DB integration remain open;
this does not complete R4 or authorize a consumer cutover.

Completion gates:

- Specify composite/byte-key ordering, uniqueness, forward/reverse ranges,
  prefix bounds and continuation semantics under intervening writes. Retained
  node/element layouts need their own admission; std BTreeMap is not assumed safe.
- Exercise representative DB primary/secondary-index operations and Auth expiry
  insert/remove/prune workloads, including cross-root consistency and capacity
  refusal. Storage supplies collection/transaction mechanisms; DB and Auth keep
  query, authorization and business policy.
- Qualify growth, reclamation, compatible reopening, explicit conversions and
  portable recovery with the same failure and budget evidence as other roots.

#### R5 — Payloads and service adapters (queued, selected by consumer need)

Reuse R2/R3/R4 for bounded byte/chunk payloads, settings/control records,
scheduling checkpoints, metrics and append/evict logs. Prioritize an actual
consumer; do not prebuild all standard-library containers.

Each selected adapter must establish its size/count limits, ownership and
reclamation, atomic updates, cursor/retention behavior, versioned portable path
and installed recovery costs. Scheduler effects remain application-owned and
timer/executor state is reconstructed. Entropy is refreshed rather than restored
as a persistent stream. Whole-canister backup restore needs separate lifecycle
and external-effect qualification; it does not replace logical recovery evidence.

#### R6 — Authorized consumer pilot and cutover (per-consumer gate)

The first candidate is Canic profiles after R1–R3. It need not wait for unrelated
R4/R5 features. DB, Auth and other consumers add their own applicable gates.

For each separately authorized pilot:

- Inventory its actual data sizes, operations, schemas, transaction boundaries,
  upgrade/downgrade expectations and external-effect reconciliation needs.
- Supply an explicit importer for any legacy data using its admitted reader and
  governed handles. Preserve source artifacts and prove corruption refusal,
  interruption recovery and the policy for restoring a checkpoint after new
  writes. Neither package-name reuse nor matching keys proves format compatibility.
- Run the consumer's installed lifecycle and matched workload, measuring body
  and complete-call costs, memory peaks, growth/reclamation and all selected
  conversion/recovery paths. Record the supported operational envelope before
  claiming readiness or scheduling cutover.

An actual compiler/allocator/runtime replacement is qualified when selected:
incompatible direct reopening must refuse, a compatible source exporter must
remain usable, and a fresh target must reconstruct independently of the old heap.
Universal cross-compiler migration, arbitrary owning graphs and automatic
semantic conversion are not prerequisites for R1–R3 within the supported contract.

Scope admission to each concrete feature rather than blocking all application
work on general heap portability. Stop or revise an affected feature if its
retained representation cannot be justified, transient ownership cannot be
excluded, its recovery path has never restored data, or compatible source readers
cannot be maintained. Operational migration budgets become a release requirement
for the state sizes and transitions actually supported.

Production requires its own custody, type coverage, reclamation, large-state,
dependency, quiescence, corruption, migration and recovery qualification.
No arbitrary Rust persistence, automatic semantic migration, universal downgrade,
GC, compiler portability or production readiness is claimed by this design.
