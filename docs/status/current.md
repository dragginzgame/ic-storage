# Current handoff — 2026-11-08

## Delivered: Storage 0.68.8

Published `d3d892c0707d8e6a26a509a5ea88019e34babdba`, tag `storage-v0.68.8`, from `5f4a91f2d25c1719d9f065511a87029cf217057b`; registry/download/receipt hashes and clean archive VCS identity match.
Batch `batch-I89m70rP` passed all 12 stages and final archives; local main reconciled. Evidence: `target/evidence/storage-0688-deploy/`, captured receipts under `target/release-frozen/batch-I89m70rP/target/release-validation/`, recovery under `.git/release-state/frozen/batch-I89m70rP/`.
Installed original/O3/Os/Oz IO/upgrade/rollback/Logging pass in `target/qualification/runtime/attempt.PdoT0O/`. Borrowed u8/bool encodings preserve APIs/images; benchmark Wasm −1,520 bytes. No adapter publication. Root Wire 0.17.12 remains a deferred working-tree edit, verified against local preservation branch `preserve/storage-0688-wire-01712`; released root Wire stays 0.17.11.
Prior 0.68.7 (`2944b9b6cfde16564c30de1aa60f66474748cb77`, source `f2368407310c2be7b727499a68d44a47ff8655ce`, `batch-oldv4xmM`) passed all 12 stages, publication/readback and reconciliation. Evidence: `target/evidence/storage-0687-deploy/`; captured receipts and durable recovery remain. Runtime 0.20 with IC0-only production, original/O3/Os/Oz installed proof and unchanged API/layout are retained below.
Prior 0.68.6 (`904f570d438f13b65e07e59d914a6dd4b843b905`, `batch-dByg2fKB`) passed full delivery/registry/reconciliation; `target/evidence/storage-0686-deploy/` and its captured receipts/recovery remain. Benchmark Wasm −8,358 bytes and handler savings up to 11.35% versus 0.68.5; 368/344/0 improved/equal/regressed cells. The [IO](../audits/recurring/storage-btree-io-2026-11-07.md), [empty-value](../audits/recurring/storage-empty-values-2026-11-07.md) and [save/search](../audits/recurring/storage-save-search-2026-11-07.md) studies retain matrices and rejected experiments.
Failed `batch-qZrTDTJ9` retained (native test Wasm gate corrected, no release effects); interrupted `batch-dByg2fKB` resumed after verified stale-lock recovery. No batch cleanup.

## Delivered: Storage 0.68.5

Published and non-yanked at `56102f4990bcf11a3ceaebe6bb2b1fb3ff9d380c`, tag `storage-v0.68.5`, from source `cc29dfe87c9252347db3c0db4c314b03177a889b`.
Batch `batch-KNdyHdLP` passed all 12 release stages and final archive qualification.
Registry and downloaded archive checksums match the final receipt and clean VCS identity.
Publication and guarded local reconciliation passed; no adapter release or cleanup.
Evidence: `target/evidence/storage-0685-deploy/`; receipts remain under
`target/release-frozen/batch-KNdyHdLP/target/release-validation/` and recovery under
`.git/release-state/frozen/batch-KNdyHdLP/`.
Combine Cell header writes: Wasm −662 bytes, batch-32 handler −0.93%; other depth cells equal.
The [study](../audits/recurring/storage-write-paths-2026-11-07.md) owns 73-type/native/runtime proof.
Both B-tree trials were reverted. Public APIs and retained formats are unchanged.
Host Artifacts/FS/Process 0.75.0 and matching Build/editor 0.78.0 are adopted.
All 88 helper tests and three-group offline metadata recovery pass;
focused adoption evidence remains in `target/evidence/ic-host-075-adoption/`.

## Rejected terminal-iterator experiments

The [ordered-iterator study](../audits/recurring/storage-ordered-iteration-2026-11-07.md)
retains both rejected projection trials; deployed 0.68.4 entry shortcuts remain.
Raw evidence: `target/evidence/storage-projection-last/`, `storage-next-back/`.
The local 5–10 KB Wasm-growth guideline remains binding absent exceptional benefit.

## Delivered: Storage 0.68.4

