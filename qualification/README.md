# Qualification

The independent workspace here contains host runners, native feasibility models,
canister fixtures and comparative benchmarks. Product code stays
under `crates/`; product crates never depend on this workspace. All packages
remain unpublished.

The [native feasibility experiment](ic-storage-native-feasibility/README.md) separately
tests scoped ownership, forgotten tickets and native poisoning with live host
allocations. `make native-feasibility-test` runs offline and joins CI. It does
not retain std headers or establish general heap admission.
Its `make native-layout-probe` diagnostic compiles host/wasm32 record variants,
inspects private Vec debug metadata and rejects equal-size layout equivalence;
it does not grant retained-state admission.

The [checkpoint provider experiment](ic-storage-checkpoint-provider-probe/README.md) uses
published Storage 0.74.1 through governed handles. Ordinary installed upgrades
discard the source heap and reconstruct scalar/vector values from the bounded
checkpoint. It exercises independent opening and failed-import rollback, not
retained native allocation validity. Run `make checkpoint-provider-lint`,
`make checkpoint-provider-build` and `make checkpoint-provider-test`.
The [native bridge](ic-storage-retention-probe/native-checkpoint.md), exercised by
`make native-checkpoint-test`, now connects retained values to that same provider
and reconstructs a relocated native allocation after explicit heap replacement.
The [governed job profile](ic-storage-native-feasibility/migration-job.md), run with
`make native-job-test`, adds durable epochs, cancellation, frozen resumption,
staged import and explicit checkpoint-based restart of unfinished imports.

The [record profile](ic-storage-native-feasibility/records.md), run with
`make native-record-test`, adds installed V1→V2 field addition and checked
V2→V3 narrowing, rollback and portable recovery while preserving the vector.

These fixtures cover M0 and a bounded native scalar/vector contract, including
installed lifecycle and portable migration between two allocation domains.
General records, private collection layouts and actual compiler transitions
remain outside that evidence. The [current design](../docs/DESIGN.md) owns the
supported runtime policy and implementation priorities.

The [provider-custody protocol](ic-storage-retention-probe/provider-custody.md) adds
WAT-only region claim/settlement and refusal/rollback cases to `make retention-test`.
It tests custody across installed upgrades without claiming Rust pointer validity.
`make native-provider-probe` separately combines the Rust scalar diagnostic with
that WAT source and checks artifact admission. It executes no combined Wasm.
`make native-provider-test` adds the installed scalar lifecycle under its explicit
[foreign-allocation contract](ic-storage-native-feasibility/scalar-provider-contract.md),
using the prepared server. The vector profile has its own bounded admission below.
The [provider-region controller](ic-storage-native-feasibility/provider-region.md)
adds host issuance/growth/reuse tests to `make native-feasibility-test` and
vector artifact checks to `make native-provider-probe`. The separate
[vector link contract](ic-storage-retention-probe/vector-link.md) now permits its
bounded installed lifecycle through `make native-provider-test`; scalar admission
continues to reject that vector module. General native-heap admission remains open.
The [vector metadata protocol](ic-storage-retention-probe/vector-provider.md) adds
installed full-width descriptor and settlement checks to `make retention-test`;
its canisters contain no Rust vector code.

## First experiment: retained scalar

The Rust runner compiles independently authored WAT fixtures through `wat`,
validates complete core modules through `wasmparser`, and installs them through
`ic-testkit` on PocketIC. Bounded byte reads and complete SHA-256 identities use
the replacement `ic-host-artifacts` 0.80.1. This
isolates the IC retention mechanism before introducing Rust canister startup,
an allocator or a graph. WAT's explicit `ic0` imports are protocol fixtures, not
a second Rust SDK; product bindings still belong to Platform Runtime.

The qualification-owned artifact adapter preserves source/module byte limits,
core-module refusal, full instruction/type validation and lowercase evidence
digests. `wat` stays in the 1.263 patch line alongside the selected 0.263 parser
and encoder. `make retention-lint` runs its focused contract tests. The provider
collector retains these shared host sources along with the calling test sources.

The simulator remains on `ic-testkit` 0.106.0 and its prepared server CLI. Its
transitive Artifacts/FS/Process 0.79 dependencies remain locked to that supplier;
this migration removes direct legacy Host use but does not migrate Testkit's
server lifecycle or claim a completely legacy-free graph. The new Host byte API
has no gzip or simulator CLI replacement. Do not force incompatible Host versions
into Testkit or add direct FS/Process dependencies that these callers do not use.

