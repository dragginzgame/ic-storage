# IC Storage tooling adoption and qualification boundaries

Read this before changing the shared snapshot, tool setup, release adapters,
formatting or evidence retention. [AGENTS.md](../../AGENTS.md) owns the local overlay;
[.toko-tooling.snapshot](../../.toko-tooling.snapshot) owns the exact shared
revision, selected roots, payload hashes and executable modes. The former
live-policy exception is retired: dirty sibling rules never override the reviewed
local baseline. Future policy changes require reviewed snapshot adoption.

## Current supplier and Toko 0.39.17 adoption

The 2026-11-12 registry review selects published Toko Tooling 0.39.17,
IC Host Artifacts/FS/Process 0.76.5, IC Wire 0.21.4, IC Platform 0.31.3 and
Runtime 0.21.3. Platform's qualification graph also selects Types 0.19.5 and
macros/runtime-guard 0.21.3. Product and qualification catalogs now name the
requested patch floors with compatible requirements; Toko retains its required
exact selection in both workspaces. Testkit 0.101.0 and PocketIC 16.1.0 stay fixed.

The starting lockfiles already contained the selected Host, Wire and Platform
packages. Targeted Cargo resolution changes only Toko 0.39.15 → 0.39.17 relative
to those saved inputs; unrelated selections are preserved. Both locked caches
were prepared separately. The crates.io website API returned HTTP 403, so the
version review used the canonical sparse registry index instead. An initial
update named Types in the product graph, where it is deliberately absent; the
corrected package-scoped update preserves that boundary.

The authenticated installer retains all 40 entrypoints and 104 installed files.
The new archive checksum is
`aaeab49adb84ea0d66cf86ecc48d826125f9c3ac6eafaf2a362bccfeba7acbfd`.
It adds the requested directory-module rule to the installed baseline. The
preceding 28 exact-content module moves already satisfy that rule, including
test-only directories. No snapshot-owned file was hand-edited. Executable pins
are unchanged; existing prepared tools pass admission without reinstalling them.
The optional new Rustdoc checker is not added to this consumer's entrypoints.

Storage still selects only Runtime's `ic0` feature for Wasm. Native products
have no Platform/Runtime dependency, and Host/Build/Testkit remain outside the
production graph. Custom deployment commands retain their snapshot prerequisite
and the three independent release groups retain their existing requirements.
This adoption and Git readiness work do not select a release or publication.

Snapshot/pins/prepared-tool admission and both production graphs pass. Product
native/Wasm Clippy, 178 product unit tests, 88 native repository-helper tests,
qualification Clippy and 49 benchmark/runtime native tests pass; two installed
entrypoints remain ignored in the native invocation. Installed original/O3/Os/Oz
IO, corruption, custody, upgrade and rollback pass at
`target/qualification/runtime/attempt.exkRp9`.

The focused Make-adapter test exposed a pending `deps-update` adoption gap: the
isolated Makefile lacked `make/cargo-dependencies.mk`. The root now declares
that companion, and the consumer fixture copies its complete declared closure.
The `--make-only` substitute checks pass, including pre-effect snapshot admission;
no real release, commit or publication is performed by those fixtures.

Evidence and saved starting locks are under
`target/evidence/supplier-refresh-20261112/`. Earlier performance numbers remain
bound to their recorded suppliers, not relabelled as results for this graph.

## Qualification Platform 0.31.2 and Testkit 0.101 adoption

The independent workspace selects published Platform 0.31.2, Runtime 0.21.2
and Testkit 0.101.0. The CLI in `ci/runtime-tools.env` also selects Testkit
0.101.0; its server remains PocketIC 16.1.0. The existing async client imports,
polling budgets and caller-owned server lifecycle need no API changes.
Testkit's snapshot-cache preparation changes have no Storage callers.

This targeted lock refresh changes Platform 0.29.3 to 0.31.2 and Testkit
0.100.2 to 0.101.0, adding num-bigint 0.5.1 for Platform's native management
codec. Runtime/macros/guard 0.21.2 and Types 0.19.4 were already in the starting
qualification lock and are preserved. The fixture's Runtime requirement now
names that selected patch. No other lock selections, product source, features
or toolchain change; the pending Toko 0.39.15 work remains intact.

Platform's principal-only composition fixture still uses one Runtime executor.
Testkit's new native Platform codec must remain outside product Wasm; the
production dependency gate retains that separation. Evidence, starting locks,
supplier selection and qualification logs:
`target/evidence/platform-testkit-adoption-20261112/`.

Native Clippy covers runtime, benchmark/example callers and the Platform
composition fixture; Wasm Clippy qualifies the composition fixture. All 47
selected native tests pass. Two installed-server tests remain excluded from
native execution and their runtime contracts are exercised by `make test-runtime`.
Installed original/O3/Os/Oz IO, ownership, collection/directory corruption,
upgrade and rollback checks pass at `target/qualification/runtime/attempt.FwcGrd`.
Production graph, dependency boundary, pin and prepared-server checks pass.
This is focused Linux adoption proof, not a new performance comparison, macOS
result or release qualification. No publication is included.

## Current Toko 0.39.15 adoption

