#!/usr/bin/env bash
# Exercise the real archive admission before any compilation, with bad observations.
set -euo pipefail
package_names="$(bash scripts/ci/read-storage-packages.sh --all)"
IFS=$'\t' read -r -a packages <<< "$package_names"
source vendor/toko-tooling/scripts/ci/fixture-lifecycle.sh
mkdir -p "$PWD/target/fixtures"
fixture="$(fixture_create "$PWD/target/fixtures/ic-storage-package-admission.XXXXXX")"
trap 'status=$?; if [[ "$status" != 0 ]]; then
    fixture_replay_logs "$fixture/package.log" "$fixture/drift.log" "$fixture/msrv.log"
fi; exit "$status"' EXIT
echo "Retained archive-admission fixture: $fixture"
mkdir -p "$fixture/bin"
REAL_CARGO="$(command -v cargo)"
REAL_YQ="$(command -v yq)"
REAL_RUSTUP="$(command -v rustup)"
export REAL_CARGO REAL_YQ REAL_RUSTUP
pin="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
# Packaging is setup, not the behavior under fault injection. Prepare it once;
# One rejection still runs the real extraction and pre-compilation admission path.
"$REAL_CARGO" "+$pin" package --workspace --registry crates-io --locked --offline --no-verify --allow-dirty > "$fixture/package.log" 2>&1
PACKAGE_SELECTED_COMMAND=""
if [[ -n "${RELEASE_GROUP:-}" ]]; then
    selected_names="$(bash scripts/ci/read-storage-packages.sh)"
    IFS=$'\t' read -r -a selected_packages <<< "$selected_names"
    selection=()
    for package in "${selected_packages[@]}"; do selection+=(--package "$package"); done
    "$REAL_CARGO" "+$pin" package "${selection[@]}" --registry crates-io --locked --offline --no-verify --allow-dirty >> "$fixture/package.log" 2>&1
    PACKAGE_SELECTED_COMMAND="+$pin package ${selection[*]} --registry crates-io --locked --offline --no-verify --allow-dirty"
fi
export PACKAGE_SELECTED_COMMAND
metadata="$("$REAL_CARGO" "+$pin" metadata --no-deps --locked --offline --format-version 1)"
target="$(printf '%s' "$metadata" | jq -er '.target_directory')"
"$REAL_CARGO" "+$pin" build --locked --offline --example repo-tool >> "$fixture/package.log" 2>&1
export QUALIFIED_REPO_TOOL="$target/debug/examples/repo-tool"
PREPARED_ARCHIVES="$fixture/archives.sha256"
PACKAGE_TOOLCHAIN="$pin"
export PREPARED_ARCHIVES PACKAGE_TOOLCHAIN
for package in "${packages[@]}"; do
    version="$(printf '%s' "$metadata" | jq -er --arg name "$package" '.packages[] | select(.name == $name) | .version')"
    shasum -a 256 "$target/package/$package-$version.crate" >> "$PREPARED_ARCHIVES"
done
cat > "$fixture/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" == *' --example repo-tool -- qualify-archives '* ]]; then
    while [[ "$1" != -- ]]; do shift; done
    shift
    exec "$QUALIFIED_REPO_TOOL" "$@"
fi
if [[ "$REJECT_MODE" == drift && "$*" == *' metadata '* && "$*" == *'--manifest-path'* ]]; then
    "$REAL_CARGO" "$@" | jq '(.packages[] | select(.name == "ic-storage") | .manifest_path) = "/foreign/Cargo.toml"'
    exit
fi
if [[ "$*" == *' package '* ]]; then
    [[ "$*" == "+$PACKAGE_TOOLCHAIN package --workspace --registry crates-io --locked --offline --no-verify --allow-dirty" || ( -n "$PACKAGE_SELECTED_COMMAND" && "$*" == "$PACKAGE_SELECTED_COMMAND" ) ]] || exit 98
    shasum -a 256 -c "$PREPARED_ARCHIVES" > /dev/null
    exit 0
