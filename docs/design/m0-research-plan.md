# Superseded M0 research plan

Archived on 15 November 2026 when the maintainer selected native retained
collections with explicit schema conversion and a portable runtime-migration
path. This is a historical record, not the active architecture or next-work plan.
[The current design](../DESIGN.md) owns subsequent decisions. M0's implemented
[byte contract](m0-layout.md), tests and recorded measurements remain valid only
for their declared scope; they do not qualify native Rust persistence.

The earlier plan follows, with repository-relative links adjusted for this
archive location. Its proposed future work and status statements are historical.

---

# Architecture

Status: research architecture, 15 November 2026. Proposed first implementation path, not a production format commitment or authorization to migrate existing canisters. The experiment plan below defines the first proof and stop conditions.

Review disposition, 15 November 2026: retain the managed byte-defined heap as the first experiment. The Motoko source review and cdk-rsx-store README support the need for explicit runtime ownership; neither qualifies this Rust design. Small WAT and controlled Rust startup/growth fixtures have local installed evidence. M0 now also has installed shared/cyclic graph upgrades, allocator continuation and selected trap rollback evidence through a fixture-only custody kernel. Production custody, exhaustive interruption testing and migration remain unqualified.

## 1 Recommendation

Start with **managed orthogonal persistence in retained Wasm main memory**, using explicit persistent record layouts, durable object references and generated accessors. Applications declare persistent roots and supported fields; routine upgrades do not serialize and reconstruct the object graph.

This is a deliberate subset of the desired experience. It does not make arbitrary Rust Vec, String, Box, Rc, closures, trait objects, futures or existing static variables transparently persistent. Full ordinary-Rust persistence would require a substantially broader compiler, allocator, layout and lifecycle contract.

Build the smallest retained-heap experiment before designing a collection library. Use a controlled wasm32 build first to isolate retention from memory64 toolchain work. Treat successful deployment of that target with memory retention as an experiment, not an established fact. A wasm64 cohort is a separate qualification milestone.

The design goal is a safe foundation with explicit obligations. No claim of zero future debt, Motoko parity, constant-cost upgrades or better performance is made before measurement.

## 2 What the prior Storage review establishes

### Successor rule

This repository is the intended successor to `ic-storage`, with the goal of
subsuming the persistence capabilities our applications currently obtain from
Storage and ic-stable-structures. Develop one long-term storage architecture;
do not assume permanent parallel product development. `../ic-storage` stays on
hold and unchanged during this experiment.

“Superset” describes the desired application capabilities, not identical APIs,
stored layouts, memory limits or performance. Retained main memory and stable
memory have different contracts. Before retiring a capability, record its
replacement, limits and installed evidence; before moving a consumer, qualify
its schema/data migration, lifecycle, resource costs and recovery procedure.
Existing bytes are never implicitly admitted by the new runtime. Production
integration or migration needs separate authorization. A failed experiment is
reason to revisit the architecture, not silently weaken these gates.

The first milestone is a scalar Wasm retention probe, documented in
[qualification](../../qualification/README.md). It precedes Rust startup and
managed-graph qualification and cannot establish functional replacement alone.
The initial wasm32 WAT probe passed on PocketIC 16.1.0 on Linux, including
retaining upgrades, fresh transient state and post-upgrade trap rollback. This
qualifies that small protocol fixture only. The subsequent
[controlled Rust startup probe](../../qualification/ic-storage-rust-startup-probe/README.md)
also passed on that cohort: passive initialization and bounded volatile clearing
restore changed statics/BSS without changing the retained sentinel. This is a
std-linked, allocation-free fixture, not a qualified managed heap runtime.

Public ic-storage was inspected at commit `98eb9313b7b241607a5dd81a7a4e34aa6c04f755`. This review covered manifests, runtime/custody contracts, backing memory, record codecs, selected adapters and upgrade tests. It did not execute the repository's tests.

| Responsibility recorded at the pinned revision | Consequence for OP |
| - | - |
| Core 0.74.1 provides governed stable allocations, exclusive leases and collections | Do not transplant its directory/extent machinery as the new heap architecture |
| MemoryRuntime admits names and binds opaque allocation IDs before opening leases | Preserve appropriate consumer ownership requirements, but reassess their mechanism |
| Default Wasm backing uses IC stable memory | Retained main memory is a different substrate, not an adapter rename |
| Storable encodes application records; StableCell caches a value and persists encoded bytes | Routine OP access must not retain a hidden encode/store/reload pipeline |
| StableVec uses bounded record slots; ordered collections have their own engines | No presumption that these engines belong in a managed object heap |
| Logging and Metrics adapters implement explicit checkpoints | Persisting all their live runtime state is not an equivalent replacement |
| Installed tests reject incompatible backing/collection upgrades and preserve the previous application | Reuse the failure scenarios as requirements; their passing status does not qualify OP |