Both workspace catalogs and locks select published Toko Tooling 0.39.15,
checksum `6cc984822e80e52ff7203aadcaf293a13f9fcf33f84175871321cb3a7de703a9`.
The authenticated installer retains all 39 previous entrypoints and adds
`make/cargo-dependencies.mk` with its declared dependencies: 40 entrypoints,
104 installed files. The existing
baseline, release scripts and selected tool versions are unchanged.

The root Makefile includes the shared dependency entrypoint after `consumer.mk`.
`make deps-update` runs ordinary `cargo update -v` sequentially for the root
and qualification workspaces. Optional `DEPENDENCY_PACKAGE` and
`DEPENDENCY_PRECISE` selectors apply to both; Cargo rejects absent packages.
The command has no automatic CI/release prerequisite, edits no manifest
requirements and does not roll back a partially completed update. Inspect both
lockfiles after a failure. It performs no registry audit or extra reporting.

This adoption updates only Toko selections. The pre-existing root lock change
to Runtime 0.21.2 is preserved; qualification retains Runtime 0.21.1 and the
existing Testkit/server pair. No product version, release gate or source changes
are included. Evidence: `target/evidence/toko-03915-adoption-20261112/`.

Snapshot/member, pin and prepared-tool admission pass. The real `deps-update`
caller passes offline with the already-selected Toko package/version, reaches
both workspaces sequentially and preserves both lockfile hashes. The default
help goal, Perl syntax, formatting and documentation checks pass. Publication
routing with a substituted helper shows the snapshot prerequisite before the
helper. An earlier Make dry-run still reached the recursive native helper and
refused the dirty source; no publication occurred. Its output is retained, and
future routing previews must substitute that recursive helper explicitly.
No product build, installed runtime rerun or release was needed for this update.

## Current Toko 0.39.12 and Testkit 0.100.2 adoption

Both workspace catalogs and locks select published Toko Tooling 0.39.12,
checksum `acb1fd3f459a63940669e925d427fd6821abf02a7525bf9ef38d323dc778a8d2`.
The authenticated installer retains 39 entrypoints and 102 files. The baseline
adds direct-supplier freshness review; the LOC caller gains corrected table
width handling. Tool pins and pre-effect deployment snapshot gates are unchanged.

The independent graph and CLI select published Testkit 0.100.2, checksum
`3a48cc6270053eec22a3d6a8cd5dbc622589153c66b405bf9cfbfd53f379f0a3`.
Drivers use its upstream async PocketIC re-export directly, with no compatibility
client. Builders, convenience operations and rejection handling follow the new
API. Expected initialization refusals use reinstall on a fresh empty canister to
receive a typed rejection; only execution trap codes qualify rollback/refusal.
The selected server remains 16.1.0. Five-minute polling budgets and awaited
best-effort deletion replace the old absolute HTTP/deletion guarantees; the
managed CLI retains its server lifetime bound. These are native qualification
changes, not production Storage API or format changes.

Cargo refreshed only Toko in the root lock. The qualification update follows
Testkit's required native graph, including PocketIC 16.1.0, Host 0.76.4 and
Types 0.19.3, removing its older Platform 0.27 / Types 0.18 identities. Both
locked dependency caches were prepared. Prior tools and evidence remain retained.

The 2026-11-11 registry review covered all 21 direct registry suppliers across
both workspaces. Qualification still selects Logging 0.24.6 (latest 0.24.16),
Platform 0.29.3 (latest 0.31.1) and thiserror 2.0.18 (latest 2.0.21). These optional
refreshes are deferred to the next requested repository-wide supplier refresh;
Platform additionally needs its own compatibility qualification. The transitive
thiserror 1.x identity follows the native upstream graph. Other direct supplier
locks select the latest stable non-yanked releases at review time. The pinned
ICSS Git research control remains intentionally separate from registry freshness.

Evidence: `target/evidence/toko-testkit-adoption-20261111/`, including original
locks, preserved pending diff, registry review, installer and qualification logs.
Snapshot/member/pin/tool checks, both production target graphs, dependency and
Candid boundaries, root/qualification LOC callers, formatting and documentation
links pass. Native Clippy covers both clients and the benchmark's owned/reference
feature path; 46 native tests pass. Two installed-server entrypoints are ignored
by native tests and exercised through the installed runner instead.

`make test-runtime` passes original/O3/Os/Oz IO, collection/directory/custody and
Logging upgrade/rollback checks at `target/qualification/runtime/attempt.FMfrJq`.
The seven-case benchmark smoke at `target/evidence/storage-comparison/attempt.on4yqw`
covers all five collections, both unbounded Vector initialization refusals and
ICSS's fixed-Option setup refusal. Reproduce it with:

```sh
BENCH_CASES=Map-U64-U64,Set-U64-Unit,Vector-U64-U64,Cell-U64-Array32,Region-U64-U64,Vector-U64-Bytes,Map-U64-OptionU64 \
  BENCH_PATTERNS=uniform make benchmark-storage BENCH_ROWS=8 BENCH_SAMPLES=1
```

