#!/usr/bin/env bash
# Rustdoc checks all private library surfaces; only enumerated existing debt survives.
set -euo pipefail
[[ $# == 1 && -n "$1" ]] || { echo 'usage: check-qualification-docs.sh TOOLCHAIN' >&2; exit 2; }
export RUSTUP_TOOLCHAIN="$1" RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
mkdir -p target/qualification-docs
evidence="$(mktemp -d "$PWD/target/qualification-docs/attempt.XXXXXX")"
printf 'Qualification documentation evidence: %s\n' "$evidence"
host="$(rustc -vV | sed -n 's/^host: //p')"
[[ "$host" =~ ^[A-Za-z0-9_.-]+$ ]] || exit 1
for target in "$host" wasm32-unknown-unknown; do
    RUSTDOCFLAGS='-D warnings --force-warn missing-docs' cargo doc \
        --manifest-path qualification/Cargo.toml --locked --offline --workspace \
        --lib --no-deps --features ic-storage-benchmarks/owned --target "$target" \
        --message-format=json > "$evidence/$target.jsonl"
    perl scripts/ci/check-qualification-docs.pl ci/qualification-docs-debt.json \
        "$evidence/$target.jsonl"
done
