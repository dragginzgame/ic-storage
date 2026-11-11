# Current handoff — 2026-11-11

## Completed delivery and next work

Storage 0.73.3 is published on crates.io and tagged on GitHub. It contains compatible
lease/Vector optimizations and the preserved qualification rustix correction.
Package versions are Storage 0.73.3, Metrics Storage 0.76.1 and Logging
Storage 0.72.1. No API/format change or consumer reset is required for this batch.

The [three optimization experiments](../measurements/storage-scalar-scans-2026-11-11.md#lease-and-vector-experiments--2026-11-11)
retain a live-lease bitmap and Vector checked-fact reuse. Combined Map reads
regressed the wider type controls and were removed completely; the ordered
engine's three trial files match pretrial HEAD. All-collection application Wasm
falls from 123,076/104,985 to 116,288/99,172 total/code bytes; ICSS is
119,324/77,133. Small individual consumers remain larger than ICSS.
Vector 32-record reads improve 3.7%; Auth-shaped session reads save only 116
instructions. Map scalar value scans still cost about 2.37 times ICSS.

Forty relevant retained-change native tests pass, alongside native/Wasm Clippy.
The final 16 size consumers each pass 127 calls/two upgrades. Installed Storage
IO/admission/upgrade across 1,031 allocations and decoder/update/upgrade rollback
pass. The scalar matrix, 140 Vector read/mutation medians and 240 Auth-shaped
query/update medians have matched reference results. The complete type
catalogue and fresh macOS execution were not rerun. All 12 Storage release
stages subsequently passed, including 123 library tests and archive qualification.
Evidence: `target/evidence/lease-map-vector-20261111/`; final collection/size
attempts `HUpsEh`/`gSebAh`. Both locks remain unchanged by these experiments.

[Storage #157 — Update qualification rustix past RUSTSEC-2026-0486](https://github.com/dragginzgame/ic-storage/issues/157)
changes only qualification rustix 1.1.5 → 1.1.6. Product dependencies and unrelated
Windows selections are preserved. Locked metadata and refreshed RustSec feed
`901cf64f2d04dc7f9f26bbbd50a3010378de034d` confirm the advisory is absent;
paste/serde_cbor maintenance warnings remain. Native benchmark/runtime Clippy
and 26+4 unit tests pass (two installed-only tests ignored); focused Logging
IO/upgrade/rollback and awaited deletion pass at `attempt.5vYTuv`.
Evidence is `target/evidence/issue-157/`. #157 is closed after confirmed
integration on main in `8263d7447e1b7319f44572ab968cdb8423457046`.
The last issue inventory found no other open issues or PRs.

The completed delivery selected only Storage: the unchanged published adapters'
compatible Storage 0.73.2 requirements already admit this patch. Release metadata
advanced the local catalog requirement without republishing those adapters.
Further
ordered-map density or scan changes need a distinct experiment and format
decision; do not restore the rejected root-cache or combined-read candidates.

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

Wire 0.21.1, Build 0.83.0, Platform 0.29.3, Runtime 0.21.1 and Toko 0.39.10
remain selected in their owning graphs; Testkit 0.95/PocketIC 16.1.0 and Rust
1.99.1 are unchanged. See [adoption records](../adoption/storage-tooling.md).
The new measurements use these current selections. Historical measurements
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