Published and non-yanked at `663662663c702e2003ac88d25596f5bfd71be470`, tag
`storage-v0.68.4`. Shared batch `batch-Er9VPHO8` passed all 12 release stages,
final archive qualification, publication and guarded local reconciliation.
Downloaded archive checksum and clean VCS identity match the final receipt;
evidence: `target/evidence/storage-0684-deploy/`. Receipts remain in
`target/release-frozen/batch-Er9VPHO8/target/release-validation/`; durable recovery
is `.git/release-state/frozen/batch-Er9VPHO8/`. No cleanup or adapter release.

Prior Build proof: `target/evidence/ic-build-adoption/`; Runtime 0.19.5 original/O3/Os/Oz
installed IO, custody, upgrade and rollback: `target/qualification/runtime/attempt.uMwbLm`.
The [backing study](../audits/recurring/storage-icss-backing-parity-2026-11-06.md)
and ordered-iterator study own the key-buffer and entry-last matrices, tradeoffs
and unchanged API/layout contract. Evidence: `target/evidence/storage-owned-key/`
and `storage-iterator-last/`. All 23 adapter and 49 benchmark tests and focused
installed qualification passed; full type timing was not rerun.

## Delivered: Storage 0.68.3

Published at `50dbd7f8b8fbc1516ccf9630a0e26bc3539a5b90`, tag `storage-v0.68.3`.
Batch `batch-RAD7_qIV` passed all 12 gates, archives, registry verification and reconciliation.
Evidence: `target/evidence/storage-0683-deploy/`; no cleanup.
Receipts/recovery for `batch-RAD7_qIV` remain in the standard captured/durable paths above.
Raw routing evidence: `target/evidence/storage-map-routing/` (`qKBykW`/`SpnBn9`/`LLDlVi`).
Installed original/O3/Os/Oz proofs: `target/qualification/runtime/attempt.UjEWo6` and `attempt.F3ZzEd`.
Build/Wire adoption and initial CLI refusal: `target/evidence/build-076-wire-0176/`.

## Earlier delivered groups and retained recovery

Storage 0.68.2 (`1c08ecf`, tag `storage-v0.68.2`) passed all 12 stages, registry
verification and reconciliation in `batch-1QTqeCag`; adapters did not change.
Evidence: `target/evidence/storage-0682-deploy/`. Captured/durable batch state
remains under `target/release-frozen/` and `.git/release-state/frozen/`.
Failed `batch-DqMimxOk` remains intact: inherited publication context broke a fixture
before version/Git/registry effects. The corrected fixture passed in the new batch.
Initial adoption evidence: `target/evidence/shared-tooling-0310/`. Delivery also
qualified the existing Wire 0.17.5 lock selection.

Storage 0.68.1 is published at `6ee75e906987fe20f73bb837c9107d9b5312da7e`,
with tag `storage-v0.68.1`; archive checksum/VCS readback and all 12 stages passed.
It adopted published Runtime 0.19.4 and shipped the earlier Vector/backing work.
Evidence: `target/evidence/storage-0681-deploy/`; installed `attempt.XawP1b`.

The prior coordinated hard cut published Storage 0.68.0 (`6f87f36`), Metrics
Storage 0.70.0 (`388c366`) and Logging Storage 0.66.0 (`7ac245c`), with their
group tags, all 12 stages and independently verified archives. Receipts remain
under `target/release-validation/`; evidence: `target/evidence/storage-068-deploy/`.
Installed proof: `attempt.P0oWYq`. Neither adapter has been republished since.
No publication needs replaying; history remains in owning release notes. Hosted CI is separate.

## Breaking collection baseline

Storage 0.68 owns private copied ICSS 0.7.2 engines behind governed adapters;
Map/Set/regions share one B-tree, Vector uses fixed-stride slots and Cell caches
its value. No production ICSS dependency or copied MemoryManager exists.
Old independent engines, collection checksums, node cache and Vector windows/
epochs are retired. [Provenance](../storage/upstreams.md) retains notices,
source identities and the deliberate safety/integration adaptations.

Existing collection images require an explicit host rebuild/reset or fresh
admitted allocation. BTR2 with explicit page size, SVC1 and SCL1 are current;
there is no legacy reader, migration or automatic reset. Directory encoding and
application codecs are unchanged. Engine/codec failures trap; framing admission
and reservation retain typed errors. Native mutation unwinds poison the owner;
Wasm relies on IC rollback. Positive cache ceilings do not enable a node cache.

