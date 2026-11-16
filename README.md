# IC Storage New

A fresh experiment in **Rust Orthogonal Persistence**, inspired by Motoko's
persistence model on the Internet Computer.

The selected direction is **native retained Rust collections with explicit
type migration and a portable recovery path**. Compatible upgrades retain state
directly; schema changes use versioned conversions; incompatible runtime or heap
layouts require export under a compatible source executable and reconstruction
under the target. Initially support one explicit runtime contract. Major runtime
changes may use planned downtime and an operator-run export/import procedure;
general compiler portability is not a prerequisite for the application API.
See the [current design](docs/DESIGN.md#supported-runtime-contract).

The implementation is partial: an unpublished [scoped native vector core](crates/ic-storage-native/README.md)
and installed scalar/vector lifecycle and migration fixtures provide the current
native foundation. A bounded scalar `Store` derive supplies metadata, logical
codecs and scoped getters; qualification fixtures exercise explicit field addition
and checked scalar narrowing, including rollback and portable recovery. Owning
records and general application conversion remain planned. The byte-defined M0
memory-image model remains an evidence baseline, not the native collection ABI or a
complete persistence system. Its [earlier research plan](docs/design/m0-research-plan.md)
is archived; its format and historical results remain scoped to that experiment.

This is the intended successor to `ic-storage`, aiming to subsume the persistence
capabilities our applications need. The existing repository is on hold and stays
unchanged. Replacement requires capability and migration qualification; there is
no current package, API or stored-format compatibility claim. See the
[successor rule](docs/DESIGN.md#successor-rule).

## Development

Use the pinned Rust 1.100.0 toolchain (also the support floor). The M0 library is
`no_std` and allocation-free; the native core uses `no_std` plus `alloc` with a
caller-supplied allocator. M0's root-owned SHA-256 dependency is `sha2` 0.11.1
with defaults disabled, selected from the latest stable registry release on
15 November 2026. The root selects Toko Tooling 0.44.5 exactly as a
development dependency. Its authenticated snapshot is installed through the
canonical exporter under `vendor/toko-tooling/`; [TOKO-TOOLING.md](TOKO-TOOLING.md)
owns the common rules and links to immutable upstream references.

Explicit setup and offline admission are separate:

```sh
make install-tools
make tools-check
cargo fetch --locked
make install-hooks
```

Common tools are selected through `.tools/host/bin` and
`.tools/rust/common/bin`; Make sets PATH automatically. Add those directories
to PATH for interactive use. Compiler setup remains explicit; checks never
download a compiler or dependencies.

Focused development commands:

```sh
make check
make fmt-check
make lint
make m0-test
make m0-lint-wasm
make native-feasibility-test
make native-layout-probe
make doc-check
make deps-list
make deps-tree
make deps-boundaries
make toko-tooling-check

# First installed Wasm retention experiment (explicit setup, then offline):
cargo fetch --manifest-path qualification/Cargo.toml --locked
make retention-setup
make retention-tools-check
make retention-lint
make retention-test
make rust-startup-lint
make rust-startup-build
make rust-startup-test
make managed-graph-lint
make managed-graph-build
make managed-graph-test

# Optional installed instruction measurements against published ic-storage:
make benchmark-simple-lint
make benchmark-simple
```

`make deps-check` aggregates snapshot admission, declaration/pinning checks and
dependency boundaries. `make deps-pins` requires Cargo.lock to be tracked by Git.
`deps-boundaries` checks
direct IC dependency policy and excludes tooling from the normal/build graph
across all targets. Neither check proves installed IC behavior.

`make deps-list` uses Toko's package inventory (local versions, workspaces and
release groups); `make deps-tree` shows external dependencies too.
`make deps-update` uses Toko's updater for the root and independent qualification
workspace. It explicitly permits network access and updates Cargo.lock
within root requirements. Use `DEPENDENCY_PACKAGE=name` to select one package,
with optional `DEPENDENCY_PRECISE=version`. Review the diff. To change Toko versions, edit the
exact root pin, update Cargo.lock, then run `make toko-tooling-update` followed
by `make toko-tooling-check`; builds never install snapshots implicitly.

`make ci` checks formatting, dependency policy, strict Clippy for the product and
qualification runner/canister, native M0 graph/corruption and session-isolation
tests, wasm32 library checks
and Rustdoc. GitHub Actions runs it and the
retention/startup/graph tests on Linux and both macOS architectures after explicit setup, with
one separate Gitleaks scan per run.
The separate `retention-test` runs the first installed Wasm probe through
Testkit/PocketIC. The [Rust startup probe](qualification/ic-storage-rust-startup-probe/README.md)
adds controlled transient initialization and artifact admission; neither is a
managed heap implementation. Its separate
[managed graph profile](qualification/ic-storage-rust-startup-probe/managed-graph.md) uses the
product library and a fixture-only retained-memory kernel to test shared/cyclic
graphs, allocator continuation and selected installed rollback cases. Local
development uses the individual commands above. The canonical pre-commit hook
formats the selected commit payload without modifying unrelated work.

[Simple structure benchmarks](qualification/ic-storage-simple-benchmarks/README.md) compare
the current M0 scalar and linked sequence with Storage 0.74.1's cell and vector.
They report IC instruction counts and check complete state after every sample;
these are implementation costs, not raw memory-speed measurements.

The [native ownership experiment](qualification/ic-storage-native-feasibility/README.md)
tests scoped moves, owner escape refusals and forgotten-ticket quarantine. It
uses live host allocations. A compiler diagnostic probe also measures private
Vec header fields on the host and wasm32 and detects equal-sized reordered
records; complete native layout admission and reopening remain open.
Its [live allocation probe](qualification/ic-storage-native-feasibility/allocation-boundary.md)
also tests real std vectors, allocation reuse and reopening within the same live
host arena. It does not establish allocation custody across an IC upgrade.
A [bounded checkpoint codec](qualification/ic-storage-native-feasibility/checkpoint-format.md)
tests reconstruction after source teardown. The
[installed provider probe](qualification/ic-storage-checkpoint-provider-probe/README.md)
recovers through fresh Storage handles after ordinary upgrades discard the old
heap, with failed-import rollback tests. The
[connected native bridge](qualification/ic-storage-retention-probe/native-checkpoint.md)
now tests compatible reopening, governed export and relocated native recovery;
general runtime migration and production authority remain open.
The [governed job fixture](qualification/ic-storage-native-feasibility/migration-job.md)
adds guarded jobs, cancellation, frozen-source upgrades and staged import.
The shared [native core](crates/ic-storage-native/README.md) now supplies scoped
vector operations to that fixture; `make native-core-test` checks ownership limits
and the optional scalar derive's application-facing contract.

Library code lives in `crates/`. See [qualification/](qualification/README.md)
for fixture contracts, commands and evidence scope. Production canister
qualification, publication and deployment are not configured. The package
remains `publish = false`. The GitHub repository is
[`dragginzgame/ic-storage-new`](https://github.com/dragginzgame/ic-storage-new).

Read [AGENTS.md](AGENTS.md) and the
[research architecture](docs/DESIGN.md) before implementation.
The design draws on Motoko and the
[pinned cdk-rsx-store interface description](docs/DESIGN.md#pinned-cdk-rsx-store-design-basis).