Sources: [workspace](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/Cargo.toml), [runtime](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/crates/ic-storage/src/runtime/mod.rs), [memory](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/crates/ic-storage/src/memory/mod.rs), [record contract](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/crates/ic-storage/src/storable/mod.rs), [Cell](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/crates/ic-storage/src/adapter/cell.rs), [Vector](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/crates/ic-storage/src/adapter/vector/mod.rs), [safety contract](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/SAFETY.md), [upgrade tests](https://github.com/dragginzgame/ic-storage/blob/98eb9313b7b241607a5dd81a7a4e34aa6c04f755/qualification/ic-storage-benchmarks/src/tests/cell_upgrade.rs).

The new repository should initially contain no ported B-tree, allocation-governance framework, Metrics/Logging checkpoint package or release framework. Existing storage remains usable until a separately authorized migration is qualified.

## 3 Platform facts and research assumptions

IC upgrade controls expose `wasm_memory_persistence = opt keep`. The specification requires the enhanced-orthogonal-persistence private custom section on the destination when keep is selected, and prevents an omitted retention choice for a source carrying that marker. Globals are initialized from the new module. [IC abstract behavior](https://docs.internetcomputer.org/references/ic-interface-spec/abstract-behavior/).

Upgrade creates a new execution instance and invokes its startup/post-upgrade behavior. Retaining linear-memory bytes does not retain other Wasm execution state. Failed upgrade hooks roll the upgrade back. These semantics must be tested with the actual module transformation and server cohort. [IC canister interface](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/).

The management API also warns about outstanding callbacks across upgrades. The initial supported deployment procedure must quiesce work and drain callbacks before replacement. A timeout or cleared local task is not proof that a remote effect was cancelled. [IC management API](https://docs.internetcomputer.org/references/ic-interface-spec/management-canister/).

Rust type layout may vary between compilations; repr(C) on an outer struct does not freeze the implementation of nested standard-library containers. Therefore raw retained Rust heap bytes are not the initial persistence ABI. [Rust layout reference](https://doc.rust-lang.org/reference/type-layout.html).

The Rust target documentation classifies wasm64-unknown-unknown as Tier 3 and describes additional toolchain requirements. Memory64 support in the IC is not evidence that the selected Rust/CDK/dependency cohort works correctly on it. [Rust target documentation](https://doc.rust-lang.org/rustc/platform-support/wasm64-unknown-unknown.html).

Motoko's published design combines retained memory, a controlled long-lived layout, compatibility metadata, transient reconstruction and a collector whose state survives upgrades. The lesson is coordinated tooling and runtime ownership. Do not copy its memory addresses, subtype rules, object representation or collector as this project's default. The linked design was read at commit `4fc510aa9a7f1093357df50d4a5ebad6660bf296`. [Motoko design](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/design/OrthogonalPersistence.md), [published overview](https://docs.internetcomputer.org/languages/motoko/fundamentals/actors/orthogonal-persistence/enhanced/).

The reported Foundation Rust OP effort is not a dependency, delivery assumption or compatibility target. Reassess it if a public specification and usable implementation become available.

This proposal uses public specifications, the Motoko design document and the pinned cdk-rsx-store interface description below. The implementation-source consultation recorded below is also part of this research context. It does not claim clean-room provenance. Record any further source consultation.

### Motoko implementation consultation

On 15 November 2026, at the maintainer's request, the research agent read selected EOP implementation paths at `4fc510aa9a7f1093357df50d4a5ebad6660bf296`:

- [Persistence metadata and root lifecycle](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/rts/motoko-rts/src/persistence.rs), [descriptor compatibility](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/rts/motoko-rts/src/persistence/compatibility.rs), and selected memory-compatibility branches in [idl.rs](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/rts/motoko-rts/src/idl.rs).
- Selected startup, actor recovery and upgrade code in [compile.ml](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/src/codegen/compile.ml), and data-segment conversion/startup injection in [linkModule.ml](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/src/linking/linkModule.ml).
- [Enhanced memory growth](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/rts/motoko-rts/src/memory/ic/enhanced_memory.rs), initialization and state definitions in [incremental GC](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/rts/motoko-rts/src/gc/incremental.rs), its [barriers](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/rts/motoko-rts/src/gc/incremental/barriers.rs), and [enhanced roots](https://github.com/caffeinelabs/motoko/blob/4fc510aa9a7f1093357df50d4a5ebad6660bf296/rts/motoko-rts/src/gc/incremental/roots/enhanced.rs).

The inspected paths coordinate retained metadata and collector state with compiler-generated roots, memory-specific type compatibility, and controlled reinitialization of runtime data. A concrete boundary is restoring transient GC flags from the retained phase before barriers execute. Ordinary EOP actor recovery reconstructs the root wrapper around retained values; it does not reconstruct the entire graph through serialization.

Research implications: qualify startup before graph ergonomics; distinguish service-type compatibility from storage compatibility; and test reconstruction of transient runtime state explicitly. These observations do not select Motoko's layout, collector or type rules for this project. M0 remains the independently specified byte-defined arena above and below.

Scope is source reading, not a complete audit, build, benchmark or installed upgrade test. No upstream implementation or test code was copied into this repository. This agent context has now seen implementation source and must not describe a later implementation as clean-room; any such workflow requires an unexposed implementer and a reviewed specification under the adopted baseline.

### Pinned cdk-rsx-store design basis

On 15 November 2026, the maintainer selected `Neutrinomic/cdk-rsx` as an additional design basis alongside Motoko. The reviewed input is [ver1/crates/cdk-rsx-store/README.md](https://github.com/Neutrinomic/cdk-rsx/blob/ae5d3791361ff00a609ae9adf80e064ca08b14a4/ver1/crates/cdk-rsx-store/README.md), pinned to commit `ae5d3791361ff00a609ae9adf80e064ca08b14a4` (resolved from `main`). Future upstream changes require deliberate review and an updated pin.

The README describes owner-private module identities, scoped access, private child attachments, generation-based stale-binding rejection, complete catalog admission before interpreting typed pointers, and IC rollback rather than a separate transaction mechanism. These contracts inform the ownership, access and upgrade boundaries of this repository.

| Reference contract | Application to this experiment |
| - | - |
| Scoped capabilities and guards that cannot cross await | Preserve synchronous access sessions; references must not escape them |
| Explicit attachment ownership and fresh generations | Inform later module ownership and stale-handle rejection; do not add reuse to M0 |
| Closed catalog admission before typed access | Require complete descriptor admission before exposing managed objects |
| Business errors can still publish writes; native poisoning is not IC rollback | Preserve the distinction between a borrow scope, ordinary errors and message traps |

The reference's Nyxus path retains native module cells; its Hylix path decodes records into a frame and publishes dirty records on finalization. Neither is automatically equivalent to this proposal's byte-defined retained arena. Keep the current M0 representation and no-serialization proof until a documented design change selects and qualifies an alternative. A future stable-memory path must still satisfy this repository's governed Storage-handle requirement.

Review scope: this README only. No implementation source or linked engine guides were reviewed for this pin, and no upstream tests were run. This is a design-reference pin, not a Cargo dependency, compatibility commitment or evidence of installed upgrade safety. Independently author our implementation from the documented contracts; do not copy or adapt upstream implementation code.

Additional consultation, 15 November 2026: at the maintainer's request, compared
the same pinned revision with the current M0; upstream `main` still resolved to
that revision. Read the Store, Nyxus, Hylix and Collections READMEs, the native
representation certificate README (through line 130), and the engine, upgrade,
build, compatibility, status and Minima guides. Inspected Nyxus's
[data contract](https://github.com/Neutrinomic/cdk-rsx/blob/ae5d3791361ff00a609ae9adf80e064ca08b14a4/ver1/crates/cdk-rsx-engine-nyxus/src/data.rs),
the first 200 lines of its
[collection wrappers](https://github.com/Neutrinomic/cdk-rsx/blob/ae5d3791361ff00a609ae9adf80e064ca08b14a4/ver1/crates/cdk-rsx-collections/src/nyxus/collections.rs),
Store's binding/catalog implementation and selected read/write, lifecycle and
backend excerpts, plus the pinned toolchain/setup declarations.

The central distinction is explicit retained native Rust allocations and
compiler-derived physical/interpretation admission in Nyxus, versus M0's
byte-defined records and offset/identity references. Nyxus collection wrappers
use allocator-parameterized standard-library containers; ordinary Global-owned
containers remain excluded. Hylix encodes stable records but still retains Store
anchors; Minima is a separate experimental stable-collection foundation, not a
third Store memory selector. Their broader collection and conversion APIs do not
establish arbitrary Rust-value support, kernel migration or production readiness.
No upstream build, test or comparative benchmark was run. This consultation
changes neither the selected M0 architecture nor the pin. No upstream code was
copied or adapted; future implementation must preserve the independent-authoring
requirement and must not claim an unexposed clean-room context.

## 4 Scope of the first implementation

| Supported in the first graph experiment | Explicitly excluded |
| - | - |
| Fixed-width integers and byte arrays | usize/isize as persistent fields |
| Fixed-layout records with explicit IDs and offsets | Arbitrary repr(Rust) object layout |
| Explicit nullable typed object references | Native references and raw pointers in persistent data |
| One named persistent root | General dynamic root registration |
| Shared references and a cycle | Automatic garbage collection |
| Append-only allocation with a bounded arena | Free, reuse, compaction or finalizers |
| Synchronous field reads/writes | Futures, closures, vtables and function-table indices |
| Exact-schema compatible upgrades | Automatic field addition, type widening or migrations |

Small byte blobs, persistent vectors and text follow only after retention and allocator continuation are proven. Their representations will be managed descriptors and heap objects, not retained standard-library container layouts.

This route provides automatic retention for the declared heap. Application field access can use generated load/store methods without authoring upgrade serialization. It remains more explicit than Motoko's normal-language state model.

## 5 Owners and dependency boundaries

| Owner | Responsibility |
| - | - |
| Storage runtime | Persistent layout, object identity, root directory, allocation and schema admission |
| Declaration tooling | Field IDs/layouts, typed accessors, reference descriptors and schema manifest |
| Build integration | Controlled linking/startup, data-segment handling, final-module validation and artifact manifest |
| Platform Runtime | System bindings, message context and volatile executor state |
| Host deployment | Retention mode, source/destination artifact admission, callback quiescence and install outcome |
| Application | Business invariants, root selection, external effects and explicit migrations |
| Qualification | Native corruption model, installed experiments and recorded environment identities |

Begin with one unpublished runtime crate and, when needed, private probe/runner packages in the independent `qualification/` workspace. Add a declaration macro only after hand-written probe descriptors establish the required semantics. A host artifact-checking tool is real implementation work, not a placeholder package.

No second executor or application retry engine. Wire may encode external messages or a later export format; it must not become the routine persistent object representation. Stable memory remains outside the managed arena and is not silently used as a second copy of it.

## 6 Memory and startup contract

There is one linear memory, partitioned into disjoint regions:

```text
low addresses
  volatile stack and module data
  bounded transient allocation region
  fixed bootstrap anchor and bounded persistent descriptors
  persistent objects and allocator metadata
high addresses
```

Exact addresses are part of the experiment manifest, not a production promise. The Rust startup probe selects an 8 MiB low reserve and one 64 KiB anchor page above it; a later persistent arena starts after that page. Both modules use the same boundary, and admission rejects a build that does not fit. These independently selected constants have been checked against the two Rust fixture outputs, not against a production workload.

The persistent region comprises both the anchor/descriptor/index area and the object arena. Protection of retained state applies to both, not only to object payloads. The arena is never a normal Rust global allocator region. No pointer from it may refer into stack, module data or transient allocation.

The desired memory kernel uses no_std without general alloc or third-party initialization that allocates. The first Rust startup qualification deliberately selects a std-linked but allocation-free fixture: Platform Runtime 0.22.5's IC0-only binding path currently brings in std. This keeps IC bindings with their canonical owner rather than inventing a local SDK. The fixture uses only scalar atomics, fixed stack arrays and reply bindings; admission rejects memory-growth instructions and indirect calls in its compiler output. This does not prove arbitrary std code allocation-free or qualify a no_std dependency graph. Later transient allocation needs a separately bounded allocator and must never acquire persistent pages through a default dlmalloc heap boundary.

The later [managed graph profile](../../qualification/ic-storage-rust-startup-probe/managed-graph.md)
links the safe M0 library into that controlled fixture. Its global allocator
traps on any request; compiler-linked panic formatting uses an admitted fixed,
immutable transient function table. Its distinct artifact profile allows exactly
one kernel-owned growth instruction and retains exact final-byte admission.
These choices do not weaken the scalar profile or qualify general std allocation.

Prefer a controlled passive-data initialization pipeline. A host transformer may convert admitted active data segments into passive segments and install a small, audited bootstrap that reloads only the module's volatile data region. It must also initialize the new module's BSS and stack correctly. Reject unknown start functions, out-of-region segments or unsupported initialization patterns.

The qualification transformer now exercises this bounded sequence for two small Rust fixtures. It is not production build machinery or something a derive macro or Cargo feature can guarantee. Do not assume that a different constant string or a new global leaves the old heap untouched without final artifact admission.

The bootstrap must run without reading uninitialized Rust statics, using allocator state, formatting errors or executing arbitrary application code. It must not zero an executing Rust stack. A small generated Wasm bootstrap establishes the new volatile region, including BSS, and stack before entering Rust admission code. The linker/build report must identify those regions explicitly; passive data segments alone do not describe BSS or prove that all initialization writes are safe.

The Wasm start function only prepares transient execution state and leaves the heap unavailable. It must not infer fresh installation from zero or missing metadata, initialize retained state, or grow the retained memory to conceal a truncated heap. `canister_init` alone selects fresh initialization; `canister_post_upgrade` selects read-only retained-state admission, then enables access on success. This distinction follows the IC's separate initialization and upgrade entry points. [Canister lifecycle specification](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/#canister-upgrades).

Only after fresh heap initialization or retained-state admission may application initialization or root access occur. A layout mismatch may rewrite volatile scratch during attempted startup, but it must not mutate the persistent region before refusal. A failed upgrade from valid old state must preserve the old application's usable state.

Read the anchor from a build-owned fixed location with a bounded header size. Validate lengths, counts, arithmetic, physical memory size and the admitted region bounds before following any stored offset. Metadata cannot redefine the boundary protecting itself. Start with admission closed in a freshly initialized global; never trust a retained "already admitted" bit. A persisted schema epoch describes the stored contract, not permission to bypass admission in a new executable.

Final-module checks are mandatory on the actual deployable bytes after optimization. Validate memory count/width/minimum/maximum, required custom-section marker, layout version, all data segments, startup function, stack region, imports and exported lifecycle wrappers. For M0, reject remaining active data segments and unsupported constructors/startup paths; admit only the controlled initialization sequence. A sidecar manifest alone is insufficient. Changed volatile-layout bounds must still exclude the fixed persistent region.

These checks constrain cooperating code; they do not sandbox malicious or arbitrary unsafe dependencies inside the same Wasm memory.

Confine persistent-region read/write and memory-growth operations to one private, reviewed kernel; ordinary Rust stack/static access stays in the volatile region. Prefer numeric-offset Wasm load/store shims over manufacturing Rust references into retained memory; validate the complete span before access. The Rust-to-shim boundary must have explicit memory-effect and aliasing semantics so optimization cannot cache or reorder accesses incorrectly. The native model uses checked slices. Neither path exposes persistent native pointers. Keep the safe crate's unsafe prohibition; any required kernel package or lint exception needs a narrow review when implemented. Generated lifecycle composition has one export owner: Storage admission runs before user post-upgrade logic, and normal endpoints cannot obtain a heap capability until admission succeeds.

For this bounded fixture, the selected alternative is an exclusive slice over
only the retained interval, reusing the safe byte decoder/writer without copying
or serialization. A slice over the whole live memory would overlap the running
stack and is prohibited. A private busy guard excludes reentry, all system growth
occurs after slices end, and global allocation traps. Narrow unsafe expectations
are confined to the qualification kernel; the product crate still forbids unsafe
Rust. The linked custody contract records these conditions and their evidence.

## 7 Persistent representation

Choose a byte-defined ABI rather than Rust struct memory. Fixed-width integers are little-endian. Padding is explicit and initialized. Generated accessors read/write fields at admitted offsets; they do not cast retained bytes to &T.

The experimental anchor records:

- Format magic/version and memory-layout identifier.

- Arena bounds and persistent allocation cursor.

- Root and schema-directory locations and bounded counts.

- Allocation identity counter and schema epoch.

- Lifecycle state and any future migration state.

- A bounded integrity check for bootstrap metadata.

Each object has a fixed header: object/allocation identity, schema identifier, payload size and flags. Payload fields follow the admitted descriptor.

A persistent reference is the fixed-width pair `(arena_relative_offset: u64, allocation_id: u64)`, pointing to an object's header. `(0, 0)` is null. Non-null references require a nonzero allocation identity; offset zero is valid for the first object. A nonzero offset with identity zero is invalid. The first allocator never reuses an identity or address. Identity exhaustion refuses before mutation. Even on wasm32, addition to the arena base, span arithmetic and conversion to the executable address width are checked.

Before field access, validate the object start, bounds, header identity, schema and field span. Never accept an offset merely because it lands somewhere inside linear memory. A bounded allocation index or validated object-start map supplies this check in the prototype.

Do not persist Rust TypeId, compiler type names, symbol addresses or function pointers as authority. Use explicit schema/root identifiers and complete descriptors. Hashes may index descriptors, but collision or mismatch must never authorize a layout.

The graph's meaning includes sharing: two references to the same allocation identify the same object. Reading an object does not copy its graph. Cycles are legal references even before reclamation exists.

A checksum detects some corruption; it neither authenticates the module nor proves every object is valid. Safe accessors must fail on invalid representation instead of creating an invalid Rust reference.

## 8 Safe access and mutation semantics

### Collection support boundary

The representation is intended to support many data structures, but support
requires an explicit persistent layout and validated operations for each type.
The current M0 implements only fixed `Node` and `CounterState` records, nullable
Node references, sharing and cycles. Its allocator is bounded and append-only;
there is no general collection framework, resizing or reclamation yet.

| Candidate structure | Additional contract needed |
| --- | --- |
| Records, optional values and enums | Complete field descriptors, stable tags and schema compatibility |
| Arrays, vectors, byte buffers and strings | Element/encoding rules, validated length/capacity, growth and reclamation |
| Maps, sets and trees | Node/bucket layouts, key ordering or hash rules, balancing and mutation invariants |
| Shared or cyclic graphs | Durable references and ownership/reclamation rules; M0 already exercises references and cycles |

These are implementation directions, not supported APIs. A collection's logical
shape can be retained without retaining the compiler's ordinary `Vec`, `String`,
`HashMap` or pointer layout. References inside persistent structures use durable
object identities; session handles provide temporary safe access to them.
Functions, closures, trait-object vtables, futures and live runtime resources
cannot be retained as arbitrary executable state. Where appropriate, persist a
stable data identifier or explicit workflow state and reconstruct transient
behavior through admitted code after upgrade.

Structure choice still controls lookup/traversal cost, allocation overhead,
locality, fragmentation and migration work. A linked sequence and a vector can
serve the same logical workload with different costs. The
[simple benchmarks](../../qualification/ic-storage-simple-benchmarks/README.md) compare these
current implementations and do not establish that every collection is equally
fast. Qualify a small contiguous collection before extrapolating from Node
traversal to general collection performance.

### Session API direction

Proposed ergonomic direction, not a compiled API:

```rust
// CounterState and Node are generated descriptor/accessor types,
// not Rust structs whose in-memory layout is persisted.
heap.with_update(|cx| {
    let state = cx.root::<CounterState>(COUNTER_ROOT)?;
    let before = state.counter(cx)?;
    state.set_counter(cx, before.checked_add(1).ok_or(Error::Overflow)?)?;
    let left = state.left(cx)?;
    left.set_value(cx, 42)?;
    Ok(())
})
```

The callback receives a scoped capability. Generated handles permit bounded scalar reads and writes; they do not expose &mut T or an interior pointer into a growable native buffer. Enforce no leaked borrow and no async suspension while a session is live. M0 exposes no async API. When implementing the API, prove that a callback cannot return or store a future capturing its session or access handles; absence of an async method or a `!Send` marker is insufficient. Handles from another heap instance/session must not confer access, even if their numeric object identities coincide. Persisted reference bytes are interpreted only in their owning admitted heap.

Persistent graph references are identity values, not unique Rust ownership claims. Access through one mutable session serializes conflicting mutation. Host/native models use the same span and identity checks.

**with_update is a borrow scope, not a transaction.** Returning Err after successful earlier mutations does not roll those mutations back. An IC message trap provides message-level rollback; a normal reply containing an error does not.

Each library operation validates bounds/resources before mutation. If an unexpected fault occurs after an operation begins changing its invariants, trap on the IC; the native model poisons the session unless its test harness explicitly restores a snapshot. Do not claim Rust unwinding supplies IC rollback.

Allocation prepares a complete initialized object before publishing a reference to it. The arena cursor, object index and root updates must follow a documented order within one synchronous message. Test every interrupted publication point.

No application Drop implementation runs on persisted bytes. Upgrade does not invoke destructors from the old binary. Deleting a future persistent field must be a runtime schema/migration action, not a cast to a new Rust type followed by drop.

## 9 Upgrade admission

The first compatible upgrade requires exact persistent layout/schema agreement. Code, constants and transient initialization may change. Layout evolution is not needed to prove OP.

Admission order:

1. Bootstrap the new volatile environment without touching the persistent region, including its metadata.

2. Read and bound-check the old anchor and descriptor directory.

3. Check memory width/layout, format version, allocator version and root identities.

4. Compare full record/reference descriptors under the declared compatibility rule.

5. Check the complete bounded object index, object headers and reference fields, including unreachable allocated objects: alignment, nonoverlap, unique identities, cursor/count consistency, reserved bytes, root schema and reference target schemas. Bound work by admitted counts and byte lengths; cycles must not cause unbounded recursion.

6. Reconstruct transient runtime state.

7. Enable the transient admission capability and permit application methods. M0 has no schema migration and does not advance the persisted schema epoch on a code-only upgrade.

Any failure traps the upgrade. Do not initialize an empty heap because a magic value is missing during post-upgrade. Fresh installation and retained upgrade are separate paths.

For the small first experiment, scanning the bounded object set is acceptable and must be measured. It is not proof of heap-size-independent upgrades. A later design may check bounded metadata and trust continuously maintained invariants, with lazy access checks; that requires its own corruption and scalability argument.

Later evolution begins conservatively:

| Change | Initial policy |
| - | - |
| Code-only change, same descriptors | Admit |
| Rename a Rust source symbol while explicit persistent identity/layout remains equal | Admit after manifest comparison |
| Reorder/change persistent fields or offsets | Refuse |
| Change integer width, optionality, reference target or meaning | Refuse |
| Add/remove root or object field | Refuse until an explicit evolution rule exists |
| Change allocator, memory width or ABI | Refuse; separate migration experiment |
| Change transient caches/constants | Reinitialize; verify no persistent references point to them |

Do not import Motoko subtyping rules into a Rust managed ABI. Storage compatibility, service-interface compatibility and business-semantic compatibility are separate checks.

## 10 Transient state and outstanding work

Declared roots define the application's durable graph. Physical retention also preserves unreachable arena allocations; M0 does not reclaim them and validates all indexed objects. Executor queues, callback closures, Wakers, timers, locks, borrow flags, caches, metrics registries, random generators and native handles are transient unless separately redesigned as data-only state.

They must be reconstructed, not resumed from retained arbitrary heap bytes. Reinitialization cannot overwrite managed data or run destructors against stale pointers.

M0 exports only synchronous probe endpoints and makes no outgoing calls; it has no timers, heartbeat or async executor. Its host runner stops the canister, waits for stopped status, confirms the expected source module, then requests the admitted upgrade. A later consumer integration must define how to block new work and drain callbacks before replacement; this repository has no such integration yet. An inability to drain is a blocked upgrade, not permission to discard uncertain operations. Durable business intents remain application data.

After upgrade, Scheduling rebuilds timer demand from current application state; it does not reuse retained timer handles. Entropy follows its fresh-generation/reset contract; retaining its old random pool simply because bytes survived is forbidden. Metrics and Logging need an explicit continuity decision rather than automatic persistence of their entire registries.

Snapshot load is not treated as the same operation as code upgrade. It can restore older runtime state and external-effect intent. Support requires a separate host protocol and installed proof; it is excluded from the first supported lifecycle.

## 11 Allocation and reclamation roadmap

M0 uses a bounded bump allocator with persistent cursor and checked growth. No free list, reuse, moving collector or application finalizer. Dropping a handle does not reclaim its object. The experiment must report this leak behavior plainly.

Before production use, select and qualify reclamation. The recommended research direction is tracing from declared persistent roots, allowing sharing and cycles. Reference counting alone does not reclaim cycles and would add ownership traffic to every edge.

First test nonmoving stop-the-world tracing on a bounded heap at a safe point. Before reuse, prove that no unregistered transient handle can outlive that point. Add a generation/identity discipline for stale references.

Incremental collection is a later project: it requires persisted collector state, barriers, mutation rules, root handling, work limits, memory reserve and upgrade compatibility during every phase. It cannot be added as a background loop around the prototype.

Compaction is separate again. Offset references simplify explicit rewriting, but do not make it automatic or prove that hidden pointers are absent.

No live-memory capacity guarantee follows from retaining bytes. wasm32 addressability is bounded, and actual IC limits are deployment inputs. A managed heap is not automatically a replacement for every existing stable-memory dataset. Qualify capacity and headroom before selecting consumers.

## 12 Migration and failure recovery

Normal compatible upgrades must not serialize the whole graph. Exceptional schema/heap changes may require an explicit graph migration with alias/cycle preservation.

For a small migration, build a candidate graph and switch roots only after validation within one qualified atomic upgrade/message. For a large migration, design a resumable state machine before implementation: old graph retained, applications blocked or explicitly restricted, bounded chunks, durable progress, object-ID mapping, commit barrier and recovery from interruption.

Do not introduce that machinery into M0. A plain field-copy loop does not preserve shared references or cycles.

Existing ic-storage bytes are not OP objects. A future importer must read the specific old format and produce managed objects, verify application totals/invariants, then cut over under explicit consumer authority. Keep old storage readable until that decision and proof exist. Never reinterpret a stable-memory directory or Storable record as a managed heap.

There are two different rollback cases. A rejected upgrade from valid old state must leave the old application usable. After a successful migration, upgrading back to old code with explicit memory retention is safe only if that code admits the resulting representation. The management API's `reinstall` mode is destructive and is not this downgrade procedure. A backup of the binary alone is not a data rollback plan.

## 13 Tooling and deployment contract

Each artifact carries an embedded persistence manifest and an external report tied to the exact final Wasm hash:

- Compiler, linker, optimizer, dependency and transform identities.

- Memory target/features and layout constants.

- Persistent format/allocator/schema descriptors.

- Required lifecycle wrappers and retention marker.

- Supported source format versions and migration policy.

The host supplies explicit keep mode and refuses accidental replace/reinstall in the normal upgrade command. The runtime refuses a missing/invalid retained anchor in post-upgrade. These protect against mistakes, not a controller deliberately replacing code or clearing memory.

The build transformer and module validator are part of the trusted implementation. A stripped custom section, late optimizer rewrite or unchecked linker change invalidates prior evidence.

Keep first runtime dependencies minimal. Platform Runtime owns IC system bindings. Verify that the selected path works with the no-allocation bootstrap before admitting it; an incompatibility is a dependency-boundary decision, not implicit permission to add a local IC SDK or bypass Platform. Pure Wasm memory operations belong to the private heap kernel and are distinct from `ic0` bindings. Host/Build/Testkit remain qualification/tooling dependencies.

Do not port the current repository's release framework into a research scaffold. First deliver reproducible artifacts and installed evidence. Shared release integration follows when a releasable product actually exists.

## 14 Decision gates and nonclaims

Proceed in this order: retention/startup proof; graph/allocator continuation; incompatible-upgrade rollback; managed API/type restrictions; bounded reclamation; schema evolution; consumer pilot; production qualification.

Stop or narrow scope if the selected IC/Rust cohort cannot safely initialize retained memory, if ordinary dependencies cannot be isolated from the arena, or if the managed API imposes unacceptable application cost.

If transparent ordinary Rust containers remain a hard requirement, treat compiler-supported Rust OP as a separate research track. It must address layout of nested generic types, allocator ownership, relocations, globals, function/vtable references, Drop, unsafe code, root discovery and code-version compatibility. A trait derive alone cannot establish those guarantees.

The next deliverable is an installed experiment with measured, declared scope—not a copied storage library and not a production OP claim.

# First installed experiment

Status: proposed research protocol, 15 November 2026. Companion to the architecture above. This is the next implementation milestone, not production acceptance.

## 1 Question

Can two different Rust Wasm modules safely share an explicitly managed object graph through retained main memory, with correct initialization, allocator continuation and rejection of incompatible upgrades?

Success proves only the chosen type subset, memory layout, toolchain and installed server cohort. It does not prove transparent persistence for ordinary Rust values, reclamation, schema evolution or production safety.

## 2 Freeze the experiment inputs

Record:

- Rust compiler/target, linker and optimizer versions and complete flags.

- Dependency locks and source identities.

- Module transformer and validator source hashes.

- Final A, B and negative-variant Wasm hashes.

- Simulator/server version and, where available, independent replica execution identity.

- Exact install/upgrade arguments, including keep and skip-pre-upgrade choices.

- Layout/format/schema manifest and raw evidence outputs.

Read old ic-storage as pinned evidence at `98eb9313b7b241607a5dd81a7a4e34aa6c04f755`. The prototype starts fresh; its format is not that repository's stable-memory format.

Maintain an inputs log naming the public Motoko design, the implementation consultation recorded above, the pinned cdk-rsx-store README and specifications consulted. Do not call the work clean-room merely because implementation source was not copied.

The scaffold's tooling and native lint/documentation checks alone establish no installed persistence behavior. The WAT and controlled Rust startup probes now have separately scoped local PocketIC evidence under qualification; there is still no managed heap implementation. Inspect each probe's recorded inputs and final Wasm before treating it as an experiment baseline.

## 3 Establish platform retention first

Begin with an allocation-free probe using fixed offsets and scalar sentinels. Before the graph test, prove:

| Component | Probe |
| - | - |
| Linear memory | A writes sentinels in persistent and volatile regions; B reports what survived |
| Globals | A changes mutable globals; B checks its newly initialized globals |
| Data segments | Change constant contents and segment placement between modules; verify B gets its own data without overwriting retained arena bytes |
| BSS/static state | A dirties transient cells; B reconstructs new zero/default state |
| Stack | Exercise nested calls before and after upgrade; verify stack pointers/ranges and persistent sentinels |
| Startup | Distinguish start/init/post-upgrade; start prepares only transient state, and missing metadata in post-upgrade refuses without initialization |
| Memory growth | Grow after upgrade and verify no overlap, cursor reset or lost old data; reject oversized minimum-memory requirements and truncated retained regions |
| Marker/mode | Exercise keep, omitted option and missing/stripped required marker with recorded outcomes |

Use the memory-retention custom section's exact platform encoding. Do not infer its binary name/prefix from a conceptual name in documentation; inspect and validate the emitted custom section.

Try wasm32 first. If the selected platform rejects that combination, record the refusal and select a separately qualified wasm64 toolchain before proceeding. Do not declare that Rust OP is impossible or proven based on target selection alone.

Exit gate: exact startup behavior is observed, and the controlled build prevents initialization from touching persistent regions.

## 4 Declare the experimental representation

The [M0 layout version 1](m0-layout.md) now freezes the complete byte
contract, including metadata partition, exact schemas, root/index, integrity
coverage, mutation ordering and admission rules. The following is a summary;
the versioned contract owns exact values. The library implements native-image
admission, append-only allocation and scoped graph mutation. Native tests cover
sharing/cycles, reopening, growth across a page boundary, resource refusals and
poison after unexpected writes; compile-fail examples enforce session isolation.
The model borrows caller-owned capacity, reinspects each operation and provides
no native rollback. The separate fixture kernel supplies live-memory custody for
the selected Wasm cohort, with retaining upgrades, full-byte comparisons, shared
mutation, page-crossing allocation and selected failed-upgrade rollback tests.
This is not a production custody contract or coverage of every interruption point.

Layout constants for this experiment:

- Low 8 MiB reserved for the admitted stack, module data and transient space.

- One 64 KiB anchor/schema/index page immediately above it.

- Persistent arena after that page.

- At most 1,024 objects; no deallocation or reuse.

- A hard arena cap recorded in the manifest; tests include growth refusal.

These constants are experimental. They must fit the emitted module and installed limits before any test is considered valid.

Use a 48-byte object header:

| Offset | Representation |
| - | - |
| 0 | allocation identity, u64 |
| 8 | explicit 128-bit schema identity |
| 24 | payload byte length, u64 |
| 32 | flags, u32 |
| 36 | 12 reserved zero bytes |

Header and payload starts are aligned to eight bytes. Fields are accessed as byte-defined values, never by casting the header to a Rust struct.

Define two payloads:

| Record | Offsets |
| - | - |
| CounterState | counter u64 at 0; left reference at 8; right reference at 24; total 40 bytes |
| Node | value u64 at 0; next nullable reference at 8; total 24 bytes |

A reference occupies 16 bytes: arena-relative header offset u64 plus allocation identity u64. The arena base comes from the admitted layout. Null is the all-zero pair. Offset zero with a nonzero identity is a valid first-object reference. Non-null references must match an admitted object-start entry and header; the object schema must match the declared target.

The fixed allocation index occupies bytes 1024–33792 of the metadata page. The versioned contract partitions the complete 64 KiB without overlap and specifies SHA-256 coverage over that page with its digest field zeroed. Growth, allocation and root publication update the digest; payload-only mutations do not. A metadata digest is not a checksum of every object or a replacement for full representation validation. The metadata page cannot grow into the arena.

The format is unpublished and disposable. Changing it during research invalidates old fixtures unless an explicit compatibility test says otherwise.

## 5 Construct the graph with module A

On fresh installation only:

1. Initialize the anchor and descriptors.

2. Allocate one complete Node with value 7 and null next; after publication in the allocation index, set next to its own validated reference.

3. Allocate CounterState with counter 41 and both left/right pointing to that Node.

4. Publish the root only after both objects and index entries are complete.

5. Return the graph's logical values, reference identities, cursor and managed-region digest.

This tests shared identity and a cycle without needing collections or garbage collection. Root traversal must be bounded even when edges cycle.

A normal update mutates the shared node through left and verifies the change through right. A subsequent message checks retention. Queries may observe state but cannot establish durable mutation.

## 6 Upgrade to module B

B has different executable logic, constant strings and transient initialization. Its persistent layout and descriptors are identical.

Upgrade with explicit keep. B must:

- Observe counter 41 and the current shared-node value.

- Confirm left and right identify the same object and next identifies itself.

- Increment the counter to 42 and mutate the shared node.

- Allocate a new Node from the retained cursor.

- Link the new Node through right and verify left still points to the original node.

- Survive another message and another compatible upgrade.

Run a separate near-page-boundary population variant within the same object limit so allocation after upgrade crosses a physical page boundary. This exposes an allocator that appears correct only when no new memory is needed.

No pre-upgrade graph serialization, stable-memory mirror or deserialization restore routine is allowed in this proof. The host may record diagnostic snapshots as test evidence.

## 7 Refuse incompatible modules and prove rollback

Build negative B variants from the same test source:

| Variant | Expected result |
| - | - |
| counter changes width or offset | Refuse descriptor compatibility before persistent mutation |
| Node.next changes target/representation | Refuse |
| Root identity, format or allocator version differs | Refuse |
| Module data exceeds the volatile reserve | Reject final artifact before deployment |
| Persistent anchor/header/index is malformed | Refuse safely; never initialize over it |
| Anchor is entirely zero during post-upgrade | Refuse; do not take the fresh-install path |
| A late post-upgrade fault occurs after a test-only persistent write | Upgrade fails; old managed bytes and old application remain usable |

For each runtime-rejected upgrade starting from valid A state, call A's old methods and compare its managed-region digest, cursor, identities and logical values with the pre-upgrade record. Also verify the expected old module hash and a later successful allocation/update. Artifact-validator refusals must prove no install request was sent; they are not installed rollback evidence.

Use separate disposable fixtures for corruption committed before an attempted upgrade. Require bounded rejection and preservation of those pre-attempt bytes, not recovery of a usable application from already-corrupt state. Test invalid anchors, indexes, object headers and reference targets separately. Corruption injected only within a failing upgrade is instead covered by the valid-A rollback case.

Do not require cycle balances, logs or every external observation to be unchanged; test the promised application and memory rollback boundary.

Keep deliberate replace/reinstall testing confined to disposable canisters. Such an operation can destroy memory before post-upgrade code can protect it. The normal deployment command must refuse it; runtime checks are not protection against a deliberately destructive controller.

## 8 Native corruption and mutation tests

Use a byte-backed model with explicit bounds. Never use a growable Vec pointer as a persistent reference.

Mutate anchor lengths, counts, offsets, identities, type IDs, object spans, reserved bytes and reference targets. Include overflow, interior-object pointers, repeated IDs, null mismatches, cycles and out-of-arena references.

Inject failure at object initialization, index publication, cursor update and root publication. Model an IC trap by restoring a test snapshot. Also test ordinary Result::Err without restoration, to demonstrate that the borrow scope is not a transaction.

If a native panic occurs after partial mutation, poison/discard the instance unless the harness explicitly restores it. Passing native unwinding tests does not establish installed rollback.

No unrestricted typed reference may escape an access session. Add compile-fail tests for prohibited persistent fields, borrow escape, futures capturing a session and cross-session handle use when declaration tooling is introduced. Include valid offset-zero references and rejected identity exhaustion in native representation tests.

## 9 Measure and report

Measure the actual final transformed artifacts, not pre-transform Rust output:

- Total uncompressed Wasm, code section and metadata/data overhead.

- Initial memory reservation, live object bytes, metadata/index bytes and unreachable bump allocations.

- Upgrade instructions, growth instructions and subsequent read/write cost.

- Cost at several graph populations, up to the declared object limit.

- Native validation cost versus installed execution cost.

Use matched compiler/optimization/locks for A/B comparisons. Record raw measurements rather than claiming a constant upgrade bound from two tiny samples.

For comparison with old ic-storage, use the same logical counter/shared-graph workload with explicit IDs on its stable-memory implementation. Existing stable collections already avoid whole-heap serialization; do not compare only against a naive dump/restore implementation and call that a storage improvement.

Do not compare wasm32 and wasm64 sizes without separating pointer-width/toolchain effects.

## 10 Required evidence and next decision

The first milestone passes only with:

- Reproducible A/B and negative artifacts plus validated manifests.

- Observed retention/startup behavior on the selected installed cohort.

- Correct sharing, cycles and post-upgrade growth.

- Incompatible-layout refusal and old-state usability after failure.

- No graph serialization during ordinary upgrade.

- Bounded native corruption tests and an explicit unsafe-boundary review.

- Reported memory/instruction/Wasm measurements and the no-reclamation limitation.

Then choose the next bounded experiment: generated managed accessors; blob/vector support; reclamation; or memory64. Do not begin all four simultaneously.

Production remains blocked until reclamation, large-state behavior, upgrade compatibility, dependency integration, callback quiescence, recovery, consumer migration and operational procedures have their own evidence.

The milestone proves a managed retained-heap mechanism. It does not prove ordinary Rust OP or replace existing ic-storage users automatically.