`make benchmark-storage-size` passes 127 calls and two upgrades for each of 16
modules at `target/evidence/storage-consumer-size/attempt.P4PlrP`. All recorded
consumer byte sizes exactly match the preceding cursor trial; the all-collection
Storage application remains 116,214 bytes. The migrated admission driver also
passes 864 Storage and 600 ICSS observations against retained lease/Vector-trial
fixtures. That is client compatibility evidence, not a new performance claim.
The pending production Rust files match their pre-adoption hashes.

The full type catalogue, release gates and fresh macOS runs are not part of this
focused Linux qualification. No package version bump, deployment or sibling
change is included.

## Current IC Build 0.83 adoption

The root native helper selects published IC Build 0.83.0, checksum
`47b532cca843e5855ceeb42b39c651492aa3ede7c3abb53f5b0b389e28e92b5b`,
from producer commit `662516e51bf20ad8a61465488ae30e366e170328`.
The separate metadata editor in `ci/release-tools.env` also selects 0.83.0;
`make install-release-tools` prepares it through the shared verified store.
Existing editor versions and release evidence remain retained.

Only Build changes in the root lock. The independent qualification graph has no
Build dependency and its catalog/lock are unchanged. Progress and scheduled
maintenance API removals have no Storage callers; the native Cargo, environment,
archive and metadata APIs compile without adaptations. Native helper Clippy,
all 88 helper tests, snapshot/pin/production graph admission, editor admission
and the focused bootstrap-recovery fixture pass. The fixture runs the actual
published renderer with substituted Cargo/tool-selection effects; it is not a
live release proof. Evidence: `target/evidence/build-083-adoption/`.
The adoption leaves production code and package versions unchanged. Subsequent
authorized deployment is tracked in the [current handoff](../status/current.md);
release qualification and publication remain separate from these focused checks.

## Previous Cargo-owned 0.39.10 selection

Both workspace catalogs and locks select published Toko Tooling 0.39.10, checksum
`ea181684d772d4632ba534b80bd47bb8b2044278ea346f2c02e37d2ea9991a57`.
The authenticated preview/install retains the 39 entrypoints and 102 files.
Only the dependency declaration checker and hash-dependency catalog change from
0.39.9: the producer admits its reviewed Platform Digest adapter under precise
owner/version/feature constraints. Storage's own production restrictions remain.
Baseline, canonical root files and tool pins are unchanged in this refresh.
Snapshot-only installation is followed by separate selected-tool admission;
custom deployment entrypoints retain their pre-effect snapshot checks.

IC Wire 0.21.1 is selected in both graphs. Only Wire and Toko change relative
to the preceding locks; no unrelated selections are refreshed. Wire 0.21 changes
generated TypeScript names and 0.21.1 optimizes host/browser traversal; Storage
does not generate those clients. Its checkpoint framing and codec bytes remain
unchanged. The adapters expose Wire errors, so Metrics Storage 0.76.0 and
Logging Storage 0.72.0 select the new public type identity. Core Storage uses
Wire only for development.

Evidence is under `target/evidence/deploy-toko-wire-20261111/`, including before
locks, authenticated preview/install and focused checks. The authorized delivery
includes the existing optimization batch and all three affected packages. Release
gates, publication receipts and final identities remain separately owned by
the [release workflow](../../RELEASING.md) and [current handoff](../status/current.md).
Snapshot/member/tool/pin/production checks and consumer compilation pass. Installed
original/O3/Os/Oz IO, collection/directory/custody and Logging upgrade/rollback
qualification passes at `target/qualification/runtime/attempt.NdKepQ`.

## Previous Cargo-owned 0.39.9 selection

Both workspace catalogs and locks select published Toko Tooling 0.39.9, checksum
`1dd7bdb5bbb9bb8076dd35d8dc179c9e49196d9945c8e585990e40091b468b48`.
The authenticated installer retains 39 entrypoints and 102 files. Relative to
0.39.8, the baseline's dependency-policy references and the declaration checker
change: native dev-only PocketIC dependencies are admitted by the common rule.
Storage keeps its stricter existing graph boundaries and Testkit 0.95 selection.
Tool pins are unchanged; snapshot-only installation and selected-tool admission
remain separate. Custom release entrypoints retain pre-effect snapshot checks.

The requested supplier adoption selects Wire 0.20.0 and Build 0.82.0. The
qualification fixture selects Platform 0.29.3; both graphs resolve Runtime
0.21.1. Platform's required Macros/guard 0.21.1 and Types 0.19.1 follow its
published dependency graph. Testkit's older native infrastructure identities
remain isolated. Core Storage still selects only Runtime's `ic0` feature on Wasm
and no Platform on native. No production ICSS dependency is introduced.

Wire's public error identity changes require future minor releases of Metrics
Storage and Logging Storage; their pending notes record the broadened varint
acceptance. Existing encoder bytes and checkpoint formats remain valid. Build's
removed acquisition APIs are not used by Storage's native release helper.
The helper's 88 tests pass against 0.82.0 without a source adaptation.

Evidence is retained in `target/evidence/adoption-scalar-20261111/`, including
the initial pending diff, before-locks, authenticated preview/install, selected
tool and production-graph checks, adapter tests and scan attribution. Earlier
pending work remains preserved. No package version bump, deployment, Git
delivery or sibling edit is included. See the
[scan investigation](../measurements/storage-scalar-scans-2026-11-11.md) for the
matched runtime/size evidence and its scope.

