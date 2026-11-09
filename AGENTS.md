# Repository Instructions

For scope, delegated delivery, validation, compiler support and reporting, use
[the reviewed local baseline](DRAGGINZGAME.md) and its linked rules. Each maintained workspace root selects exact crates.io `toko-tooling` in its
dependency catalog; normal Cargo.lock records its
authenticated archive identity and [.shared-tooling.snapshot](.shared-tooling.snapshot)
records the authenticated package checksum and installed payload. Rust 1.99.1
remains the minimum and pinned development/release compiler. Product-specific
safeguards and scoped permissions below remain binding.

## Active shared rules and local overlay

Apply the reviewed local [Shared Tooling baseline](DRAGGINZGAME.md) and its
linked rules. [.shared-tooling.snapshot](.shared-tooling.snapshot) owns the exact
policy and executable revision. The maintainer-approved cleanup retires live
inheritance from the sibling checkout; uncommitted upstream rules do not apply
until reviewed adoption. Other repositories remain read-only unless explicitly
authorized under the shared contribution rules.

Before changing tooling, formatting, release adapters or retention, read
[the local adoption boundaries](docs/shared-tooling-adoption.md). Never patch
vendored files or attribute dirty shared source to a registry installation.
Every real package inherits `toko-tooling.workspace = true` in unconditional
`[dev-dependencies]` and records its cargo-machete exemption. Both maintained
workspace roots pin `=0.36.1`; no selection-only package or artificial Rust import
is needed. After changing the exact pins, resolve each normal Cargo.lock with
Cargo, then run `make shared-tooling-update`. Use `make shared-tooling-install`
to reinstall the existing Cargo selection. Commit declarations, Cargo lockfiles
and snapshot receipt together. Custom deployment targets must run
`shared-tooling-check` before effects. The three public package gates remain unchanged.

The virtual repository-root workspace owns shared package metadata and the
dependency catalog. The library packages are `crates/ic-storage/`, `crates/ic-metrics-storage/` and
`crates/ic-logging-storage/` adapter. Logging Storage owns bounded IC Wire checkpoints
over governed Storage maps; IC Observability's Logging owns
logging semantics and validated restoration. Logging checkpoints use discriminator 3 and an explicit retained-text ceiling.
The maintainer retired discriminator-2 rows by hard cut: hosts own explicit row
retirement or fresh allocations; no legacy decoder or automatic reset is allowed.
Hosts own clocks, identity,
retention scheduling and checkpoint cadence. All three packages participate in
workspace archive qualification; core Storage and each storage adapter release
independently with group-bound receipts and publication.
Storage owns allocation governance, bootstrap, exclusive leases and governed
collections over those leases. Allocation internals remain private modules. Metrics Storage owns validated IC Wire metric
snapshots over Storage maps and depends on the external
`ic-metrics` recording and state-validation crate. Hosts own metric attribution, lifecycle and cadence.
Storage owns allocation, bootstrap, backing custody and collections in one crate.
There is no separate Memory package or compatibility facade. Package source, tests and canonical
READMEs live under their packages.
The repository is IC Storage; Storage and the two storage adapters are the
maintained packages. Renamed adapter identities require separate publication.
The former Memory package is retired.
Follow the [workspace layout rules](rules/rust-workspaces.md).

The current hard cut uses Storage-owned memory, codecs, named directory and
collection engines. Previous manager/ledger, snapshot-directory and collection images are retired.
No legacy decoder or automatic reset is allowed. Hosts own fresh/reset whole
backing and current application framing admission. See [the current allocation
contract](docs/current-ledger.md). IDs are opaque u64 values; host policy uses
namespace grants and configured resource limits, without numeric slot pools.

Product contracts and numeric limits remain owned by this crate. Host support
and prerequisites are declared in [docs/host-support.md](docs/host-support.md).

## Stable-memory governance

