# IC Storage tooling adoption and qualification boundaries

Read this before changing the shared snapshot, tool setup, release adapters,
formatting or evidence retention. [AGENTS.md](../AGENTS.md) owns the local overlay;
[.shared-tooling.snapshot](../.shared-tooling.snapshot) owns the exact shared
revision, selected roots, payload hashes and executable modes. The former
live-policy exception is retired: dirty sibling rules never override the reviewed
local baseline. Future policy changes require reviewed snapshot adoption.

## Current Cargo-owned 0.36 selection

Each maintained workspace root declares `toko-tooling = "=0.36.0"` in its
`[workspace.dependencies]`. All ten real packages inherit the dependency in
unconditional `[dev-dependencies]` and include the cargo-machete exemption.
The normal Cargo lockfiles own the registry version and archive SHA-256;
`.shared-tooling.snapshot` records the authenticated installed payload.
There is no separate tooling lock, metadata selection or selection-only package.
No Rust imports are needed: this dependency supplies source tooling.

The producer's released installer bootstraps this migration, fetching payload
bytes from crates.io. Its sibling checkout is not a payload or live-policy
source. Existing entrypoints and local edits are preserved. After changing both
exact pins, resolve their lockfiles with Cargo and use `make shared-tooling-update`;
`make shared-tooling-install` reinstalls the selected version. Cargo builds do
not install tooling. Root `shared-tooling-check` admits the installed snapshot and root members;
`check-pins` also admits the independent qualification workspace through the
same Cargo-selection helper and requires both selections to agree.

The 0.34 metadata table and `toko-tooling.lock` are removed. The previous
selection-only package was already retired; real repository tools, all three
product packages and all seven qualification packages remain. Historical
adoption evidence and caches are retained. No sibling changes or deployment are
authorized by this migration. Public release requirements remain unchanged.

Evidence: `target/evidence/toko-tooling-036/`; qualification and remaining gaps
are recorded in the current handoff.

## Previous 0.31.12 source selection

Storage selects pushed Shared Tooling 0.31.12 at
`eb3c8e0d2274d2c7babb2c15b3355bb80a796f0f`. The canonical exporter refreshed
and independently verified the 161-file closure. The sibling exporter was under
active development, so the approved clean source checkout at
`.tools/shared-tooling-source-eb3c8e0` supplies committed export inputs; none of
its sibling's uncommitted changes are adopted. Canonical transient execution
scratch uses `/dev/shm`; retained evidence is under `target/evidence/shared-03112/`.

The refresh adds CLOC terminal colours and an updating sibling-scan counter,
and adopts the clarified ordinary-issue closure policy: focused local fixes can
close issues with delivery state stated explicitly; issue-specific external
acceptance requirements remain binding. No release, publication, hook activation,
compiler change or dependency selection is part of this adoption. Pending
architectural changes and both incoming lockfiles remain intact.

Focused checks pass: snapshot/source verification, dependency pins, local
documentation links, shell/Perl syntax and the native `ic-storage::engine` CLOC
selection. Redirected output is plain; terminal rendering is checked separately.
No hosted CI, release or publication qualification is claimed.

## Previous 0.31.10 source selection

Storage selects pushed Shared Tooling 0.31.10 at
`c5285791991c731d266fb5c85dc3ee876d29f6b9` through the canonical exporter.
The 161-file closure adds source selectors to CLOC and authenticated tool-download
caching. CI uses the revision-pinned shared cache action before host/CI setup,
then saves after setup before product checks. One full-history Gitleaks scan runs
in the non-matrix tooling job, including documentation-only changes; release and
publication targets do not install or run the scanner. Hosted execution remains
CI qualification, not something established by local workflow lint.
IC Build/editor stays at 0.79.0; both lockfiles and pending benchmark work are
preserved. Snapshot/pin checks, workflow actionlint and the
`ic-storage::engine` CLOC selection pass. Evidence lives under
`target/evidence/shared-03110/`.

## Previous 0.31.9 source selection

Storage selects Shared Tooling 0.31.9 at
`d95f271bc7b49951ccf286c3655cef7446b8cf56`, verified against pushed main.
The canonical exporter reads that exact committed revision from the existing
sibling object store; its uncommitted 0.31.10 changes remain untouched and are
not adopted. The independently verified closure contains 161 files. No new
checkout, live sibling-policy inheritance or hand-edited vendor payload is used.

This refresh scopes formatting to explicit owned Cargo members, adds nested
LOC reporting through `CLOC_DEPTH`/`cloc.sh -d`, adopts the shared hash dependency
inventory and clarifies publication impact for private maintenance. Existing
stored hashes and algorithms are unchanged. Root Cargo admission uses the shared
external-IC/hash guard; the independent research graph keeps its existing
explicit boundary policy because its optional ICSS Git comparison is intentional.
Review of that graph finds no new direct hashing dependencies. No new release
mode, automatic publication or cleanup is selected.

IC Build 0.79.0 is published and selected both as the helper dependency and the
exact metadata-editor executable. Storage uses Build Cargo metadata, environment,
lock projection and archive qualification APIs; it has no removed
`WasmBuildBatchConfig` or outcome-variant imports to migrate. Existing Host 0.75
requirements remain compatible. The only new root-lock selection is IC Build;
incoming Runtime/Wire edits and the independent lockfile are preserved.
Rust 1.99.1 remains the support floor and selected compiler.

Focused qualification passes: snapshot verification, dependency pins and both
Cargo boundaries, workspace formatting, selected editor admission, nested CLOC,
88 native release-helper tests and strict helper Clippy. Consumer dispatch and
interrupted-metadata bootstrap fixtures pass, as does ShellCheck 0.11.0 for the
edited fixture. The formatter's explicit member selection exposed an obsolete
empty-metadata stub; the corrected stub exercises all members despite a narrower
default-member list and preserves jobserver checks. The initial failed run is
retained. The independent lockfile matches its incoming hash; the root lock delta
against incoming work contains only IC Build's version and checksum.

Evidence and incoming lock identities live under
`target/evidence/shared-0319-build-079/`. Source selection, installed editor
admission and focused consumer qualification are distinct from release delivery.
No package versions, commits, tags or publication are changed by this adoption.

