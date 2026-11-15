# Changelog

## [0.1.3] - Unreleased

### 🧪 Testing

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
