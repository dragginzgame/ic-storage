#!/usr/bin/env bash
# The real native adapter and Shared must emit identical exact file plans.
# Only Git source observation is substituted; no release effects are dispatched.
set -euo pipefail
pin="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
cargo "+$pin" build -p ic-storage --example repo-tool --locked --offline
metadata="$(cargo "+$pin" metadata --locked --offline --no-deps --format-version 1)"
target="$(printf '%s' "$metadata" | jq -er .target_directory)"
mkdir -p "$target/fixtures"
fixture="$(mktemp -d "$target/fixtures/ic-storage-group-plans.XXXXXX")"
echo "Retained native group plan fixture: $fixture"
mkdir "$fixture/bin"
cat > "$fixture/bin/git" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ $# == 2 && "$1" == show && "$2" == aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:* ]] || exit 1
path="${2#*:}"
[[ "$path" != /* && "$path" != *..* ]] || exit 1
cat -- "$path"
STUB
chmod +x "$fixture/bin/git"
shasum -a 256 Cargo.toml Cargo.lock ci/cargo-release-groups.json crates/*/Cargo.toml \
    CHANGELOG.md releases/history.md releases/*/*.md > "$fixture/source.sha256"
while IFS= read -r group; do
    previous="$(RELEASE_GROUP="$group" "$target/debug/examples/repo-tool" version)"
    ledger="$(jq -er --arg group "$group" '.groups[] | select(.name == $group) | .changelog' ci/cargo-release-groups.json)"
    version="$(awk '/^## \[[0-9]+\.[0-9]+\.[0-9]+\]($| - Unreleased$)/ {sub(/^## \[/, ""); sub(/\].*$/, ""); print; exit}' "$ledger")"
    if [[ -z "$version" ]]; then version="$(bash vendor/toko-tooling/scripts/ci/next-release-version.sh "$previous" patch)"; fi
    kind=""
    for candidate_kind in patch minor major; do
        if [[ "$(bash vendor/toko-tooling/scripts/ci/next-release-version.sh "$previous" "$candidate_kind")" == "$version" ]]; then kind="$candidate_kind"; break; fi
    done
    [[ -n "$kind" ]]
    PATH="$fixture/bin:$PATH" RELEASE_GROUP="$group" RELEASE_PREVIOUS="$previous" \
        RELEASE_VERSION="$version" RELEASE_KIND="$kind" RELEASE_DATE=2026-10-31 \
        RELEASE_SOURCE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa RELEASE_REMOTE=origin RELEASE_BRANCH=main \
        "$target/debug/examples/repo-tool" release-files > "$fixture/$group.native.files"
    RUSTUP_TOOLCHAIN="$pin" bash vendor/toko-tooling/scripts/ci/cargo-release-group.sh files "$group" > "$fixture/$group.shared.files"
    cmp "$fixture/$group.native.files" "$fixture/$group.shared.files"
done < <(jq -r '.groups[].name' ci/cargo-release-groups.json)
shasum -a 256 -c "$fixture/source.sha256"
echo 'All three native file plans match Shared exactly; source inputs remain unchanged.'