## Previous 0.31.0 qualification

Storage selects Shared Tooling 0.31.0 at
`74313941af2fce4399bec0e6909ca6229673c3f8`, verified against pushed main and
exported with the canonical exporter. The 159-file closure replaces the old
runner/guide with `run-release-batch.pl` and `release-batches.md`; it also adopts
strict snapshot annotation/checksum and npm schema-3 admission. This repository
has no npm workspace. There are no active pre-0.31 batch journals to migrate;
completed standard release receipts remain with their original source/tooling.

Grouped releases now capture automatically. Storage declares one candidate setup
hook: prepare selected tools and locked dependencies before version lookup;
check existing tools offline on recovery. Publication stays explicitly selected.
Both single-group and explicit multi-group rosters route through Shared; native
group hooks retain their receipt/source/archive admission. Consumer fixtures
separately exercise outer batch argv and inner hook dispatch using effect
substitutes, including setup refusal and failure propagation. No test creates a
Git commit/tag, publishes a package or invokes a maintainer release command.
The real setup target passes prepare/check in the normal checkout with the
existing selected tooling. Snapshot/source verification, dependency pins,
Perl syntax, ShellCheck 0.11.0 and local documentation links also pass.
These focused consumer checks do not establish live captured delivery, remote
reconciliation or a new native-host acceptance result. Those remain release proof.
Evidence, failed observations and incoming lock hashes are under
`target/evidence/shared-tooling-0310/`. Both selected lockfiles are preserved,
including the maintainer's unrelated pending root dependency selection.

Storage 0.68.2 subsequently qualifies the actual Storage-only batch delivery on
Linux: captured setup, all 12 native/archive stages, push, publication, recorded
release identity and guarded reconciliation pass in `batch-1QTqeCag`. The downloaded
archive matches the receipt and tag. The first live attempt exposed inherited
publication context in the consumer routing fixture; the fixture now clears its
owned selections, and the failed batch remains retained without mutation.
See the current handoff and `target/evidence/storage-0682-deploy/`. This is actual
single-group delivery proof, not a claim of multi-group isolation, interrupted
publication recovery or another native host. No alternate implementation checkout
or generic producer fixture was used.

## Previous 0.30.8 qualification

IC Storage selects committed Shared Tooling VERSION 0.30.8 at
`dc9df707b3b1e616973494eab961c1c214d12260`, verified against GitHub main.
The canonical exporter retains the 159-file selection; only the frozen-release
runner payload changes. It now accepts underscores in generated batch IDs when
resuming, preserving existing recovery paths. Impact/export/check evidence is
under `target/evidence/shared-tooling-0308/`; exporter scratch used `/dev/shm`.
Frozen-release mode remains disabled pending consumer qualification. The standard
Storage release path, package versions and both dependency selections are unchanged.
Shared owns the forced-underscore regression fixture; no producer fixture or
live release is replayed to qualify this consumer refresh.
Snapshot/source comparison, pins, Perl syntax, local documentation links and
diff checks pass. Both lockfiles match their incoming hashes; the sibling is clean.

## Previous 0.30.7 qualification

IC Storage selects committed Shared Tooling VERSION 0.30.7 at
`22a853e26c05ec323f06c57fa316c08b808f252b`, verified against GitHub's `main`.
The canonical exporter refreshed 159 files from the clean sibling with an explicit
commit selection. Exporter scratch used `/dev/shm`; retained impact, export and
consumer-check logs are under `target/evidence/shared-tooling-0307/`.

The existing `check-candid-boundary` target now enables Shared's `--cbor` mode
over all 164 Rust files in `crates` and `testing`, without source allowances.
Root Cargo admission uses Shared's central CBOR inventory; its duplicate local
CBOR list is removed. Independent Cargo admission retains an explicit nine-package
CBOR policy, including `icrc-cbor`: Shared's combined `--external-ic` mode would
also prohibit the authorized optional ICSS Git comparison dependency. The checker
does not offer CBOR-only Cargo admission. This bounded duplication preserves the
existing research control; it is not a new exception to CBOR ownership. No first-party
browser roots or additional Cargo manifest templates were found. Lexical source
checks do not resolve arbitrary aliases, macro expansion or copied parsers.

Frozen-release tooling is exported but not enabled: actual Storage release and
publication hooks need independent-checkout qualification before that opt-in.
Existing group admission and archive obligations remain in force. The reviewed
lint and feature stages differ (`--all-targets` default features versus explicit
`ic-storage/test-support`); Wasm and archive checks also retain distinct contracts.
No duplicate compilation was removed or release-speed improvement claimed.
Both catalogs and all members retain Rust 1.99.1; package versions, dependency
selections and pending collection work are preserved. Source adoption is separate
from activation of a new release mode, installed runtime proof and publication.

Snapshot/source identity, pin admission, both Cargo boundaries, combined source
checking, local documentation links and the consumer release-adapter substitutes
pass. Actual Cargo/rustc identities are 1.99.1. Both lockfiles match their incoming
hashes, and the sibling remains clean. No runtime rebuild or release was needed
for this tooling-only refresh; the pending Storage Runtime-export blocker remains.

## Previous 0.30.5 qualification

IC Storage selects committed Shared Tooling VERSION 0.30.5 at
`be3771031ebfb340583b238dee5446be65929c62`, verified against GitHub's `main`.
The clean sibling was exported through the canonical exporter with an explicit
commit selection. No sibling edits or extra checkout were used. The exporter
used its own transient `/dev/shm` copies; retained local logs are under
`target/evidence/shared-tooling-0305/`.

`make check-candid-boundary` runs Shared's lexical checker once over `crates`
and `testing`, including the native helper, benchmarks and independent runtime
fixtures. The reviewed policy has no allowances because these sources contain
no Candid trait references. `validate-policy` owns the check. Its documented
macro expansion, alias and type-resolution limitations remain applicable.

