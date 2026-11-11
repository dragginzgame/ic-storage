# Current handoff — 2026-11-11

## Selected delivery

The maintainer authorized Storage 0.73.4 delivery from normal `main` to
`origin/main` and crates.io, using the managed frozen batch and all 12 release
stages. Metrics Storage 0.76.1 and Logging Storage 0.72.1 retain their versions:
their implementations and compatible public requirements are unchanged.
Delivery is pending; the published state below remains authoritative until verified.

The pre-freeze crates.io review confirms Build 0.83.0, Host 0.76.4,
Metrics 0.29.11, Runtime 0.21.1, Testkit 0.100.2 and Toko 0.39.12 are current.
Root Logging 0.24.16 and Wire 0.21.2 are current. Qualification retains Logging
0.24.6, Wire 0.21.1 and Platform 0.29.3 versus published 0.24.16, 0.21.2 and
0.31.1. These optional independent-graph updates remain deferred to the next
requested repository-wide supplier refresh, including Platform compatibility
qualification. This release preserves the measured dependency graphs. Registry
evidence: `target/evidence/deploy-storage-0734/freshness.json`.

## Pending ordered optimizations

Storage 0.73.4 has pending compatible notes; manifests remain at published
Storage 0.73.3, Metrics Storage 0.76.1 and Logging Storage 0.72.1.
The [cursor borrowing experiment](../measurements/storage-scalar-scans-2026-11-11.md#cursor-borrowing-experiment--2026-11-11)
avoids an intermediate node copy during iteration. Mutation and storage
source remain unchanged; public APIs, owned entries and formats are unchanged.
The preceding compact-frame trial is removed because it regressed unrelated
endpoint/removal costs. No alternate implementation remains.

The 98 scalar medians have 34 improvements, 64 unchanged and no regressions.
Key scans improve about 2.2%, entry/value scans about 1%; point/mutation controls
are identical. All-collection application Wasm is 116,214/99,098 total/code bytes,
74 fewer in each measure; ICSS is 119,324/77,133. Individual consumers remain
larger than ICSS. Scalar Map entry scans still cost about 2.35 times ICSS.
The broader 2,052-row ordered matrix improves 900 medians with 1,152 unchanged
and none regressed; reference medians and stable capacities match throughout.
The narrow borrowing change is retained.

Twenty-eight engine and eight public contract tests, native/Wasm Clippy and
formatting pass. The 16 size modules pass 127 calls/two upgrades each.
Installed Storage IO/admission/upgrade across 1,031 named allocations and
decoder/update/upgrade trap rollback pass. New tests cover mixed-direction jumps,
variable key/value payloads and retained entries across backing reuse.
Evidence: `target/evidence/cursor-experiment-20261111/`; final scalar/size attempts
`cvR5I7`/`xPLBOi`. Both locks and dependency selections were unchanged during
that experiment; the subsequent tooling/Testkit adoption is recorded below.
Full release gates, complete type catalogue and fresh macOS execution were not
rerun for this pending batch. Nothing in this batch is committed or deployed.

The [previous lease/Vector experiments](../measurements/storage-scalar-scans-2026-11-11.md#lease-and-vector-experiments--2026-11-11)
shipped in Storage 0.73.3. [#157 — Update qualification rustix past RUSTSEC-2026-0486](https://github.com/dragginzgame/ic-storage/issues/157)
is closed after default-branch integration; its advisory and native/installed
proof remains in `target/evidence/issue-157/`. The unrelated paste/serde_cbor
maintenance warnings remain. The last issue inventory found no other open
issues or PRs. Further density changes require a separate experiment and format
decision; do not restore rejected root-cache, combined-read or compact-frame trials.

The [direct first/last follow-up](../measurements/storage-scalar-scans-2026-11-11.md#direct-extrema-follow-up--2026-11-11)
removes their unused rank-selection path while retaining all structural checks.
Scalar batches of 32 improve 1.37%/1.54%; the broader 2,052-row comparison has
126 improvements, 1,926 unchanged and no regressions, solely in First/Last.
ICSS medians and capacities match. Decoder outlining was tried and removed
because it increased ordered-consumer Wasm by 60 bytes.

Size probes now actually call first/last. On identical expanded probes, Map,
Map+Region and all collections each save 133 total / 117 code bytes. All-collection
Storage is 116,949/99,701 total/code versus ICSS 119,816/77,566. Do not compare
these expanded-probe totals to earlier endpoints as an engine-only size change.
Thirty engine/eight public tests, native/Wasm Clippy, 16 consumers with 131 calls
and two upgrades each, and installed Storage IO/upgrade/rollback pass. Evidence:
`target/evidence/ordered-followup-20261111/`. No release is included.

## Pending tooling and Testkit adoption

Both catalogs/locks and the authenticated snapshot select Toko 0.39.12;
qualification and the prepared CLI select Testkit 0.100.2 with PocketIC 16.1.0.
The async client migration is complete. Polling budgets and best-effort deletion
follow the new API; the managed CLI bounds server lifetime. Product dependencies
and the pending cursor Rust changes are preserved. See the
[adoption record](../adoption/storage-tooling.md#current-toko-03912-and-testkit-01002-adoption)
for supplier freshness dispositions and commands.

Snapshot/pin/tool/graph/boundary checks, native Clippy and 46 tests pass.
Installed original/O3/Os/Oz IO/upgrade/rollback, five-collection benchmark and
expected refusals, admission drivers and all 16 size consumers pass on Linux.
Consumer Wasm sizes are unchanged from the cursor trial. Evidence:
`target/evidence/toko-testkit-adoption-20261111/`. Fresh macOS and release gates
remain separate; this adoption is uncommitted and undeployed.

## Published state and selected suppliers

Published Storage 0.73.3, Metrics Storage 0.76.1 and Logging Storage 0.72.1
have annotated GitHub tags and matching crates.io archive checksums.
Storage batch `batch-Gm8WnqRN` completed at
`861be6f38a730fa5bfaf8bfb73674e6e397c7c4e`, passed all 12 release stages,
and was reconciled into normal main. Tag: `storage-v0.73.3`.
The qualified archive, downloaded crate and sparse registry index agree on SHA256
`66d8e9cdb30962bfb6a43bc9baeed9015b7c09b57dee283a0253288d35ece0f1`.
Evidence: `target/evidence/deploy-storage-0733/`, batch release-validation
receipts beneath `target/release-frozen/` and `.git/release-state/frozen/`.
Earlier batch `batch-dkC14l_X` and `target/evidence/deploy-build-083/` retain
the preceding Storage/adapters delivery evidence.

The root lock already selects Wire 0.21.2; qualification retains Wire 0.21.1.
That pre-existing root selection was preserved during the extrema experiment.
Build 0.83.0, Platform 0.29.3, Runtime 0.21.1 and Toko 0.39.12 remain selected;
qualification and CLI now use Testkit
0.100.2/PocketIC 16.1.0. Rust 1.99.1 is unchanged. See [adoption records](../adoption/storage-tooling.md).
The cursor measurements above used Testkit 0.95 and Toko 0.39.10. Historical measurements
retain their actual graphs and must not be presented as fresh supplier proof.

Earlier backing-custody, routing-write, inline-value, lazy-cursor and direct-read
changes remain in the [measurement record](../measurements/storage-scalar-scans-2026-11-11.md).
Their prior 122-library/26-public/42-benchmark checks and installed proof are
historical evidence, not newly rerun tests. The
[full suite](../measurements/storage-full-suite-2026-11-10.md) retains 192,444
observations but predates the current graph; its qualification Runtime was
0.21.0 while a root 0.21.1 change arrived during retries.

## Persistence and preservation boundaries

The [replacement inventory](../design/independent-storage-replacement.md) records
the independent Cell/Vector/Memory/backing/ordered engines and restricted
handoffs. ICSS remains external test/benchmark-only. Current formats are
STORNAM1, STORCELL, STORVECT and STORMAP1 version 1. Earlier images require
explicit fresh/reset backing and host framing admission; no compatibility reader
or automatic reset exists. A future format proposal must not silently broaden
the earlier reset authorization. No deployed consumer was migrated here.

Preserve failed attempts, frozen sources, archives, receipts, tool caches and
`.git/release-state/`. Earlier batches `batch-RkNljHBk`, `batch-0cbV8nRz`,
`batch-c41aaSnt` and `batch-iGEMdpfB` retain evidence; their narratives are in
[the prior handoff](https://github.com/dragginzgame/ic-storage/blob/256157b7d93297499e40bcbb61916f3ee383fa46/docs/status/current.md).
Keep preservation stash `f652e8c7400f56f0cab6ad968d05c4c0aee6479d`.
No artifact cleanup is authorized by this handoff trim. Explicit cargo clean
can remove target evidence; durable decisions and recipes remain in the docs.

Canic/Auth whole-endpoint costs remain outside Storage qualification.
Canic/IcyDB have unrelated pending work. Preserve authorized IcyDB preparation
at `/tmp/icydb-control-layout`, branch `storage/control-allocation`,
base `03c5713e`: native checks passed, installed composition is unqualified,
and durable relocation or draft-PR approval remains pending.
Do not modify or deliver siblings.