## Previous Cargo-owned 0.39.8 selection

Both workspace catalogs and locks select published Toko Tooling 0.39.8, checksum
`baf4177553a6a818837ae9765afc021ac3e3c0722e4622f773a990dc4b41cbc9`.
The authenticated installer preserves the 39 entrypoints and 102 managed files.
Among installed files, only the canonical root `LICENSE.md` changes; its prior
repository-scope appendix is no longer in the producer's canonical file. The
baseline, contribution file, executable bytes and selected tool versions are
unchanged. Historical release grants are not reinterpreted by this adoption.

Snapshot-only installation is appropriate for this unchanged executable set;
selected tool admission and the prepared PocketIC server are checked separately.
All ten packages still inherit the root licence file. The existing exact-pin
exception records now select 0.39.8. The adoption changed only Toko in either
lock; pending IC Build 0.81.2 and IC Host 0.76.3 selections remain.
Rust remains 1.99.1. Every inventoried product/qualification Rust file is unchanged.

A separate root-lock update to Runtime 0.21.1 arrived during the subsequent
benchmarks and is preserved. Their independent qualification lock remains on
Runtime 0.21.0; the captured results do not qualify the new root Runtime selection.
Snapshot/member admission, pins, production graphs, canonical licence, selected
tools and the prepared server pass their focused checks.

Evidence is retained under `target/evidence/toko-0398/`: authenticated preview
and installation, before-locks, source hashes and focused admission checks. No
product version change, commit, publication or sibling edit is part of this
adoption. The separately requested [complete comparison](../measurements/storage-full-suite-2026-11-10.md)
validates 192,444 observations, the size consumers and allocation diagnostics.
Its observed server timeouts also justify the consumer runner's one-hour
lifetime; this does not change producer-owned files, request budgets or Rust code.

## Previous Cargo-owned 0.39.7 selection

Both catalogs and locks select published Toko Tooling 0.39.7, checksum
`fd795f6ef878496ac27a990a9208b87fa786a9148c18580338ef8ac79df501d7`.
The authenticated installer preserves all 39 entrypoints and 102 files. The
baseline changes its confidential-repository selection rule; Storage retains its
existing public-package metadata and canonical root files. The only executable
changes are diagnostic documentation URLs in the release Make adapter and batch
runner. Tool selections and executable behavior are unchanged, so installation
uses the documented snapshot-only mode without repeating tool setup.

All ten packages resolve their inherited `license-file` to the one canonical
root `LICENSE.md`; `CONTRIBUTING.md` also matches the released producer bytes.
Each of the three product package file lists contains exactly one `LICENSE.md`.
No maintained nested licence files or copied engine directory remain. External
package metadata is unchanged by this adoption.

Only Toko changes in each lockfile relative to the preserved pre-adoption inputs.
Cargo's initial unrelated Metrics downgrade was reversed with Cargo, retaining
the already-selected 0.29.11. Its missing archive was fetched with the existing
lock, without changing selection. Evidence is under `target/evidence/toko-0397/`.
Snapshot/member admission, pins, canonical-file checking, package file lists,
documentation links and substituted Make release callers pass. All 169
inventoried Rust sources retain their pre-adoption hashes. No native collection
suite or installed performance run is repeated for these tooling-only changes.
No product Rust changes, commit, deployment or sibling edit is included.

## Previous Cargo-owned 0.39.6 selection

Both catalogs and locks select published Toko Tooling 0.39.6, checksum
`00fcdf9fa866007e0d8e802f711257cd23e3081a467daf69225b3844f91dbc1b`.
The authenticated installer retains the 39 selected entrypoints and 102 files;
reference docs remain upstream. Only the baseline changes among installed files.
Reviewed 0.39.5 reference links retain their immutable revisions.

Both workspace catalogs now own `license-file`: the product root selects
`LICENSE.md`, and the qualification root selects `../LICENSE.md`. All ten real
packages inherit that selection. This is applied together with removal of the
remaining copied ordered engine and its private helpers. The obsolete product
file-hash gate is removed; the canonical root check remains. See the
[replacement record](../design/ordered-map-replacement.md) for actual inputs,
qualification and remaining performance/release requirements.

The installer and member/pin admission pass. Existing release groups, pre-effect
snapshot checks and two-lock release adapter remain unchanged. Rust stays at
1.99.1; only Toko changes in the locks. Evidence is retained under
`target/evidence/toko-0396/`. No deployment or sibling edit is included.

## Previous Cargo-owned 0.39.5 selection

Both workspace catalogs and locks select published Toko Tooling 0.39.5, checksum
`392d5edc7893549e7186f8549534935fda8467f96d016487fbef78ed7a44f300`.
The authenticated installer was previewed with the complete retained selection:
39 entrypoints produce 102 installed files, down from 178. Shared reference docs,
rules, audit procedures and task catalogs are no longer installed. The unused
fleet-maintenance runner retires with its inputs. The CI download-cache action
is now an explicit root rather than an incidental documentation companion.

