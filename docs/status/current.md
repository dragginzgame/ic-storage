# Current handoff — 2026-11-14

## Published baseline and pending delivery

Storage 0.74.0, Metrics Storage 0.77.0 and Logging Storage 0.73.0 are published
on crates.io and tagged on GitHub. The completed batch `batch-jkhHj3FX`
reconciled main to `98e8d09504e8de2c668f7c7028180d86a5425752`; all three
12-stage release gates passed. Registry/archive verification and installed
qualification are retained under `target/evidence/suppliers-deploy-20261113/`
and `target/qualification/runtime/attempt.Og2iy6`.
[The preceding handoff](https://github.com/dragginzgame/ic-storage/blob/98e8d09504e8de2c668f7c7028180d86a5425752/docs/status/current.md)
retains earlier delivered evidence; local commit `87b1792` retains the complete
pre-adoption handoff and pending supplier/API qualification narrative.

Source commit `87b1792bf38686ad0c98cacbc0b4e2afa3a38eb6` prepared Storage
0.74.1, Metrics Storage 0.78.0 and Logging Storage 0.74.0. Package versions
remain unchanged. The authorized Storage patch deployment captured that source
in `target/release-frozen/batch-T5JZrPdp`; its journal is under
`.git/release-state/frozen/batch-T5JZrPdp`. Admission, lint, features and policy
passed, as did all 90 native repo-tool tests. The tooling stage failed because
Toko 0.42.6's hook formed an invalid helper path with inherited `CDPATH`.
No version preparation, release tag, push or publication occurred in that batch.
Retain `target/evidence/deploy-20261113/storage-release.log` and the frozen
checkout's gate logs. Failed evidence is not a release receipt. The adapter
minor batch has not started; core patch and adapter minors use separate batches.

## Current tooling adoption

Both workspace catalogs and locks now select authenticated Toko 0.43.0.
The LOC interface is now `toko-cloc`; use `--manifest Cargo.toml` or
`--manifest qualification/Cargo.toml` to select one workspace. The old Make
target and settings are retired. The existing PATH launcher matches the
published bytes. Baseline, release runner, hooks and product gates are unchanged.
See [the adoption record](../adoption/storage-tooling.md#toko-0430-adoption) and
`target/evidence/toko-0430-adoption-20261114/` for focused validation.
Snapshot/pins, tool admission, formatting, docs and focused ShellCheck pass.
Actual LOC reports select ten combined, three product or seven qualification
members; the retired Make target is absent. Supplier lock selections are unchanged.
Preserve the earlier failing fixture `target/fixtures/ic-storage-hooks.oU8S2d`.
No managed-file patch or weaker hook assertion is introduced.
The preceding 0.42.8 hook suite passed with `CDPATH` set, clearing the known
hook blocker; that unchanged-hook evidence remains applicable. Full product
release gates still require execution.
Incoming Metrics 0.30.5 and concurrent Logging 0.25.5 lock selections are
preserved; tooling adoption does not qualify those supplier updates.
No sibling edit, product release or publication is part of this refresh.
A subsequent delivery must admit the updated source; never alter the retained
failed batch or reuse its failed gate as a passing proof.

## Pending product and supplier work

The prepared batch includes native IO scope-guard simplification, Toko vendor
layout and common lint adoption, native release catalog projection/recovery
changes, Build 0.84 and Host 0.77 integration, Wire 0.22 and Testkit 0.104.
Build 0.84 is required for Host 0.77's shared `OutputLimits` API. Product Wasm
retains its existing Platform Runtime boundary; core production remains Wire-free.
The composition fixture uses Platform 0.32/Runtime 0.22. Testkit 0.104 still
selects PocketIC 16.1.0 and the existing bounded polling/server lifetime contract.

The 2026-11-14 Testkit adoption advances the qualification catalog, lock and
managed CLI together from 0.102.0 to published 0.104.0. Only Testkit changes in
the qualification lock; the product lock and pending supplier/tooling selections
are preserved. Removed Testkit Wire helpers have no Storage callers. Evidence:
`target/evidence/testkit-0104-adoption-20261114/`. This is local adoption, not
publication or sibling delivery. Both native consumers pass all-target Clippy;
50 unit tests pass (two installed-only tests remain ignored in that invocation).
Installed Linux IO, corruption, custody, collections, Logging, upgrades and
rollback pass for original/O3/Os/Oz at
`target/qualification/runtime/attempt.r8NYr6`. Snapshot/pins, formatting and docs
pass. Product Cargo.lock is byte-identical to this adoption's starting input.

Both adapters hard-cut public codec errors to the unit `StoreError::Codec`,
removing the public Wire payload, conversion and error source. Their internal
bounded Wire checkpoint encoding remains. Malformed/oversized categories,
retained bytes, admission and no-write refusals stay unchanged. This requires
the two pending minor releases, not a persisted-state reset. Product versions
are advanced only by the release runner. Detailed notes live in the group ledgers.

Focused evidence: `target/evidence/logging-host-adoption-20261113/`,
`target/evidence/testkit-adoption-20261113/`,
`target/evidence/wire-022-adoption-20261113/` and
`target/evidence/adapter-codec-errors-20261113/`. Adapter tests/doctests,
selected native/Wasm Clippy and boundary checks pass for their recorded inputs.
The earlier Testkit 0.102 installed evidence remains at
`target/qualification/runtime/attempt.qEgdWm`; the fresh 0.104 result above
qualifies the currently selected Wire and Logging/Metrics graph. No new macOS
proof or completed release gate is asserted. The unchanged 0.42.5 retention proof
cleared [Toko #381](https://github.com/dragginzgame/toko-tooling/issues/381).

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
