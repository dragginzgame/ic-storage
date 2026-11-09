#!/usr/bin/env bash
# Exercise actual workflow scripts with substituted read-only Git observations.
set -euo pipefail
root="$(pwd -P)"
mkdir -p "$root/target/ci-selection"
fixture="$(mktemp -d "$root/target/ci-selection/attempt.XXXXXX")"
echo "Retained CI selection fixture: $fixture"
rg -F 'bash scripts/ci/validation-cargo.sh features $(VALIDATION_TOOLCHAIN)' Makefile > /dev/null
yq -o=json '.' .github/workflows/ci.yml > "$fixture/workflow.json"
jq -r '.jobs["change-scope"].steps[] | select(.id == "classify") | .run' "$fixture/workflow.json" > "$fixture/classify.sh"
jq -e '
  (.jobs["change-scope"].steps[0].with["fetch-depth"] == 0) and
  (.jobs["change-scope"].steps[1].env.CHANGE_HEAD == "${{ github.sha }}") and
  (.jobs["change-scope"].steps[1].env.CHANGE_REF | contains("github.base_ref")) and
  (.jobs["change-scope"].steps[1].env.CHANGE_BASE | contains("github.event.pull_request.base.sha")) and
  (.jobs.docs.needs == "change-scope") and
  (.jobs.docs.steps[-1].run == "make toko-tooling-check check-doc-links") and
  (.jobs["tooling-lint"].needs == null) and
  (.jobs["tooling-lint"].steps | any(.run == "make lint-tooling")) and
  (.on.pull_request | has("paths") or has("paths-ignore") | not) and
  (.on.push | has("paths") or has("paths-ignore") | not) and
  (.jobs.test.steps | any(.uses and .with["fetch-depth"] == 0)) and
  (.jobs.test.steps | any(.id == "evidence" and (.run | contains("make validate-ci"))
    and .env.CHANGE_HEAD == "${{ github.sha }}"
    and (.env.CHANGE_BASE | contains("github.event.pull_request.base.sha"))
    and .env.CHANGE_EVENT == "${{ github.event_name }}")) and
  (.jobs.test.steps | any(.name == "Retain CI selection and stage evidence"
    and .with.path == "target/ci-plans/" and (.if | contains("!cancelled()")))) and
  (.jobs.runtime.steps | any((.run // "") | contains("make test-runtime"))) and
  (.jobs | has("msrv") | not) and
  all(.jobs | to_entries[] | select(.key == "test" or .key == "runtime");
    .value.needs == ["change-scope","docs"] and
    .value.if == "${{ !cancelled() }}" and
    .value.steps[0].env.CHANGE_RESULT == "${{ needs.change-scope.result }}" and
    .value.steps[0].env.DOCS_RESULT == "${{ needs.docs.result }}" and
    .value.strategy.matrix.os == ["ubuntu-24.04","macos-15","macos-15-intel"] and
    .value["runs-on"] == "${{ matrix.os }}" and
    (.value.steps | length) > 3 and
    all(.value.steps[1:-1][];
      .if | startswith("${{ needs.change-scope.outputs.scope != '\''docs'\''")) and
    .value.steps[-1].if == "${{ needs.change-scope.outputs.scope == '\''docs'\'' }}")
' "$fixture/workflow.json" > /dev/null
# Required matrix checks must fail, not disappear as successful skips, when a
# prerequisite job fails. Execute each actual admission script for both outcomes.
for job in test runtime; do
    jq -r --arg job "$job" '.jobs[$job].steps[0].run' "$fixture/workflow.json" > "$fixture/$job-admission.sh"
    CHANGE_RESULT=success DOCS_RESULT=success bash -e -o pipefail "$fixture/$job-admission.sh"
    for outcome in failure skipped cancelled; do
        if CHANGE_RESULT="$outcome" DOCS_RESULT=skipped bash -e -o pipefail "$fixture/$job-admission.sh"; then
            echo 'required check admitted failed classification' >&2; exit 1
        fi
        if CHANGE_RESULT=success DOCS_RESULT="$outcome" bash -e -o pipefail "$fixture/$job-admission.sh"; then
            echo 'required check admitted failed docs' >&2; exit 1
        fi
    done
done
# Read-only observations replace Git effects; no fixture commits/tags are created.
mkdir "$fixture/bin"
cat > "$fixture/bin/git" <<'GIT'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  cat-file) [[ "$FIXTURE_MODE" != missing-history ]] ;;
  merge-base) [[ "$2" == --is-ancestor ]] || printf '%s\n' "$CHANGE_BASE" ;;
  ls-tree) printf '100644 blob %s\tci/docs-only-paths.txt\0' "$CHANGE_BASE" ;;
  show)
    cat "$POLICY_SOURCE"
    if [[ "$FIXTURE_MODE" == policy-change && "$2" == "$CHANGE_HEAD:"* ]]; then printf 'another.md\n'; fi
    ;;
  diff)
    if [[ "$2" == --check ]]; then [[ "$FIXTURE_MODE" != whitespace-failure ]]; exit; fi
    paths=(README.md)
    mode=100644
    case "$FIXTURE_MODE" in
      code) paths=(crates/ic-storage/src/lib.rs) ;;
      mixed) paths+=(crates/ic-storage/src/lib.rs) ;;
      policy-path) paths=(ci/docs-only-paths.txt) ;;
      workflow) paths=(.github/workflows/ci.yml) ;;
      packaged) paths=(crates/ic-storage/README.md) ;;
      unknown) paths=(docs/unreviewed.md) ;;
      executable) mode=100755 ;;
      symlink) mode=120000 ;;
    esac
    for path in "${paths[@]}"; do
      printf ':100644 %s %s %s M\0%s\0' "$mode" "$CHANGE_BASE" "$CHANGE_HEAD" "$path"
    done
    ;;
  *) exit 97 ;;