Root `TOKO-TOOLING.md`, canonical files, executable callers and runtime inputs
remain receipt-owned. Consumer links to retired shared references use immutable
upstream revision `12459b6029115b830b8477879d3bea2f030b6e63`; the baseline retains
its own separately pinned detailed-policy revision. Read the
[upstream adoption guide](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/adoption/consuming-snapshots.md)
before changing the selection. There is no local shared-reference library or
`docs/toko-tooling/` namespace. Storage's current guides are grouped by design,
operations, validation and adoption; historical evidence paths remain intact.

Future selection changes update both Cargo pins and their locks, then run
`make toko-tooling-update` and `make toko-tooling-check`. A separately resolved
Cargo selection is installed with `make toko-tooling-install`. Preserve the
recorded executable roots when changing document selection.

Storage keeps `TOKO_TOOLING_RELEASE := 1`, its three release groups, pre-effect
snapshot checks and the native two-lock release adapter. This update does not
enable another release transaction. Rust remains 1.99.1. Only Toko changes in
the lockfiles; product sources and all other dependency selections are preserved.
Evidence is retained under `target/evidence/toko-0395/`.

Snapshot/member admission, selected-tool setup/checks, dependency pins, native
and Wasm production graphs, both-workspace dependency boundaries and formatting
pass. The affected-document check validates 559 local references in 40 files;
the maintained documentation gate also passes. Actual CI-selection fixtures and
Make release-caller fixtures pass with Git/release effects substituted. They do
not establish a live release or hosted CI result. All 176 pre-existing Rust
files retain their exact hashes. The moved adoption record has an intent-to-add
entry so the pin checker can find its evidence path; no contents are staged.
No commit, publication, deployment or sibling edit is included.

Product-licence completion is still blocked: the five pre-existing licence-file
deletions remain pending, while the ordered engine still contains attributed
DFINITY-derived code and package metadata still names the previous licences.
The existing product checksum gate therefore cannot pass. This adoption does
not remove that gate, relabel the remaining engine, or claim clearance. Complete
the [independent replacement and final package review](../design/independent-storage-replacement.md)
before release. Tooling installation and this outstanding product work have
separate validation outcomes.

## Previous Cargo-owned 0.39.3 selection

Both workspace catalogs and locks select published Toko Tooling 0.39.3, checksum
`9286bc781005e242e90234024a41416e92f1cf8a3098033864affab7dbf61197`.
The authenticated installer preserves all 87 selected responsibilities using the
new upstream guide paths and adds the canonical contribution policy (88 roots,
178 installed files). Its own exporter retires unchanged owned predecessors. Consumer-owned links now follow the canonical topic paths;
product documentation and pending source work remain in place.

Storage explicitly sets `TOKO_TOOLING_RELEASE := 1` before `make/consumer.mk`.
Existing group selection, pre-effect snapshot checks, independent package releases
and the native two-lock release adapter remain authoritative. The opt-in shared
qualification release transaction is not enabled by this update. Rust remains
1.99.1; host, Build, Testkit, Runtime and Wire selections are unchanged.

The 0.39.3 `docs/toko-tooling/` ownership namespace is a declared destination for
a later qualified producer migration. This release explicitly keeps its current
export paths. No consumer-specific relocation or namespace gate is added.

The independently authored implementation policy follows the already authorized
[replacement plan](../design/independent-storage-replacement.md). The maintainer
reaffirmed replacement of remaining DFINITY-derived code during this adoption.
Current product grants/notices and package licence metadata remain while that
scoped migration is unfinished. The new canonical root `LICENSE.md` is installed
as snapshot content. The product gate also verifies exact existing notice bytes,
including Apache, using the shared checksum helper; the retired MIT-checker
interface is not copied or retained. Final package/notice review remains with the
replacement plan. Snapshot installation does not claim completion of the canonical
product-licence policy or clearance of the remaining ordered engine.

Only Toko changes in either lockfile. Evidence is under
`target/evidence/toko-0393/`, including the initial pending diff, source-preservation
hashes, path mapping, installer observations and focused caller checks.
Snapshot/member admission, selected-tool setup/checks, both-workspace pins and
production/dependency boundaries, formatting and affected documentation links
pass. The consumer's Make release-dispatch fixture passes with effects substituted;
this is caller qualification, not a live release. The current LOC reporter admits
all ten packages across both workspaces; its macro/conditional-source diagnostics
remain advisory. All 176 pre-existing Rust
source files are byte-for-byte preserved. No product version change, release,
publication, sibling edit or Git delivery is included.

## Previous Cargo-owned 0.38.2 selection

Both workspaces select published Toko Tooling 0.38.2, authenticated by checksum
`45c12cc2aa598d14e137dd7db50f12e943470b4ce351dfcaffeccde6821c1876`.
The supported installer preserves the selected 87 roots and installs 173 files. The
product/qualification layout adopted with 0.38.0 remains in place. The updated
contribution rule permits scoped intent-to-add for task-owned files required by
validation; this admits the relocated lockfile without staging its contents.

The new shared qualification release transaction is opt-in. This adoption keeps
the existing qualified native consumer's two-lock preparation and recovery
contract; it does not change release file ownership or enable a second renderer.
Switching to the shared transaction needs separate caller/receipt/recovery
qualification before deleting that implementation.

