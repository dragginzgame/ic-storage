# Logging Storage release notes

Current releases use the Storage naming. [Earlier published history](https://github.com/dragginzgame/ic-storage/blob/23deb81c48d58558ac3b0ce36bf266ddc25dca91/releases/logging-stable/CHANGELOG.md)
retains its original package and release identities.

## [0.67.0] - Unreleased

### ⚠️ Breaking

- Integrate Storage's direct collection results. Hosts exchanging governed maps
  with Logging Storage must use the matching Storage API. Checkpoint bytes,
  retained-text admission, continuity checks and adapter results are unchanged.

## [0.66.0] - 2026-11-06

### ⚠️ Breaking

- Align governed handles and errors with Storage 0.68. Existing underlying
  collection images require host rebuild/reset or fresh allocations;
  discriminator-3 checkpoint payloads and restoration semantics are unchanged.
  [Details](0.66.md).

## [0.65.0] - 2026-11-05

### ⚠️ Breaking

- Adopt IC Wire 0.17 for logging checkpoint codec errors. Hosts exchanging Wire limits or
  errors must align their dependency to the new type identity. Codec bytes,
  budgets and stored layouts are unchanged; no backing reset is needed.
  Codec-only builds no longer select Wire's metadata derive. [Details](0.65.md).

## [0.64.1] - 2026-11-04

### 🔧 Changed

- Move retained-record fixture encoding into the codec's test module, following
  the shared test-source layout. Production codecs and checkpoint bytes are
  unchanged. [Details](0.64.md).

## [0.64.0] - 2026-11-03

### ⚠️ Breaking

- Expose Wire 0.16 codec errors while preserving checkpoint bytes and budgets.
- Adopt fallible Storage map operations and propagate collection admission errors
  through `StoreError::Storage`; host map constructors now return `Result`.

- Rename `ic-logging-stable` to `ic-logging-storage`. Update Cargo dependencies and Rust
  imports; the new package is a separate crates.io identity with no compatibility
  facade. [Adoption](../../docs/migration/storage-naming.md).

- Adopt Storage's owned allocator and map format. Hosts must use fresh/reset
  whole backing and rebuild against its owned traits and namespace admission;
  adapter Wire schemas remain unchanged.

- Consume allocation and runtime types from the consolidated `ic-storage` crate;
  hosts must replace their `ic-memory` imports. [Details](0.64.md).

- Adopt Logging 0.24.0 for public log, limit and restoration-error types. Hosts
  exchanging these values must align their Logging dependency. Checkpoint bytes,
  retention, limits and the Rust 1.99.1 support floor are unchanged. [Details](0.64.md).

