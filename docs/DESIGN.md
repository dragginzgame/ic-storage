# Architecture

Status: research architecture, 15 November 2026. Proposed first implementation path, not a production format commitment or authorization to migrate existing canisters. The experiment plan below defines the first proof and stop conditions.

## 1 Recommendation

Start with **managed orthogonal persistence in retained Wasm main memory**, using explicit persistent record layouts, durable object references and generated accessors. Applications declare persistent roots and supported fields; routine upgrades do not serialize and reconstruct the object graph.

This is a deliberate subset of the desired experience. It does not make arbitrary Rust Vec, String, Box, Rc, closures, trait objects, futures or existing static variables transparently persistent. Full ordinary-Rust persistence would require a substantially broader compiler, allocator, layout and lifecycle contract.

Build the smallest retained-heap experiment before designing a collection library. Use a controlled wasm32 build first to isolate retention from memory64 toolchain work. Treat successful deployment of that target with memory retention as an experiment, not an established fact. A wasm64 cohort is a separate qualification milestone.

The design goal is a safe foundation with explicit obligations. No claim of zero future debt, Motoko parity, constant-cost upgrades or better performance is made before measurement.

## 2 What the current repository establishes

Public ic-storage was inspected at commit `98eb9313b7b241607a5dd81a7a4e34aa6c04f755`. This review covered manifests, runtime/custody contracts, backing memory, record codecs, selected adapters and upgrade tests. It did not execute the repository's tests.

| Verified current responsibility | Consequence for OP |
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

This proposal uses public specifications and the Motoko design document. It does not claim clean-room provenance. Record inputs and any later implementation-source consultation.

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

Begin with one unpublished runtime crate and private probe/runner packages. Add a declaration macro only after hand-written probe descriptors establish the required semantics. A host artifact-checking tool is real implementation work, not a placeholder package.

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

Exact addresses are part of the experiment manifest, not a production promise. For the first probe, reserve a fixed low region, for example 8 MiB, and one 64 KiB anchor/descriptor page above it; place the persistent arena after that page. Both modules use the same boundary. Reject a build that does not fit. These are proposed experiment constants, not values taken from Motoko or validated against Rust output.

The persistent arena is never a normal Rust global allocator region. No pointer from it may refer into stack, module data or transient allocation.

Initial modules use no_std without general alloc or third-party initialization that allocates. This deliberately removes default allocator assumptions from the first retention proof. Later transient allocation uses a separately bounded allocator; it must never acquire persistent pages through a default dlmalloc heap boundary.

Prefer a controlled passive-data initialization pipeline. A host transformer may convert admitted active data segments into passive segments and install a small, audited bootstrap that reloads only the module's volatile data region. It must also initialize the new module's BSS and stack correctly. Reject unknown start functions, out-of-region segments or unsupported initialization patterns.

This is proposed build machinery, not something a derive macro or Cargo feature can guarantee. Prove it experimentally before building on it. Do not assume that a different constant string or a new global leaves the old heap untouched.

The bootstrap must run without reading uninitialized Rust statics, using allocator state, formatting errors or executing arbitrary application code. It must not zero an executing Rust stack. A small generated Wasm bootstrap can establish the volatile region and stack before entering Rust admission code.

Only after the persistent anchor is admitted may application initialization or root access occur. A layout mismatch may rewrite volatile scratch during attempted startup, but it must not mutate the persistent arena before refusal. Installed rollback must preserve the old application's usable state.

Final-module checks are mandatory on the actual deployable bytes after optimization. Validate memory count/width, required custom-section marker, layout version, all data segments, startup function, stack region, imports and exported lifecycle wrappers. A sidecar manifest alone is insufficient.

These checks constrain cooperating code; they do not sandbox malicious or arbitrary unsafe dependencies inside the same Wasm memory.

Confine linear-memory read/write/grow operations to one private, reviewed kernel. Prefer numeric-offset Wasm load/store shims over manufacturing Rust references into retained memory; validate the complete span before access. The native model uses checked slices. Neither path exposes persistent native pointers. Generated lifecycle composition has one export owner: Storage admission runs before user post-upgrade logic, and normal endpoints cannot obtain a heap capability until admission succeeds.

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