IC Wire 0.19 is adopted alongside this refresh in both workspaces. Its native
codec implementation is unchanged from 0.18; the new optional Platform schema
and packed-finite adapters are not selected here. Public adapter errors expose
the new Wire identity and therefore require minor adapter releases. Core Storage
uses Wire only for development/qualification. No stored-data reset, schema
migration or product source change is included. Existing unrelated locked
suppliers, pending CI fixes and relocation work are preserved. Evidence lives
under `target/evidence/wire-019-toko-0382/`.

Snapshot verification, both-workspace pins, dependency boundaries, formatting
and documentation links pass. All 42 adapter unit tests, five adapter doctests,
44 native qualification tests and four release-group/recovery tests pass.
Installed qualification passes original/O3/Os/Oz IO, checkpoint, upgrade and
rollback cases. Release Make dispatch passes with substituted effects; no live
release was invoked. Both locks change only Wire and Toko from the pre-adoption
selections. `qualification/Cargo.lock` retains an intent-to-add index entry;
no file contents are staged, and no commit or push is included.

## Previous Cargo-owned 0.38.0 selection

Both catalogs and lockfiles select published Toko Tooling 0.38.0 (checksum
`0e2de9853b58a63373084b551d4ce8c444480aa33d45c35ca21e8de7ab8f9933`).
The authenticated installer preserves the selected snapshot entrypoints. The
updated baseline and workspace rule separate product and qualification packages.

The three product packages remain under `crates/`. The seven unpublished
packages formerly at `testing/runtime-qualification/crates/<package>/` now live
at `qualification/<package>/`. Their virtual root, catalog, lockfile and policy
moved from `testing/runtime-qualification/` to `qualification/`. There is no
intermediate `crates/` directory or compatibility path. Private versions remain
0.0.0, and none belongs to a product release group. The existing release adapter
continues to project product dependency references and the qualification lock
atomically; qualification package versions remain independent.

Make, CI, formatting, graph checks, release metadata and its fixtures, benchmarks,
runtime source inventories and maintained guides use the new paths. Historical
reports and completed release evidence retain their original source identities;
old ignored build outputs remain in place. Package-local tests and the native
repository helper remain with their product owner. No product-to-qualification
Cargo dependency is introduced.

The pending CI fixes and pre-existing Metrics/Logging lock selections are
preserved. This adoption does not publish packages or change product APIs,
stored formats, Rust 1.99.1 or runtime/tool selections. Focused evidence is under
`target/evidence/toko-0380/`.

Snapshot integrity, formatting, shell/workflow lint, dependency boundaries and
local links pass. Cargo metadata confirms three product members and seven
unpublished qualification members, with no reverse dependency. Compiler admission
uses Cargo membership rather than directory-depth assumptions, with a grouped
member regression fixture. All 88 native release-helper tests pass (the changed
sorted-path expectation was rerun after repair), as do 44 native qualification
tests; two installed-only tests remain ignored in the native suite and are
covered by installed qualification. The selected fixture/size-consumer Wasm
builds and separate Storage/ICSS benchmark feature builds pass. Installed IO,
upgrade, rollback and logging qualification passes in original/O3/Os/Oz modes.
At that adoption the pin check was pending tracking of the relocated lockfile.
The 0.38.2 adoption above resolves it through authorized intent-to-add. No commit,
push, product version change or publication is included.

## Previous Cargo-owned 0.37.5 selection

Both workspace catalogs and lockfiles select published Toko Tooling 0.37.5
(checksum `9fc1a752f4f8203d999c4cd593afb9ea18717b28d407492fc98aab5722526be3`).
Only Toko Tooling changes in either lockfile. The supported authenticated update
preserves all 87 entrypoints and 171 files, including the offline guides and
active Candid checker explicitly retained during 0.37.4 adoption.

The changed payload is `rules/cargo-dependencies.md` and
`scripts/ci/check-cargo-dependency-boundaries.sh`. Their narrow exception admits
Candid only as an unconditional dev dependency of unpublished
`ic-wire-benchmarks`; production/build, target-specific, public and other-owner
edges remain rejected. Storage is not that owner, so its production and local
Candid restrictions are unchanged. The baseline's reviewed policy revision,
compiler, tool selections and release gates remain unchanged. This adoption
does not activate unrelated dependency updates or a fleet audit.

Snapshot integrity, selected-tool admission, both-workspace pin and Cargo
boundary checks, the local Candid/CBOR checker, formatting and documentation
links pass. All 173 package/qualification Rust source files match their
pre-adoption hashes. Standard/custom deployment check gates are unchanged.
Evidence is retained under `target/evidence/toko-0375/`. Existing Platform/Runtime,
async qualification and allocation-optimization work is preserved. No product
version change, commit, push or deployment is included.

## Previous Cargo-owned 0.37.4 selection

Both workspace catalogs and Cargo lockfiles select published Toko Tooling 0.37.4
(checksum `e83d18a633f4b4fbd5f21ae4fd431ee11bf2d825ab40b5413a4cee35a51295b7`).
Cargo resolution changed only Toko Tooling; locked cache preparation covers both
workspaces. The authenticated registry installer preserves the existing 83 roots
and adds four explicit roots: `rules/engineering-details.md`,
`rules/rpc-budgets.md`, `docs/candid-boundary.md` and
`scripts/ci/check-candid-boundary.pl`. The preview would otherwise retire these
formerly implicit companions, including a checker called by Storage's Makefile
and offline policy references. No snapshot-owned file is edited by hand.

