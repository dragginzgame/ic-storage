# Host support and qualification

IC Storage supports native development and maintainer workflows on these hosts.
Its canister runtime target remains `wasm32-unknown-unknown`.

| Host | Architecture | Native CI label | Qualification |
| --- | --- | --- | --- |
| Ubuntu 24.04 | x86-64 | `ubuntu-24.04` | 0.25.12 development gate and Rust 1.88.0 all-target check passed |
| macOS 15 | ARM64 | `macos-15` | 0.25.12 development gate and Rust 1.88.0 all-target check passed |
| macOS 15 | x86-64 | `macos-15-intel` | 0.25.12 development gate and Rust 1.88.0 all-target check passed |

The passing development gates and MSRV checks are recorded in
[CI run 37323134668](https://github.com/dragginzgame/ic-memory/actions/runs/37323134668)
for source `5481bc765b9c7bed4cfb928c9efa9d1aa6838a22`, with selected lockfile
SHA-256 `b67be11d2a175dc0ab35cb1429548a7f5dfb1adc030d3693d9348bfd1a51e2d0`.
The development jobs used Rust 1.99.0 for native tooling tests and the full gate
described below. Separate MSRV jobs logged and used Rust 1.88.0 for all-target
compilation. Subsequent changes, including the common release workflow, require their own
matching native CI evidence.

The earlier
[0.25.11 run](https://github.com/dragginzgame/ic-memory/actions/runs/37320179267)
does **not** qualify Rust 1.88.0 on any host. Its MSRV jobs installed 1.88.0, but
the repository toolchain pin selected 1.99.0 for the dependency and compilation
commands, as recorded by `rustc -Vv` in their logs. The corrected 0.25.12 workflow
sets `RUSTUP_TOOLCHAIN` explicitly for those commands.

The labels follow the [GitHub runner image matrix](https://github.com/actions/runner-images/blob/main/README.md).
Configured jobs alone are not passing evidence. Linux and cross-compilation
results do not qualify native macOS behavior. Link matching successful CI runs
when recording release qualification; retain failed runs and their limitations.

## Prerequisites

The maintainer selected Rust 1.100.0 as both the minimum supported compiler and
pinned development/release compiler in [#159](https://github.com/dragginzgame/ic-storage/issues/159).
Both workspace catalogs and every package must match `rust-toolchain.toml`.
`make validation-compilers-check` admits installed Cargo/rustc identities offline,
rejecting mismatched floors/selectors before archive work. It installs nothing.
The historical matrix evidence above retains its original compiler identities;
current native macOS acceptance remains separate from local Linux evidence.

- Rustup with Rust 1.100.0, Clippy, rustfmt and `wasm32-unknown-unknown`.
- Git, GNU Make, Bash 3.2 or newer, curl, tar with gzip/xz support, Perl,
  `sed`, `awk`, and either `sha256sum` or `shasum` with SHA-256 support.
  On macOS, install Xcode Command Line Tools for native compilation and Git.
  The system `make` is sufficient when it is GNU Make; Homebrew `gmake` is an
  alternative. Do not substitute BSD Make.
- The tracked root and independent runtime-qualification `Cargo.lock` files,
  with cached dependencies before offline validation. In a fresh checkout run
  `make fetch-dependencies` for the root selection. Prepare the independent graph
  separately with `cargo fetch --locked --manifest-path qualification/Cargo.toml`
  when installed qualification is needed. Preserve both selections; never
  regenerate them to make a check pass.
- The common jq/yq/ripgrep/cloc set under `.tools/host/bin` for host checks and
  reports; jq/yq support `make check-pins`
  with the actual workspace and exception catalog. The reviewed snapshot owns `ci/tool-versions.env`.
  Interrupted manifest/lockfile recovery also uses yq to locate Cargo's selected
  target directory. Checks are offline and never install prerequisites.
- Actionlint and ShellCheck for `make lint-tooling`. CI installs exact
  snapshot versions and hashes from `ci/tool-versions.env` using the
  reviewed shared installers. Local installation is a separate network step.
- Hosted evidence readback uses `gh` and `unzip` from the declared runner images.
  The command requires an explicit token with `actions:read`; local portable
  checks substitute GitHub responses and need no credentials or network.
- Crates.io credentials are needed only for maintainer publication. Tests use
  deterministic command substitutes and never need credentials or network.

## Developer setup and formatting

Prepare the complete local common toolset after installing the declared Rust
toolchain, then activate the reviewed hook once per clone:

```sh
make install-tools
make tools-check
make install-hooks
git config --get core.hooksPath # .githooks
make fmt-check
```

CI uses this same setup, including local cargo-sort 2.1.4.
`make fmt` sorts manifests and
formats Rust in both the root workspace and `qualification`.
`make fmt-check` checks the same inputs without changing them. Neither command
builds, installs tools, fetches dependencies or changes selected lockfiles.
Both first use the shared prerequisite checker to require the exact cargo-sort
pin and available rustfmt under the selected validation toolchain. Admission is
offline and probes versions only; unavailable tools fail before formatting.
Bare `make` prints available commands rather than preparing dependencies.

Common setup covers host and Cargo tools. Memory explicitly selects the IC
bundle through the ordered local tool targets, retaining all three sets:

```sh
make install-tools
make tools-check
export PATH="$PWD/.tools/host/bin:$PWD/.tools/ic/bin:$PWD/.tools/rust/common/bin:$PATH"
make check-pins
```

Make selects those local paths automatically. Setup does not edit shell profiles.
Before installing common tools, aggregate setup checks the Rust catalog and
probes Rust/Cargo without automatic toolchain installation. The selected IC
bundle performs its own platform/catalog admission after common setup.
Prepare a supported host and the declared toolchain before retrying a refusal.
Offline host checks identify the failing tool, expected version, selected path
and repair command; they still authenticate payloads before execution.
The reviewed `make/consumer.mk` selects the common modules for setup, checks,
formatting, hooks and releases; `make/tools.mk` owns installation, offline checks
and LOC targets. Memory supplies both-workspace formatter commands and its pinned
compiler. Use `make toko-tooling-check` for snapshot integrity and
`make format-tools-check` for prepared formatter admission.
Existing jq/yq/ripgrep-only checkouts must run `make install-host-tools` once to
add pinned cloc 2.10, then `make host-tools-check` before offline qualification.
See [common local setup](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/adoption/local-setup.md) for system bootstrap packages and
[IC tools](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/tools/ic-tools.md) for selection and admission. Runtime qualification selects
only `wasm-opt` through `IC_TOOLS`; direct IC installer calls also pass
`--tool wasm-opt`. Run explicit `make install-ic-tools` after adoption to prepare
this selection. Earlier bundles and failed attempts remain retained.
PocketIC setup belongs to Testkit and is separate from `install-tools`.
Host and IC setup activate independently. Verified payloads and failed candidates
live in the shared per-user store, with retained references under `.tools/`;
earlier physical local bundles remain intact. `install-host-tools` and
`install-ic-tools` can prepare either set separately, with offline checks through
`host-tools-check` and `ic-tools-check`. Ordinary validation never installs tools.
CI explicitly installs and checks both sets on each declared native test host;
configured jobs alone do not qualify this adoption. The pin checker requires locks
already tracked by Git. Adopt new locks through an explicitly authorized commit
or PR before the real-checkout declaration/release gate can pass; qualification
alone does not authorize staging them.

Installed runtime qualification uses the published `ic-testkit` 0.101.0 server CLI
selected in `ci/runtime-tools.env`, installed through Shared Tooling's exact Cargo
binary installer. It has a separate locked registry tool graph, outside both
workspace lockfiles. Prepare it and Testkit's authenticated server selection explicitly:

```sh
make install-runtime-server
make runtime-server-check
make install-ic-tools
cargo +1.100.0 fetch --locked --manifest-path qualification/Cargo.toml
make test-runtime
```

Prepare the root cache with `make fetch-dependencies` too. The offline check
prints Testkit's admitted absolute server path. `test-runtime` builds both supplied
artifacts offline, then qualifies the original Wasm and the selected Binaryen
133 `-O3`, `-Os` and `-Oz` outputs. Each receives the same startup, IO rejection
and upgrade rollback checks through Testkit's managed `run` contract. Generated
Wasm, input hashes and all optimizer/server/runtime logs remain under
`target/qualification/runtime/`, including failures. Normal library builds and
their Wasm budgets remain unoptimized. The runner consumes
`IC_TESTKIT_POCKET_IC_URL`; it never downloads or
spawns a server. Testkit 0.101.0 re-exports PocketIC's async client and
selects server 16.1.0. Drivers own a current-thread Tokio runtime, explicit
five-minute polling budgets and awaited best-effort instance deletion. The
managed CLI bounds server lifetime; polling budgets are not absolute HTTP
deadlines, and deletion does not guarantee server acknowledgement. This pairing
requires installed IO/upgrade qualification on Linux and both macOS hosts, supplied by the separate
runtime CI jobs. Historical 16.0.0 client/server results remain scoped to their
original inputs. Failed CLI setup, server attempts and runtime logs remain
available; no old bundle is converted or deleted. Library and release gates do
not acquire this server dependency.

### Platform 0.28 / Runtime 0.21 adoption

The 2026-11-09 adoption selects Runtime 0.21 for the production `ic0` path and
Platform 0.28 / Types 0.19 for the independent runtime-composition fixture.
Platform's removed PocketIC facade is replaced by Testkit 0.95's maintained
async client. Testkit still selects Platform 0.27 / Types 0.18 transitively;
those native infrastructure identities are deliberately independent of the
composition fixture, not a second production executor or product dependency.

The root lock changes only Runtime (0.20.3 → 0.21.0). The independent graph
removes `ic-platform-pocket-ic`, adds Testkit and resolves the required supplier
versions. Tokio 1.54.1 supplies the caller-owned current-thread runtime. Tests
remain sequential, use explicit 300-second request budgets and await instance
deletion. The active graph gate now verifies Testkit ownership of the native
qualifier. No synchronous wrapper, alias facade or old client path remains.

Focused Linux evidence: `target/evidence/platform-028/`. Native/Wasm graph
admission and strict selected Clippy pass, including isolated default and full
Runtime composition. Forty-two native library/driver tests pass; the two
explicit installed tests remain opt-in, with their scenarios exercised by the
installed suite. The original, `-O3`, `-Os` and `-Oz` fixtures pass IO, corruption,
backing custody, collection/logging upgrades and rollback checks in
`target/qualification/runtime/attempt.sG98t5`. No new macOS proof is claimed.

All eight matched size-consumer pairs pass 118 calls and two upgrades per
backend (`target/evidence/storage-consumer-size/attempt.2K9aQP`). No code-section
payload grows or shrinks. All-collection Storage stays 136,284 total / 117,787
code bytes; ICSS remains 119,326 / 77,127. Map, Set, Region and Map+Region total
modules each shrink one byte; other total sizes are unchanged. This is not an
instruction-speed claim.

The migrated native comparison driver passes a focused Map smoke run. Replaying
the frozen admission Wasm through the new async driver reproduces all 216
previous observations exactly, including instructions, sizes and sentinel
checks. Those frozen-Wasm runs qualify client migration, not new Runtime code;
the rebuilt installed/size fixtures qualify Runtime 0.21. Tooling integrity,
pins, Candid boundary, formatting, local links and the dependency-admission
failure fixtures pass. Existing optimization source is preserved. Release and
publication gates remain separate; no deployment is included.

For a focused Storage adapter change, build only its fixture and run the ignored
installed adapter test through the same prepared Testkit server. This covers cell
load rejection, byte/page preservation, leases and update/upgrade rollback without
replaying Memory's IO suite or the release optimizer matrix:

```sh
cargo +1.100.0 build --manifest-path qualification/Cargo.toml \
  --target-dir target -p ic-storage-test-fixtures --locked --offline --profile wasm-size \
  --target wasm32-unknown-unknown --example wasm-storage-qualification
testkit_cli="$(RUSTUP_TOOLCHAIN=1.100.0 bash scripts/dev/install-rust-tools.sh \
  --consumer "$PWD" --package ic-testkit --version 0.101.0 \
  --bin ic-testkit-server --profile release --features cli --check)"
IC_STORAGE_QUALIFICATION_WASM="$PWD/target/wasm32-unknown-unknown/wasm-size/examples/wasm_storage_qualification.wasm" \
  "$testkit_cli" run --directory "$PWD/.tools/ic-testkit-server" --ttl 900 -- \
  cargo +1.100.0 test --locked --offline \
    --manifest-path qualification/Cargo.toml \
    --target-dir target/runtime-qualification storage::tests::installed_storage_adapters \
    -- --ignored --nocapture
```

Retain the supplied Wasm, source hashes and Testkit server/test output with the
task's qualification evidence. The ignored test requires explicit server URL and
fixture inputs; ordinary tests never prepare or start a server.

For backing-custody changes, build the `ic-storage-test-fixtures` example
`wasm-io-qualification` for the same Wasm target/profile and pass its artifact as
`IC_MEMORY_QUALIFICATION_WASM`. Through the same prepared Testkit `run` command,
select `ownership::tests::installed_backing_custody -- --ignored --nocapture`.
This qualifies two same-canister participants, duplicate default-backing refusal,
handles retained after runtime drop, final-lease release and upgrade reopening.
The normal installed runner includes this proof alongside its existing IO and
Storage qualification. Custom host backings must implement `MemoryBacking` with
one consistent physical identity; generic `Memory` alone is insufficient.

`make cloc` reports the root library workspace by default using locked, offline
Cargo metadata; it neither builds nor installs tools. Select the independent
qualification graph explicitly with
`make cloc CLOC_MANIFEST=qualification/Cargo.toml`. Each report
uses only the selected workspace and its target configuration; reports do not
combine the two graphs. `CLOC_ROOT` still selects a containing checkout, rather
than an independent manifest. The selector is the reviewed fix for
[Shared Tooling #41](https://github.com/dragginzgame/toko-tooling/issues/41).
Fleet reports run centrally in Shared Tooling: use `make cloc` there for sibling
Rust workspaces and `make cloc-tooling` there for sibling CI/tooling. Memory no
longer vendors either fleet reporter. Its shared Make include reports that owner
if an omitted optional fleet report is requested; it never invokes a sibling
implicitly. Local workspace reports and their independent-manifest selection
remain available.

The LOC selector and generic manifest/output fixture qualify at Shared Tooling.
Actual reports retain their selected workspace configuration. These fixes track
[Shared Tooling #47](https://github.com/dragginzgame/toko-tooling/issues/47) and
[#48](https://github.com/dragginzgame/toko-tooling/issues/48) without patching
reviewed exports. CI continues to retain scratch under `$RUNNER_TEMP`.
Fleet and cross-workspace fixture qualification remains in Shared Tooling.
Memory no longer selects or dispatches those three upstream regression programs;
its local workspace fixture and workspace report remain available. The two fleet
reporter copies are also retired at the maintainer's explicit selection.
See [#34](https://github.com/dragginzgame/ic-memory/issues/34).

The common aggregate includes cargo-sort and cargo-sort-derives alongside the
host tools and selected `wasm-opt`. Memory additionally selects the published
IC Build release editor through `ci/release-tools.env`; `make install-release-tools` and
`make release-tools-check` prepare/admit it explicitly. Candid extraction is no
longer a common Cargo tool selection. Narrow
`make install-rust-tools` and offline `make rust-tools-check` remain useful for
explicit setup retries; they do not establish readiness of the complete set.
`ci/runtime-tools.env` owns the separately prepared Testkit CLI version. Runtime
CI caches bind that pin and the IC catalog to Shared Tooling's compiler, native
environment and installer identity; tool admission still checks restored bytes before qualification.
The snapshot's `ci/tool-versions.env` owns the common pins. Host, IC and Cargo
setup use independent snapshot defaults; Memory has no exception catalog.
Shared Tooling 0.6.1 stores Cargo
builds and receipts in its per-user store, selected through checkout-local
references. Generations bind the exact prepared compiler, native environment,
published crate source and packaged lockfile. Tool builds use a standard
environment that excludes application flags, wrappers and arbitrary Cargo
configuration. The managed source cache must remain available for offline
checks and subsequent setup. Setup never installs a compiler implicitly.
Normal validation checks all common tools offline before builds, and formatting
retains its exact two-workspace rustfmt/cargo-sort checks.

Make selects `.tools/rust/common/bin` itself; interactive shells use the export
above. After adopting 0.6.1, run `make install-rust-tools` followed by
`make rust-tools-check`, after host setup. Also run `make install-runtime-server`
and `make runtime-server-check` for Memory's independently selected Testkit CLI.
The adapters use its installer-returned path; the selected CLI/server versions
remain unchanged. Existing physical slots, shared generations and failed build
evidence remain retained. Older local slots do not satisfy the new offline check.
Shared 0.6.1 stops setup on lock, admission and activation failures, preserving
the original status and existing selections. Always run the offline check after
setup. See [Shared #120](https://github.com/dragginzgame/toko-tooling/issues/120).
See [Rust setup](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/tools/tool-inventory.md#rust-development-tools).

Verified host/IC bundles use the same per-user store; command paths stay
`.tools/host/bin` and `.tools/ic/bin`. Use the same `TOKO_TOOLING_STORE` setting
for setup, offline checks and failure collection. Never relocate binaries manually
or select a sibling checkout's PATH. See
[shared storage configuration](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/adoption/local-setup.md#shared-executable-storage).
The shared selector owns both the complete local Rust tree and its admitted
shared generations. Memory adds its product logs and server evidence without
duplicating Rust selection. Failed store admission retains safe local evidence
and diagnostics without following unadmitted references.
Fixture substitutes qualify routing, refusal, Rust-only selection and archive
deduplication; actual setup/check and native CI supply installed-tool evidence.

`make check-pins` also checks Cargo inheritance against each manifest's owning
workspace catalog. The independent runtime-qualification root keeps its approved
scope and lockfile; it is not merged into the library graph. `make check-pins`
reuses the shared metadata fixture for ordinary/inline dependency tables,
aliases, target/dev/build declarations, rejected child overrides and independent
workspace discovery. Its workspace-version reader is a fixture dependency;
the Rust release adapter still owns this crate's `workspace.package.version`.

The actionlint and ShellCheck setup commands use the shared
`install-ci-tool.sh TOOL` entrypoint directly. Existing arguments and pins are unchanged.
Rejected checksums or versions preserve the selected executable and retain the
failed candidate. Snapshot integrity and focused Linux checks are separate from
native macOS qualification; see the
[adoption evidence](../release-workflow-qualification.md#0283-shared-cargo-and-installer-adoption).
Host installation and offline verification always authenticate the complete
jq/yq/ripgrep/cloc set, including ripgrep 15.2.0 and its PCRE2 support
alongside jq/yq; CI uses this same set rather than a separate package-manager
installation. Prepare the expanded host set with `make install-host-tools`
before offline checks. Previous selections and rejected candidates are retained.

The pre-commit hook formats an export of the exact index and refreshes only the
selected files. It refuses partial staging and preserves unrelated working
edits; a formatter failure leaves the real index and files unchanged. The
installer refuses conflicting hook settings or executable private hooks rather
than silently replacing them. Reconcile those obligations before activating it.
The recorded shared installer resolves the physical checkout path, including
when entered through a logical alias such as macOS temporary paths.
CI and release preparation check formatting independently of hook activation.
The shared Make execution check refuses inherited ignore-errors, dry-run,
question, touch and version-only modes before hook or release validation dispatch.
Rerun without those modes; ordinary Make selections and jobserver settings remain
inherited by the real targets.
`make test-hooks` exercises selected-file refresh, partial source/config staging,
formatter failure isolation, inherited Make mode rejection, selection forwarding
and preservation of unrelated edits in disposable
repositories without creating commits or tags, including setup through a checkout
path alias. It uses the actual consumer
formatting targets and retains missing/wrong formatter rejection. It
runs on every declared native CI host. See the
[0.28.4 focused evidence](../release-workflow-qualification.md#0284-portable-fixtures-and-formatter-admission).
Linux hook evidence
does not qualify native macOS; record matching native runs separately from the
historical runtime gates above.

## Native checks

Focused tooling checks are `make toko-tooling-check tools-check test-tools check-pins test-tooling test-hooks fmt-check`,
`make test-release-adapters` and `make lint-tooling`. Compilation must wait for any existing build to finish.

CI runs `make validate-toolchain` on each declared host. Its feature-enabled
native all-target check retains the former MSRV matrix's distinct proof; default
library, Wasm and archive consumers remain separately qualified. The full gate covers library/integration/compile-fail tests,
doctests, strict Clippy, warning-denied documentation, Wasm compilation and raw
size budgets, and package verification. Local development uses focused checks;
full validation requires explicit authority under the
[shared cadence](../../TOKO-TOOLING.md#validation-and-evidence). Release, publication
and installed runtime qualification retain their separate authority.

Release-flow tests separately cover Rust adapter recovery/input binding, actual
Make dispatch/log retention through substituted command effects. Generic runner
reconciliation qualifies at Shared Tooling. See the [focused evidence](../release-workflow-qualification.md).
They do not prove a live push or publication. The native release prerequisites
and evidence checks are shared by the maintainer workflow in
[RELEASING.md](../../RELEASING.md). Live publication and downstream deployment need
their own authorization and observations.

The repository helper uses registry `ic-host-fs` for streaming hashes and
durable file replacement, and `ic-host-artifacts` for digest parsing. Both
development dependencies disable unused default features and are selected only
for native targets. Durable publication synchronizes files and parent directories;
an error after rename may leave complete replacement bytes visible. Release
recovery reconciles those bytes against the saved intent before restoring or
retrying. These dependencies are absent from canister graphs. `shasum` remains a setup
and CI prerequisite through common scripts, rather than a release adapter
subprocess. Product receipts, release identities and Wasm budgets stay local.
Cargo metadata discovery uses IC Build's re-export of Cargo's canonical command
builder and schema, executing through this helper's selected `ic-host-process`.
This keeps Host limit and error types within their owner when IC Build selects
a different Host release. The existing locked/offline selection and output
bounds remain enforced before metadata decoding.
Host 0.10's single pathname writer takes explicit replacement options. Byte,
receipt and archive publication retain complete `NamedWriteError` values through
the release adapter, including producer/cleanup failures and the distinct
after-publication state. No raw-source extraction or local compatibility writer
is used. [Adoption evidence](../release-workflow-qualification.md#0334-host-010-adoption).

Native CI routes disposable fixtures into a dedicated runner temporary directory.
Failed validation archives those fixtures, setup/gate logs, retained tool candidates
and qualification receipts before uploading them for 14 days. The artifact contains
`evidence.tar.gz`; extract it to inspect legal Unix filenames, permissions and
symlinks. Hidden recovery evidence is included; Git metadata is excluded and
symlinks are retained without following their targets. Archive failures preserve
original inputs and partial output. No selected inputs produce no artifact.
Compiler and feature-check failures remain in the native validation log. Logging preserves the failing
command's status through Bash pipefail; failure collectors run only after a failed
job step.
The native matrix also runs a controlled host-tool fixture failure with substitute
downloads/binaries, archives it through this same collector, uploads/downloads the
exact returned artifact ID and digest through the reviewed downloader, then
verifies the archive digest against an oracle kept
outside the upload. Payload checks preserve original status/logs, unusual names,
modes and unfollowed links. There is no artifact-name fallback or retry of the
producing failure. Reported HTTP 404 responses at artifact metadata/ZIP readback
receive bounded retries, with each attempt retained; other failures stop readback.
Configured coverage and local tests do not prove a live upload.

`make test-failure-evidence` runs the common archive and consumer selection fixtures
offline. They are also part of `test-tools`. For CI transport qualification,
`bash scripts/ci/test-failure-evidence.sh NEW-ROUNDTRIP-DIRECTORY` retains a new
fixture and emits its archive path to `GITHUB_OUTPUT` when set. Verify a downloaded
archive with `bash scripts/ci/verify-failure-evidence.sh FIXTURE ARCHIVE`; digest
refusal happens before extraction. Existing fixture destinations are refused.
The archiver and tool selector share the single recorded
[tooling snapshot](../../.toko-tooling.snapshot); `make toko-tooling-check`
checks all recorded exports. The collector's optional third argument is `full` (default)
or `compact`. CI selects `compact`: complete active host/IC sets that pass fresh
offline checks retain snapshot pins, check logs and IC receipts; failed, unknown
and unselected bundles remain complete. Concurrent tool-set mutation must stop
before evidence collection. Product logs and recovery evidence remain selected.
If shared tool selection fails, the collector archives available product logs,
complete selections already emitted by the selector and its stderr/status. It
warns on stderr and sets `collection-complete=false` in `GITHUB_OUTPUT` when
provided. CI uploads this archive before reporting incomplete tool collection;
an archive path alone does not prove all tool payloads were retained. Store
admission and reference resolution remain with the shared selector.
If the checkout's `.tools` or `target` root is itself a symlink, the collector
retains that link and product logs, skips its children and reports incomplete
collection. Evidence from the other physical root stays selected. The archive
never follows the link to collect outside payloads.

For interactive CI inspection, use the unchanged shared helper:

```sh
GH_REPO=dragginzgame/ic-storage bash scripts/dev/gh-ci.sh --commit HEAD --all-workflows --limit 100
```

When `--logs` cannot retrieve failed-step evidence, the helper reports the gap
and retains partial logs and retrieval errors. Empty output is accepted only for
a completed successful, neutral or skipped run. Follow up through the selected
run's job details; missing logs do not establish a completed failure diagnosis.

This requires an authenticated GitHub CLI session. The bounded listing is evidence
for the resolved commit; apply the [maintenance rule](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/agent-maintenance.md)
before claiming complete workflow coverage. Uncommitted edits have no CI result.

## Rust workspace locations

The repository root is a virtual Cargo workspace. Shared package metadata,
dependencies, profiles and the selected lockfile remain there; the library
packages, examples, tests, current wire fixtures and package guides live in
`crates/ic-storage/`, `crates/ic-storage/`, `crates/ic-metrics-storage/` and
`crates/ic-logging-storage/`. All four are default workspace members.
A checkout path dependency selects the corresponding package, for example
`ic-storage = { path = "../ic-storage/crates/ic-storage" }`. Storage's internal
Memory edge is path-plus-version and shares the root Memory identity.

The approved independent root remains `qualification/`, with
its own lockfile and sole package at
`crates/ic-storage-runtime-qualification/`. Its locked fetch/run commands and
caller-owned PocketIC server requirements retain that root. Layout adoption
does not merge or update either dependency graph. Package archives contain the
library source, examples, tests, fixtures, package guide and root-owned MIT
licenses with their original copyright notices and MIT terms; repository CI,
governance and operational evidence remain outside the
crate archive.
