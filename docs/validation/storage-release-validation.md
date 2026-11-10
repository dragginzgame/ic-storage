# Storage release validation selection

Storage adopts Shared Tooling's [group-plan contract](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/releases/release-group-validation.md)
through the existing native release adapter. The reviewed Shared snapshot already
contains the required runner; this adoption does not refresh vendored scripts or
add a second release runner. ShellCheck's separate exact-input reuse remains
unchanged and is not used as evidence for an omitted group stage.

## Complete roster and narrow plans

[The canonical policy](../../ci/release-validation.json) owns the 12 ordered stages.
Full Make dispatch projects admitted group names and uses Shared's `roster`
interface; the native selector derives its vocabulary from that same catalog. `make validate` and `make validate-toolchain` always run every stage.
GitHub CI retains that full roster for non-docs pushes and broad PR changes;
private adapter PRs use the [qualified CI selector](ci-selection.md#private-adapter-prs).
Installed runtime jobs, native hosts, prerequisites and publication requirements
are unchanged.
Lint precedes expensive tests and artifacts.

The native selector compares the admitted source with the selected group's
previous annotated release tag. It verifies ancestry and the previous package
version, and records the tag, resolved commit, complete changed-path list, host
and tracked input inventory. Shared observes the tag and complete diff; missing, lightweight or unrelated
baselines select the full roster. Observation errors refuse. Storage checks the
previous package version for an available baseline. It never compares only the last commit.

| Change scope | Selection |
| --- | --- |
| Core Storage | All 12 stages, including both adapters and reverse consumers |
| Private Metrics Storage implementation/tests, its release notes and handoff | Eight stages; omit release-tool fixtures, core tests/size probes and Logging tests |
| Private Logging Storage codec/tests, its release notes and handoff | Eight stages; omit release-tool fixtures, core tests/size probes and Metrics tests |
| Public entrypoints, manifests, either lockfile, compiler/features, CI/release scripts, shared source, examples, cross-group or unknown paths | All 12 stages |

The two adapter scopes are closed, reviewed directory sets, not an inference
that any Rust file is private. Changes to their `lib.rs` stay full. Narrowing also
requires that the union of the entire locked dependency graph has no reverse
path from core, the other adapter or another local package to the selected adapter. New dependency
relationships therefore cannot silently retain obsolete exclusions after a new
release baseline. All lock and manifest edits themselves already select full.

Every plan retains an explicit reason for each exclusion. Lint, feature-enabled
compilation, rustdoc and Wasm test compilation select the adapter package for an
admitted narrow plan; full plans keep `--workspace`. The consumer Cargo wrapper
requires the plan's hash in Shared's runner checkpoint and matching source/group
before narrowing. `RELEASE_GROUP` alone does not narrow any command. The feature
check explicitly enables `ic-storage/test-support` for either selection.
Production policies, formatting and whole-workspace archive integration always
run. Final
selected-group archive qualification, supplier-first publication, clean source,
index, configuration and unchanged unrelated identities are still independently
admitted. No native host or installed-runtime requirement is waived.

## Receipt and recovery boundary

After ordinary release admission, `repo-tool` writes a fresh plan and selection
context under `target/release-plans/`, then dispatches Shared's
`run-validation-targets.sh --plan` through `make validate-release-plan`.
The runner owns canonical plan admission, input checkpoints, ordering, retained
logs and blocked/not-applicable/PASS/FAIL events. Inputs include the complete
tracked inventory, selector, recipes, policy, context and plan.

Validation receipts use schema **2** and embed a schema-2 group-evidence record containing the canonical Shared proof.
They bind the existing source/group intent, lock bytes, compiler/configuration,
host, selected baseline, complete plan, input checkpoint and hashes of retained
stage logs/events/timings. Shared verifies ordered stage transitions, timings, original selection, before/after
checkpoints and regular logs. Excluded stages must remain not-applicable. Missing, failed, interrupted, duplicate, modified or unknown
records refuse before version preparation or publication.

Recovery reselects the same source/baseline plan and hashes every retained proof
and record. It preserves Shared's original successful admission; it does not invoke
current-input verification against release-projected manifests or rewrite proof.
Release-owned version projection remains the existing separate admission step;
it does not rewrite stage checkpoints or grant reuse. A new source or failed run
requires a fresh plan/gate. There is no stage-reuse claim in this receipt format.
Historical full-workspace receipts remain untouched and are not promoted into
schema-2 evidence; finish an older saved attempt with its original source/helper.

## Qualification

Consumer tests exercise all three selections, full-roster command equivalence,
required versus excluded failures, source/input drift, unknown/changed rosters,
missing and changed logs, interruption and fresh recovery. The actual Shared
runner is used with substituted Make commands; no fixture creates Git commits,
tags, pushes or publications. The native release-helper suite covers real consumer
receipt admission, metadata recovery and all groups' identity preservation.
The local qualification passed 84 native helper tests, consumer Make/release-plan
fixtures, strict helper Clippy, both real adapter test stages (42 unit tests and
five doctests), ShellCheck, formatting and policy checks. Evidence is retained in
`target/group-validation-adoption/`. These checks establish dispatch and admission;
they do not claim a full repository gate, live release or macOS run.

This adoption does not claim a measured production release speedup. Narrow plans
omit four of 12 stages, but unchanged applicable stages still execute and Cargo
retains its ordinary cache behavior. A tooling or dependency adoption itself
selects the full gate, including this implementation when it is released.

## Qualification lifecycle

[Storage #156](https://github.com/dragginzgame/ic-storage/issues/156) owns this
consumer integration. The focused native validation tests compare complete and
selected dispatch on the same fixture source, then invoke the actual
`Repository<Processes>` collector. All three groups cover required/excluded
failures, input/source drift, immutable successful proof recovery, changed logs
and missing proof. Earlier native preparation/publication recovery tests retain
their separate passing evidence; generic parser matrices remain with Shared.

Renew this consumer mapping when the ordered roster, selected stage recipes,
selector, dependency independence, group ownership or shared evidence contract
changes. Scope outside the reviewed private paths always selects the full gate;
new or uncertain exclusions need fresh focused qualification first. This is
mapping qualification, not whole-gate or Cargo execution reuse. Ordinary CI runs
one admitted plan, or the full roster; it never replays every group. Non-docs
pushes retain repository-wide qualification.
Required native/installed acceptance and separately requested release gates keep
their existing owners. The issue remains open until the implementation is
integrated into the default branch; local passing fixtures do not close it.

### Scoped compilation and ordinary CI qualification

The follow-up passes 87 native helper tests, including private-adapter PR
selection, full push/mixed/unknown fallback, missing history, exact tested source,
required versus excluded failures and mandatory narrow-CI upload evidence.
Actual Shared-runner → Make → Cargo command capture qualifies both adapter
selections. Full-gate command equivalence and lint-first failure propagation
remain covered; stale or absent runner checkpoints refuse package narrowing.
Both adapters' real locked/offline native lint, feature, rustdoc and Wasm test
compilation pass. Workflow selection, native failure-evidence bindings, ShellCheck,
Actionlint and snapshot verification pass. Evidence: `target/ci-group-selection/`.
These focused checks do not claim a full release gate or hosted native-host run.
Archive qualification remains workspace-wide; no measured total speedup is claimed.