The standalone baseline now links detailed policy at immutable revision
`626da7cd9626cc2e152594862a2d82ea82ef51da`. Its dependency-selection rule applies
latest stable releases within authorized update scope, permits justified
independent versions, and requires scoped approval for older direct selections.
It does not authorize unrelated dependency updates. Storage retains its
consumer-owned architecture, governance, format, compiler, size budget and
release gates. Existing exact tooling-pin exceptions remain required by the
installation contract. No new exception or unresolved policy conflict was
identified in this scoped adoption; this is not a fleet dependency audit.

The payload changes only the baseline and four policy/adoption documents;
executable bytes and modes remain unchanged. Snapshot integrity, selected-tool
admission, both-workspace pins, the active Candid/CBOR checker, formatting and
local documentation links pass. Actual rustc remains 1.99.1. Standard and custom
release/publication callers retain their pre-effect `toko-tooling-check` gates;
unchanged executable caller qualification is not rerun as a release.
Focused evidence is retained in
`target/evidence/toko-0374/`. Pending optimization work, tool selections, product
versions and release requirements are preserved. No commit, push or deployment
is included.

## Previous Cargo-owned 0.37.2 selection

Both workspace catalogs and lockfiles select published Toko Tooling 0.37.2
(checksum `7660f9041b01073ba4038044983f18f3ac5a73aeebb3a9d054c70810357e20e3`).
The supported `make toko-tooling-update` refreshed the authenticated receipt and
admitted selected tools. All 83 entrypoints and 171 installed file hashes/modes
are unchanged from 0.37.1. This release fixes dependency-report classification;
those reporting files are outside Storage's selected export closure. No new
entrypoints or live sibling source are adopted.

Only Toko Tooling changes in either lockfile. Existing product work, Rust 1.99.1,
release requirements and tool selections are preserved. The 0.37.1 caller and
runtime-tool qualification below remains applicable to the unchanged payload.
Snapshot integrity, both-workspace pin admission, tool checks, formatting and
local documentation links pass. Evidence for this patch is under
`target/evidence/toko-0372/`; no commit, push or deployment is included.

## Previous Cargo-owned 0.37.1 selection

Published Toko Tooling 0.37.1 is selected exactly in both workspace catalogs and
lockfiles. Cargo metadata locates the authenticated released installer; the
reviewed preview and snapshot-only installation preserve all 83 entrypoints.
The canonical installer retires unchanged owned old names. The receipt is now
`.toko-tooling.snapshot`, the baseline is `TOKO-TOOLING.md`, commands use
`toko-tooling-*`, variables use `TOKO_TOOLING_*`, and package LOC policy uses
`package.metadata.toko-tooling.loc`. There are no local compatibility aliases.
All ten real packages retain inherited dev-dependencies and cargo-machete exemptions.

Future updates use `make toko-tooling-update`; separate Cargo selection changes
require `make toko-tooling-install`. `make toko-tooling-check` admits declarations,
lock selection and installed bytes. The independent workspace remains covered
by `make check-pins`. This adoption changes only Toko Tooling's lock selections;
Rust 1.99.1, product versions and release requirements are unchanged. Existing
optimization work and historical evidence/caches are preserved. No hook activation,
commit or deployment is part of this adoption. Snapshot/pin admission, selected
tool setup/checks, both-workspace formatting and LOC classification, CI selection,
workflow/shell lint, substituted release Make dispatch and failure-evidence
archive round trips and release-stage reuse pass. The unchanged Testkit 0.79.0
CLI is prepared in the new tool store, and PocketIC 16.1.0 passes offline server
admission; no new installed application qualification is claimed. LOC reports zero
unclassified Rust lines in both workspaces. Historical sections below retain
their original executable names as evidence, not supported aliases. Focused
evidence is retained under `target/evidence/toko-0371/`.

## Previous Cargo-owned 0.36.1 selection

Published Toko Tooling 0.36.1 is selected exactly in both workspace catalogs and
lockfiles. The supported installer refreshes the authenticated snapshot; payload
bytes come from crates.io. The patch corrects test-only block/statement CLOC
attribution inside call-argument closures. All ten real packages retain inherited
dev-dependencies and their existing cargo-machete exemptions. No dependency
versions besides Toko Tooling changed during this adoption. Product release
requirements remain unchanged. Evidence: `target/evidence/deploy-storage-071/`.

## Previous Cargo-owned 0.36.0 selection

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
retired. The [measurement index](../measurements/README.md) owns compact topic summaries,
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

The [adoption audit](../audits/2026-11-05-shared-tooling.md) records the directory,
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
[Shared contribution contract](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/contributions.md). This refresh stays in
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

The [consumer mapping](../validation/storage-release-validation.md) keeps narrow private adapter
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
adoption; verified delivery and qualification are in the [handoff](../status/current.md).
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

