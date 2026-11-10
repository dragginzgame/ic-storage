# Releasing IC Storage

Release and publication require delivery authorization for this repository and
destination. Delegated delivery includes routine group, version and order choices
under the shared contribution rules. Ordinary fixes leave edits unstaged. See the
[contribution rules](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/contributions.md), [shared release contract](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/releases/releases.md)
and [host prerequisites](docs/operations/host-support.md). The native `repo-tool` example
supplies consumer policy to the existing Shared runner; it is not canister code.

Workspace tooling pins, every member's inherited dev-dependency, normal Cargo
lockfiles and the installed snapshot must agree. `toko-tooling-check` runs
before standard releases and consumer publication, qualification, preparation,
batch resume and empty-directory cleanup. Change the exact root pins, resolve
both Cargo lockfiles, then run `make toko-tooling-update`; Cargo never installs
tooling implicitly. Existing product qualification remains required.

## Release groups

Shared 0.31 captures grouped releases automatically. Select one `RELEASE_GROUP`
or an explicit supplier-first comma-separated `RELEASE_GROUPS` roster. Git-only
release remains the default; deployment selects publication in the same batch:

```sh
make RELEASE_GROUP=storage RELEASE_PUBLISH_TARGET=publish release-patch
```

The captured checkout is `target/release-frozen/BATCH`; durable Git, intent and
receipts belong to `.git/release-state/frozen/BATCH`. Later normal-checkout edits
are deferred. `release-candidate-setup` prepares selected tools and locked caches
before version lookup; recovery checks prepared tools offline without installing.
The existing native hooks retain their source, receipt, group and archive checks.
Use the shared status/resume/reconcile/retire commands in
[the batch guide](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/releases/release-batches.md). Reconcile only after preserving deferred
work and verifying publication. Run manual publication from the qualified captured
checkout, never from an unrelated normal-tree HEAD. Retire only a selected complete
batch after its commits and evidence have been reconciled. No old-name aliases or
schema-1 batch conversion exist. Historical releases retain their original tooling.

All three crates remain in one development workspace and one dependency catalog.
The approved [group policy](ci/cargo-release-groups.json) is the executable owner;
root `Cargo.toml` lists each group once beside its member declarations and ledger.

| Group | Packages | Version source | Canonical ledger |
| --- | --- | --- | --- |
| `storage` | Storage | `crates/ic-storage/Cargo.toml` | [Storage](releases/storage/CHANGELOG.md) |
| `metrics-storage` | Metrics Storage | `crates/ic-metrics-storage/Cargo.toml` | [Metrics Storage](releases/metrics-storage/CHANGELOG.md) |
| `logging-storage` | Logging Storage | `crates/ic-logging-storage/Cargo.toml` | [Logging Storage](releases/logging-storage/CHANGELOG.md) |

All members use literal `package.version` fields and group members match their
primary. Root `Cargo.toml` remains a virtual dependency/shared-metadata catalog
without `workspace.package.version`. There are no intermediate group manifests
or relocated packages. The initial layout adoption preserved all numeric versions.

Root [CHANGELOG.md](CHANGELOG.md) is an index. Earlier coordinated history through
0.58.1 is preserved once in [releases/history.md](releases/history.md), with the
original scopes/dates and retained historical details/evidence. Each group owns
one numbered pending entry in its canonical ledger. New detailed notes live at
`releases/<group>/<major>.<minor>.md`; the policy names the current core detail
file as metadata. Update it during source work when the group's next minor line
changes. An old crate-local changelog is only a navigation pointer.
The unpublished independent runtime workspace keeps its own package versions
and dependency selection. Its benchmark and fixtures consume local Storage and Logging Storage packages.
Both groups declare that workspace's catalog and lockfile as release metadata.
Each transaction advances only its own requirement and local lock identity there;
independent registry selections and unrelated groups remain unchanged. Benchmark execution is not a release gate.

Release starts require explicit `RELEASE_GROUP` or `RELEASE_GROUPS`; exact-version
resume and manual publication require `RELEASE_GROUP`. There is no implicit
whole-workspace release. Missing selection refuses before
cache setup, helper compilation or release effects. For example:

```sh
make RELEASE_GROUP=storage release-minor
make RELEASE_GROUP=metrics-storage release-minor
make RELEASE_GROUP=logging-storage release-minor
make RELEASE_GROUP=metrics-storage release-resume VERSION=0.59.0
make RELEASE_GROUP=metrics-storage qualify-release
make RELEASE_GROUP=metrics-storage publish-dry-run
make RELEASE_GROUP=metrics-storage publish
```

