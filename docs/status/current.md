# Current handoff — 2026-11-12

## Active work

The maintainer authorized implementing the consumer audit findings and filing
sibling adoption issues. Storage's local, unpublished changes add
`insert_discard`/`remove_discard` on Map and regions, returning prior presence
without fetching or decoding the old payload. Returning operations share the
same engine and preserve their behavior. Length/block/key checks, governance,
native poison guards and installed rollback remain. No image format changed.
Metrics and Logging adopt these APIs after their existing validations.

Pending notes select Storage 0.73.6, Metrics Storage 0.76.3 and Logging Storage
0.72.3. Package manifests remain at published versions. Before publishing the
adapters, align their Storage minimum with the release containing these methods;
qualify and publish the supplier first. No release, commit or deployment was
performed for this work. Both locks subsequently advanced to Metrics 0.29.12 and
Logging 0.24.17 in separate work; those edits are preserved. Qualification also
includes the local Metrics adapter for its optional checkpoint benchmark.

[The discard study](../measurements/storage-discard-mutations-2026-11-12.md)
owns reproduction, matched instruction/IO data, size and qualification evidence.
All 96 discard replacement/deletion medians improve over same-engine returning
controls; 87 beat ICSS and nine 256-byte cases lose. This is not a whole-consumer
performance claim. Focused native tests/Clippy, installed Storage old-image upgrade
and rollback, Logging lifecycle tests, and both size modes pass. Each size mode
qualifies 16 modules, 131 calls and two upgrades per module. Returning application
Wasm grows 135 bytes for Map / 198 for all collections. No full release validation
or fresh macOS proof was run for this change.

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
  discard-mutation adoption after the supplier is released.
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
continuation. Sibling source remains read-only. Publication or
consumer deployment needs its own request. No current cold-authentication speedup
has been established by the discard work.

## Current published baseline and suppliers

Storage 0.73.5, Metrics Storage 0.76.2 and Logging Storage 0.72.2 were published
on 2026-11-12 through `batch-lzNupEJ_`, all 12 stages and archive qualification
passed, and the batch was reconciled into main. The current local work starts
from `ca0c1e478bc7cc114f6fa3142c74d47d1d32dfc6`.
[The prior handoff](https://github.com/dragginzgame/ic-storage/blob/ca0c1e478bc7cc114f6fa3142c74d47d1d32dfc6/docs/status/current.md)
retains release commits, hashes and completed historical narratives. Evidence:
`target/evidence/deploy-storage-0735/`, `target/release-frozen/batch-lzNupEJ_/`
and `.git/release-state/frozen/batch-lzNupEJ_/`. Release CI was asynchronous at
that handoff; no fresh hosted/macOS result is asserted here.

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
