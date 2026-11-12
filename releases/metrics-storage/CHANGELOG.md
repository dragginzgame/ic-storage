# Metrics Storage release notes

Current releases use the Storage naming. [Earlier published history](https://github.com/dragginzgame/ic-storage/blob/23deb81c48d58558ac3b0ce36bf266ddc25dca91/releases/metrics-stable/CHANGELOG.md)
retains its original package and release identities.

## [0.76.2] - 2026-11-12

### 🧰 Tooling

- Move the tests module to `tests/mod.rs` because it has child modules. Rust
  paths and test contents are unchanged. See [the detailed notes](0.76.md).
- Adopt inherited Toko Tooling 0.39.17 and Wire 0.21.4 selections without changing
  checkpoint behavior.

## [0.76.1] - 2026-11-11

### 🔧 Changed

- Advance the Storage requirement to 0.73.2 with its native tooling refresh.
  Metrics APIs and checkpoint formats are unchanged. See [the detailed notes](0.76.md#0761---unreleased).

## [0.76.0] - 2026-11-11

### ⚠️ Breaking

- Adopt IC Wire 0.21.1 and its public codec-error identity. Wire now accepts
  redundant varints; encoder bytes and stored checkpoint formats are unchanged.
  Unchanged-state saves preserve the original stored bytes, including accepted
  noncanonical encodings. State, identity, budget and regression checks remain.
  See [the adoption notes](0.76.md).

## [0.75.0] - 2026-11-10

### ⚠️ Breaking

- Adopt Storage 0.73 and its public type identity. Hosts must explicitly supply
  fresh/reset whole backing and update application framing admission; previous
  backing and collection images are rejected without automatic migration.
  Checkpoint payload formats, validation and adapter APIs remain unchanged.

### 📦 Dependencies

- Qualify the root workspace with locked IC Metrics 0.29.11.
- Inherit Toko Tooling 0.39.7 and the canonical root licence file.

## [0.74.0] - 2026-11-09

### ⚠️ Breaking

- Adopt IC Wire 0.19 and its public codec-error type identity. Consumers matching
  Wire errors must select the same minor. Stored checkpoint formats and
  admission limits are unchanged; no backing reset or data migration is needed.

### 📦 Dependencies

- Qualify the root workspace with locked IC Metrics 0.29.10.

## [0.73.0] - 2026-11-09

### ⚠️ Breaking

- Adopt Storage 0.71's governed bucket backing and matching public type identity.
  Hosts must explicitly retire/reset old snapshot-directory backing or provide
  fresh backing; incompatible images are rejected without automatic migration.
  Checkpoint payload formats and adapter semantics remain unchanged. Remove
  any `StorageConfig::wire` overrides and review Storage's allocation limits.

### 🔧 Tooling

- Adopt the workspace's exact Toko Tooling 0.36.1 development selection.

## [0.72.0] - 2026-11-08

### ⚠️ Breaking

- Adopt IC Wire 0.18. Hosts exchanging Wire limits or errors must use the
  matching Wire type identity and align Storage and its adapters. Encoded
  directory and checkpoint bytes, budgets and admission are unchanged;
  no backing reset or stored-data migration is needed.

## [0.71.1] - 2026-11-08

### 🔧 Tooling

- Inherit the workspace's exact Toko Tooling 0.36.0 dev-dependency and
  cargo-machete exemption. Publish against Storage 0.69.1 with its B-tree
  improvements; adapter APIs and checkpoint formats are unchanged.

## [0.71.0] - 2026-11-08

### ⚠️ Breaking

- Integrate Storage's direct collection results. Hosts exchanging governed maps
  with Metrics Storage must use the matching Storage API. Metric checkpoint
  bytes, continuity checks and adapter results are unchanged.

## [0.70.0] - 2026-11-06

### ⚠️ Breaking

- Align governed handles and errors with Storage 0.68. Existing underlying
  collection images require host rebuild/reset or fresh allocations; metric
  snapshot payloads and restoration semantics are unchanged. [Details](0.70.md).

## [0.69.0] - 2026-11-05

### ⚠️ Breaking

- Adopt IC Wire 0.17 for metric checkpoint codec errors. Hosts exchanging Wire limits or
  errors must align their dependency to the new type identity. Codec bytes,
  budgets and stored layouts are unchanged; no backing reset is needed.
  Codec-only builds no longer select Wire's metadata derive. [Details](0.69.md).

## [0.68.0] - 2026-11-04

### ⚠️ Breaking

- Adopt Metrics 0.29 instrumentation types and its explicit persistence API.
  Hosts exchanging measurement values or restoration errors must align their
  Metrics dependency. Stored rows and restoration semantics remain unchanged;
  no backing reset is needed. [Details](0.68.md).
  [Observability #130 — Restrict Metrics to instrumentation](https://github.com/dragginzgame/ic-observability/issues/130).

## [0.67.0] - 2026-11-03

### ⚠️ Breaking

- Adopt Wire 0.16 codec errors and Metrics 0.28 measurement/restoration types.
  Align direct dependencies when exchanging those values. Services own report
  DTO schemas; stored rows, arithmetic and validated restoration remain unchanged ([#154 — Remove Metrics-native schema coupling](https://github.com/dragginzgame/ic-storage/issues/154)).
  [Details](0.67.md).

## [0.66.0] - 2026-11-02

### ⚠️ Breaking

- Expose Wire 0.15 codec errors while preserving checkpoint bytes and budgets.
  Align native metric schemas with the same Wire identity through Metrics 0.27.
- Adopt fallible Storage map operations and propagate collection admission errors
  through `StoreError::Storage`; host map constructors now return `Result`.

- Rename `ic-metrics-stable` to `ic-metrics-storage`. Update Cargo dependencies and Rust
  imports; the new package is a separate crates.io identity with no compatibility
  facade. [Adoption](../../docs/migration/storage-naming.md).

- Adopt Storage's owned allocator and map format. Hosts must use fresh/reset
  whole backing and rebuild against its owned traits and namespace admission;
  adapter Wire schemas remain unchanged.

- Consume allocation and runtime types from the consolidated `ic-storage` crate;
  hosts must replace their `ic-memory` imports. [Details](0.66.md).

- Adopt Metrics 0.27.0 for public measurement and restoration-error types. Hosts
  exchanging these values must align their Metrics dependency. Arithmetic,
  snapshots, limits and the Rust 1.99.1 support floor are unchanged. [Details](0.66.md).