`make check-storage-boundary` also admits both workspaces' direct dependency
declarations through Shared's existing metadata checker, including aliases,
optional, development, build and target-specific declarations. Both policies
forbid the named Serde CBOR, Ciborium, Minicbor, cbor4ii and DAG-CBOR packages;
root production keeps its existing IC dependency exclusions. The independent
benchmark's permitted ICSS codec reference is unchanged. Source review found no
CBOR imports/codecs or first-party browser roots. This is named dependency
enforcement and source review, not a new general CBOR import/type-resolution
scanner; Shared's Candid checker does not supply that coverage.

New raw comparisons use `target/evidence/storage-comparison/`. The maintainer
subsequently authorized cleanup of the 83-file, 8,813,675-byte top-level measurement
inventory. Sixty-nine bulk/repeated exports and obsolete prototype patches were
retired. The [measurement index](measurements/README.md) owns compact topic summaries,
selected data and the distinction from protected qualification/recovery records.
The full type catalogue retains all 565 case statuses and selected absolute results
in two small data files, within the 100 KiB topic budget. Complete raw tables and
source archives are not a tracked archive or promised as durable evidence.
Existing target artifacts were left untouched and remain subject to explicit clean.
No release/recovery state, tool cache, preservation stash or pending code was removed.
No new permanent archive or history rewrite was created by the cleanup.

The refresh also adopts LOC/formatter target-local scratch defaults and release
changelog heading fixes. Registry selections, package versions, collection
engines, stored bytes and production runtime settings are unchanged. Consumer
qualification results are recorded in the current handoff after the focused
checks; this adoption does not qualify a release or another native host.

## Previous 0.30.0 qualification

The [adoption audit](audits/2026-11-05-shared-tooling.md) records the directory,
fixture ownership, lint and diagnostic changes. Evidence lives under
`target/shared-tooling-0300-adoption/`. Release groups, public APIs, stored layouts
and publication identities remain unchanged. Remaining root lock selections are
preserved; the unused CDK-only closure is pruned, and the fixture dependency is confined to the unpublished qualification workspace.

The follow-up refresh to `b37a882` fixes exported LOC documentation links and
single-workspace table labels. Make's root/independent LOC callers, JSON manifest
identity, snapshot integrity and affected documentation links are checked under
`target/shared-tooling-b37a882/`. Existing Rust and installed-runtime evidence
remains applicable; executable Rust and release policy did not change.

## Previous 0.29.4 qualification

The new retained-state guard admits the current Git-side recovery roots without
cleanup. It runs before release work and around adapter hooks, preserves original
hook failures, and refuses oversized/stale bulk state or nested build trees.
Storage's native archives and receipts already use Cargo's target directory.
The remaining dependency-adoption evidence default now uses
`target/dependency-adoption/`; the actual caller fixture checks that location.
Evidence for this refresh is in `target/shared-tooling-0294-adoption/`.
Shared retention and full command-stub runner tests pass, as does Storage's
actual dependency-adoption caller fixture. The exported runner and guard match
the tested source byte-for-byte; snapshot/source verification, dependency pins,
ShellCheck 0.11.0, Perl syntax and documentation links pass. Both lockfiles are
unchanged. This qualifies tooling adoption, not a live release or runtime change.
Git-side release state contains 191 files and 32,079 logical bytes; the allocated
filesystem footprint is 776 KiB. Publication-state and retired-checkouts roots
are absent. No evidence or caches were deleted by this adoption.

Implementation, delivery and checkout authority follow the
[Shared contribution contract](../rules/contributions.md). This refresh stays in
the normal Storage checkout, creates no additional checkout and executes no release.
Existing temporary work retains its separately approved preservation requirements.

The 0.29.3 adoption moves owned inline Rust test bodies into `tests.rs` or
`tests/`, including the independent qualification workspace and native helper.
Ordinary module discovery preserves private access; no path attributes or
production visibility expansion are introduced. Small cache instrumentation stays
at its observation sites. Benchmarks retain the pending audit/growth work.
Shared's narrower Metrics rule still permits this repository's governed metric
snapshot adapter; benchmark calculations already have no Metrics dependency.
Grouped changelogs own fixed-issue links; the root stays a navigation index.
The CI classifier's new VERSION exception remains disabled for this Cargo
workspace. Evidence is in `target/shared-tooling-0293-adoption/`; uncommitted
Shared 0.29.4 work was excluded by the explicit committed-revision export.

Qualification passed 124 selected unit/fixture tests across Storage, the native
helper, Logging's fixture encoder and the independent workspace. Two installed
runtime tests retain their existing ignored status in unit runs; no new installed
runtime proof is claimed for a source-layout-only change. Strict Storage/helper
Clippy, selected formatting, ShellCheck 0.11.0, actual workflow CI selection,
snapshot/source verification, pins and local documentation links pass. Both
lockfiles match the incoming copies. Ordinary Rust test bodies now all reside in
`tests.rs` or `tests/`; the benchmark binary's inline module contains wiring only
because its library shares `src/` and already owns `tests.rs`.

Consumer fixture scratch now defaults to workspace `target/fixtures`, with
`target/repo-tool-fixtures` for the native helper and `target/ci-selection` for
workflow checks. The native group-plan script uses Cargo's observed target
directory. Inherited TMPDIR no longer redirects those ordinary fixtures;
explicit evidence-directory arguments and child-tool alias/TMPDIR probes remain.
Failure/cleanup-status probes, the aliased archive round trip, Rust fixture
collision/preservation checks, compiler admission and dependency-caller checks
pass; evidence is under `target/fixture-location-adoption/`. Historical temporary
directories and failed artifacts are untouched. The active local audit overlay
now describes the current Storage owners and Rust 1.99.1; older audit reports
retain their original scope and hashes.

The 0.29.2 adoption includes UTF-8 runner evidence and Bash 3.2 manifest-order
failure propagation fixes. Storage's existing real-runner receipt test now covers
non-ASCII selection reasons through the shared verifier and local receipt admission.
The boundary checker recognizes Testkit as the native PocketIC SDK dependency
owner; the current qualified Platform client graph and both lock selections are
unchanged by this tooling refresh. That earlier graph does not qualify a future
transferred Testkit client. Root release-group ordering already matches the new
policy. Shared now names Storage's allocation ownership directly, removing the
obsolete local policy exception. Evidence is retained in
`target/shared-tooling-0292-adoption/`, including the pre-adoption UTF-8 failure
under `target/shared-tooling-0291-adoption/`.

