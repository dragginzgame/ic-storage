# Repository Instructions

Apply the installed [Toko Tooling baseline](TOKO-TOOLING.md) and its
revision-bound references. [.toko-tooling.snapshot](.toko-tooling.snapshot)
records the adopted payload; do not edit it or follow a moving sibling baseline.
This overlay owns product safeguards, local commands and approved exceptions.
The baseline owns common action authority, validation, compiler and reporting rules,
including scoped `git add -N` when validation requires new task-owned paths.

Before tooling, formatting, release-adapter or retention changes, read
[the local adoption boundaries](docs/adoption/storage-tooling.md). Both workspace
catalogs and locks must select the same tooling release and snapshot. Custom
deployment targets must run `toko-tooling-check` before effects; the three public
package gates remain unchanged.

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
Follow the [workspace layout rules](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/rust-workspaces.md).

The current hard cut uses Storage-owned memory, codecs, named directory and
collection engines. Previous manager/ledger, snapshot-directory and collection images are retired.
No legacy decoder or automatic reset is allowed. Hosts own fresh/reset whole
backing and current application framing admission. See [the current allocation
contract](docs/design/current-ledger.md). IDs are opaque u64 values; host policy uses
namespace grants and configured resource limits, without numeric slot pools.

Product contracts and numeric limits remain owned by this crate. Host support
and prerequisites are declared in [host support](docs/operations/host-support.md).

## Stable-memory governance

