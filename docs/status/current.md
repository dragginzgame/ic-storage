# Current handoff — 2026-11-12

## Published work

The maintainer authorized implementing the consumer audit findings and then
deploying this repository. Storage 0.73.6 adds
`insert_discard`/`remove_discard` on Map and regions, returning prior presence
without fetching or decoding the old payload. Returning operations share the
same engine and preserve their behavior. Length/block/key checks, governance,
native poison guards and installed rollback remain. No image format changed.
Metrics and Logging adopt these APIs after their existing validations.

Storage 0.73.6, Metrics Storage 0.76.3 and Logging Storage 0.72.3 are published
on crates.io and tagged on GitHub. Both published adapters require Storage
0.73.6. Both locks retain Metrics 0.29.12 and Logging 0.24.17; qualification
includes the local Metrics adapter for its optional checkpoint benchmark.

[The discard study](../measurements/storage-discard-mutations-2026-11-12.md)
owns reproduction, matched instruction/IO data, size and qualification evidence.
All 96 discard replacement/deletion medians improve over same-engine returning
controls; 87 beat ICSS and nine 256-byte cases lose. This is not a whole-consumer
performance claim. Focused native tests/Clippy, installed Storage old-image upgrade
and rollback, Logging lifecycle tests, and both size modes pass. Each size mode
qualifies 16 modules, 131 calls and two upgrades per module. Returning application
Wasm grows 135 bytes for Map / 198 for all collections. All 12 release stages
passed for each group; no fresh hosted/macOS result is asserted here.

The [histogram comparison study](../measurements/metrics-histogram-comparisons-2026-11-12.md)
retains full-width numeric equality in Metrics Storage. A fresh 0.29.12 baseline
shows ~8.6% selected load and ~22% unchanged-save savings (34.5% repeated-call),
for 1.17 KB more benchmark Wasm. Thirty native tests and installed matched runs
pass. Temporary profiling code/dependencies are removed; numeric admission,
stored bytes and error order remain. Wire decoding and canonical restoration
are separate remaining costs, with no sibling changes or issue writes.

## Consumer ownership and next actions

- [Storage #118](https://github.com/dragginzgame/ic-storage/issues/118) is reopened
  for this implementation. Actual adapter checkpoint calls are now measured:
  changed histogram saves improve 0.31–0.48%, removals 0.20–0.58%, with 736 fewer
  benchmark Wasm bytes. No-op/rejection writes remain zero. The prefix-buffer
  trial was rejected and restored; the discard study owns its evidence.
- [IcyDB #635](https://github.com/dragginzgame/icydb/issues/635) owns key-only scans
  and bounded region existence probes using already published APIs.
- [Blob #129](https://github.com/dragginzgame/ic-blob-storage/issues/129) owns
  discard-mutation adoption; the supplier is now published.
- [Relay #24](https://github.com/dragginzgame/ic-relay/issues/24) owns measurement
  of bounded record rewrites before any state-layout redesign.
- Existing [Auth #132](https://github.com/dragginzgame/ic-auth/issues/132) and
  [Canic #764](https://github.com/dragginzgame/canic/issues/764) received adoption
  notes. Complete their existing ownership migration; do not create a second
  authentication engine. Their current installed regression ceiling is 1.3M;
  the earlier <1M request remains an optimization goal, not that release gate.
- [Storage #158](https://github.com/dragginzgame/ic-storage/issues/158) owns
  routing-key locality/blob density experiments and any explicit format decision.

The maintainer now owns issue follow-up; no issues/comments changed in the
continuation or deployment. Sibling source remains read-only and consumer
deployment needs its own request. No current cold-authentication speedup
has been established by the discard work.

## Current published baseline and suppliers

The 2026-11-12 delivery captured source `a2552c509975a08b668614c0bc8d19acb3ee50ef`
in `batch-fgFa7wx8`, published supplier first, then reconciled the clean normal
checkout to `4668fee8bac275d55bb0d5ef4c6b71725573e708`. GitHub main and annotated
tags were verified. Downloaded crates.io archives match registry and final-receipt
SHA-256 values; neither adapter retains the previous Storage minimum.

| Package | Version | Release commit | Archive SHA-256 |
| --- | --- | --- | --- |
| Storage | 0.73.6 | `0ab2afdc7e482c3d0f352c44d533bab0d2a2cc09` | `99884d9c4cde4747cf7ccff910789033f3ee07c05b9d69ed77288df9fcdaeaee` |
| Metrics Storage | 0.76.3 | `39efb2b9f8424af1292d4bf948c20a6de255ba22` | `996a99e738f09792e2ac60a2e2a8e222a935d5be730a9fe101cd360028870c9b` |
| Logging Storage | 0.72.3 | `4668fee8bac275d55bb0d5ef4c6b71725573e708` | `ddf42f5459b78c0d7da24ab528e23c317615fca06341d85da1d54b82ebb094ab` |

Evidence: `target/evidence/deploy-storage-0736/verification.json`,
`target/deploy-0736-batch.log`, `target/release-frozen/batch-fgFa7wx8/` and
`.git/release-state/frozen/batch-fgFa7wx8/`. Installed Logging qualification was
refreshed against Logging 0.24.17: `target/qualification/runtime/attempt.Ge7c1K`
passes lifecycle, query non-persistence, update/upgrade rollback and reopen.
[The pre-release handoff](https://github.com/dragginzgame/ic-storage/blob/a2552c509975a08b668614c0bc8d19acb3ee50ef/docs/status/current.md)
retains earlier delivery evidence and the implementation narrative.

Toko 0.39.17, Rust 1.99.1, Host 0.76.5, Wire 0.21.4, Platform 0.31.3,
Runtime 0.21.3, Testkit 0.101.0 and PocketIC 16.1.0 remain selected.
[The adoption record](../adoption/storage-tooling.md#current-supplier-and-toko-03917-adoption)
owns graph/tool/layout evidence. Original installed fixture evidence is retained
at `target/qualification/runtime/attempt.exkRp9`.

## Performance and preservation boundaries

The [scalar study](../measurements/storage-scalar-scans-2026-11-11.md) owns shipped
cursor/extrema/removal improvements and remaining scalar scan gaps. The
[identity study](../measurements/storage-identity-lookups-2026-11-12.md) owns
node-first allocation and rejected 4–64 KiB refill trials. Node-first was shipped
without a demonstrated cold Auth win. Refill trials worsened repeated-read and
mutation costs and are removed. Do not restore rejected root-cache, combined-read,
compact-frame or decoder-outlining implementations. New placement trials must
measure first/second/third reads independently, mutation costs and application Wasm.

Independent engines and current formats remain STORNAM1, STORCELL, STORVECT and
STORMAP1 version 1. ICSS is an external benchmark/test dependency only. Earlier
images require explicit reset and host framing admission; no automatic reset or
compatibility reader exists. A new format must not inherit an earlier consumer's
reset authorization. See [replacement records](../design/independent-storage-replacement.md).

Preserve failed attempts, archives, receipts, frozen sources, tool caches and
`.git/release-state/`, including earlier batch evidence linked from the prior
handoff. Keep stash `f652e8c7400f56f0cab6ad968d05c4c0aee6479d`. Trimming this
handoff authorizes no artifact cleanup. Target evidence can disappear under an
explicit cargo clean; durable recipes and decisions remain in measurement docs.
Preserve unrelated sibling work, including authorized IcyDB preparation at
`/tmp/icydb-control-layout`, branch `storage/control-allocation`, base `03c5713e`:
native checks passed, installed composition is unqualified, and durable relocation
or draft-PR approval remains pending.
