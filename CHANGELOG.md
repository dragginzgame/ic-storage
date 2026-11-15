# Changelog

## [0.1.10] - 2026-11-16

### 🧪 Testing

- Extract a reusable, unsafe-free scoped u64-vector core under `crates/` and use
  it in the installed native fixture. Check owner/reference/future escape,
  forgotten views and allocation refusal. Extend artifact admission to measure
  nested balanced stack frames and guard every access in the new compiler output.
- Add controller-authorized cancellation before heap replacement. Retire the
  sealed epoch, preserve the original vector allocation and resume native writes;
  identical retries leave later writes intact. Exercise cancellation rollback,
  stale-job refusal, compatible reopening and fresh export after cancellation.
- Add explicit frozen-source resumption across compatible upgrades. Rebuild
  transients, verify the sealed job against retained logical values and keep
  writes blocked; preserve subsequent cancellation and replacement recovery.
- Add staged checkpoint import with a version-2 durable cursor, batches of up
  to 64 values and explicit compatible resumption. Keep partial vectors
  inaccessible; verify exact retries, native/checkpoint disagreement and
  rollback before publishing the complete root.

### ▶️ Try it

Run `make native-core-test` for the scoped vector core and ownership checks.
Run `make native-job-test` with cached dependencies and the prepared local server
for cancellation, frozen resumption, staged import and governed admission cases.

## [0.1.9] - 2026-11-15

### 🧪 Testing

- Connect compatible native reopening to checkpoint export and relocated
  recovery by discarding and rebuilding the checkpoint runtime's transient
  domain. Admit its initialization/globals/table behavior before enabling the
  profile; exercise poisoned and grown memory, native refusal and reset rollback.
  The private Global allocator is rebuilt, not retained or declared compatible.
- Add a separate governed migration record with controller-authorized export,
  sequential durable epochs, source/destination domain checks and single-use
  consumption. Exercise controller revocation, stale/replayed jobs, frozen
  retries and rollback after stable/native publication at empty/full capacity.

### ▶️ Try it

Run `make native-checkpoint-test` with the prepared local server and cached
dependencies to exercise compatible reopening and explicit replacement recovery.
Run `make native-job-test` for the controller and durable-job admission cases.

## [0.1.8] - 2026-11-15

### 🧪 Testing

- Link checked vector accesses to the private provider, derive Rust inputs from
  admitted metadata and reject changed final artifacts. Document the bounded
  foreign-allocation and initialized-owner contract before installed execution.
- Exercise real Rust Vec operations across installed A→B→A upgrades, including
  growth, shrink, removal, clear/reuse and empty/full capacity. Verify native
  update/upgrade rollback, destination-only malformed-header refusal and lost
  custody.
- Add an explicit 512-element checkpoint envelope while preserving version-1
  bytes and its 16-element bound. Exercise governed provider recovery at empty,
  17-element and full capacity, failed-export/import rollback and subsequent
  shrink/growth.
- Connect the retained vector to the same governed checkpoint provider through
  scalar-only calls and disjoint native/Global memory regions. Freeze/export
  atomically, reject retained-heap reopening and reconstruct under a relocated
  allocation domain using explicit heap replacement. Exercise full capacity,
  return migration, continued native allocation and failed export/import rollback.

### ▶️ Try it

With cached dependencies and the prepared local server, run
`make native-checkpoint-test` for the connected native migration experiment.

## [0.1.7] - 2026-11-15

### 🧪 Testing

- Add checked vector access helpers and compiler instrumentation for all 49
  emitted memory operations. Exercise metadata/spare-capacity refusal, bounded
  operation-specific writes, offset overflow and overlapping copies in installed
  WAT tests. Guard imports remain unresolved in the rewritten Rust diagnostic;
  it is not executed or admitted as a native persistence implementation.

- Exercise installed vector-descriptor custody before native integration. Reject
  oversized length/capacity before wasm32 narrowing, require settlement before
  publication, and preserve the full header/payload after traps or refused
  upgrades. The WAT-only test includes missing source guards and full-region
  capacity boundaries; it does not execute a Rust vector.

- Check the compiled vector's transient stack discipline and direct-call graph.
  Reject overwritten frame locals, incorrect restoration, branch/return escapes,
  recursion and unreviewed control operators. Retain the operator listing for
  review; payload-address admission and installed vector execution remain open.

## [0.1.6] - 2026-11-15

### 🧪 Testing

- Exercise a transient single-vector allocator over exclusively loaned backing:
  fresh issuance preserves initialized payload, growth/shrink and destruction
  permit reuse, and invalid roots or forgotten owners refuse further access.
  Compile the same controller for wasm32 and inventory its stack/copy operations;
  scalar admission rejects this larger profile and no vector Wasm is executed.

- Add `make native-provider-probe` to compile, combine and inspect the bounded
  Rust/Wasm provider diagnostic. Reject overlapping compiler bounds, unexpected
  initialization/code/imports and altered final artifacts. Keep raw Rust functions
  private and admit trusted entrypoints that settle only after native access ends.

