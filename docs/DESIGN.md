# Architecture

Status: selected research direction, 15 November 2026. Native retained
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

This repository currently implements M0, a safe, byte-defined record model with
a fixture-only live-memory kernel. It does not implement the native allocator,
native collection admission, converters or portable checkpoint described here.
Keep the existing M0 format, tests and evidence; do not reinterpret those bytes
as native Rust values or expand M0 into a competing collection architecture.
There is no new package, upstream dependency or selected adoption of cdk-rsx
code implied by this decision.

### Successor rule

This remains the intended successor to ic-storage, aiming to subsume the
persistence capabilities our applications need, including capabilities served
by ic-stable-structures. Develop one product architecture. The sibling
ic-storage repository remains on hold and unchanged.

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

Complete layout admission is the decisive first gate. For every retained
representation, record how its complete concrete layout is measured or
authoritatively specified, including private collection nodes, allocator
metadata and initialized versus unused storage. The referenced
[representation certificates](https://github.com/Neutrinomic/cdk-rsx/blob/ae5d3791361ff00a609ae9adf80e064ca08b14a4/ver1/crates/cdk-rsx-memory-abi/README.md)
depend on compiler-derived metadata, private-layout recipes and controlled std
builds; an ordinary derive macro does not reproduce those mechanisms. Reject
unresolved private layouts. Matching sizes, source hashes or successful upgrade
tests cannot substitute for establishing the representation.

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

Use ordinary-looking Rust records with a derive-based persistence declaration
as the intended application surface. The following syntax and collection names
are illustrative, not implemented APIs or a decision to retain std Vec:

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

The derive describes the contract and connects its checks; it is not a layout
certificate, allocator authority or upgrade admission decision. Compiler tooling
or a representation we control must separately establish the complete actual
layout, including collection internals. The kernel must establish allocation
and pointer validity before generated views can access a retained value.
Implement the derive only after the small native lifecycle and portable proof
establish these mechanisms; this surface does not change the gate ordering.

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

Admit the complete supported source contract before following saved native
pointers. The contract must bind the actual target's physical layouts, ownership
and interpretation policies, allocator/kernel geometry and compiler/SDK class.
Measure nested and private representations; do not accept an outer record's
size or a compiler hash as a substitute. Any compatibility qualification must
retain the same read/write invariant and exact tested artifact provenance.
It cannot make different layouts equivalent by relabelling their digests.

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

The [live allocation boundary experiment](../qualification/ic-storage-native-feasibility/allocation-boundary.md)
now exercises one narrower mechanism: park a vector while its original arena
allocation remains live, validate a unique token against an allocation ledger,
and reconstruct a fresh std header through the public raw-parts API. It derives
pointers from the original allocation rather than integer addresses. This is
not an IC reopening argument: it does not establish a new allocation domain
after instantiation. Ephemeral std headers are a candidate alternative for this
subset, not a selected product ABI or a solution for private collection nodes.

The private [allocation-ledger image](../qualification/ic-storage-native-feasibility/ledger-format.md)
specifies fixed-width versioned metadata without relying on Rust struct layout.
It rejects active-owner export, corrupt framing and contradictory allocation
records. Decoding grants no allocation custody and never installs a live ledger;
the domain/pointer mechanism and complete root inventory remain separate gates.

The [single-root suspension fixture](../qualification/ic-storage-native-feasibility/suspension.md)
now checks complete allocation accounting for one live vector, freezes its arena
and consumes its original token exactly once on resume. Extra or forgotten
allocations refuse admission; dropping or forgetting suspension leaves the arena
unavailable for reuse. Matching image bytes cannot substitute for the token.
This is bounded host inventory evidence, not a durable root inventory or an IC
allocation-domain proof.

The next [allocation-domain experiment](design/native-allocation-domain.md)
separates backing-region validity from allocator-issued block authority. It
investigates explicit custody transfer to a fresh controller over independently
owned backing, followed by allocation issuance without overwriting initialized
payloads. A bounded [scalar provider contract](../qualification/ic-storage-native-feasibility/scalar-provider-contract.md)
now supplies the first installed experiment; allocator issuance and native vectors
still require their complete compiler/startup contract. Decoding a ledger or
declaring a foreign symbol cannot establish either authority.

The private [backing-transfer fixture](../qualification/ic-storage-native-feasibility/transfer.md)
now consumes explicit source authority, ends that controller and issues the same
span through a fresh controller over the original live Box. It preserves values
and supports reclamation while rejecting duplicate claims and failed issuance.
This supplies host controller-replacement evidence, not an IC backing provider.

Destination geometry now bounds usable allocation space independently of the
physical backing. Native spans outside that region refuse before issuance and
return original custody. Host checkpoint tests explicitly discard that refused
native source and reconstruct its logical state into a smaller fresh arena.
This connects both routes under an actual allocator-capacity constraint; it does
not qualify a different compiler, retained ABI or installed native transition.

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

### Transitional checkpoint provider

Select published ic-storage 0.74.1 as the initial legacy provider baseline behind
a narrow checkpoint adapter, subject to the installed migration proof. This is
a transitional dependency on a frozen provider contract, not resumption of
development in the sibling repository or a second product architecture. Pin its
exact package/artifact, lockfile, stable format and owned checkpoint key/locator
before the first provider-backed proof. Production access continues to use
governed Storage handles; this design does not authorize raw ungoverned stable
allocations or silently select a different provider version.

The [installed provider fixture](../qualification/ic-storage-checkpoint-provider-probe/README.md)
now pins that reader, its directory/cell formats and an owned stable key. It
exports the bounded scalar/vector checkpoint, performs ordinary upgrades that
discard the source Wasm heap, and reconstructs through new governed handles.
Local PocketIC evidence covers A→B→A, continued mutation, refused imports and
rollback after a target stable write. These are transient Global-owned vectors,
not retained native allocations: this proves independent provider opening, not
complete layout admission, direct-reopen refusal or the full native migration gate.

This repository owns the checkpoint envelope, locator/adapter specification,
source/target conformance fixtures and maintenance of the qualified reader
dependency for supported checkpoints. The legacy provider implementation and
stored-format contract remain owned by ic-storage. Preserve the matching reader,
source and reproducible artifacts for as long as those checkpoints are supported.
A required provider fix or format change is separately governed work; the sibling
freeze does not authorize patching it here. If a supported reader cannot be
maintained, block cutover instead of guessing its format.

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
implemented. The first small native export/import proof must exercise this
boundary before application converters are expanded.

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

Existing repository-recorded evidence is scoped to M0 and its fixtures. The
design review and documentation update do not independently rerun or verify
those historical results; each linked record owns its artifacts and test scope:

| Evidence | What it establishes |
| --- | --- |
| [WAT retention probe](../qualification/README.md) | Selected IC retention and rollback behavior on local PocketIC |
| [WAT provider custody](../qualification/ic-storage-retention-probe/provider-custody.md) | Exclusive claim/settlement, destination refusal and rollback across installed upgrades; no Rust pointer validity |
| [WAT vector descriptor custody](../qualification/ic-storage-retention-probe/vector-provider.md) | Full-width length/capacity admission, descriptor publication and full provider-payload rollback across upgrades; no installed Rust vector |
| [Installed Rust scalar provider](../qualification/ic-storage-native-feasibility/provider-boundary.md#installed-scalar-lifecycle) | Explicit fixed foreign-allocation model, admitted Rust access/settlement, compatible upgrades and refusal/rollback; no native vector or allocator issuance |
| [Provider-region controller](../qualification/ic-storage-native-feasibility/provider-region.md) | Host fresh vector issuance over exclusive loaned backing, growth/shrink/reuse and refusal; the shared wasm32 diagnostic is compiled but not admitted or executed |
| [Rust startup/growth probe](../qualification/ic-storage-rust-startup-probe/README.md) | Controlled transient initialization and bounded growth for the admitted wasm32 artifacts |
| [M0 graph probe](../qualification/ic-storage-rust-startup-probe/managed-graph.md) | Byte-defined sharing/cycles, allocator continuation and selected installed rollback cases |
| [M0 vs Storage benchmarks](../qualification/ic-storage-simple-benchmarks/README.md) | Current implementation instruction costs, including session read reuse |
| [Native ownership experiment](../qualification/ic-storage-native-feasibility/README.md) | Host-only scalar/vector confinement, tracked moves, forgotten-ticket quarantine and ordinary Err/panic behavior |
| [Compiler layout diagnostics](../qualification/ic-storage-native-feasibility/README.md#compiler-layout-diagnostics) | Stable-compiler debug metadata exposes selected private Vec header fields on host/wasm32; equal-sized reordered records are distinguished |
| [Live allocation custody](../qualification/ic-storage-native-feasibility/allocation-boundary.md) | A private host arena supports real std vectors, same-domain parking/reopening, run reuse and selected failure/forgetting cases |
| [Scalar/vector checkpoint](../qualification/ic-storage-native-feasibility/checkpoint-format.md) | Bounded logical bytes, refusal cases and reconstruction after source-arena teardown; candidate failure preserves the existing host root |
| [Installed checkpoint provider](../qualification/ic-storage-checkpoint-provider-probe/README.md) | Fresh governed handles reconstruct scalar/vector state after ordinary upgrades discard Wasm memory; selected failed imports preserve the previous module, heap observations and stable bytes |

These do not qualify retained native allocator metadata, standard-library
layouts across upgrades, retained destruction, native pointer retention, schema
conversion or a native incompatible-runtime transition.
M0 remains a regression/evidence baseline; it is not the new persistent ABI.
The [frozen M0 format](design/m0-layout.md) remains unchanged.

The native ownership experiment exercises the same safe frame interface with
Global and explicitly supplied live arena allocations. Its
[scoped arena tests](../qualification/ic-storage-native-feasibility/scoped-ownership.md)
cover actual run custody, quarantine, reclamation and entry after fresh native
issuance. Its std Vec is not retained or certified.
The separate live-arena and checkpoint experiments establish same-domain
reopening and a host logical round trip. The provider fixture adds installed
bounded checkpoint recovery without establishing cross-instance native allocation
validity. Gate 1 is still incomplete. Shared-owner cloning, provider
loans and owning elements
remain excluded rather than being inferred safe from the scalar-vector cases.

The compiler diagnostic probe supplies a candidate measurement source for private
headers without a nightly compiler. Its recursive metadata observations are not
representation certificates: allocation/initialization and validity semantics,
debug-metadata fidelity, controlled std provenance and binding to the actual
optimized executable remain unresolved. Do not use a diagnostic report or its
successful collection marker to authorize retained interpretation.

Proceed with bounded experiments in this order:

1. **Complete layout and custody feasibility.** Use one integer record and one
   retained vector on Rust 1.100.0. Inventory every retained representation and
   provide authoritative layout evidence, initialization and interpretation
   rules, allocation/pointer validity and the ownership-escape argument. A
   working Allocator API or matching sizeof is insufficient. Resolve private
   layouts through a supported mechanism or choose a controlled representation;
   do not build a wrapper library around an unresolved certificate. Define the
   small subset's portable contract at the same time.
2. **Compatible native lifecycle and reclamation.** Install A, populate that
   subset, upgrade to compatible B, grow/shrink/remove/release/reuse under B and
   perform a supported compatible downgrade. Prove startup separation, rejection
   before native interpretation, owner/reference confinement and failed-update/
   upgrade rollback. Exercise partial construction, malformed metadata, ordinary
   business Err, trap/poison, owner extraction and forgotten guards/tickets.
3. **Incompatible-runtime portable proof.** Export the same integer record and
   vector under A; make B deliberately incompatible with A's allocator or layout.
   Verify direct reopen fails, then use the independent checkpoint provider to
   reconstruct under B without opening A's heap. Verify values, roots, ownership
   and continued allocation/reclamation, plus partial export/import, corrupt or
   truncated input, restart and recovery. Shared/cyclic types remain outside this
   initial subset; their identity-preserving round trips are required before
   later admission. A real compiler/SDK transition remains a separate case if
   this proof uses synthetic incompatible layouts. This gate precedes investment
   in application converters or further collection types.
4. **Application-type conversion.** Add a field while preserving an unchanged
   collection; change element types with explicit identities/ownership; verify
   overflow, incompatible-source refusal and failed-conversion rollback and
   destruction. Admit shared owners only with confinement, tracked sharing and
   identity-preserving portable tests. Every newly admitted type must already
   have its export/import contract and exercised round trip.
5. **Scale and recovery.** Qualify the chosen large-state protocol, capacity
   failures, bounded progress, downtime and post-cutover recovery policy before
   any consumer requires them.
6. **Matched performance and consumer pilot.** Measure equivalent native
   collections against Storage and M0 where semantics match, including body and
   complete-call costs, memory, allocation/reclamation and all upgrade paths.
   Select a separately authorized consumer only after migration gates pass.

Do not broaden the collection library before the native lifecycle and migration
proofs establish the direction. Stop or revise the mechanism if source layouts
cannot be completely admitted, transient ownership cannot be excluded, migration
cannot fit the required operational budgets, or compatible source readers cannot
be maintained. A portable path that has never restored data is not an escape
route.

Production requires its own custody, type coverage, reclamation, large-state,
dependency, quiescence, corruption, migration and recovery qualification.
No arbitrary Rust persistence, automatic semantic migration, universal downgrade,
GC, compiler portability or production readiness is claimed by this design.
