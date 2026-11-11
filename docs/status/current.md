# Current handoff — 2026-11-11

## Published state

Storage **0.73.4** is published on crates.io and delivered to GitHub main with
annotated tag [storage-v0.73.4](https://github.com/dragginzgame/ic-storage/releases/tag/storage-v0.73.4).
Release commit: `d41fe827aee543bc841ead22debb4e47efccacda`.
Managed batch `batch-Ugq73XHy` completed all 12 release stages, including
whole-workspace archive integration, and was reconciled into normal main.
Metrics Storage **0.76.1** and Logging Storage **0.72.1** retain their versions;
their implementations and compatible public requirements are unchanged.

The qualified archive, downloaded crate and crates.io index agree on SHA256
`b91508b8e1d7cf355fec15a3857596ba26176a977cc951386720c9c9b2304939`.
Remote main and the annotated tag resolve to the release commit. Evidence:
`target/evidence/deploy-storage-0734/`, batch validation logs under
`target/release-frozen/batch-Ugq73XHy/` and durable state under
`.git/release-state/frozen/batch-Ugq73XHy/`. Hosted CI remains asynchronous;
this delivery does not claim a fresh macOS result.

## Delivered optimizations and qualification

The [cursor borrowing experiment](../measurements/storage-scalar-scans-2026-11-11.md#cursor-borrowing-experiment--2026-11-11)
avoids an intermediate complete-node copy. Its 98 scalar medians have 34
improvements, 64 unchanged and none regressed; the broader 2,052-row comparison
improves 900 with 1,152 unchanged. Key scans improve about 2.2%, entry/value
scans about 1%; point/mutation controls and ICSS reference medians are unchanged.
The original all-collection probe saves 74 total/code bytes. The rejected
compact-frame trial is removed.

The [direct first/last follow-up](../measurements/storage-scalar-scans-2026-11-11.md#direct-extrema-follow-up--2026-11-11)
removes unused rank selection while retaining structural checks. Scalar batches
of 32 improve 1.37%/1.54%; the broader comparison improves 126 of 2,052 medians,
with 1,926 unchanged and none regressed. Expanded size probes now call first/last:
Map, Map+Region and all collections each save 133 total / 117 code bytes against
identical old-engine probes. All-collection Storage is 116,949/99,701 total/code
bytes versus ICSS 119,816/77,566. These expanded endpoints are not directly
comparable to older probe totals. Decoder outlining was rejected after adding
60 Wasm bytes. Individual consumers remain larger than ICSS, and scalar Map
entry scans still cost about 2.35 times ICSS.

Thirty engine/eight public tests, native/Wasm Clippy, 16 size consumers with
131 calls and two upgrades each, and installed Storage IO/upgrade/rollback pass.
The latter covers 1,031 named allocations and decoder/update/upgrade traps.
Evidence: `target/evidence/cursor-experiment-20261111/` and
`target/evidence/ordered-followup-20261111/`. APIs and stored formats are unchanged.
No alternate experiment implementation remains.

Both catalogs/locks and the authenticated snapshot select Toko **0.39.12**.
Qualification and the prepared CLI use Testkit **0.100.2** with PocketIC **16.1.0**.
The async client migration uses polling budgets and awaited best-effort deletion;
the managed CLI bounds server lifetime. Native Clippy and 46 tests, installed
original/O3/Os/Oz IO/upgrade/rollback, five-collection benchmark/refusal probes,
admission drivers and size consumers passed on Linux. See the
[adoption record](../adoption/storage-tooling.md#current-toko-03912-and-testkit-01002-adoption)
and `target/evidence/toko-testkit-adoption-20261111/`.

## Suppliers and remaining work

The pre-freeze crates.io review confirms Build 0.83.0, Host 0.76.4,
Metrics 0.29.11, Runtime 0.21.1, Testkit 0.100.2 and Toko 0.39.12 are current.
Root Logging 0.24.16 and Wire 0.21.2 are current. Qualification retains Logging
0.24.6, Wire 0.21.1 and Platform 0.29.3 versus published 0.24.16, 0.21.2 and
0.31.1. Those optional updates remain deferred to the next requested repository-wide
supplier refresh, including Platform compatibility qualification. The release
preserves the measured graphs and Rust 1.99.1. Registry evidence:
`target/evidence/deploy-storage-0734/freshness.json`.

The [previous lease/Vector experiments](../measurements/storage-scalar-scans-2026-11-11.md#lease-and-vector-experiments--2026-11-11)
shipped in Storage 0.73.3. [#157](https://github.com/dragginzgame/ic-storage/issues/157)
is closed after default-branch integration; its rustix advisory proof remains in
`target/evidence/issue-157/`. Unrelated paste/serde_cbor maintenance warnings
remain. The last issue inventory found no other open issues or PRs; it was not
repeated as part of deployment. Further density changes require a separate
experiment and format decision. Do not restore rejected root-cache,
combined-read, compact-frame or decoder-outlining trials.

The [full suite](../measurements/storage-full-suite-2026-11-10.md) retains 192,444
observations but predates the current graph. Earlier measurements retain their
actual suppliers; neither that suite nor fresh macOS qualification was rerun here.
Prior delivery evidence remains in `target/evidence/deploy-storage-0733/`,
`target/evidence/deploy-build-083/` and batches `batch-Gm8WnqRN` / `batch-dkC14l_X`.

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