- Add `make native-provider-test` and its installed CI step. Exercise native scalar
  reads/writes across A→B→A upgrades, incompatible/corrupt headers, abandoned
  custody, and update/post-upgrade trap rollback on the local test server. This
  does not qualify native vectors or allocator issuance.

- Exercise a fixed-region Wasm custody protocol across installed upgrades:
  require explicit settlement, reject duplicate or abandoned claims, and retain
  source state after incompatible targets or traps. Destination refusal also
  holds when the source has no pre-upgrade guard. This does not yet qualify a
  Rust allocation provider.

### 📚 Documentation

- Narrow the IC backing-provider investigation to an explicit foreign-allocation
  boundary. Record the fixed scalar's unsafe-code argument, artifact admission
  and installed evidence separately from the remaining native allocator gate. See the [provider investigation](qualification/ic-storage-native-feasibility/provider-boundary.md).

### ▶️ Try it

With the already prepared pinned compiler, wasm target and locked dependencies:

```sh
make native-provider-probe
```

This emits compiler and linked-artifact diagnostics; no combined Wasm is executed.

With the previously prepared local test server, run the installed custody cases
alongside the original retention probe using `make retention-lint` followed by
`make retention-test`. Run `make native-provider-test` for the combined Rust/Wasm
scalar lifecycle.

## [0.1.5] - 2026-11-15

### 🧪 Testing

- Exercise allocator clones with independent live domains and cloning failures.
  Scoped detachment keeps each vector paired with its issuing allocator;
  failed cloning poisons access while preserving ownership for destruction.

## [0.1.4] - 2026-11-15

### 📚 Documentation

- Define the next allocation-domain experiment: independently owned backing,
  explicit custody transfer and fresh allocator issuance. Separate the host
  controller proof from the still-required IC provider and startup contract.

### 🧪 Testing

- Connect the existing safe scoped owner to arena-backed vectors and freshly
  reconstructed records without reallocating. Exercise real allocation refusal,
  retirement/reuse, panic and forgotten-ticket quarantine, and allocator lifetime
  confinement; the default Global-backed experiment remains supported.

- Enforce destination usable-region limits before native transfer and during
  allocation/reuse. Exercise unchanged-custody refusal and explicit portable
  reconstruction into a smaller arena after discarding the incompatible source.

- Transfer live backing through explicit consumed source authority, replace the
  allocator controller and issue the retained span afresh before vector creation.
  Exercise nonzero offsets, duplicate/lost authority, reservation refusal,
  allocation failure and reclamation; no cross-instance provider is implied.

- Add a private, fixed-width allocation-ledger image with version, geometry and
  integrity checks. Exercise independent golden bytes, truncation, corruption,
  resealed invalid metadata and active-owner refusal. Decoding does not grant
  allocation custody or create native pointers; IC reopening remains unresolved.

- Exercise single-root live suspension with complete allocation accounting,
  extra/forgotten-owner refusal, arena freezing and original-token single-use
  resume. Dropping or forgetting suspension keeps the arena unavailable for reuse.

- Connect portable checkpoints to source suspension in host handoff tests:
  recover after source/token loss, retry refused imports, cancel before target
  publication and export a new epoch after further writes. Keep durable migration
  coordination and installed native reopening explicitly outside this evidence.

## [0.1.3] - 2026-11-15

### ▶️ Try it

From the repository root, with the pinned compiler, cached dependencies and
previously prepared test server, these checks run offline:

```sh
make native-feasibility-test
make native-layout-probe
make checkpoint-provider-lint
make checkpoint-provider-build
make checkpoint-provider-test
```

Expect the ownership, layout diagnostic and installed checkpoint tests to pass.
First-time online setup is explicit: `make install-tools`,
`cargo fetch --manifest-path qualification/Cargo.toml --locked`, then
`make retention-setup`; it downloads dependencies and the pinned local test server.

### 🔧 Tooling

- Adopt Toko Tooling 0.44.4 through its canonical exporter and both workspace
  pins. Rename qualification package directories to their existing Cargo package
  names, updating workspace paths, fixture inputs, commands and documentation.
  Package identities, product versions and historical evidence remain unchanged.

### 🧪 Testing

- Validate the whole host allocation ledger before vector reopening, rejecting
  inconsistent extents, overlap, generation identities, occupancy and initialized
  lengths. Exercise fourteen corruptions and valid boundary cases; this remains
  live-domain qualification, not an IC pointer-validity proof.

- Add a host-only native ownership experiment with scoped scalar/vector access,
  tracked detach/attach/retire, owner-escape rejection and forgotten-ticket
  quarantine. Exercise ordinary error preservation and panic poisoning through
  `make native-feasibility-test`, included in CI. Native layout admission,
  persistence and native incompatible-runtime migration remain unimplemented.
- Add `make native-layout-probe` to inspect compiler-emitted host/wasm32 field
  layouts, distinguish equal-sized reordered records and exercise malformed
  metadata refusal. Keep raw diagnostics under target and explicitly separate
  those observations from persistence admission.
