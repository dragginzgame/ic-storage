#!/usr/bin/env bash
set -euo pipefail

# Consumer policy only: the shared helper owns archive creation and path safety.
[[ $# == 2 || $# == 3 ]] || { echo 'usage: collect-failure-evidence.sh TEMP-ROOT REPOSITORY-ROOT [full|compact]' >&2; exit 2; }
mode="${3:-full}"
[[ "$mode" == full || "$mode" == compact ]] || { echo 'invalid tool evidence selection' >&2; exit 2; }
# Anchor the script path before cd so inherited CDPATH cannot enter ROOT.
# The sentinel keeps command substitution from trimming pathname newlines.
ROOT="$0"
[[ "$ROOT" == /* ]] || ROOT="$PWD/$ROOT"
ROOT="$(cd -P "${ROOT%/*}/../.." && printf '%s/.' "$PWD")"
ROOT="${ROOT%/.}"
temp_root="$1"
repository_root="$2"
# Preserve operand bytes; physical resolution and path admission belong to the
# shared archiver. Anchoring avoids interpreting relative roots as cd options.
[[ "$temp_root" == /* ]] || temp_root="$PWD/$temp_root"
[[ "$repository_root" == /* ]] || repository_root="$PWD/$repository_root"
[[ -d "$temp_root" && -d "$repository_root" ]] || { echo 'evidence roots must be existing directories' >&2; exit 1; }
inputs=()
complete=true
shopt -s nullglob
for path in "$temp_root/ic-storage-fixtures" "$temp_root/tools-setup.log" "$temp_root/tools-check.log" \
    "$temp_root/dependencies.log" "$temp_root/validation.log" \
    "$temp_root/runtime-host-setup.log" "$temp_root/runtime-host-check.log" \
    "$temp_root/runtime-ic-setup.log" "$temp_root/runtime-ic-check.log" \
    "$temp_root/runtime-setup.log" "$temp_root/runtime.log" "$temp_root"/formatting.*; do
    if [[ -e "$path" || -L "$path" ]]; then
        inputs+=("$temp_root" "${path#"$temp_root/"}")
    fi
done
bundles=()
select_tools=false
diagnostics=""
for local_root in target .tools; do
    if [[ -L "$repository_root/$local_root" ]]; then
        # Retain the final link, never its children. Their rejection must not
        # discard independent logs or evidence from the other physical root.
        inputs+=("$repository_root" "$local_root")
        complete=false
        [[ -n "$diagnostics" ]] || diagnostics="$(mktemp -d "$temp_root/tool-evidence.XXXXXX")"
        printf 'Evidence incomplete: %s is a symlink; retaining the link without following its target.\n' \
            "$local_root" >> "$diagnostics/local-selection.log"
    else
        case "$local_root" in
            target) paths=(target/qualification target/release-validation target/package-qualification) ;;
            .tools)
                paths=(.tools/ic-testkit-server)
                bundles=("$repository_root"/.tools/host-set.* "$repository_root"/.tools/ic-set.*)
                if [[ ${#bundles[@]} != 0 || -d "$repository_root/.tools/rust" ]]; then
                    select_tools=true
                fi
                if [[ -L "$repository_root/.tools/rust" || ( -e "$repository_root/.tools/rust" && ! -d "$repository_root/.tools/rust" ) ]]; then
                    # The shared selector owns physical Rust trees. Retain an
                    # inadmissible leaf locally without traversing it.
                    paths+=(.tools/rust)
                    complete=false
                    [[ -n "$diagnostics" ]] || diagnostics="$(mktemp -d "$temp_root/tool-evidence.XXXXXX")"
                    printf 'Evidence incomplete: .tools/rust is not a physical directory; retaining the leaf without following it.\n' >> "$diagnostics/local-selection.log"
                fi
                ;;
        esac
        for path in "${paths[@]}"; do
            if [[ -e "$repository_root/$path" || -L "$repository_root/$path" ]]; then
                inputs+=("$repository_root" "$path")
            fi
        done
    fi
done
if [[ -n "$diagnostics" ]]; then
    cat "$diagnostics/local-selection.log" >&2
fi
if [[ -n "$diagnostics" ]]; then
    inputs+=("$temp_root" "${diagnostics#"$temp_root/"}")
fi
if [[ "$select_tools" == true ]]; then
    # Shared owns selector admission, failed-output isolation and tool fallback.
    # Keep its receipts separate from Memory's local path-admission diagnostics.
    diagnostics="$(mktemp -d "$temp_root/tool-evidence.XXXXXX")"
    archive_dir="$(mktemp -d "$temp_root/ic-storage-evidence.XXXXXX")"
    collection_status=0
    ic_selection=()
    [[ "$mode" != compact ]] || ic_selection=(wasm-opt)
    bash "$ROOT/vendor/toko-tooling/scripts/ci/collect-tool-evidence.sh" "$archive_dir/evidence.tar.gz" \
        "$repository_root" "$diagnostics" "$mode" "$ROOT/vendor/toko-tooling/ci/tool-versions.env" \
        "$ROOT/vendor/toko-tooling/ci/ic-tools.tsv" ${ic_selection[@]+"${ic_selection[@]}"} \
        ${inputs[@]+"${inputs[@]}"} || collection_status=$?
    case "$collection_status" in
        0) ;;
        1) complete=false ;;
        *) exit "$collection_status" ;;
    esac
elif [[ ${#inputs[@]} != 0 ]]; then
    archive_dir="$(mktemp -d "$temp_root/ic-storage-evidence.XXXXXX")"
    bash "$ROOT/vendor/toko-tooling/scripts/ci/archive-evidence.sh" "$archive_dir/evidence.tar.gz" "${inputs[@]}"
fi
# Publish completeness only after the selected evidence has been archived.
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'collection-complete=%s\n' "$complete" >> "$GITHUB_OUTPUT"
fi