The library [dependency catalog](../../Cargo.toml) owns direct package requirements
and feature selections; its [lockfile](../../Cargo.lock) owns resolved identities.
The independent qualification [catalog](../../qualification/Cargo.toml)
and [lockfile](../../qualification/Cargo.lock) retain their separate
host graph. The [Build editor pin](../../ci/release-tools.env) and
[Testkit CLI pin](../../ci/runtime-tools.env) select executables independently.
Read those owners for versions rather than copying their inventories here.

The Wasm production graph admits one Runtime provider and forbids the
Platform facade and `ic-stable-structures`, as declared in [the local overlay](../../AGENTS.md). Library admission,
executable setup/check and installed qualification remain separate authorities.
Selection alone does not establish acceptance or permit relabelling older proof.
The [release ledger](../../CHANGELOG.md) and [current handoff](../status/current.md)
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
The canonical [preview and refresh procedure](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/adoption/consuming-snapshots.md#preview-before-refreshing)
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
`TOKO_TOOLING_SETUP_WAIT_SECONDS=0..300` selects another canonical integer
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
[`ci/release-tools.env`](../../ci/release-tools.env), with regular source copies and
no source symlinks. `make install-release-tools` explicitly prepares it;
`make release-tools-check` admits it offline. The common setup/check aggregates
include that tool. Standard release bootstrap can prepare the selected editor;
ordinary helper recovery only checks it. Neither path changes dependency
selection or installs a Rust compiler.

Earlier adoption decisions and exact qualification results remain in the
numbered changelogs, [release-workflow evidence](../release-workflow-qualification.md)
and Git history. Their former setup exceptions or tool selections are historical
evidence, not another current source of policy.

## Ownership and consumer qualification

See [the implementation ownership boundary](../design/ownership.md) for runtime, collection
and host-tooling owners. Shared Tooling owns the exported runner, installers,
formatter prerequisites, Make modules, archive primitives and their generic
regression matrices. Storage owns which capabilities it selects and the following
consumer proofs:

| Retained proof | Local caller |
| --- | --- |
| Snapshot hashes, modes and selected roots | `make toko-tooling-check` |
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

The production graph check uses Shared's canonical observer with Storage-owned
exclusions for native tooling and test dependencies. It checks the default
`wasm32-unknown-unknown` library graph; it does not replace compilation, artifact
size budgets or installed qualification. `test-support` adds no dependencies.

## Tool setup and formatting

The shared catalog owns common host/Cargo pins and the IC tool catalog. Storage
selects its validation compiler, library MSRV, independent qualification workspace
in its own manifests and Makefile, and Testkit CLI in `ci/runtime-tools.env`.
Follow [host support](../operations/host-support.md) for prerequisites and matching native evidence.
Source adoption, explicit setup, local activation and native qualification are
separate; an exported script or configured CI job is not acceptance evidence.

Storage selects only `wasm-opt` for installed runtime qualification, including
native offline reuse and compact failure evidence. The full canonical catalog
remains admitted; unselected tools are not downloaded or required. Full evidence
retention and prior installations remain unchanged.
Storage extends the common ordered setup/check targets with `install-ic-tools`
and `ic-tools-check`, and exports `.tools/ic/bin`. Common setup supplies
`.tools/host/bin` and `.tools/rust/common/bin`. Prepared bundles, failed attempts
and old selections remain retained; refreshing source never silently installs
or deletes tools. Fleet reports run in Shared Tooling; local `make cloc` selects
the root workspace, or an explicit `CLOC_MANIFEST` for the independent graph.

Formatting covers the root workspace, `qualification` and the
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
with Testkit. See the [qualification runner](../../qualification/README.md).

## Release and evidence boundaries

The shared runner owns ordering, locks, saved intent and Git effects.
Storage selects direct delivery and owns its Rust source/package receipts,
metadata transaction and proof admission; IC Build supplies the available Cargo
observers and projection checks through explicit native development dependencies.
The unused alternate release delivery and validation checkpoint paths remain
unselected. Group selection uses the existing runner, with exact file plans
checked against Shared. Every core release aligns versioned root path requirements
with member versions while preserving operators and adapter versions. See [RELEASING.md](../../RELEASING.md).

Normal checks remain offline. Authorized release entry points separately allow
locked cache preparation before helper compilation; an explicitly offline caller
stays offline. Never run a release command, create a fixture commit or alter the
real index merely to qualify adoption.

Storage selects its product logs and archive contents, while shared primitives
own tool selection, checksums and archive construction. Full retention is the
default; CI requests compact retention only for freshly admitted complete active
sets. Failed, unknown and unselected bundles stay full. The workflow binds the
exact upload ID/digest, repository/run and token. Preserve unsuccessful collection
observations, gate logs, prior receipts and package archives across retries.

## Stable-memory governance policy

Production stores consume checked Storage handles after admission and durable
allocation commitment. [AGENTS.md](../../AGENTS.md#stable-memory-governance) and
[SAFETY.md](../../SAFETY.md) record this boundary. The common rule was adopted with
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
[The consumer contract](../validation/ci-selection.md) and exact prose allowlist own CI dispatch;
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
Platform/Wire types and Wasm bytes; it has no normal local Storage dependency.

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
have no decoder or implicit reset. See [the current directory contract](../design/current-ledger.md).