Metrics Storage 0.70 and Logging Storage 0.66 align public Storage handles/errors
with 0.68. Metric snapshot and discriminator-3 logging payload semantics remain
unchanged, but their underlying collection images require the same host action.
See [Storage](../../releases/storage/0.68.md),
[Metrics Storage](../../releases/metrics-storage/0.70.md) and
[Logging Storage](../../releases/logging-storage/0.66.md) release notes.

## Current tooling and comparison evidence

Pending **Storage 0.68.9** keeps combined Vector header IO and shared framing admission, with no API/image change. The [opening study](../audits/recurring/storage-vector-open-2026-11-08.md) records 704/0/0 improved/equal/regressed reopen handler cells (−689 instructions each), unchanged 712 depth controls, benchmark Wasm −22,199 bytes and consumer fixture +96 bytes. Native admission/IO, ICSS interoperability, benchmark/lint and installed original Storage upgrade/rollback pass. Evidence: `target/evidence/storage-vector-open/`, baseline `attempt.LcZri5`, candidate `attempt.ClisQS`. Deployment authorized: Storage-only patch to origin/main and crates.io; qualification and publication pending. Root Wire 0.17.12 remains unrelated, unadopted and preserved; delivered 0.68.8 codec evidence remains in the [codec study](../audits/recurring/storage-byte-codecs-2026-11-07.md).
Shared Tooling 0.31.0 remains selected; [adoption boundaries](../shared-tooling-adoption.md) retain its proof. The [node-load study](../audits/recurring/storage-node-loads-2026-11-07.md) rejects all three earlier trials; those source changes remain reverted. Evidence stays under `target/evidence/storage-{sibling-load,split-load,split-owned}/`; isolate population/heap-page effects before more node-lifetime changes.
Released 0.68.8 selects Runtime 0.20.0 in both locks; released root Wire is 0.17.11 (unused opt-in bool adapter), benchmark remains 0.17.6. Composition selects Runtime directly; native production excludes Runtime/Types and Wasm selects IC0 only. No API/layout change.
Dependency graph/lint, native composition, pins/boundaries/formatting and installed original/O3/Os/Oz IO/upgrade/rollback/Logging pass. Evidence: `target/evidence/runtime-020-adoption/`, installed `target/qualification/runtime/attempt.AeDrGP/`; unchanged PocketIC pairing and ICSS supplier.

The [type catalogue](../audits/recurring/benchmark-full-catalogue-2026-11-05.md)
retains 73 types / 565 cases across five forms, 53 verified refusals and 18
non-Ord roles. Its earlier 46,604-call results are historical, not new timings.
All comparisons use an eight-MiB synthetic Wasm stack; production is unchanged.
Canic configuration preparation and Auth inner decoding remain outside scope.

## Historical evidence and preservation

The [measurement index](../measurements/README.md) owns compact evidence and its
retention limits. The authorized 2026-11-06 cleanup retired bulk/repeated output;
raw target artifacts remain transient and removable by explicit cargo clean.
No cache, stash, target or release/recovery evidence was removed in this pass.
CI group evidence remains in `target/ci-group-selection/` and
[the validation mapping](../storage-release-validation.md).

Preserve `storage-tooling-030-preserved-pending-lock-inputs`,
`storage-0670-preserved-pending-lock-inputs`, all `metrics-068-*` stashes,
`storage-0660-unselected-lock-edits`, `storage-0660-preserved-pending-inputs`,
`storage-0655-unselected-lock-edits`, `storage-0654-unselected-lock-edits`,
`storage-0653-unselected-lock-edits`, `storage-0653-hotspot-integration`, all other
preservation stashes, failed attempts, historical caches, receipts and archives.
Closed #153/#154 and publication evidence remain linked by the delivered handoff.

## Deferred work and boundaries

The [consumer inventory](../local-memory-audit.md) lists declarations, not live
usage. Canic receipt bounds and complete Auth endpoints remain unqualified.
Any adoption must explicitly replace old collection images and host framing.

The authorized IcyDB layout preparation exists only in `/tmp/icydb-control-layout`,
branch `storage/control-allocation`, base `03c5713e`. Preserve its complete
uncommitted tracked/untracked work; do not clean or continue implementation there.
Its 685 focused tests and native/Wasm lint passed; installed composition is unqualified.
Approval is pending for durable retention at `/home/adam/projects/icydb-control-layout`
or an authorized draft PR. Do not modify the sibling or create an unsolicited PR.