The earlier proof collection/recovery work completed
[Storage #156](https://github.com/dragginzgame/ic-storage/issues/156).

The 152-file snapshot adds Shared's canonical validation evidence, baseline and
roster interfaces and their companions. Storage invokes the shared verifier for
initial group admission, retaining its complete proof and checkpoint/log hashes
inside the existing native receipt. Generic event/timing parsing has been removed.
Recovery preserves the original proof at its original source location; Storage's
existing source/configuration/lock projection admission qualifies release-owned
version edits without reinterpreting or rewriting the old checkpoints.

The ordered full-stage roster lives once in `ci/release-validation.json`.
Full dispatch uses Shared's roster interface; native product selection derives
from the same catalog and retains group/package ownership and baseline-version
checks. Shared observes tag identity/ancestry/changed paths. Unknown or unavailable
scope keeps the full gate; failed observations refuse. No producer fixture alone
qualifies this consumer's recovery, host or publication boundaries.

The [consumer mapping](storage-release-validation.md) keeps narrow private adapter
plans, including private adapter PRs, and full repository CI on non-docs pushes. Group evidence now has schema 2 inside the
pending schema-2 validation receipt; earlier local group-schema-1 attempts remain
with their original helper. Historical released receipts and archives are untouched.
ShellCheck's separate reuse path and tool/compiler/dependency pins remain unchanged.
Rust 1.99.1 remains both support floor and development/release compiler.

Production Storage imports `ic-platform-runtime` 0.20.0 only on Wasm. Native
production selects no Platform/Runtime, runtime guard or Types provider. Wasm
production selects `ic0` alone: safe public `stable64_*` wrappers cover size,
growth, reads and writes. The execution boundary restored in Storage 0.65.2
remains intact; the default product graph excludes executor, CDK, guard, Types
and Candid. Platform 0.19's Types 0.18 changes are confined to explicitly selected
development/runtime-composition graphs, not Storage's public codec types.
Logging's Wasm qualification entrypoints explicitly select CDK as a dev dependency.
The separate unpublished `ic-storage-platform-qualification` consumer composes
candidate Storage with the canonical Runtime's `runtime` feature for issue #152.
The facade supplies only Principal types in that fixture; selecting its older
runtime would introduce a conflicting executor guard. Runtime 0.20's ordinary
entrypoint macros remain under `ic_platform_runtime::cdk`; Logging's fixture
already uses that path. No production API or stored layout changes. Adoption
checks require one canonical Runtime/guard on native and Wasm and compile both
graphs; its native test checks governed cell creation/reopen.
The native qualifier uses Platform 0.19.0's owned PocketIC client 0.19.0,
retaining the 16.1 protocol and Testkit-selected server 16.1.0. Fallible build,
installation, clock and cycle operations propagate or diagnose client errors.
Rollback assertions accept only canister rejects with the expected trap code;
transport/protocol errors cannot qualify as rollback evidence. Installed IO,
all five collection forms, custody, directory faults, upgrades and Logging were
requalified under this pairing. No direct registry client declarations or
conversion layer are added. Evidence: `target/platform-019-adoption/`.

Platform 0.18 removed its stable-structures facade. The unpublished comparison
benchmark alone retains an explicitly named `ic-platform-stable-benchmark` alias
for Platform 0.17, with only its `stable-structures` feature. Its system calls use
Runtime 0.20 and its native driver uses Platform 0.19. Production has no upstream
collection dependency. Optional crypto/certification profiles remain unselected.

The maintainer-requested upstream comparison additionally selects the immutable
DFINITY revision `b5d1650463f477affb3d358e86b1113953fc9937` through the optional
`dfinity-head` benchmark feature. This research control is declared only in the
approved independent workspace: the published facade cannot expose unpublished
upstream code. It is not a production integration, local fork, supplier adoption
or permission for other direct IC dependencies. The default published comparison
still uses the Platform 0.17 facade. Both identities are retained in that
workspace's lockfile; normal product graphs remain upstream-free.

Both catalogs require and select published Wire/Derive 0.16.0. Public codec
errors and Storage's configured Wire limits carry that Rust identity; hosts
exchanging those values must align their direct dependency. Storage does not
select Wire's optional Platform Account schemas. Owned codec bytes, budgets,
directory/collection framing and restoration contracts are unchanged.

Metrics 0.28.0 supplies arithmetic and validated Serde without a Wire dependency
or native metadata feature, completing the supplier change under [Storage #154](https://github.com/dragginzgame/ic-storage/issues/154)
and [Observability #128](https://github.com/dragginzgame/ic-observability/issues/128).
Storage owns its record codecs; services own report DTO/schema declarations.
The obsolete direct Metrics schema test is removed, while frozen rows, invalid
numeric states, continuity and reopen coverage remain. Native production checks
refuse Metrics metadata feature unification and require one Wire identity;
the independent qualifier also selects one Wire provider. No foreign trait
implementations, supplier patches or replacement integration crate are added.
Metrics 0.28.0 is published; Storage qualification uses that registry supplier.
Storage 0.66.0, Metrics Storage 0.67.0 and Logging Storage 0.64.0 publish this
adoption; verified delivery and qualification are in the [handoff](status/current.md).
The library graph no longer selects Testkit Native: it has no allocation API
callers, and Storage owns backing IO metering. Native 0.80's memory-module removal
therefore requires no API migration here. Installed qualification selects Testkit
CLI 0.79.0 independently. Wire endpoint and platform-types integration remain unselected.
Metrics Storage's public measurement/error types carry Metrics 0.28 identity;
Logging Storage's public log/limit/restoration-error types carry Logging 0.24
identity. Hosts must align their direct supplier dependencies. Both suppliers now
share this repository's Rust 1.99.1 floor; arithmetic, logging semantics, encoded
records and runtime ownership remain unchanged.

Storage 0.65.0 and Metrics Storage 0.66.0 delivered Build 0.74.0 and Host 0.74.1;
Logging Storage 0.64.0 subsequently completed its separate publication.
The current native helper selects Host 0.75.0 and Build 0.79.0 together because
archive qualification passes Host process-limit types into Build. The removed
Host provenance/sampling and Build wrapper, prepared-input and progress APIs have
no Storage callers. The metadata editor follows the selected Build version;
explicit setup and offline recovery qualification remain separate from selection.
The independent runtime workspace and installed Testkit/server selections are unchanged.

The library [dependency catalog](../Cargo.toml) owns direct package requirements
and feature selections; its [lockfile](../Cargo.lock) owns resolved identities.
The independent qualification [catalog](../testing/runtime-qualification/Cargo.toml)
and [lockfile](../testing/runtime-qualification/Cargo.lock) retain their separate
host graph. The [Build editor pin](../ci/release-tools.env) and
[Testkit CLI pin](../ci/runtime-tools.env) select executables independently.
Read those owners for versions rather than copying their inventories here.

The Wasm production graph admits one Runtime provider and forbids the
Platform facade and `ic-stable-structures`, as declared in [the local overlay](../AGENTS.md). Library admission,
executable setup/check and installed qualification remain separate authorities.
Selection alone does not establish acceptance or permit relabelling older proof.
The [release ledger](../CHANGELOG.md) and [current handoff](status/current.md)
record completed changes and scoped work without becoming release gates. A
dependency update activates neither maintenance schedules nor a snapshot refresh.

### Local compiler decision

The maintainer approved [#151](https://github.com/dragginzgame/ic-storage/issues/151):
Rust 1.99.1 is the library support floor and pinned development/release compiler.
This explicit owner decision supersedes the reviewed snapshot's earlier
lowest-possible-floor guidance; snapshot files are unchanged. Root and independent
workspace floors, inherited member declarations and observed Cargo/rustc identities
must agree. Duplicate default compiler jobs and native archive runs are retired;
feature-enabled native checks, all native hosts, Wasm/proc-macro compilation,
archive consumers and installed runtime qualification remain. No cache is removed.

### Earlier adoption and qualification records

The compact baseline derives `rules/engineering-details.md` as a companion;
specialist contracts remain binding when the task touches them. Reporting metrics
are opt-in, and focused checks follow the completed coherent edit batch. The
adopted npm Platform boundary adds no dependency to this Rust-only consumer.
The refresh adds the canonical Rust formatting driver and updates Make dispatch,
preserving common tool pins and the installer build-policy identity.
The preceding consumer-owned Build/Testkit selections were 0.69.6. Both dependency catalogs
select Platform 0.7.3 without enabling optional provider/gzip features.
The stable-structures 0.7.2 substrate and PocketIC 16.1.0 pair remain selected.
Release 0.50.7 passed the complete native, MSRV and installed-runtime matrix
on Linux and both macOS architectures; snapshot integrity alone is not acceptance.
See [the adoption issue](https://github.com/dragginzgame/ic-storage/issues/127).
The canonical takeover exporter retires the imported Storage snapshot after
verifying its original bytes. Storage PR #18's release retry protection is
therefore included; uncommitted Shared sibling changes are excluded.
The canonical [preview and refresh procedure](consuming-snapshots.md#preview-before-refreshing)
exports committed bytes from a clean source checkout. Direct callers remain
explicit roots and required companions are derived by the exporter.

The Platform 0.7.3 / Tooling 0.69.6 refresh preserves the incoming library lock
selection and advances only Platform in the independent lock. The selected
Platform interfaces are unchanged; native Tooling adds its portable source-mode
fix and reuses existing observation/process helpers. Build and Testkit CLI
selections advance separately to 0.69.6. Shared source and its executable closure
remain at the reviewed 0.27.3 revision, with no dirty upstream rules adopted.

The 0.27.3 refresh retains that closure and verifies every payload's bytes and
executable mode against the independently selected source commit, using the
trusted source verifier's paired `--source` and `--revision` options. Ordinary
builds retain offline snapshot checks. Consumer hook fixtures now use the shared
closure copier and stage all copied regular-file changes, including companions
absent from HEAD, while leaving opaque gitlinks intact.

The optional sccache pin does not change the common tool roster or enable a
compiler cache. Scheduled maintenance remains inactive unless explicitly
configured and qualified under the adopted report-only policy. The supplying
owner tracks shared dependency defects; no waiting issue is created per consumer.

The release prerequisite inventory remains consumer-owned: native system/compiler
bootstrap precedes common Cargo tools and the selected Build editor; both Rust
compilers and the Wasm target, host/IC tools and locked caches are admitted before
validation. The separate installed-runtime path prepares Testkit and its selected
server before qualification. Existing Make dispatch fixtures exercise ordering
and failure stopping; they do not replace actual native or installed acceptance.

The current refresh uses a clean detached committed source and canonical impact
preview. The shared Rust driver now owns the complete formatting sequence. The
Makefile's `FORMAT_RUST_ARGS` selects `testing/runtime-qualification` as the extra
workspace and `testing/package-consumer.rs` as an edition-2024 standalone source.
The paired overrides and copied adapter are retired. Root sorting/formatting runs
before extra-workspace sorting/formatting, followed by standalone Rustfmt, with
each pass visible and timed. Working-tree writes remain nontransactional; check
mode is non-mutating and staged-hook publication waits for the complete formatter
to succeed.

Standalone selections always run uncached, including workspace passes, so source,
module and configuration changes cannot be hidden by an incomplete result receipt.
Prepared formatter admission still runs first. Consumer fixtures exercise the real
new driver, source/config changes, exact ordering, jobserver descriptor forwarding,
failed standalone admission before workspace passes and retained failure logs.
The hook fixture's exported-index closure includes the driver and preserves
partial staging and unrelated work. This implements the local migration for
[#135 — Adopt shared formatter orchestration without dropping package-consumer formatting](https://github.com/dragginzgame/ic-storage/issues/135),
using the capability delivered for
[Shared #314 — Support standalone Rust sources in shared formatting orchestration](https://github.com/dragginzgame/shared-tooling/issues/314).
Remote issue closure and delivery remain separate from these working-tree changes.

Shared 0.26.26 replaces broad module-path screening with the shared Rust lexer's
balanced-attribute inspection for workspace-only cache admission. The driver and
`RustSource.pm` are both source-bound; the hook fixture copies/stages the module,
and the jobserver fixture follows and compares that companion closure. Standalone
selection still disables all reuse, so Storage's active formatting behavior
stays uncached. Shared 0.26.25's scanner filename qualification changes are outside
this consumer's selected executable closure. Tool pins and build identity remain
unchanged.

The refreshed hook accepts opaque tracked gitlinks, preserving their staged
identities while formatting selected regular files. This repository has no
production submodule integration to migrate. The Node guide is a required policy
companion, not Node activation. The Clap-based runtime scanner is not selected by
this snapshot and requires no consumer compiler package or dependency changes.

The gzip policy names Tooling's opt-in encoder as the application/fixture owner;
Cargo archives and system tar used for packaging/evidence retain their explicit
infrastructure boundary. No gzip dependency/feature or product encoding changes.

The 0.26.22 refresh uses a clean detached copy of the committed source and the
canonical impact preview, preserving the existing roots and payload closure.
Shared 0.26.21 makes emoji categories the current/future root changelog format;
Storage already uses it, and older release entries are preserved. Shared
0.26.22 explicitly prohibits owned `CandidType` models and strengthens existing
native error, machine-output, bounded-process, terminal-rendering and source-scan
review rules. Storage's maintained Rust sources contain no `CandidType`
derives or implementations. These are reviewed governance changes, not new
executable scanners or a completed repository-wide hygiene audit. Existing
caller qualification and negative tests retain their owning gates.

The refresh uses the canonical exporter from a clean detached copy of that
commit and retains the existing capabilities. The companion-copy and Make-adapter helpers become explicit roots because the
consumer release fixture calls them for formatter closure and parallel lint-order
qualification, keeping its target roster and effect stubs local. Explicit Cargo-tool
setup waits up to 60 seconds for an admitted exact-slot owner;
`SHARED_TOOLING_SETUP_WAIT_SECONDS=0..300` selects another canonical integer
budget. Offline checks/preflight and Host/IC setup remain immediate. No foreign
lock is removed and no owner is killed. After waiting, compiler/source/lock and
completed receipts are admitted again before reuse or activation.

The merged [Shared PR #302](https://github.com/dragginzgame/shared-tooling/pull/302)
owner-cleanup fix remains selected. Cache initialization now uses idempotent
private directory creation with existing path/configuration admission, without a
global source-preparation lock. Cargo owns download serialization; exact selected
tool slots still serialize builds. Old source-preparation slots and generations
remain retained.

The earlier snapshot selected 0.26.11 timing work while its committed `VERSION`
still declared 0.26.10. The previous refresh recorded the producer's committed
0.26.19 identity; sibling working-tree edits remain excluded.
The refresh retains common tool pins, compiler selections and the tool-store
build-policy identity. Existing prepared tools can be admitted without rebuilding.
The retired checkout-local Rust build directory no longer affects admission;
current shared-store checks and retained evidence remain in force.

Cargo admission now preserves exit status 75 for an admitted busy tool slot.
Plain checks remain immediate; a direct caller may explicitly select
`--check --wait-seconds N` for a bounded read-only wait. The runtime-server Make
adapter preserves the busy diagnostic without recommending installation. Missing
or invalid preparation retains its setup advice. No automatic wait or installer
retry is added to normal checks or helper recovery.

Normal release commands now start fresh from current clean committed metadata,
without scanning old journals or replaying their effects. Recovery requires
`release-resume VERSION=X.Y.Z`; the selected source, payload, refs, locks and
destination remain admitted. This replaces the earlier automatic recovery rule.
The runner may discard only its new candidate's stale plan/staging/PR metadata;
consumer validation receipts, failed gate logs and retained archives preserve
their existing custody. No release is executed during adoption.

Shared 0.26.17 diagnoses unsupported retention-probe prefixes/selections and
avoids partial generated probes. The canonical helper supports column-one,
single-line EXIT traps; multiline consumer actions are not silently reinterpreted.
Shared 0.26.19 allows archived Draft labels beneath known historical release
sections while refusing active drafts and conflicting current candidates.
The source commit message says 0.27.19, but its committed VERSION and exported
snapshot declare 0.26.19; adoption records the committed declaration.

The RPC-budget rule is adopted as governance; Storage owns no generated
cross-runtime application RPC client, so no product budget or codec is added.

`make tools-report` runs every existing offline checker and retains individual
failures and durations. It is diagnostic; fail-fast `tools-check` remains release
admission. The validation-target runner is selected only as its required companion,
without changing this consumer's validation/release ordering. Installer `--describe`
reports admitted selection inputs as JSON without executing or preparing tools.
It does not replace installed-tool checks. Explicit Cargo setup now reports source
preparation, lock waiting, selection/cache reuse, compilation and receipt/build
admission with start/outcome/duration on stderr, delivering
[Shared #303 — Report Cargo tool setup phases and durations](https://github.com/dragginzgame/shared-tooling/issues/303).
Checks remain quiet and offline; stdout remains the selected executable path.
No owner is killed, no deadline extended and no build identity changed.

Formatting now uses the same literal catalog parser as installation, rejecting
duplicate assignments and executable configuration. Both local workspace adapters
remain selected. `check-storage-boundary` opts into the shared external IC package
inventory while preserving this consumer's stricter no-direct-substrate rule.
The existing production graph policy remains local; structured graph explanations
are available for diagnosis. The existing consumer lint-order fixture uses the shared parallel-Make probe;
its expected stages and effect stubs remain local. Optional common CI cache setup
does not replace this consumer's current cache bindings.

Ordinary expensive hosted integration follows Shared's asynchronous cadence.
Focused checks, enforced GitHub requirements and separate release/publication/
deployment and issue-specific acceptance remain in force. No CI dispatch or
branch-protection change is part of adoption. DFINITY work stops at the owned
Platform boundary and does not depend on future DFINITY contributions.

Validation runs the existing all-target Clippy gate after prerequisite admission
and before fixture suites, tests, Wasm builds and packaging. Sequential recipes
preserve that ordering under parallel Make; the feature-specific fixture lint
also remains in its focused target. Release notes may use a numbered
` - Unreleased` heading, which the shared finalizer dates during preparation.

Shared 0.25.3 binds explicit Cargo feature sets to selected tool installations
and admission receipts. Build and Testkit 0.69.6 binaries select `--features cli`
in setup, offline admission and bootstrap calls. Library dependencies retain
disabled defaults. Prepare the selections with `make install-tools` and
`make install-runtime-server`; previous installations and failed attempts remain
retained, and ordinary checks never repair tools.

Memory's failure collector delegates tool selection and archiving to Shared's
`collect-tool-evidence.sh`. Failed selector output remains diagnostic data;
it cannot admit physical payloads. Memory retains product log selection,
redirected-parent admission and hosted upload/download verification.

The retired Perl manifest/lock renderers leave the snapshot. Repo-tool calls
published IC Build's pure rendering API and retains its independent semantic
projection, saved release intent, receipts and rollback. Changelog finalization
still uses Shared's canonical helper with retained bounded inputs. Interrupted
manifest/lock bootstrap uses the published editor selected in
[`ci/release-tools.env`](../ci/release-tools.env), with regular source copies and
no source symlinks. `make install-release-tools` explicitly prepares it;
`make release-tools-check` admits it offline. The common setup/check aggregates
include that tool. Standard release bootstrap can prepare the selected editor;
ordinary helper recovery only checks it. Neither path changes dependency
selection or installs a Rust compiler.

Earlier adoption decisions and exact qualification results remain in the
numbered changelogs, [release-workflow evidence](release-workflow-qualification.md)
and Git history. Their former setup exceptions or tool selections are historical
evidence, not another current source of policy.

## Ownership and consumer qualification

See [the implementation ownership boundary](ownership.md) for runtime, collection
and host-tooling owners. Shared Tooling owns the exported runner, installers,
formatter prerequisites, Make modules, archive primitives and their generic
regression matrices. Memory owns which capabilities it selects and the following
consumer proofs:

| Retained proof | Local caller |
| --- | --- |
| Snapshot hashes, modes and selected roots | `make shared-tooling-check` |
| Actual catalogs, installed tools and native reuse | `make tools-check`, formatter admission and native CI |
| Two-workspace pin policy and exceptions | `make check-pins` |
| Default production Wasm normal/build dependency exclusions | `make check-production-graph` |
| Release source, receipts, rollback and Make dispatch | repo-tool tests and `make test-release-adapters`, with substituted Git/Cargo effects |
| Actual index formatting and failure isolation | `make test-hooks` |
| Archive selections, hosted transport bindings and retained failures | `make test-tools` and `make test-failure-evidence` |

Fleet reporters, exporter regression fixtures and generic runner, parser,
installer, Make, formatter, LOC and checksum matrices are not consumer tests.
Run them at Shared Tooling's owner when qualifying its implementation. No local
product proof, native host or installed-tool admission is removed by that split.

The production graph check uses Shared's canonical observer with Memory-owned
exclusions for native tooling and test dependencies. It checks the default
`wasm32-unknown-unknown` library graph; it does not replace compilation, artifact
size budgets or installed qualification. `test-support` adds no dependencies.

## Tool setup and formatting

The shared catalog owns common host/Cargo pins and the IC tool catalog. Memory
selects its validation compiler, library MSRV, independent qualification workspace
in its own manifests and Makefile, and Testkit CLI in `ci/runtime-tools.env`.
Follow [host support](host-support.md) for prerequisites and matching native evidence.
Source adoption, explicit setup, local activation and native qualification are
separate; an exported script or configured CI job is not acceptance evidence.

Memory selects only `wasm-opt` for installed runtime qualification, including
native offline reuse and compact failure evidence. The full canonical catalog
remains admitted; unselected tools are not downloaded or required. Full evidence
retention and prior installations remain unchanged.
Memory extends the common ordered setup/check targets with `install-ic-tools`
and `ic-tools-check`, and exports `.tools/ic/bin`. Common setup supplies
`.tools/host/bin` and `.tools/rust/common/bin`. Prepared bundles, failed attempts
and old selections remain retained; refreshing source never silently installs
or deletes tools. Fleet reports run in Shared Tooling; local `make cloc` selects
the root workspace, or an explicit `CLOC_MANIFEST` for the independent graph.

Formatting covers the root workspace, `testing/runtime-qualification` and the
standalone edition-2024 package consumer through declarative `FORMAT_RUST_ARGS`.
Shared owns each timed pass; the consumer owns the roster and selected compiler.
Standalone selections deliberately run without result reuse. Formatting builds,
fetches, installs and selected-lock changes remain prohibited. Hook setup is
explicit and real index effects stay in disposable fixtures or explicitly
authorized commits.

Testkit owns PocketIC selection, provisioning, offline admission and managed
lifecycle. The separately selected CLI is outside both maintained lockfiles;
`install-runtime-server` prepares it explicitly, `runtime-server-check` is offline
and `test-runtime` uses prepared tools and both caches. Installed qualification
retains its separate authority. No local server installer or supervisor competes
with Testkit. See the [qualification runner](../testing/runtime-qualification/README.md).

## Release and evidence boundaries

The shared runner owns ordering, locks, saved intent and Git effects.
Memory selects direct delivery and owns its Rust source/package receipts,
metadata transaction and proof admission; IC Build supplies the available Cargo
observers and projection checks through explicit native development dependencies.
The unused alternate release delivery and validation checkpoint paths remain
unselected. Group selection uses the existing runner, with exact file plans
checked against Shared. Every core release aligns versioned root path requirements
with member versions while preserving operators and adapter versions. See [RELEASING.md](../RELEASING.md).

Normal checks remain offline. Authorized release entry points separately allow
locked cache preparation before helper compilation; an explicitly offline caller
stays offline. Never run a release command, create a fixture commit or alter the
real index merely to qualify adoption.

Memory selects its product logs and archive contents, while shared primitives
own tool selection, checksums and archive construction. Full retention is the
default; CI requests compact retention only for freshly admitted complete active
sets. Failed, unknown and unselected bundles stay full. The workflow binds the
exact upload ID/digest, repository/run and token. Preserve unsuccessful collection
observations, gate logs, prior receipts and package archives across retries.

## Stable-memory governance policy

Production stores consume checked Storage handles after admission and durable
allocation commitment. [AGENTS.md](../AGENTS.md#stable-memory-governance) and
[SAFETY.md](../SAFETY.md) record this boundary. The common rule was adopted with
Shared 0.20.0; policy adoption alone does not qualify consumer integrations.

IC Storage owns allocation governance and collections consuming `RuntimeMemory`;
its directory and tree engines stay private. Hosts still own exclusive backing custody;
Rust import visibility cannot detect aliases or enforce application policy.

## Explicit cleanup boundary

Cargo retains its default workspace `target/` location; the independent runtime
workspace keeps its own default for commands without an explicit target selection.
Plain `cargo clean` removes the owning workspace target, including retained logs,
release receipts and runtime evidence beneath it. Retention does not override an
explicit clean. Copy evidence outside target before cleanup when it must survive.
The separately managed `.tools/` stores and user Cargo caches are outside that
cleanup boundary. No cleanup was performed to qualify this adoption.

## Documentation-only CI

The consumer opts into the selected revision’s classifier through the exporter.
[The consumer contract](ci-selection.md) and exact prose allowlist own CI dispatch;
unknown and broad code changes keep full native qualification. Private adapter
PRs now use the consumer's qualified group selector; non-docs pushes retain the
full roster. Documentation checks remain separate from native evidence, and
archive/native-host/runtime obligations are unchanged.

## Exact-input ShellCheck reuse

`make lint-tooling` retains Actionlint and ShellCheck coverage. The named
`make check-shellcheck-stage` runs the ShellCheck portion ordinarily. To opt in to
retained exact-input proofs, supply an exclusive trusted private directory and an
explicit release group, for example:

```bash
VALIDATION_STAGE_STATE=/absolute/private/stage-state RELEASE_GROUP=storage \
  make check-shellcheck-stage
```

The caller freshly admits the selected group policy and locked metadata, records
the current Git HEAD/index and metadata-byte projection identity, and rechecks
them after the stage. The stage inventory includes the actual shell scripts,
environment files, hook, Makefile and selected ShellCheck/jq executable bytes.
The fixed per-input/log allowance is 64 MiB, covering the pinned ShellCheck
0.11.0 macOS binaries (61,551,911 bytes on ARM64 and 35,176,792 on Intel).
Larger inputs still refuse; tools are hashed in full rather than truncated.
Shared also binds its runner, interpreter/modules, command, observer, host and
bounded environment. ShellCheck explicitly uses `--norc --shell=bash`; home
configuration cannot silently change the selected contract. Actionlint always
executes separately. New inventory entries or changed bytes/tools/configuration
force execution; unknown, missing or failed evidence cannot supply a passing proof.

Each invocation retains a new admission referencing the original immutable
execution and logs. Source projection identities are consumer observations, not
release authority or rebound native gate receipts. No Cargo compilation stage is
reused, and no Cargo version projection is accepted by the stage executor.
Existing full release gates, archives, source/intent/receipt authorization and
registry provenance remain unchanged.

`make test-validation-stage-reuse` qualifies the actual Make caller and ShellCheck
in a retained source copy. It admits all three groups, uses the canonical editor
between groups, preserves unrelated versions and the original checkout/index,
and requires one execution plus two fresh admissions sharing the original proof.
Native CI runs this focused qualification on Linux and both macOS architectures.
The local Linux run measured 7.77 s for execution and 0.19 s for each reuse
admission. These are stage measurements, not a claimed reduction of the 11.6-minute
full gates in the earlier investigation.

## Destination dependency adoption

`make check-dependency-adoption` is a focused, read-only candidate check before
planning downstream versions or building release artifacts. Prepare proposed
supplier selections in the requested checkout or an explicitly authorized durable
contribution checkout with both existing lockfiles, using
source/archive qualification for unpublished suppliers. Record the required
runtime/public-type/security/minimum-capability alignment and its introducing
path; classify other refreshes as optional with a reason. An available compatible
patch does not require raising every consumer minimum or republishing a group.

The caller explicitly selects the Storage/Metrics Storage/Logging Storage Wasm
normal graph, with no extra features, and the independent native runtime qualifier
normal graph at the selected compiler's host target. It admits both locked metadata
graphs and singleton/boundary policy before either selected Clippy command. Host,
build and dev identities outside these runtime/public-type contracts are not a
whole-lock duplicate ban. Evidence is retained on failure, and lock bytes are
checked before compilation and at completion. The independent qualifier exchanges
Platform/Wire types and Wasm bytes; it has no normal local Memory dependency.

Conflicts must be repaired supplier-first in the proposed graph, preserving the
singleton and exact member catalog versions. The check prints representative
introducing paths for every distinct conflicting identity. These are candidate
graph and public API compilation results, separate from native/Wasm installed
qualification, final archives and actual registry availability before publication.
The existing full gates and release-group decisions remain authoritative.

### Storage allocation consolidation

The maintainer moved allocation ownership into Storage on 2026-11-02.
Shared's current governance names `ic_storage::RuntimeMemory` as the checked
allocation boundary; no local ownership override remains. The storage group
contains only Storage; both storage adapters depend on it.

The subsequent allocation hard cut removes the fixed-slot manager. The current
collection baseline copies ICSS 0.7.2 engines privately behind governed adapters;
it does not restore an upstream dependency or independent memory manager. `make check-production-graph` forbids the
retired substrate. Both selected lockfiles omit it. Named allocation and all
collections require fresh/reset backing; old manager, map and vector formats
have no decoder or implicit reset. See [the current directory contract](current-ledger.md).