Production storage integrations must obtain handles from `ic-storage` after
successful admission and durable allocation commitment. Applications and
components must not independently assign memory IDs, construct memory managers
or access unmanaged stable memory. Host backing setup and this crate's internal
directory implementation remain their explicit infrastructure boundary.
Generic storage engines and upstream collections preserve this guarantee only
when their production integrations use checked runtime handles. See the
[shared policy note](docs/shared-tooling-adoption.md#stable-memory-governance-policy)
for the upstream contribution and the `ic-storage` package integration boundary.

## Development and qualification commands

- The maintainer selected Rust 1.99.1 as both support floor and pinned
  development/release compiler in [#151](https://github.com/dragginzgame/ic-storage/issues/151).
  Both workspace catalogs and all packages must agree with `rust-toolchain.toml`.
  Admit actual
  Cargo/rustc identities, retain each distinct feature/target/host proof, and run
  otherwise identical compiler graphs once. Never delete historical tool caches.
- Before editing or compiling, check for active Cargo, rustc and rustdoc builds.
- Select checks for the affected contract: `make shared-tooling-check`,
  `make check-pins`, `make check-production-graph`,
  `make fmt-check`, or an appropriately selected Rust test such as
  `cargo test -p ic-storage --locked --offline --example repo-tool` for the native
  release helper. `make lint-tooling`, `make test-tools`, `make test-tooling`,
  `make test-release-adapters`, `make test-hooks`, `make test-fixture-cell` and
  `make test-failure-evidence` are aggregates; select their relevant scripts or
  tests when a narrower proof suffices. `make runtime-server-check` admits the
  installed server; `make test-runtime` qualifies installed IO, upgrade and
  rollback with explicitly prepared tools and both caches.
- Full repository suite: `make validate`; releases use the qualified
  [group validation mapping](docs/storage-release-validation.md) and require every
  applicable stage, under the shared
  [validation cadence](DRAGGINZGAME.md#validation-and-evidence).
  Development packaging qualifies working-tree edits; release
  admission and the separate `make package` target still require clean source.
  Release, publication and installed runtime qualification retain their separate
  authority. Never commit merely to satisfy a check.
- Dependency preparation: `make fetch-dependencies` is a separate network step.
  Both maintained workspaces track their existing selected lockfiles. The
  independent graph is prepared separately with
  `cargo fetch --locked --manifest-path testing/runtime-qualification/Cargo.toml`.
  Validation and release preparation require those lockfiles and populated
  cache, and run offline without changing dependency selection.
  The documented release entry points also authorize locked cache preparation
  before helper compilation. An explicitly offline caller stays offline; ordinary
  helper commands and validation never fetch online automatically.
- Release commands require an explicit request for the selected repository,
  release and destination. Never invoke them merely to qualify adoption.
  Tooling tests substitute Git effects; do not create commits/tags/pushes to
  satisfy fixture prerequisites, including in disposable repositories.

## Formatting and Git hooks

Follow the reviewed [Git hook rules](rules/git-hooks.md).
The reviewed hook and installer are vendored unchanged. `make fmt` and
`make fmt-check` cover the library, `testing/runtime-qualification` and the
standalone `testing/package-consumer.rs` source,
using the [selected catalog](ci/tool-versions.env) before the pinned Rust formatter.
Setup is explicit in [docs/host-support.md](docs/host-support.md); formatting never installs tools,
builds, fetches dependencies or changes selected lockfiles. The shared formatter
prerequisite checker owns exact cargo-sort and rustfmt admission; this consumer
owns the selected pin and toolchain. The Makefile's declarative `FORMAT_RUST_ARGS`
owns the two-workspace and standalone-source roster; Shared owns orchestration.
Selections containing standalone sources always run uncached.

`make install-hooks` changes only local `core.hooksPath`. The pre-commit hook
refreshes selected index entries. Exercise it only in disposable fixtures during
qualification; explicitly authorized commits may invoke it on their selected
real index entries. Preserve partial-staging refusal and unrelated work.
Source adoption, local activation and native host qualification are distinct.
The recorded installer resolves physical checkout paths, so logical checkout
aliases work without a consumer setup adapter.

## Release recovery policy

Read the reviewed [release contract](docs/releases.md).
The common vendored runner owns release ordering, locks, saved intent and Git
effects; Rust consumer adapters own metadata and qualification receipts. See
[RELEASING.md](RELEASING.md) for actual commands and evidence. Qualify canonical
runner substitutes, consumer Make dispatch and Rust adapters separately; no stub
pass proves a native or live release. Preserve failed gate logs, earlier receipts
and retained archives across retries. Never invoke a maintainer release command
to prove adoption.

## Contributions, compatibility and changelogs

Follow the reviewed shared [contribution rules](rules/contributions.md),
[hard-cut baseline](DRAGGINZGAME.md#pre-10-contracts) and
[changelog rules](rules/changelogs.md). Storage configures direct release delivery
under Shared's delegated-delivery contract. An ordinary PR does not select PR
release delivery; checkout locations follow Shared's durable-work contract.

Root `CHANGELOG.md` is an index. Each group owns `releases/<group>/CHANGELOG.md`
and new detailed notes under that directory; shared coordinated history lives
once in `releases/history.md`. Keep existing historical detail/evidence paths.
Historical documents may describe retired formats; maintained code and fixtures
use only the current contract. Do not add legacy decoders, Serde field aliases
or tests whose sole purpose is prohibiting a retired name.

## Independent runtime qualification

`testing/runtime-qualification` is the approved unpublished independent workspace.
Its own root catalog and lockfile own PocketIC integration and the private
`ic-storage-test-fixtures` crate, which shares passive protocols between its
installed test canisters and the native runner. Root library gates exclude that
graph. Size probes live under package `tests/fixtures/`, benchmark helpers under
`benches/`, and the native release helper under `tools/`.
Installed qualification retains its explicit server and Wasm
requirements. This does not authorize other nested workspaces. Follow
[the adoption boundary](docs/shared-tooling-adoption.md) when changing its setup.

## Dependency exceptions

Registry requirements remain compatible ranges. Storage uses IC Platform Runtime's
owned stable-memory API for Wasm backing IO. Native production has no
Platform/Runtime, runtime-guard or Types dependency. Wasm selects only `ic0` from
published Runtime 0.20 for safe stable64 IO; it must not activate runtime,
executor, CDK, guard, Types or Candid. No `ic-stable-structures` dependency is
allowed in the production graph. Memory, codec traits, allocation metadata and
collection integration are owned here. The maintainer selected copied ICSS 0.7.2
engines under private `src/engine`; retain provenance and notices. The public
`adapter` layer preserves governed leases with ICSS trap behavior. Ordinary
collection operations and iterator items return successful values directly;
constructors and map-region admission retain typed errors. The maintainer authorized the ICSS-derived bucket backing hard cut on 2026-11-09.
The private backing adapts published 0.7.2 allocation/translation code to named
u64 identities, eight-page buckets and Storage custody. Snapshot-directory images
are rejected without reset. No retired image readers are admitted.
Changes require appropriate native corruption, growth, custody and installed
IO/upgrade qualification. Do not restore old manager
or collection compatibility paths during the rewrite.

The independent qualification workspace consumes PocketIC through IC Platform's
optional native client boundary; its lockfile selects owned client
`ic-platform-pocket-ic` 0.19.0 with the 16.1 protocol. Testkit 0.79.0 owns the reviewed
16.1.0 server selection. Changing that pair requires repeating installed
qualification; historical 16.0.0 client/server evidence cannot qualify the new
pairing. Keep both lockfiles, sources, features and toolchains unchanged during
pinning-only adoption.

## Rust Item Documentation Style

For public structs, traits, and enums, prefer a wrapped rustdoc block with the
item name as a short heading, a blank rustdoc line before and after the heading,
and a blank source line before attributes:

```rust
///
/// SchemaMetadata
///
/// Optional diagnostic metadata for an in-place store schema.
///
/// This metadata helps humans and frameworks diagnose which schema version was
/// declared in each generation. It is bounded and validated for durable ledger
/// encoding, but it does not perform application schema migrations or validate
/// stable data semantics.
///

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaMetadata {
    /// Optional in-place schema version.
    pub schema_version: Option<u32>,
}
```

Keep this shape for new or edited item-level docs unless local context clearly
requires a different style.

## Independent release identities

`ci/cargo-release-groups.json` owns the `storage` (Storage only), `metrics-storage` and
`logging-storage` groups and their version authorities. Root `Cargo.toml` catalogs
every member; group ledgers live under `releases/`. Adapter-only changes do not
advance Storage or the other adapter. Explicit `RELEASE_GROUP` selects
release, resume and publication; whole-workspace archive integration and shared
lockfile/public-type qualification remain required. See
[group commands](RELEASING.md#release-groups) and the retained migration records
for [Storage](docs/migration/ic-persistence.md) and
[Metrics Storage](docs/migration/ic-metrics-stable.md).

## Optimisation size budget

Keep Wasm growth from an individual change within 5–10 KB unless a clearly
demonstrated exceptional benefit justifies more. Report matched before/after
Wasm bytes alongside instruction results, distinguishing benchmark harness size
from application size. Instruction savings alone do not justify retaining a
large size increase.

## Allocation consolidation

The maintainer selected a complete package/API hard cut on 2026-11-02.
`ic-storage` owns the former allocation runtime and the native repository helper.
Consumers import runtime, leases and keys from `ic_storage`; no `ic-memory`
package, re-export crate or dependency remains. The named directory replaces the fixed-slot manager and ledger by explicit
hard cut. Production runtime interfaces use StorageConfig, AllocationGrants and
opaque AllocationId values. The supported collections are Cell, Map, Set, Vector
and map regions. Copied ICSS 0.7.2 engines are private behind governed adapters;
no production upstream dependency or compatibility decoder is admitted.
Qualification and publication state belong to the current handoff.