- Exercise real standard-library vectors with a bounded private host allocator,
  same-domain parking/reopening, run reuse and failure/forgetting cases. Document
  the pointer and ownership argument separately from unresolved IC reopening.
- Specify and test a bounded scalar/vector checkpoint with schema/epoch checks,
  completeness and integrity validation. Reconstruct after source-arena teardown
  and preserve an existing host root on corrupt input or target allocation failure;
  the native migration gate remains incomplete.
- Add an installed checkpoint-provider fixture using published Storage 0.74.1
  and fresh governed handles. Test ordinary A→B→A upgrades that discard Wasm
  memory, continued writes, rejected checkpoints and rollback after a target
  stable write. Record the provider/locator contract and add focused Make/CI
  checks; this does not qualify native heap reopening or compiler transitions.

### 📚 Documentation

- Define the intended derive-based stored-type surface, with recursive field
  eligibility, scoped ownership, portable codecs and explicit versioned
  conversions. Distinguish these declarations from runtime/layout admission.
- Describe a proposed IcyDB separation between stable index structure and
  evolving records, including when inline values or index changes require
  structural conversion or rebuilding.

## [0.1.2] - 2026-11-15

### ➕ Added

- Add allocation-free, `no_std` M0 image admission with typed refusals for
  incompatible metadata, integrity failures and invalid object/reference graphs.
  It validates unreachable objects and cycles without granting live-memory access.
- Add native M0 image initialization/readmission, explicit bounded growth,
  append-only allocation and session-bound graph access. Ordinary callback errors
  preserve successful writes; unexpected write failures poison the owner.

### 🧪 Testing

- Add opt-in installed benchmarks for M0 scalar and sequence operations versus
  published Storage 0.74.1, with complete state checks, instruction counts,
  artifact/source evidence and `make benchmark-simple`. Compile and lint both
  backends in CI without imposing a performance threshold.

- Add independent empty-state golden bytes and native corruption tests, including
  resealed malformed metadata, identity/schema mismatches and maximum index size.
  `make m0-test` and `make m0-lint-wasm` join the CI gate.
- Verify exact graph bytes, sharing/cycles after reopening, allocator continuation
  across a page boundary, unchanged resource refusals and native poison behavior.
  Add a runnable graph example and compile-fail checks for escaping handles,
  session-capturing futures and handles passed across images.

- Add an independent Rust qualification runner and small Wasm fixtures for
  retained scalar state, fresh transient state, incompatible metadata rejection,
  failed-upgrade rollback and retaining downgrades. Server preparation is explicit
  through `make retention-setup`; `make retention-test` runs offline.
- Add a controlled Rust canister startup probe with passive-data transformation,
  bounded transient clearing, artifact refusal tests and installed upgrade checks.
  `make rust-startup-build` prepares A/B; `make rust-startup-test` admits and tests
  them. The Platform binding path is std-linked and the fixture uses no heap.
- Extend installed qualification to retained memory growth, maximum-capacity
  refusal, growth rollback in updates/upgrades, and retention-mode/marker refusal.
- Add a fixture-only live-memory kernel and installed M0 graph tests for A→B→A,
  shared/cyclic references, page-crossing allocation, trapped update rollback and
  rejected/mutating post-upgrade rollback. Keep global allocation denied and
  admit the graph fixture through a distinct bounded artifact profile.

### 🔄 Changed

- Reuse admitted accounting for reads within an exclusive M0 session. Refresh it
  after fully validated writes; preserve entry/exit admission, handle checks,
  corruption refusals and poison behavior without changing the stored layout.

- Restrict the experimental image mutation API to retained-region slices so it
  cannot borrow the live Rust stack. Add retained-only admission with unchanged
  persisted layout and absolute error offsets.

### 📚 Documentation

- Select native retained collections with explicit versioned type conversion
  and a portable checkpoint path for incompatible runtimes. Define migration,
  interruption/recovery and toolchain-feasibility gates; archive the superseded
  M0 research plan while preserving its byte contract and evidence.

- Tighten native feasibility around complete layout/pointer admission, tracked
  scoped ownership and forgotten guards. Move portable migration before type
  conversion, select a transitional checkpoint-provider boundary, and restore
  explicit mutation, destruction and reclamation rules.

- Freeze the experimental M0 metadata, schema, root and allocation-index layout,
  including integrity coverage and canonical empty-state vectors, ahead of its
  implementation.

## [0.1.1] - 2026-11-15

### 🔧 Tooling

- Adopt Toko Tooling 0.44.3 with its vendor layout, canonical package inventory
  and dependency updater, formatting hook, and current workspace lint rules.
- Add Linux and macOS CI for formatting, dependency policy, Clippy and Rustdoc,
  with a separate repository secret scan.

## [0.1.0] - 2026-11-15

### 🔧 Tooling

- Bootstrap an unpublished Rust 1.100.0 workspace with authenticated Toko
  tooling, explicit tool setup, dependency inspection and policy checks, and a
  documented qualification location.
