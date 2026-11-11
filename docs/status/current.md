# Current handoff — 2026-11-11

## Pending adoption and published state

IC Build 0.83.0 is adopted locally for the native helper and metadata editor.
Only Build changes in the root lock; qualification's independent graph is unchanged.
All 88 helper tests, strict helper Clippy, snapshot/pin/production graph checks,
editor admission and focused bootstrap recovery pass. See [Build adoption](../adoption/storage-tooling.md#current-ic-build-083-adoption)
and `target/evidence/build-083-adoption/`. Pending Storage 0.73.2 is a compatible
tooling patch with no public API or stored-format change. Deployment is now
authorized to this repository's origin/main and crates.io: Storage 0.73.2,
Metrics Storage 0.76.1 and Logging Storage 0.72.1. Adapter patches publish the
updated Storage requirement. The shared batch runner owns all three complete
release gates, version preparation, Git effects and publication; no sibling
delivery or artifact cleanup is included. Native macOS qualification is separate.

Wire 0.21.1, Build 0.83.0, Platform 0.29.3, Runtime 0.21.1 and Toko 0.39.10
are selected in their owning graphs. Testkit 0.95/PocketIC 16.1.0 is unchanged.
See [supplier adoption](../adoption/storage-tooling.md#current-cargo-owned-03910-selection)
and the [current measurement record](../measurements/storage-scalar-scans-2026-11-11.md).
The latest tooling refresh retains 39 entrypoints/102 files, changing only the
dependency checker and hash-policy catalog. That refresh changed only Toko/Wire
from the preceding selection; snapshot/tool/pin/production and consumer graph
admission pass. The retained performance studies below used Wire 0.20.0 and
Toko 0.39.9; they are not a fresh benchmark of the latest dependency adoption.
Installed original/O3/Os/Oz IO, directory/custody, collection and Logging
upgrade/rollback qualification passes with Wire 0.21.1 (`attempt.NdKepQ`).

Storage 0.73.1, Metrics Storage 0.76.0 and Logging Storage 0.72.0 are published
on crates.io with annotated GitHub tags. All three registry checksums match their
qualified archive receipts. Core batch `batch-RkNljHBk` completed at
`30e834436cb22fed0455e599902ab64668ab5909`; adapter batch `batch-0cbV8nRz`
completed at `ce257f8b1c6fac366ccaab5e43c2c1876a03ea04` (Metrics release commit
`dfc8225daaddef647deb2a7c8902b1c8ce308036`). Both batches are reconciled into the
normal `main` checkout. Each group passed all 12 release stages, including
native/Wasm lint, features, tests, size consumers and workspace archives.
Receipts and logs remain under each batch's `target/release-validation/` within
`target/release-frozen/`, with durable intent under `.git/release-state/frozen/`.
No sibling delivery, canister reset or stored-format change is included.
The first core gate (`batch-c41aaSnt`) stopped before version mutation on an
empty-buffer assertion lint; its logs remain retained. The assertion is corrected
without changing product code; the successful core batch used the corrected source.

The retained [custody-registry simplification](../measurements/storage-scalar-scans-2026-11-11.md#retained-custody-registry-simplification--2026-11-11)
replaces only physical-backing claims with a sorted vector. All-collection Wasm
falls by 4,888 total / 4,944 code bytes to 122,139 / 104,119 bytes; matched ICSS
is 119,324 / 77,133. Admission comparisons validate 864 samples per Storage
version and 600 ICSS controls. Reopen/append increase by at most 420 instructions
(under 1% in the measured matrix); this is a size win, not a universal speedup.
Many distinct custom backings now pay linear insertion/removal shifts; the
usual IC backing has one identity. Live-lease and admission trees are unchanged.

The custody batch passes 28 focused native tests, native/Wasm lint, all 16
size-consumer modules with 127 calls/two upgrades each, and focused installed
Storage IO/admission/upgrade/rollback across 1,031 allocations. Evidence lives
at `target/evidence/custody-set-20261111/` and size attempt `3MVuA5`.
No collection engine, API or stored-format change is included.

The subsequent [five hot-path experiments](../measurements/storage-scalar-scans-2026-11-11.md#map-hot-path-experiments--2026-11-11)
retain unchanged-routing-write suppression, inline owned value buffers, lazy
cursor advancement and direct buffer initialization. The scoped region-root
handoff regressed instructions and was fully removed. Matched shuffled scalar
Map key scans improve 22.2%, Set scans 25.1%; 1-MiB Map reads improve another
17.4% in the final buffer step. The new reopened-Cell workload measures a 17.9%
1-MiB load-control improvement, to 4.812M instructions versus ICSS's 4.971M.
These are different workloads, not an aggregate speedup. Scalar Map value scans
still cost about 2.37 times ICSS. Custom raw-read fallback backends incur extra
chunked copying; production backings provide direct reads.

All four retained steps add 937 total / 866 code bytes: current all-collection
Wasm is 123,076 / 104,985 bytes, ICSS 119,324 / 77,133. Final qualification passes
122 library, 26 public and 42 benchmark tests, strict native/Wasm lint, all 16
size consumers, and installed Storage IO/upgrade/rollback. The full benchmark type
catalogue was not rerun; the subsequent deployment passed all release gates.
Source/artifacts, per-step matrices and the
rejected candidate are retained under `target/evidence/map-hot-paths-20261111/`;
final comparison/size attempts are `eCLz3Y`/`YBMpeR`. Formats/APIs remain unchanged.

The earlier retained [range-path experiment](../measurements/storage-scalar-scans-2026-11-11.md#retained-range-path-experiment)
reuses checked bound paths. Its ordered comparison improves 252 medians and
leaves 1,800 unchanged, with no regressions. Shuffled scalar Map scans still
cost 2.44 times ICSS. Eager bounded-path allocation adds count-only work whose
IC cost is not separately timed. Dynamic-range size consumers corrected an
unbounded-only fixture that did not instantiate the changed code.

[Structure research and symbol attribution](../measurements/storage-scalar-scans-2026-11-11.md#data-structure-investigation--2026-11-11)
confirm one AVL engine serves Map, Set and regions despite their public BTree
names. Packed B/B+trees are candidates, not implemented alternatives or a
selected format. Hashing cannot replace real ordered consumer operations.
Key-distribution breadth, split/resize tails and matched application sizes must
qualify a future experiment; the present catalogue often embeds counter keys
in otherwise constant bytes. Preserve independent implementation boundaries.

Metrics' Wire test admits redundant varints with semantic round-tripping
and unchanged-save byte preservation. Released notes are [Storage 0.73.1](../../releases/storage/0.73.md),
[Metrics Storage 0.76](../../releases/metrics-storage/0.76.md) and
[Logging Storage 0.72](../../releases/logging-storage/0.72.md); adapter minors
reflect Wire's public type identity. Adoption and delivery evidence is under
`target/evidence/deploy-toko-wire-20261111/`. No sibling changes or delivery
are authorized.

## Previous published baseline and historical evidence

Storage 0.73.0, Metrics Storage 0.75.0 and Logging Storage 0.71.0 are published
on crates.io with annotated GitHub tags. Batch `batch-iGEMdpfB` completed at
`726c674dea238fdfc4b59af0aac29761489479c2` and was reconciled locally. All groups
passed the 12-stage release plan and archive checksums were verified. Evidence:
`target/release-frozen/batch-iGEMdpfB/target/release-validation/` and durable
`.git/release-state/frozen/batch-iGEMdpfB/`. No canister reset or sibling delivery
was included. Released Metrics/Logging selections were 0.29.11/0.24.16.
Release preparation repaired the LICENSE.md fixture paths and native/Wasm backing
inference in public tests. Concurrent Build 0.81.2/Host 0.76.3 work was preserved
outside that batch; backup stash `f652e8c7400f56f0cab6ad968d05c4c0aee6479d` remains.

The [complete comparison](../measurements/storage-full-suite-2026-11-10.md)
retains 192,444 observations, all recipes and 1/8/128-page bucket controls.
It predates current inputs: its qualification Runtime was 0.21.0 while a root
0.21.1 change arrived during retries. Do not attribute those results to the
current supplier graph. Small-record density/scans trail ICSS; larger values
and terminal/rank operations have measured wins. Earlier 565-case qualification,
54,318 observations, old-image rejection and optimized lifecycle proofs remain
in the [ordered-engine record](../design/ordered-map-replacement.md), including
`attempt.Dh67gD` and the earlier `attempt.n5kPft` package qualification.
Old benchmark artifacts missing at release preparation cannot be reconstructed
from retained summaries; fresh release evidence remains at the paths above.

## Persistence, preservation and external boundaries

The [replacement inventory](../design/independent-storage-replacement.md) links
Cell, Vector, Memory, named backing and ordered-engine source/qualification
records. Engines were independently authored through restricted handoffs; ICSS
is an external test/benchmark dependency only. Current formats are STORNAM1,
STORCELL, STORVECT and STORMAP1 version 1. Earlier images require explicit
fresh/reset backing and application framing admission; no legacy reader or
implicit reset exists. No deployed consumer has been migrated here. A future
format proposal must not silently broaden the previous reset authorization.

Keep failed attempts, frozen sources, archives, receipts, `.git/release-state/`,
tool caches and preservation stashes. Target evidence can disappear with explicit
cargo clean; durable decisions/reproduction remain in the linked records. No
artifact cleanup is authorized by this handoff trim. Product group gates and
publication remain separate from focused optimization qualification.

Canic/Auth whole-endpoint costs are outside Storage qualification. Canic/IcyDB
have unrelated pending work. Preserve the authorized IcyDB preparation at
`/tmp/icydb-control-layout`, branch `storage/control-allocation`, base `03c5713e`:
native checks passed, installed composition is unqualified, and durable relocation
or draft-PR approval remains pending. Do not modify or deliver that sibling.