esac
GIT
cat > "$fixture/bin/perl" <<'PERL'
#!/usr/bin/env bash
set -euo pipefail
case "$FIXTURE_MODE" in
  classifier-failure) exit 23 ;;
  invalid-output) echo unknown; exit 0 ;;
esac
exec "$REAL_PERL" "$@"
PERL
chmod +x "$fixture/bin/git" "$fixture/bin/perl"
REAL_PERL="$(command -v perl)"
POLICY_SOURCE="$root/ci/docs-only-paths.txt"
export REAL_PERL POLICY_SOURCE
export CHANGE_BASE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa CHANGE_HEAD=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
export CHANGE_EVENT=push CHANGE_REF=refs/heads/main
for scenario in docs code mixed policy-change policy-path workflow packaged unknown missing-history classifier-failure invalid-output executable symlink tag pull-request; do
    expected=full
    export CHANGE_EVENT=push CHANGE_REF=refs/heads/main
    case "$scenario" in
      docs) expected=docs ;;
      pull-request) CHANGE_EVENT=pull_request; expected=docs ;;
      tag) CHANGE_REF=refs/tags/v1.0.0 ;;
    esac
    export FIXTURE_MODE="$scenario" GITHUB_OUTPUT="$fixture/$scenario.outputs"
    PATH="$fixture/bin:$PATH" bash -e -o pipefail "$fixture/classify.sh" > "$fixture/$scenario.log" 2>&1
    [[ "$(cat "$GITHUB_OUTPUT")" == "scope=$expected" ]] || exit 1
    printf '%s: %s\n' "$scenario" "$expected"
done
export FIXTURE_MODE=whitespace-failure GITHUB_OUTPUT="$fixture/whitespace.outputs"
if PATH="$fixture/bin:$PATH" bash -e -o pipefail "$fixture/classify.sh" > "$fixture/whitespace.log" 2>&1; then
    echo 'whitespace failure reported success' >&2; exit 1
fi
# Execute the real docs job command, not a simulated native pass.
jq -r '.jobs.docs.steps[-1].run' "$fixture/workflow.json" > "$fixture/docs.sh"
bash -e -o pipefail "$fixture/docs.sh" > "$fixture/docs.log" 2>&1
echo 'Actual workflow selection, retained native/runtime matrices, fail-closed cases and documentation checks passed.'
