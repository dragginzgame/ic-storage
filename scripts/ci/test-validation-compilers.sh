#!/usr/bin/env bash
# Actual admission and archive entrypoint with compiler/Cargo substitutes.
set -euo pipefail
root="$(pwd -P)"
pin="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
# shellcheck source=scripts/ci/fixture-lifecycle.sh
source "$root/scripts/ci/fixture-lifecycle.sh"
mkdir -p "$root/target/fixtures"
fixture="$(fixture_create "$root/target/fixtures/storage-compilers.XXXXXX")"
fixture_complete=false
trap 'fixture_finish "$?" "$fixture" "Compiler admission fixture retained"' EXIT
mkdir -p "$fixture/bin" "$fixture/scripts/ci" "$fixture/qualification" "$fixture/crates/nested/member"
cp scripts/ci/check-validation-compilers.sh "$fixture/scripts/ci/"
printf '[toolchain]\nchannel = "%s"\n' "$pin" > "$fixture/rust-toolchain.toml"
for manifest in Cargo.toml qualification/Cargo.toml; do
    printf '[workspace.package]\nrust-version = "%s"\n' "$pin" > "$fixture/$manifest"
done
printf '[package]\nrust-version.workspace = true\n' > "$fixture/crates/nested/member/Cargo.toml"
cat > "$fixture/bin/rustup" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ $# -ge 4 && "$1" == run && "$2" == "$FIXTURE_PIN" ]] || exit 99
printf '%s\n' "$3" >> compiler-events
[[ "${MISSING_COMPILER:-none}" != "$2" ]] || exit 31
version="$2"
if [[ "${WRONG_IDENTITY:-none}" == "$3" ]]; then version=0.0.0; fi
case "$3:$4" in
    rustc:-Vv) printf 'rustc %s (fixture)\nrelease: %s\n' "$version" "$version" ;;
    cargo:metadata)
        printf '{"workspace_members":["member"],"packages":[{"id":"member","manifest_path":"crates/nested/member/Cargo.toml"}]}\n'
        ;;
    cargo:--version) printf 'cargo %s (fixture)\n' "$version" ;;
    *) exit 99 ;;
esac
STUB
cat > "$fixture/bin/cargo" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> cargo-events
exit 99
STUB
chmod +x "$fixture/bin/rustup" "$fixture/bin/cargo"
export PATH="$fixture/bin:$PATH" FIXTURE_PIN="$pin"
cd "$fixture"
if MISSING_COMPILER="$pin" bash "$root/scripts/ci/package-workspace.sh" "$pin" --allow-dirty > missing.log 2>&1; then
    echo 'Archive packaging admitted a missing compiler' >&2; exit 1
fi
[[ ! -e cargo-events && ! -e target ]] || exit 1
rg -F "Prepare required compiler first: rustup toolchain install --profile minimal $pin" missing.log
printf 'rustc\n' > expected
cmp expected compiler-events
rm compiler-events
bash scripts/ci/check-validation-compilers.sh "$pin" > success.log
[[ "$(cat success.log)" == "$pin" ]] || exit 1
printf 'rustc\ncargo\ncargo\ncargo\n' > expected
cmp expected compiler-events
for identity in rustc cargo; do
    if WRONG_IDENTITY="$identity" bash scripts/ci/check-validation-compilers.sh "$pin" > "$identity.log" 2>&1; then
        echo 'Incorrect actual compiler identity admitted' >&2; exit 1
    fi
done
rm compiler-events
for manifest in Cargo.toml qualification/Cargo.toml; do
    printf '[workspace.package]\nrust-version = "0.0.0"\n' > "$manifest"
    if bash scripts/ci/check-validation-compilers.sh "$pin" > mismatch.log 2>&1; then exit 1; fi
    [[ ! -e compiler-events && ! -e cargo-events ]] || exit 1
    printf '[workspace.package]\nrust-version = "%s"\n' "$pin" > "$manifest"
done
printf '[package]\nrust-version = "0.0.0"\n' > crates/nested/member/Cargo.toml
if bash scripts/ci/check-validation-compilers.sh "$pin" > member.log 2>&1; then exit 1; fi
[[ ! -e cargo-events ]] || exit 1
rm compiler-events
printf '[package]\nrust-version.workspace = true\n' > crates/nested/member/Cargo.toml
if bash scripts/ci/check-validation-compilers.sh 0.0.0 > selector.log 2>&1; then exit 1; fi
[[ ! -e compiler-events ]] || exit 1
fixture_complete=true
echo 'One compiler admission, exact identities and early floor/selector refusal passed without installs or builds.'