A persistent reference is a fixed-width pair such as `(offset: u64, allocation_id: u64)`. Both zero represent null; other invalid combinations refuse. The first allocator never reuses an identity or address. Even on wasm32, stored offsets are checked before conversion to the executable address width.

Before field access, validate the object start, bounds, header identity, schema and field span. Never accept an offset merely because it lands somewhere inside linear memory. A bounded allocation index or validated object-start map supplies this check in the prototype.

Do not persist Rust TypeId, compiler type names, symbol addresses or function pointers as authority. Use explicit schema/root identifiers and complete descriptors. Hashes may index descriptors, but collision or mismatch must never authorize a layout.

The graph's meaning includes sharing: two references to the same allocation identify the same object. Reading an object does not copy its graph. Cycles are legal references even before reclamation exists.

A checksum detects some corruption; it neither authenticates the module nor proves every object is valid. Safe accessors must fail on invalid representation instead of creating an invalid Rust reference.

## 8 Safe access and mutation semantics

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

The callback receives a scoped capability. Generated handles permit bounded scalar reads and writes; they do not expose &mut T or an interior pointer into a growable native buffer. Enforce no leaked borrow and no async suspension while a session is live. M0 exposes no async API.

Persistent graph references are identity values, not unique Rust ownership claims. Access through one mutable session serializes conflicting mutation. Host/native models use the same span and identity checks.

**with_update is a borrow scope, not a transaction.** Returning Err after successful earlier mutations does not roll those mutations back. An IC message trap provides message-level rollback; a normal reply containing an error does not.

Each library operation validates bounds/resources before mutation. If an unexpected fault occurs after an operation begins changing its invariants, trap on the IC; the native model poisons the session unless its test harness explicitly restores a snapshot. Do not claim Rust unwinding supplies IC rollback.

Allocation prepares a complete initialized object before publishing a reference to it. The arena cursor, object index and root updates must follow a documented order within one synchronous message. Test every interrupted publication point.

No application Drop implementation runs on persisted bytes. Upgrade does not invoke destructors from the old binary. Deleting a future persistent field must be a runtime schema/migration action, not a cast to a new Rust type followed by drop.

## 9 Upgrade admission

The first compatible upgrade requires exact persistent layout/schema agreement. Code, constants and transient initialization may change. Layout evolution is not needed to prove OP.

Admission order:

1. Bootstrap the new volatile environment without touching the persistent arena.

2. Read and bound-check the old anchor and descriptor directory.

3. Check memory width/layout, format version, allocator version and root identities.

4. Compare full record/reference descriptors under the declared compatibility rule.

5. Check the prototype's bounded object index and required roots.

6. Reconstruct transient runtime state.

7. Publish the new admitted epoch and permit application methods.

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

Only declared roots and reachable managed objects are candidates for persistence. Executor queues, callback closures, Wakers, timers, locks, borrow flags, caches, metrics registries, random generators and native handles are transient unless separately redesigned as data-only state.

They must be reconstructed, not resumed from retained arbitrary heap bytes. Reinitialization cannot overwrite managed data or run destructors against stale pointers.

The initial deployment policy blocks new work, drains outstanding callbacks using the existing host/runtime procedure, then upgrades. An inability to drain is a blocked upgrade, not permission to discard uncertain operations. Durable business intents remain application data.

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

There are two different rollback cases. A rejected upgrade must leave the old application usable. After a successful migration, reinstalling old code is safe only if the resulting representation is still admitted by that old code. A backup of the binary alone is not a data rollback plan.

## 13 Tooling and deployment contract

Each artifact carries an embedded persistence manifest and an external report tied to the exact final Wasm hash:

- Compiler, linker, optimizer, dependency and transform identities.

- Memory target/features and layout constants.

- Persistent format/allocator/schema descriptors.

- Required lifecycle wrappers and retention marker.

- Supported source format versions and migration policy.

The host supplies explicit keep mode and refuses accidental replace/reinstall in the normal upgrade command. The runtime refuses a missing/invalid retained anchor in post-upgrade. These protect against mistakes, not a controller deliberately replacing code or clearing memory.

