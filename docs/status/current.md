# Current handoff — 2026-11-14

## Published baseline

Storage 0.74.1, Metrics Storage 0.78.0 and Logging Storage 0.74.0 are published
on crates.io and tagged on GitHub. Storage patch batch `batch-MUlACA2d` delivered
`f001cf4`; adapter minor batch `batch-YUvWyvVJ` delivered Metrics at `a5bbe25`
and Logging at `b5358b8`. Both batches are complete and reconciled into main.
All three 12-stage release gates passed. Downloaded registry archives match the
qualified checksums, and remote annotated tags point to the release commits.
Evidence is under `target/evidence/deploy-20261114/`; final group receipts and
stage logs remain in each batch's `target/release-frozen/` checkout.

[The pre-release handoff](https://github.com/dragginzgame/ic-storage/blob/1118f7af530e5f60323cb455bb7fca6f7710ef38/docs/status/current.md)
retains the earlier adoption narrative and evidence. The failed `batch-T5JZrPdp`
is preserved under `target/release-frozen/` and `.git/release-state/frozen/`.
It failed before version preparation or delivery; do not resume that old source
or reinterpret its failed gate. The fresh batches passed the unchanged CDPATH
hook suite and all 90 native release-helper tests for each group.

## Current tooling adoption

Both workspace catalogs and locks now select authenticated Toko 0.43.0.
The LOC interface is now `toko-cloc`; use `--manifest Cargo.toml` or
`--manifest qualification/Cargo.toml` to select one workspace. The old Make
target and settings are retired. The existing PATH launcher matches the
published bytes. Baseline, release runner, hooks and product gates are unchanged.
See [the adoption record](../adoption/storage-tooling.md#toko-0430-adoption) and
`target/evidence/toko-0430-adoption-20261114/` for focused validation, including
the ten/three/seven-member LOC selections. All release gates now pass as well.
Preserve the earlier failing fixture `target/fixtures/ic-storage-hooks.oU8S2d`.

## Delivered product and supplier changes

The releases include native IO scope-guard simplification, tooling/lint adoption,
release catalog projection/recovery fixes, Build 0.84 and Host 0.77 integration.
The freeze reviewed published Build 0.84.1, Host 0.77.2, Metrics 0.30.5,
Logging 0.25.5, Wire 0.22.1, Platform 0.32.2 and Runtime 0.22.2 selections.
The separately qualified Build metadata CLI remains 0.84.0; its optional patch
refresh is deferred to tool maintenance. Core production remains Wire-free.

Both adapter minor releases replace the public Wire error payload/conversion
with unit `StoreError::Codec`. Internal bounded Wire checkpoints and persisted
bytes remain unchanged; this API change requires no persisted-state reset.
Detailed notes and earlier focused evidence remain in the group ledgers and
`target/evidence/adapter-codec-errors-20261113/`.

Qualification and managed CLI select Testkit 0.104.0 with PocketIC 16.1.0.
Both native consumers pass all-target Clippy and 50 unit tests; installed Linux
IO, corruption, custody, collections, Logging, upgrades and rollback pass for
original/O3/Os/Oz at `target/qualification/runtime/attempt.r8NYr6`.
See `target/evidence/testkit-0104-adoption-20261114/`. No fresh macOS installed
qualification is claimed. Hosted CI on the delivered commits is separate from
the completed local gates; retain #159's host acceptance until verified.

## Performance work and consumer ownership

[Storage #118](https://github.com/dragginzgame/ic-storage/issues/118) was closed
after publication. [The discard study](../measurements/storage-discard-mutations-2026-11-12.md)
and [histogram study](../measurements/metrics-histogram-comparisons-2026-11-12.md)
own shipped instruction/IO/size results. IcyDB #635, Blob #129, Relay #24,
Auth #132 and Canic #764 retain their consumer work; sibling source stays read-only.
[Storage #159](https://github.com/dragginzgame/ic-storage/issues/159) retains its
integration/CI acceptance; Rust 1.100 is the selected floor and compiler.

[Storage #158](https://github.com/dragginzgame/ic-storage/issues/158) owns routing
locality and blob-density research. [Read-path attribution](../measurements/storage-identity-lookups-2026-11-12.md#access-path-attribution--2026-11-13)
explains the coarse trial's extra 80k event; all 36 captured read-only traces
match the pinned IC model. Whole-handler and resize tradeoffs do not justify
adopting that layout. Next assess checked routing/key read combination for
contiguous allocations while preserving bounds and value-free traversal.
Production is unchanged; no new format, reset or Auth endpoint win is qualified.
Auth's installed ceiling is 1.3M; <1M remains an optimization goal.

[The scalar study](../measurements/storage-scalar-scans-2026-11-11.md) and identity
study retain shipped results and rejected trials. Do not restore rejected root
cache, combined-read, compact-frame, decoder-outlining or prefix-buffer trials.
Measure first/second/third reads, mutations and application Wasm together.

## Safety and preservation

Independent engines retain STORNAM1, STORCELL, STORVECT and STORMAP1 version 1.
ICSS is an external benchmark/test reference only. Earlier images require
explicit host reset/framing admission; no legacy decoder or automatic reset.
New format work needs its own decision, not another consumer's reset authority.
See [replacement records](../design/independent-storage-replacement.md).

Preserve failed attempts, archives, receipts, frozen sources, tool caches and
`.git/release-state/`. Keep stash `f652e8c7400f56f0cab6ad968d05c4c0aee6479d`
and branch `preserve/post-deploy-suppliers-20261113`. No cleanup is authorized.
Preserve authorized IcyDB preparation at `/tmp/icydb-control-layout`, branch
`storage/control-allocation`, base `03c5713e`: native checks passed, installed
composition remains unqualified, and durable relocation/draft-PR approval is
pending. Handoff trimming authorizes neither sibling changes nor artifact deletion.