These are maintainer commands, not adoption checks. `release-patch` and
`release-major` keep their existing meanings. Pre-1.0 breaking public contracts
require a minor; compatible work uses a patch. Select the release kind matching
the owning pending notes. Wire 0.14 codec-error identities require minor releases.

Groups are release identities, not independent dependency graphs. Versioned
root path requirements display each member’s full current version. Every member
and unused catalog alias belongs to its owning group’s dependency roster. Every core
release updates those shared declarations, preserving their existing operators
without changing adapter versions.
The adapter then needs its own release to publish its changed requirement.
The owned-engine hard cut changes Storage and adapter type identities:
release the compatible core supplier first and then its adapters. Do not publish
an old consumer declaration under an existing version, broaden requirements to
hide a type mismatch, or remove production singleton checks. New versions of
Metrics or Logging remain real public type dependencies when exchanged with hosts.

The current group names are `storage`, `metrics-storage` and `logging-storage`.
Saved attempts from the previous names must resume using their original source
and policy; this rename does not relabel receipts, tags or existing package releases.
The adapter package names are new registry identities and must be published before
registry-only consumers can adopt them. See [naming adoption](docs/migration/storage-naming.md).

## Preparation, validation and exact effects

Commit intended source through an authorized contribution workflow first. The
standard runner requires clean source/index and an admitted branch/destination.
This repository selects `RELEASE_DELIVERY=direct`; the runner owns commits, tags,
locking, ordered effects and atomic branch/tag push. PR release delivery remains
unselected. A contribution PR does not change that policy.