The build transformer and module validator are part of the trusted implementation. A stripped custom section, late optimizer rewrite or unchecked linker change invalidates prior evidence.

Keep first runtime dependencies minimal. Platform supplies IC interfaces where compatible with the no-allocation bootstrap; otherwise a tiny private ABI shim must be explicitly reviewed. Host/Build/Testkit remain qualification/tooling dependencies.

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

Maintain an inputs log naming the public Motoko design and specifications consulted. Do not call the work clean-room merely because implementation source was not copied.

The current scaffold was not inspected or executed as part of the design review. Audit its manifests and generated Wasm before treating it as an experiment baseline.

## 3 Establish platform retention first

Begin with an allocation-free probe using fixed offsets and scalar sentinels. Before the graph test, prove:

| Component | Probe |
| - | - |
| Linear memory | A writes sentinels in persistent and volatile regions; B reports what survived |
| Globals | A changes mutable globals; B checks its newly initialized globals |
| Data segments | Change constant contents and segment placement between modules; verify B gets its own data without overwriting retained arena bytes |
| BSS/static state | A dirties transient cells; B reconstructs new zero/default state |
| Stack | Exercise nested calls before and after upgrade; verify stack pointers/ranges and persistent sentinels |
| Startup | Instrument the controlled startup path; no application initialization before admission |
| Memory growth | Grow after upgrade and verify no overlap, cursor reset or lost old data |
| Marker/mode | Exercise keep, omitted option and missing/stripped required marker with recorded outcomes |

Use the memory-retention custom section's exact platform encoding. Do not infer its binary name/prefix from a conceptual name in documentation; inspect and validate the emitted custom section.

Try wasm32 first. If the selected platform rejects that combination, record the refusal and select a separately qualified wasm64 toolchain before proceeding. Do not declare that Rust OP is impossible or proven based on target selection alone.

Exit gate: exact startup behavior is observed, and the controlled build prevents initialization from touching persistent regions.

## 4 Declare the experimental representation

Proposed layout constants for this experiment:

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

A reference occupies 16 bytes: arena-relative offset u64 plus allocation identity u64. The arena base comes from the admitted layout. Null is the all-zero pair. Non-null references must match an admitted object-start entry and header; the object schema must match the declared target.

Use a fixed bounded allocation index in the anchor page for M0. Reserve and validate its capacity explicitly; the metadata page cannot grow into the arena. Include the exact descriptors, one root entry and cursor/counter fields in its manifest.

The format is unpublished and disposable. Changing it during research invalidates old fixtures unless an explicit compatibility test says otherwise.

## 5 Construct the graph with module A

On fresh installation only:

1. Initialize the anchor and descriptors.

2. Allocate one Node with value 7 and next pointing to itself.

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
| A late post-upgrade fault occurs after a test-only persistent write | Upgrade fails; old managed bytes and old application remain usable |

After each failed installed upgrade, call A's old methods and compare its managed-region digest, cursor, identities and logical values with the pre-upgrade record. Also verify the expected old module hash and a later successful allocation/update.

Do not require cycle balances, logs or every external observation to be unchanged; test the promised application and memory rollback boundary.

Keep deliberate replace/reinstall testing confined to disposable canisters. Such an operation can destroy memory before post-upgrade code can protect it. The normal deployment command must refuse it; runtime checks are not protection against a deliberately destructive controller.

## 8 Native corruption and mutation tests

Use a byte-backed model with explicit bounds. Never use a growable Vec pointer as a persistent reference.

Mutate anchor lengths, counts, offsets, identities, type IDs, object spans, reserved bytes and reference targets. Include overflow, interior-object pointers, repeated IDs, null mismatches, cycles and out-of-arena references.

Inject failure at object initialization, index publication, cursor update and root publication. Model an IC trap by restoring a test snapshot. Also test ordinary Result::Err without restoration, to demonstrate that the borrow scope is not a transaction.

If a native panic occurs after partial mutation, poison/discard the instance unless the harness explicitly restores it. Passing native unwinding tests does not establish installed rollback.

No unrestricted typed reference may escape an access session. Add compile-fail tests for prohibited persistent fields and borrow escape when declaration tooling is introduced.

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