The 0.80.1 migration passed the artifact contract tests, strict runner/benchmark
Clippy, dependency pins/graph boundaries and Rustdoc. Installed retention,
startup, graph, ordinary checkpoint, native record, native checkpoint and migration
job suites also passed locally. The final provider cohorts are
`attempt.2aFwPS` (record), `attempt.gZsWLy` (checkpoint) and `attempt.M4OBEX` (job)
under `target/evidence/provider-link/`, each with COMPLETE and the shared host
source capture. These are local PocketIC outcomes, not native macOS evidence.

The two-page wasm32 fixture layout is deliberately smaller than the proposed
managed heap and is not its persistence ABI. Page zero is transient. At 65536
is a little-endian 64-bit magic word (`0x4f50524f42453031`); at 65544 is a 64-bit
counter. A query returns four little-endian words: magic, counter, mutable global
and transient-memory sentinel. Passive data and `(start)` initialize only the
transient sentinel. `canister_init` initializes fresh storage;
`canister_post_upgrade` rejects an unrecognized magic word without resetting it.
There is no stable-memory import or save/load pipeline.

The test exercises:

- Fresh install of A, then a committed counter/global/sentinel update.
- Rejection of a different ABI, preserving old observed state and module hash.
- A post-upgrade write followed by a trap, rolling back code and observed state.
- A→B with explicit `keep`: counter retained, globals freshly initialized,
  transient data restored by startup, and fresh-install initialization not run.
- Another update under B, then B→A using `keep`, never reinstall.
- A committed zero marker followed by rejected upgrade: corruption remains
  visible rather than being silently reset. This does not prove usability of
  corrupt application data.

The fixtures have no outgoing calls, timers or executor. Each upgrade follows
a completed management stop request and records the installed module hash.
The [IC interface specification](https://docs.internetcomputer.org/references/ic-interface-spec/abstract-behavior/)
defines explicit retention and the required EOP private custom section.

## Run

From the repository root, prepare inputs explicitly:

```sh
cargo fetch --manifest-path qualification/Cargo.toml --locked
make retention-setup
```

The setup target selects the Testkit executable from this workspace's lockfile
through Toko's canonical installer, then prepares its pinned server asset.
No test downloads a server, installs tools or updates a lockfile.

```sh
make retention-tools-check
make retention-lint
make retention-test
```

Build output belongs under `target/qualification/`; raw experimental output
belongs under `target/evidence/`. `make fmt` and `make fmt-check` include this
workspace. `make deps-list` and `make deps-update` discover it through Toko's
canonical inventory. The qualification catalog selected the latest stable,
non-yanked registry releases on 15 November 2026: Testkit 0.106.0, Host Artifacts
0.79.0 and Tokio 1.54.1. Tooling remains the admitted exact 0.44.4 pin.

## Evidence boundary

On 15 November 2026, `make retention-test` passed (one installed test, no ignored
tests) on x86_64 Linux with Rust 1.100.0 (`0ef1bc386`) and PocketIC 16.1.0. Every
scenario above ran, including typed `CanisterTrapped` assertions for the rejected
upgrades. Strict Clippy and dependency policy checks also passed.

At that WAT run, the selected qualification lockfile SHA-256 was
`c84808a07daeb66290d9261316204a52a84536a18f59e42b4b6071ebdc96c4a8`.
The admitted server executable SHA-256 was
`b44e1eccd66e02328146b3e209e3403e3a3428a25d25555aca2f67500e5ae9db`.
Local raw output and source/input hashes are under
`target/evidence/retention-test.log` and `target/evidence/retention-inputs.txt`.
The initial sandboxed attempt could not bind a loopback port; the passing run
used the same offline command with local networking allowed.

PocketIC execution is local installed-canister evidence for this server cohort,
not a production subnet deployment or native macOS execution claim. CI selects
the test on Linux and both macOS architectures, but those hosted results have
not been observed for this change.

This WAT test does not qualify Rust startup/linking, grown memory, stack/BSS
boundaries, a complete retained manifest, bounded graph admission, allocation,
migration, callbacks, or resource costs. The subsequent
[Rust startup experiment](ic-storage-rust-startup-probe/README.md) records its own inputs,
admission checks, installed results and remaining gates.

The separate [managed graph profile](ic-storage-rust-startup-probe/managed-graph.md) now
depends on the product crate by a workspace-owned path. It exercises the actual
M0 representation through installed retaining upgrades, shared references,
cycles, page-crossing allocation and selected rollback boundaries. Its unsafe
custody kernel is confined to the qualification fixture; product code remains
`no_std` and forbids unsafe Rust.

The [simple structure benchmarks](ic-storage-simple-benchmarks/README.md) compare this M0
kernel with published Storage 0.74.1 using installed scalar and sequence
workloads. `make benchmark-simple` records 216 checked instruction measurements;
it is opt-in and does not impose a CI timing threshold. Artifact admission is
shared through the retention runner library, with a separate import/export
profile for the benchmark canister.