Production storage integrations must obtain handles from `ic-storage` after
successful admission and durable allocation commitment. Applications and
components must not independently assign memory IDs, construct memory managers
or access unmanaged stable memory. Host backing setup and this crate's internal
directory implementation remain their explicit infrastructure boundary.
Generic storage engines and upstream collections preserve this guarantee only
when their production integrations use checked runtime handles. See the
[shared policy note](docs/adoption/storage-tooling.md#stable-memory-governance-policy)
for the upstream contribution and the `ic-storage` package integration boundary.

## Development and qualification commands

- The maintainer selected Rust 1.100.0 as both support floor and pinned
  development/release compiler in [#159](https://github.com/dragginzgame/ic-storage/issues/159).
  Both workspace catalogs and all packages must agree with `rust-toolchain.toml`.
  Admit actual
  Cargo/rustc identities, retain each distinct feature/target/host proof, and run
  otherwise identical compiler graphs once. Never delete historical tool caches.
- Before editing or compiling, check for active Cargo, rustc and rustdoc builds.
- Select checks for the affected contract: `make toko-tooling-check`,
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
  [group validation mapping](docs/validation/storage-release-validation.md) and require every
  applicable stage, under the shared
  [validation cadence](TOKO-TOOLING.md#validation-and-evidence).
  Development packaging qualifies working-tree edits; release
  admission and the separate `make package` target still require clean source.
  Release, publication and installed runtime qualification retain their separate
  authority. Never commit merely to satisfy a check.
- Dependency preparation: `make fetch-dependencies` is a separate network step.
  Both maintained workspaces track their existing selected lockfiles. The
  independent graph is prepared separately with
  `cargo fetch --locked --manifest-path qualification/Cargo.toml`.
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

Follow the reviewed [Git hook rules](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/git-hooks.md).
The reviewed hook and installer are vendored unchanged. `make fmt` and
`make fmt-check` cover the library, `qualification` and the
standalone `testing/package-consumer.rs` source,
using the [selected catalog](vendor/toko-tooling/ci/tool-versions.env) before the pinned Rust formatter.
Setup is explicit in [host support](docs/operations/host-support.md); formatting never installs tools,
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

Read the reviewed [release contract](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/releases/releases.md).
The common vendored runner owns release ordering, locks, saved intent and Git
effects; Rust consumer adapters own metadata and qualification receipts. See
[RELEASING.md](RELEASING.md) for actual commands and evidence. Qualify canonical
runner substitutes, consumer Make dispatch and Rust adapters separately; no stub
pass proves a native or live release. Preserve failed gate logs, earlier receipts
and retained archives across retries. Never invoke a maintainer release command
to prove adoption.

## Contributions, compatibility and changelogs

Follow the reviewed shared [contribution rules](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/contributions.md),
[hard-cut baseline](TOKO-TOOLING.md#pre-10-contracts) and
[changelog rules](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/changelogs.md). Storage configures direct release delivery
under Shared's delegated-delivery contract. An ordinary PR does not select PR
release delivery; checkout locations follow Shared's durable-work contract.

Root `CHANGELOG.md` is an index. Each group owns `releases/<group>/CHANGELOG.md`
and new detailed notes under that directory; shared coordinated history lives
once in `releases/history.md`. Keep existing historical detail/evidence paths.
Historical documents may describe retired formats; maintained code and fixtures
use only the current contract. Do not add legacy decoders, Serde field aliases
or tests whose sole purpose is prohibiting a retired name.

## Independent runtime qualification

`qualification` is the standard unpublished independent workspace.
Its own root catalog and lockfile own PocketIC integration and the private
`ic-storage-test-fixtures` crate, which shares passive protocols between its
installed test canisters and the native runner. Root library gates exclude that
graph. Size probes live under package `tests/fixtures/`, benchmark helpers under
`benches/`, and the native release helper under `tools/`.
Installed qualification retains its explicit server and Wasm
requirements. This does not authorize other nested workspaces. Follow
[the adoption boundary](docs/adoption/storage-tooling.md) when changing its setup.

## Dependency exceptions

The maintainer authorized independent replacement of maintained DFINITY-derived
production code on 2026-11-10. Cell, Vector, Memory, named backing and the ordered
engine have been implemented through restricted handoffs; the copied engine and
private helpers are deleted. ICSS remains only an external benchmark/test reference.
Follow the [replacement records](docs/design/independent-storage-replacement.md)
and [ordered-map contract](docs/design/ordered-map-replacement.md).
Both workspace catalogs select the repository-root canonical licence through
`license-file`; every real package inherits it. Qualification, performance and
release readiness remain separate from source replacement.

Registry requirements remain compatible ranges. Storage uses IC Platform Runtime's
owned stable-memory API for Wasm backing IO. Native production has no
Platform/Runtime, runtime-guard or Types dependency. Wasm selects only `ic0` from
published Runtime 0.22 for safe stable64 IO; it must not activate runtime,
executor, CDK, guard, Types or Candid. No `ic-stable-structures` dependency is
allowed in the production graph. Memory, codec traits, allocation metadata and
collection integration are owned here. The ordered engine is private under `src/ordered_map`; Map, Set and regions
share STORMAP1 version 1. Cell admits only STORCELL version 1 and Vector only
STORVECT version 1. The named backing admits only STORNAM1 version 1, with opaque
u64 identities, eight-page units and Storage custody. Earlier images require
explicit fresh/reset backing; no compatibility reader or automatic reset is allowed.
Constructors and region admission retain typed errors; ordinary collection
operations return successful values directly and codec/engine failures trap.
Changes require native corruption, growth, custody and installed IO/upgrade
qualification. Keep the single owned engine and direct public codec integration.

The independent qualification workspace consumes Testkit 0.102.0's re-exported
PocketIC async client, with caller-owned Tokio execution, explicit five-minute
polling budgets and awaited best-effort instance deletion. The managed CLI bounds
server lifetime; the client budget is not an absolute HTTP deadline. Testkit owns
the reviewed PocketIC 16.1.0 server selection. Changing that pair requires
repeating installed qualification. The composition fixture selects Platform 0.32
and Runtime 0.22 independently; native test infrastructure must not enter
Storage's production graph. Keep both lockfiles, sources, features and toolchains
unchanged during pinning-only adoption.

## Rust Item Documentation Style

For public structs, traits and enums, use a wrapped rustdoc block: item name as
heading, blank rustdoc lines around it, a concise description, and a blank source
line before attributes. Retain this local style unless context requires otherwise.
See [Rust code hygiene](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/principles/rust-code-hygiene.md) for shared conventions.

## Independent release identities

`ci/cargo-release-groups.json` owns the `storage` (Storage only), `metrics-storage` and
`logging-storage` groups and their version authorities. Root `Cargo.toml` declares
used dependencies; `make deps-list` inventories every local package and group.
Group ledgers live under `releases/`. Adapter-only changes do not
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
and map regions. All collections use independently implemented engines behind governed adapters;
no production upstream dependency or compatibility decoder is admitted.
Qualification and publication state belong to the current handoff.