Before delivery, disclose the absolute checkout path, branch, source commit,
remote destination and local versus remote effects. The configured direct path
does not authorize pushing `main`: obtain explicit authorization for that effect
and for any alternate delivery checkout, unless already authorized in the task.
Keep implementation in the requested checkout or an explicitly authorized durable
contribution location; temporary validation copies must never hold the only copy.
See the [shared checkout contract](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/contributions.md#durable-work-and-checkout-location).

Prepare missing caches explicitly with `make fetch-dependencies` and, when needed,
`cargo fetch --locked --manifest-path qualification/Cargo.toml`.
Release commands separately authorize locked cache and selected-tool preparation
before compiling the helper. Explicit caller offline settings remain in force;
ordinary validation/helper calls never fetch automatically or repair lockfiles.
`make install-tools` includes the pinned Build editor; `ci/release-tools.env`
owns its published selection. Build 0.73 supplies the group-aware renderer.
Toolchain/formatter and installed server preparation retain their separate owners.

The native release adapter selects a complete group plan under the
[Storage validation mapping](docs/validation/storage-release-validation.md). Core, shared and
uncertain changes retain all stages; qualified private adapter-only changes omit
unaffected stages with explicit reasons. Repository CI and `make validate` still
run the full roster. Clippy, compiler-floor, Wasm, runtime prerequisites and
whole-workspace archive integration remain required. Development validation may qualify dirty source; release
admission and `make package` retain clean-source checks. `package-workspace.sh` delegates archive admission to Build 0.73.5 and the native
`repo-tool qualify-archives` consumer roster. It qualifies all three current archives as one integration graph with each package's
own version. The selected group is then packaged alone: published suppliers use
their registry checksums, while coordinated members package together. The full
extracted integration graph is qualified using that final archive set; selected
receipts retain only the group's publishable archives.
This integration proof is not registry publication or isolation of incompatible stacks.

The native adapter admits saved source, group policy, compiler identities,
configuration hashes, selected lock bytes and immutable source-derived metadata.
The exact file plan matches Shared's `cargo-release-group.sh files`:
root manifest, root lock, selected ledger, every selected member manifest and declared group metadata. Other groups' package identities, notes
and unrelated requirements are preserved. An unchanged root manifest can be in
that plan without becoming a changed staged file. The index check requires exactly
the resulting changed bytes, including file modes; unrelated work is refused.

The saved source determines the exact lockfile version projection. After publishing
that projection, `cargo fetch --locked --offline` admits the selected graph and
cache without updating dependencies. Formatting and archive qualification follow;
their lockfile checks still reject external changes. Existing attempts remain
bound to their original source and recorded command plan; recover those attempts
from that source rather than changing their receipts to match newer tooling.

Returned preparation failures restore only admitted owned edits and retain all
rollback failures. Conflicting writes and independent dependency changes are
preserved and refused. For interrupted lock-first or manifest-first writes, the
launcher reads each group's version source and lock identity. It builds the helper
against coherent scratch copies through the prepared editor, leaving the real
tracked inputs unchanged. Compilation grants no release authority; the adapter
still needs saved source-bound validation before accepting an interrupted release.

The runner uses `Release GROUP X.Y.Z` subjects and annotated `GROUP-vX.Y.Z` tags.
Journals are `.git/release-state/GROUP-X.Y.Z.plan`; one repository-wide lock still
protects the shared source and Cargo lock. A retry must use the exact group,
version, remote and branch. Normal releases start fresh; recovery is explicitly
`release-resume`. Never remove another owner's lock or select a new version to
avoid conflicting evidence. Historical ungrouped and earlier group-layout releases/receipts remain retained.
Recover them with their original source/tooling; never reinterpret an unfinished
attempt under changed version ownership or ledger paths.

## Receipts and publication

Native validation/preparation/final receipts live under Cargo's selected target:
`release-validation/GROUP-X.Y.Z-validated.json`, `GROUP-X.Y.Z-prepared.json` and
`GROUP-X.Y.Z.json`. They bind group selection and exact source/destination; the
source commit binds the group policy. New validation receipts use schema 2 and
bind the selected plan, baseline, input checkpoint and retained stage evidence;
recovery refuses missing, failed or changed records. Historical receipts remain
with their original source/helper, never reinterpreted as group proofs. An admitted archive map must contain exactly
the selected members. Retained archives remain content-addressed under
`release-validation/artifacts/`; earlier attempts, logs, receipts and tools are
preserved. The common runner's saved intent, validation, prepared and final stages
remain separate evidence. Native receipt reads use Host FS's bounded regular-file
reader with an 8 MiB ceiling, including retention copies. Missing, malformed or
oversized receipts fail without replacement or fallback. Captured observations
have a 120-second Host Process deadline; inherited-IO builds, complete gates and
publication keep their existing execution policy. No fixture pass proves a full gate or live release.

Publish from the exact selected group's qualified release checkout. Admission
requires clean source, release subject/parent, exact annotated tag, retained and
working archive digests, configuration and final receipts. Observe every selected
registry package before any upload; only HTTP 404 means absence. A yanked version,
checksum conflict or failed observation refuses all effects. The core group publishes
Storage only; each adapter publishes only itself. `--no-verify` avoids replaying
compilation already proved by archive qualification. Shared retains each upload's
stdout/stderr, exact selector and independent Cargo/logger statuses.

After a lost response or partial upload, rerun `make RELEASE_GROUP=... publish`
from that qualified checkout. Matching checksums are skipped; delayed visibility
stops safely for a later explicit retry. `publish-dry-run` is not publication
proof and an upstream group's selected release may need to be available first.
Never overwrite a published version or infer publication from cache/source state.

## Focused workflow checks

Select affected checks during development; full release gates require explicit
release/validation authorization:

- Native `cargo test -p ic-storage --example repo-tool --locked --offline` covers
  consumer source/index/receipt checks, all groups, rollback, retention and upload retry
  using substituted Git/Cargo/gate/registry effects; it creates no commits or tags.
- `scripts/ci/test-release-group-plans.sh` compares real native and shared exact
  plans, substituting only Git source observation and retaining unchanged source hashes.
- `scripts/ci/test-release-adapters.sh` exercises actual Make wiring, jobserver
  propagation, failure retention and parser/cache refusal using effect substitutes.
- `scripts/ci/test-consolidated-bootstrap.sh` uses actual offline Cargo and the
  prepared editor for both interrupted states in every group, and resolves/compiles
  coherent group upgrades with unchanged unrelated identities in disposable copies.

`make test-release-adapters` aggregates the three shell fixtures. Preserve partial
staging refusal, earlier artifacts and failed logs. None invokes a live release,
registry upload or installed runtime matrix. Native macOS acceptance stays with
its declared CI job. The inherited Make guard's command-line `MAKEFLAGS=` replacement
limitation remains tracked in [Shared #30](https://github.com/dragginzgame/toko-tooling/issues/30);
retain failure-enforcing Make settings and do not bypass guards or branch protections.
