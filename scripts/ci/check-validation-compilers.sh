#!/usr/bin/env bash
# Offline admission only: setup owns compiler installation.
set -euo pipefail
[[ $# == 1 && "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || exit 2
export RUSTUP_AUTO_INSTALL=0
pin="$(yq -p toml -o yaml -r '.toolchain.channel' rust-toolchain.toml)"
[[ "$pin" == "$1" ]] || { echo 'validation compiler differs from the repository pin' >&2; exit 1; }
for manifest in Cargo.toml qualification/Cargo.toml; do
    floor="$(yq -p toml -o yaml -r '.workspace.package.rust-version' "$manifest")"
    [[ "$floor" == "$pin" ]] || { echo "support floor differs from pinned compiler: $manifest" >&2; exit 1; }
done
rustc="$(rustup run "$pin" rustc -Vv)" || {
    echo "Prepare required compiler first: rustup toolchain install --profile minimal $pin" >&2
    exit 1
}
cargo="$(rustup run "$pin" cargo --version)"
[[ "$rustc" == "rustc $pin "* && "$cargo" == "cargo $pin "* ]] || {
    echo 'observed Cargo/rustc identities differ from the repository pin' >&2; exit 1;
}
# Cargo owns member discovery, including grouped product/qualification packages.
for catalog in Cargo.toml qualification/Cargo.toml; do
    metadata="$(rustup run "$pin" cargo metadata --no-deps --locked --offline --format-version 1 --manifest-path "$catalog")"
    manifests="$(printf '%s' "$metadata" | jq -er '.workspace_members as $members | [.packages[] | select(.id as $id | $members | index($id)) | .manifest_path] | .[]')"
    while IFS= read -r manifest; do
        yq -p toml -o json '.package."rust-version"' "$manifest" |
            jq -e --arg pin "$pin" '. == {workspace:true} or . == $pin' > /dev/null || {
                echo "package support floor differs from pinned compiler: $manifest" >&2; exit 1;
            }
    done <<< "$manifests"
done
printf '%s\n%s\n' "$rustc" "$cargo" >&2
# Successful stdout carries the admitted catalog floor; diagnostics stay visible.
printf '%s\n' "$pin"
