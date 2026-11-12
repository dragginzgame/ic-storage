# Current handoff — 2026-11-12

## Current supplier adoption and delivery

Toko Tooling 0.39.17 is installed with all 40 entrypoints and 104 files. The
catalogs select Host 0.76.5, Wire 0.21.4, Platform 0.31.3 and Runtime 0.21.3;
qualification also uses Types 0.19.5 and macros/guard 0.21.3. Testkit 0.101.0
and PocketIC 16.1.0 are unchanged. See [the adoption record](../adoption/storage-tooling.md#current-supplier-and-toko-03917-adoption).
The requested module rule is satisfied by 28 exact-content moves to directory
roots. Source links, helper fixtures and private Logging CI selection follow
those paths. Both catalogs/locks match the authenticated snapshot.

Focused proof passes: supplier graph/pins/tools, native/Wasm Clippy, 178 product
unit tests, 88 helper tests and 49 benchmark/runtime tests. Installed original/
O3/Os/Oz IO, corruption, custody, upgrades and rollback pass at
`target/qualification/runtime/attempt.exkRp9`. Make-adapter substitutes also
pass after fixing the dependency-update include's fixture closure. Evidence:
`target/evidence/supplier-refresh-20261112/`; layout proof is under
`target/module-layout/`. No new performance or macOS result is claimed.

The subsequent deployment published Storage 0.73.5, Metrics Storage 0.76.2 and
Logging Storage 0.72.2, including the reviewed module, qualification and
measurement work. The node-first study remains unresolved as an Auth optimization,
as described below. Earlier adoption evidence remains linked from the canonical record.

## Performance findings and remaining experiments

The [small-read study](../measurements/storage-scalar-scans-2026-11-11.md#small-read-experiments--2026-11-12)
retains removal-admission reuse (62 total / 53 code Wasm bytes, selected removal
gains of 0.5–1.5%). Vector combined reads and Cell stack decoding were rejected;
their production/test trials are removed. Broader Cell benchmark coverage remains.

The [identity study](../measurements/storage-identity-lookups-2026-11-12.md) adds
full Principal/composite-key 1k/10k/100k comparisons. Node-first allocation leaves
initial cold hits unchanged and fails Auth's <1M target. The allocation-order
change and its immediate-NODE-reservation corruption fix are included in 0.73.5;
this is not a demonstrated Auth lookup improvement. Focused native/installed/size proofs and the
198-row comparison are in the study; siblings remain unchanged.

The [batch-placement follow-up](../measurements/storage-identity-lookups-2026-11-12.md#small-allocation-batches--rejected)
rejects both 4 KiB and population-scaled 4–64 KiB small-class refills. Their code
and exclusive tests are removed, restoring pre-trial files byte-for-byte. Ramp
improves 81/198 identity medians but regresses 64; Auth 100k remains ~1.224M and
its second same-execution read rises ~85k → 965k. Mutation/refill regressions
outweigh the selected cold-hit gains; application Wasm grows only ~0.55 KB.
Both locks and sibling sources are preserved. Raw evidence:
`target/evidence/routing-batches-20261112/`. The runner retains a qualified
`--storage-only --storage-install-wasm` earlier-image upgrade/mutation proof.
Next measure first/second/third reads independently before another placement
trial; node/key pairing and short-key scratch remain hypotheses. Larger inline
keys require a new-format decision; Auth cannot authorize retiring other records.

## Published state

All three groups published on 2026-11-12 from batch `batch-lzNupEJ_`:

| Package | Version | Release commit | Annotated tag |
| --- | --- | --- | --- |
| ic-storage | 0.73.5 | `d299e5aecf0d587e3e0f2fc9e6b0b1ee5125ffc9` | `storage-v0.73.5` |
| ic-metrics-storage | 0.76.2 | `f64a651d39c3c2d038a09f7768492aa95940df0c` | `metrics-storage-v0.76.2` |
| ic-logging-storage | 0.72.2 | `0ed6c10edfdc2075f0137772ce0d7b4ce7a35341` | `logging-storage-v0.72.2` |

Every group passed all 12 release stages and its final archive qualification.
The batch was reconciled into normal main. Registry indexes and downloaded crates
match the qualified archive SHA256 values:

- Storage: `08dced669191b0b7525f9e662e816873bd5194f10273d10ed5bfd3785bad2dfd`.
- Metrics Storage: `abb6b43d049514e864a613bfe53cfbdff4bb7fecceae39172f5c0d176950721b`.
- Logging Storage: `d43d6baf6e021cbc94c1129b8439125abab7ba4026539563e7d382306fc55978`.

Evidence is retained in `target/evidence/deploy-storage-0735/`, the managed
checkout `target/release-frozen/batch-lzNupEJ_/` and durable state under
`.git/release-state/frozen/batch-lzNupEJ_/`. Remote main and annotated tags were
verified. [Release CI](https://github.com/dragginzgame/ic-storage/actions/runs/43226207634)
was still running at handoff; this record does not claim a fresh macOS pass.

### Previous deployment

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

## Suppliers and remaining work

Current supplier selections are recorded above. The performance study
also preserves the completed concurrent Wire 0.21.3 and errno 0.3.15 lock updates
and Rust 1.99.1. The prior publication's registry review remains at
`target/evidence/deploy-storage-0734/freshness.json`; it is not a current registry audit.

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

Canic whole-endpoint costs remain outside Storage qualification. The selected
Auth authentication regression is measured in the identity study, not a full
qualification or release of Auth.
Canic/IcyDB have unrelated pending work. Preserve authorized IcyDB preparation
at `/tmp/icydb-control-layout`, branch `storage/control-allocation`,
base `03c5713e`: native checks passed, installed composition is unqualified,
and durable relocation or draft-PR approval remains pending.
Do not modify or deliver siblings.