fi
if [[ "$*" == *' check '* || "$*" == *' run '* ]]; then
    if [[ "$REJECT_MODE" == selected-msrv ]]; then
        printf '%s\n' "$*" >> "$MSRV_EVENTS"
        if [[ "$*" == *' run '* ]]; then
            previous=""
            manifest=""
            for argument in "$@"; do
                if [[ "$previous" == --manifest-path ]]; then manifest="$argument"; break; fi
                previous="$argument"
            done
            [[ "$("$REAL_YQ" -p toml -o yaml -r '.package.rust-version' "$manifest")" == "$FIXTURE_MSRV" ]] || exit 97
        fi
        exit 0
    fi
    echo 'unexpected compilation after invalid admission' >&2
    exit 99
fi
exec "$REAL_CARGO" "$@"
STUB
cat > "$fixture/bin/rustup" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$REJECT_MODE" == selected-msrv ]]; then
    # Member discovery is real read-only Cargo metadata, after the substituted
    # compiler identities. Keep compilation effects below separately captured.
    case "$*" in
        "run $FIXTURE_MSRV cargo metadata --no-deps --locked --offline --format-version 1 --manifest-path Cargo.toml"|\
        "run $FIXTURE_MSRV cargo metadata --no-deps --locked --offline --format-version 1 --manifest-path qualification/Cargo.toml")
            exec "$REAL_RUSTUP" "$@" ;;
    esac
    [[ $# == 4 && "$1" == run && "$2" == "$FIXTURE_MSRV" ]] || exit 99
    printf '%s\n' "$3" >> "$MSRV_ADMISSION_EVENTS"
    case "$3:$4" in
        rustc:-Vv) printf 'rustc %s (fixture)\nrelease: %s\n' "$2" "$2" ;;
        cargo:--version) printf 'cargo %s (fixture)\n' "$2" ;;
        *) exit 99 ;;
    esac
else
    exec "$REAL_RUSTUP" "$@"
fi
STUB
chmod +x "$fixture/bin/cargo" "$fixture/bin/rustup"
# One real orchestration probe proves rejection occurs before compilation.
if REJECT_MODE=drift PATH="$fixture/bin:$PATH" bash scripts/ci/package-workspace.sh "$pin" --allow-dirty > "$fixture/drift.log" 2>&1; then
    echo "Incorrectly admitted drift; see $fixture/drift.log" >&2; exit 1
fi
if rg -q 'unexpected compilation' "$fixture/drift.log"; then
    cat "$fixture/drift.log" >&2; exit 1
fi
rg -q 'resolved package is not the exact extracted payload' "$fixture/drift.log"
echo 'Build admission refuses a substituted payload before consumer compilation'
# Keep the actual compiler/support-floor policy. Real archive extraction and
# graph admission run; compiler checks/build/run effects are substitutes.
export FIXTURE_MSRV="$pin" MSRV_EVENTS="$fixture/msrv-events" MSRV_ADMISSION_EVENTS="$fixture/msrv-admission"
REJECT_MODE=selected-msrv PATH="$fixture/bin:$PATH" bash scripts/ci/package-workspace.sh "$pin" --allow-dirty > "$fixture/msrv.log" 2>&1
printf 'rustc\ncargo\n' > "$fixture/expected-admission"
cmp "$fixture/expected-admission" "$MSRV_ADMISSION_EVENTS"
{
    for package in "${packages[@]}"; do printf '+%s\n' "$pin"; done
    printf '+%s\n' "$pin" "$pin"
} > "$fixture/expected-compilers"
cut -d ' ' -f 1 "$MSRV_EVENTS" > "$fixture/observed-compilers"
cmp "$fixture/expected-compilers" "$fixture/observed-compilers"
echo 'Archive fixture metadata and single native dispatch follow the admitted compiler (substitutes only).'
shasum -a 256 -c "$PREPARED_ARCHIVES"
