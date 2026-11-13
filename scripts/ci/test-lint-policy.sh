#!/usr/bin/env bash
# Compiler-only fixtures prove the inherited policy, without executing unsafe code.
set -euo pipefail
[[ $# == 1 && -n "$1" ]] || { echo 'usage: test-lint-policy.sh TOOLCHAIN' >&2; exit 2; }
export RUSTUP_TOOLCHAIN="$1" RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
mkdir -p target/fixtures
fixture="$(mktemp -d "$PWD/target/fixtures/lint-policy.XXXXXX")"
echo "Retained lint policy fixtures: $fixture"
for catalog in Cargo.toml qualification/Cargo.toml; do
    root="$fixture/${catalog%Cargo.toml}workspace"
    mkdir -p "$root/member/src"
    cargo metadata --manifest-path "$catalog" --no-deps --locked --offline --format-version 1 |
        jq -r '.workspace_members as $members | .packages[] | select(.id as $id | $members | index($id)) | .manifest_path' > "$root/members"
    while IFS= read -r member; do
        yq -p toml -o json '.lints' "$member" | jq -e '. == {workspace:true}' > /dev/null
    done < "$root/members"
    printf '[workspace]\nmembers = ["member"]\nresolver = "3"\n' > "$root/Cargo.toml"
    perl -ne 'if (/^\[/) { $copy = /^\[workspace\.lints\./ } print if $copy' "$catalog" >> "$root/Cargo.toml"
    cat > "$root/member/Cargo.toml" <<'TOML'
[package]
name = "lint-policy-fixture"
version = "0.0.0"
edition = "2024"
publish = false
[lints]
workspace = true
TOML
    printf '//! Safe compiler control.\n' > "$root/member/src/lib.rs"
    cargo generate-lockfile --manifest-path "$root/Cargo.toml" --offline
    cargo check --manifest-path "$root/Cargo.toml" --locked --offline > "$root/safe.log" 2>&1
    for violation in unsafe_code unused_must_use dead_code unfulfilled_lint_expectations; do
        printf '//! Negative compiler fixture.\n' > "$root/member/src/lib.rs"
        case "$violation" in
            unsafe_code) printf '/// Export-only unsafe construct.\n#[unsafe(no_mangle)]\npub extern "C" fn forbidden_export() {}\n' ;;
            unused_must_use) printf '/// Deliberately ignore a fallible operation.\npub fn ignored_result() { std::fs::read("not-executed"); }\n' ;;
            dead_code) printf 'fn unused_private_function() {}\n' ;;
            unfulfilled_lint_expectations) printf '#[expect(dead_code, reason = "deliberately stale fixture")]\n/// Public and therefore not dead.\npub fn reachable() {}\n' ;;
        esac >> "$root/member/src/lib.rs"
        if cargo check --manifest-path "$root/Cargo.toml" --locked --offline --message-format=json \
            > "$root/$violation.jsonl" 2> "$root/$violation.log"; then
            echo "inherited $violation policy accepted the forbidden construct" >&2; exit 1
        fi
        jq -se --arg code "$violation" \
            'any(.[]; .reason == "compiler-message" and .message.level == "error" and .message.code.code == $code)' \
            "$root/$violation.jsonl" > /dev/null
    done
done
# The debt gate must reject additions and incomplete/failed compiler output.
printf '%s\n' '{"schema":1,"entries":[]}' > "$fixture/empty.json"
printf '%s\n' '{"reason":"build-finished","success":true}' > "$fixture/clean.jsonl"
perl scripts/ci/check-qualification-docs.pl "$fixture/empty.json" "$fixture/clean.jsonl"
for invalid in new failed incomplete malformed; do
    case "$invalid" in
        new) printf '%s\n' '{"reason":"compiler-message","message":{"level":"warning","code":{"code":"missing_docs"},"message":"missing documentation for a function","spans":[{"is_primary":true,"file_name":"new.rs","text":[{"text":"pub fn undocumented() {}"}]}]}}' ;;
        failed) printf '%s\n' '{"reason":"build-finished","success":false}' ;;
        incomplete) printf '%s\n' '{"reason":"compiler-artifact"}' ;;
        malformed) printf '%s\n' 'not JSON' ;;
    esac > "$fixture/$invalid.jsonl"
    if perl scripts/ci/check-qualification-docs.pl "$fixture/empty.json" "$fixture/$invalid.jsonl" \
        > "$fixture/$invalid.log" 2>&1; then
        echo "documentation debt gate accepted $invalid evidence" >&2; exit 1
    fi
done
echo 'Both inherited lint policies and documentation debt refusal pass.'
