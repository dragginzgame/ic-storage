# Conservative CI change selection

The [workflow](../.github/workflows/ci.yml) uses Shared’s reviewed
[classifier contract](ci-change-selection.md). Its executable follows the reviewed
[snapshot](../.toko-tooling.snapshot), currently Shared 0.29.4 (`628ccc7`).
The optional `--allow-version-bump` mode is disabled: Storage uses Cargo release
groups, not Shared Tooling's root VERSION convention. Cargo metadata changes
continue to require full CI.

[The exact allowlist](../ci/docs-only-paths.txt) contains only the root README,
documentation navigation and current handoff. They have no companion declarations,
are not embedded Rustdoc or generated/package inputs, and are not maintained
machine configuration. Member READMEs are embedded with `include_str!` and remain
outside the allowlist. New prose paths require explicit review; unknown paths,
code, workflow, policy and executable/symlink changes select full checks.

The classification checkout fetches complete history. Pull requests compare the
exact tested merge commit against their base using the merge base; pushes use
before/after identities and require ancestry. Policies must match in both
commits. Missing history, malformed observations, tags and classifier errors
select full qualification. Required matrix jobs explicitly fail admission when a prerequisite fails;
failed prose whitespace or documentation checks never become a successful result.
The first adoption run is full because its policy is new.

The docs job always checks snapshot integrity and local documentation links.
Tooling lint remains unconditional. All existing Linux, macOS ARM and macOS Intel
matrix check names remain present. For admitted prose, those jobs report a
separate documentation-only result without installing Rust or running native
suites. For code changes, native setup, runtime qualification, compiler admission, upload
and failure retention remain behind the non-docs guard. Native validation uses
the qualified group selection below.
No workflow-level path filter is used. Release commands still run their complete
configured gates, independently of this optimization.

`make test-ci-selection` executes the actual workflow classification script with
read-only Git observations substituted, then runs the real docs job command.
It covers docs/code/mixed/policy/workflow/packaged/unknown paths, missing history,
classifier errors, invalid output, executable/symlink changes, tags, PRs and
whitespace failure, plus failed/skipped/cancelled prerequisite admission. It checks required matrix names and the actual CI entrypoint.
Fixtures create no commits or tags. The test participates in `make test-tools`.
Linux fixture passes are not hosted native/macOS qualification and no timing or
cost reduction is claimed from these checks.

## Private adapter PRs

The native jobs fetch complete history and call `make validate-ci` with the exact
GitHub event, base SHA, tested SHA and base ref. The native `repo-tool` admits a
clean checkout matching that tested SHA. For pull requests it compares the merge
base to the tested merge commit using a complete raw, NUL-delimited diff with
rename detection disabled. Both rename paths must therefore fit the same owner.
Missing/ambiguous history, executable/symlink changes, public entrypoints,
manifest/lock/tooling changes, mixed owners and unknown paths retain the full gate.
A mismatched checkout identity refuses rather than qualifying the wrong source.

The selector reuses the release selector's closed private-path ownership and
locked reverse-dependency check (including additional local consumers); there is no second path allowlist. Only a single
independent Metrics Storage or Logging Storage change qualifies. Handoff-only
changes do not select an arbitrary adapter. Non-docs pushes to main retain the
whole-repository gate, as do `make validate` and `make validate-toolchain`.

Narrow PRs run the same eight-stage group plan as private adapter releases through
Shared's existing runner, with immutable inputs, exclusion reasons, fail-fast
execution and per-stage timings. Four compilation stages select the adapter
package; required dependencies still compile. Formatting, graph/pin/boundary
policy and whole-workspace archive integration remain required. Core changes
qualify both adapters. The installed-runtime matrix and all three native hosts
remain unchanged; this selector does not waive their obligations.

The native failure-evidence upload fixture remains required: full gates run it
within tooling tests, and narrow CI invokes that specific fixture afterwards.
Selection context and group runner logs live under `target/ci-plans/` and are
uploaded on success or failure when present. These are CI evidence, not release
receipts or permission to publish. No tag, version preparation, commit or other
release effect is invoked. An explicitly full gate refuses an inherited group plan.

Qualification uses read-only Git/process substitutes for CI event/dispatch tests,
the actual Shared runner and recursive Make recipes with captured Cargo commands,
required-failure and stale-plan checks, plus native execution of both adapters'
selected lint/features/docs/Wasm commands. The existing helper recovery tests
remain. Linux local qualification does not claim hosted/macOS acceptance or an
end-to-end time saving. This tooling change itself selects full CI on adoption.
