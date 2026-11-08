# Current handoff — 2026-11-08

## Pending delivery: Storage 0.70.0 and adapters

Retain one byte-identical 15-byte node prefix write, with encoding shared across
key types. Relative to delivered 0.69.1, 168/720 depth cells improve, 552 are equal
and none regress; all 4,320 logical results match. Broad harness Wasm shrinks
7,461 bytes; the small all-collection consumer grows 105 bytes. Public APIs,
images and dependency selections were unchanged in that isolated experiment. Native tests (92 passed, one
manual ignored), strict Clippy, seven ICSS interchange tests, all eight installed
size probes and original/O3/Os/Oz IO/upgrade/rollback qualification pass.
See [the comparison and reproduction](../audits/recurring/storage-prefix-write-2026-11-08.md).
IC Wire 0.18.0 is selected in both maintained workspaces, changing public Wire
type identities but not stored bytes. Both catalogs already explicitly select
Serde derive; no retired bulk-lint calls exist here. Preserve the incoming root
Metrics 0.29.7 lock selection. Select minor releases Storage 0.70.0, Metrics
Storage 0.72.0 and Logging Storage 0.68.0 in supplier-first order. Full release
gates remain required. Fresh installed original/O3/Os/Oz IO, upgrade and rollback
qualification with Wire 0.18.0 passes (`target/qualification/runtime/attempt.KjxWiP`).
All eight installed small-consumer comparisons also pass with Wire 0.18.0
(`target/evidence/storage-consumer-size/attempt.vvN7pa`): every Storage Wasm
size is unchanged from the prefix-write candidate; all-collection remains
147,875 bytes versus ICSS 119,378. Admission, production graph, pin, formatting
and documentation checks pass. Neither Platform nor PocketIC selection changed.
The separate overflow-initialization experiment has not started.

## Delivered: Storage 0.69.1 and adapters

Batch `batch-WAJbiFwP` completed from reviewed source
`ae1435c2cc0e9db6288e0c65697f29d1ed5b6d38`, releasing and publishing
`storage,metrics-storage,logging-storage` in supplier-first order. All three
groups passed all 12 required validation stages and final archive qualification.
The remote main branch and annotated tags were independently checked. Downloaded
crates.io archives match both registry checksums and final receipts; embedded
clean VCS identities match release commits and none of these versions is yanked.

| Package | Version | Release commit | Archive SHA-256 |
| --- | --- | --- | --- |
| ic-storage | 0.69.1 | `502d18ca5749701041fc8828cef6955d32ae6e0d` | `66ae72c144ab3d906f08884e2173dd54754b10f8c35c5e31986b7d52fe85da4d` |
| ic-metrics-storage | 0.71.1 | `4d227287ac323f6c8a3782aff9c0d6ed937dfbcc` | `1d71f6667b51eb0accd3eb520fb6491edf0263be6d00f32facfff2e8876d43ff` |
| ic-logging-storage | 0.67.1 | `5610f9018227bee1a9bc4592f6ae462b48992d33` | `72d61a152dcffe9e847b5cebe1ef576faffa2d38bd2b8591db21412ff779ab73` |

Normal `main` was reconciled through `make release-batch-reconcile`; no deferred
working changes remained. The batch checkout, durable recovery state, validation
logs and archives are retained. No sibling repository was changed or deployed.

Delivered changes:

- Combined B-tree prefix reads and shared validation, plus one direct child-array
  reader without the inherited small-node threshold. Both depth studies improved
  552/720 cells with no regressions. The broad harness savings total 30,841 bytes
  before naming cleanup; the final small all-collection consumer is 147,770 bytes,
  141 bytes below the initial 147,911-byte baseline. See
  [the measured scope and ICSS comparisons](../audits/recurring/storage-shared-prefix-2026-11-08.md).
- Role-based engine naming (`node/layout`, direct `new`/`load`/`save`, `Header`,
  `HEADER_SIZE`, `clear`) without public API or persisted-discriminator changes.
  Native/interchange tests and all eight installed size comparisons passed.
- Cargo-owned Toko Tooling 0.36.0 across all ten real packages and both maintained
  workspaces. Its authenticated snapshot retains all 83 entrypoints. The separate
  tooling lock and metadata selection are removed. Common CI/release and custom
  deployment entrypoints admit the root snapshot; `check-pins` additionally checks
  independent-workspace member declarations and matching selection.
- IC Host Artifacts/FS/Process 0.76.0 with IC Build 0.80.0 and its matching installed
  metadata CLI. Native tooling has one Host family. Previously pending compatible
  root-lock Runtime 0.20.2 and Wire 0.17.14 selections are included and qualified;
  the independent runtime workspace retains its existing registry selections.

No storage migration or destructive reset is required. Instruction figures are
inner-loop benchmark results, not whole-call throughput. Storage's small-consumer
Wasm remains larger than ICSS; no universal performance or size parity is claimed.
Development installed original/O3/Os/Oz IO/upgrade/rollback qualification preceded
the final naming-only change; naming was checked by native/interchange and installed
size tests. Release gates passed without bypasses; no new hosted CI claim is made.

Evidence: `target/evidence/deploy-0691.log`,
`target/evidence/deploy-0691-verification/`, and
`target/release-frozen/batch-WAJbiFwP/target/release-validation/`.
Durable batch intent/Git state remains in `.git/release-state/frozen/batch-WAJbiFwP`.
Earlier experiment, tooling-adoption and failed Host-adoption evidence is retained.

## Historical delivery: Toko Tooling 0.34.0

Commit `4c1ae8be4111ea2edf25453614108cb2b63a0c04` delivered the previous
metadata/lock scheme to origin/main without publishing product crates. That
selection scheme is superseded by the pending 0.36 adoption above. Evidence
remains in `target/evidence/toko-tooling-034/` and
`target/evidence/toko-tooling-034-delivery/`; prior failed migration evidence is
also retained. Deferred unrelated dependency updates were excluded from delivery.

## Previous delivery: Storage 0.69 and adapters

Batch `batch-T85dkYEj` released and published all three groups from source
`13826544bebcc2f5043aaee65291477570ea26a0`, in supplier-first order.
Each group passed all 12 validation stages and final archive qualification.
Registry status, downloaded archive checksum and embedded clean VCS identity
were independently checked against each final receipt; none is yanked.

| Package | Version | Release commit | Archive SHA-256 |
| --- | --- | --- | --- |
| ic-storage | 0.69.0 | `2f0346a875775a2a7b8b47ac0e47f1a43daaf944` | `90b93d6a307e67f7a83ea9de7046bd6a26e405957ada63679997d91024053241` |
| ic-metrics-storage | 0.71.0 | `db43ce8c3a7380667404ed88d6f20871fe5a03b0` | `d853a3a5e3c01caf193f2ff33f59fba2fcb78f481d81b03afcd28eb0f2c571f8` |
| ic-logging-storage | 0.67.0 | `6f1b7a23ef3712560311d4921d370ef3a51c9b06` | `02ae6240c89bc9596a34088cdc9a815df3130403b1b40ab355f4c8e8c4b5f840` |

Tags are `storage-v0.69.0`, `metrics-storage-v0.71.0` and
`logging-storage-v0.67.0`. Git delivery to `origin/main`, crates.io publication
and guarded normal-checkout reconciliation completed. No publication needs
replaying. Hosted CI remains separate from the local release qualification.

Evidence: `target/evidence/storage-069-deploy/`; captured receipts and archives:
`target/release-frozen/batch-T85dkYEj/target/release-validation/`; durable recovery:
`.git/release-state/frozen/batch-T85dkYEj/`. Nothing was retired or cleaned.
Failed pre-version batches `batch-JW_u9vag` (stale Make fixture expectation) and
`batch-zQCCYGgu` (native-only IO fixture imported by Wasm tests) remain intact.
Both defects were fixed in normal source and qualified before the final batch.
Neither failed batch created a version commit, tag, push or publication.

The [architectural reduction](../audits/recurring/architectural-reduction-2026-11-08.md)
ships direct collection results, canonical memory/bounds contracts, one Vector
engine and the consolidated benchmark catalogue. Current BTR2/BTN2, SVC1/SCL1,
directory and checkpoint images are unchanged; no reset of current images is
required. Consumers must migrate the breaking Rust API. Shared Tooling 0.31.12
(`eb3c8e0`) is included. The rejected inline-node iterator is not included.

The all-five application fixture is 147,911 bytes versus ICSS 119,378, saving
2,151 bytes against the retained prior Storage fixture. The 565-case/51,768-call
comparison and installed lifecycle checks passed before release; the owning
study records scope and mixed instruction results. No universal speedup is claimed.

Unrelated dependency updates are restored locally, verified identical to
`preserve/storage-0690-deferred-dependencies` except the three released local
package versions. The stash and durable branch remain. These pending Build/Host,
Runtime and Wire selections were not delivered: release selected Build 0.79.0,
Host 0.75.0, root Runtime 0.20.0 and Wire 0.17.11. The normal Cargo.lock remains
intentionally dirty for the next adoption.

## Delivered: Storage 0.68.11

Published `40752edd62489cdafb57a8f54b7cbdcec9057cc0`, tag `storage-v0.68.11`, from `ba36b9572c8ca4f481c8403abc37977cfe0e6adb`; registry/download/receipt checksum `e1b2afe4d180c2f20dd05dd1f7505dc280ce6985e63d743da1afa141f1da842d` and clean archive VCS identity match.
Batch `batch-6VCf977D` passed all 12 stages and final archive qualification; publication and guarded local reconciliation passed. Evidence: `target/evidence/storage-06811-deploy/`; captured receipts under `target/release-frozen/batch-6VCf977D/target/release-validation/`, durable recovery under `.git/release-state/frozen/batch-6VCf977D/`.
Shared tree backing and redundant directory-index removal preserve API/images; consumer Map 134,314 bytes, all-five 150,062, with mixed instruction tradeoffs recorded in the owning release notes. Installed original/O3/Os/Oz lifecycle: `attempt.dZEIjv`. No adapter publication. Deferred Build 0.79.1, Host 0.75.1, Runtime 0.20.1 and Wire 0.17.14 selections are preserved at `preserve/storage-06811-deferred-dependencies`; release selects Build 0.79.0, Host 0.75.0, Runtime 0.20.0 and Wire 0.17.11. Restore deferred lock with only local Storage projected to 0.68.11.

## Earlier adapter history

Metrics Storage 0.70.0 (`388c366`) and Logging Storage 0.66.0 (`7ac245c`)
were the previous adapter baseline. Their publication and readback passed;
no publication needs replaying. Earlier delivery records remain in Git history,
owning release notes and retained receipts/recovery state.

## Evidence and preservation

The [measurement index](../measurements/README.md) owns compact experimental
summaries. Raw target evidence remains transient; no cleanup was performed.
Preserve all failed attempts, frozen release batches, `.git/release-state/`,
archives, receipts and historical tool caches. Closed #153/#154 evidence remains
in historical delivered handoffs. Hosted CI is separate from local qualification.

Preserve `storage-tooling-030-preserved-pending-lock-inputs`,
`storage-0670-preserved-pending-lock-inputs`, all `metrics-068-*` stashes,
`storage-0660-unselected-lock-edits`, `storage-0660-preserved-pending-inputs`,
`storage-0655-unselected-lock-edits`, `storage-0654-unselected-lock-edits`,
`storage-0653-unselected-lock-edits`, `storage-0653-hotspot-integration`, all other
preservation stashes, failed attempts, historical caches, receipts and archives.
Closed #153/#154 and publication evidence remain linked by the delivered handoff.

## Deferred work and boundaries

The [consumer inventory](../local-memory-audit.md) lists declarations rather than
live usage. Complete Canic/Auth endpoint costs remain outside Storage qualification.
Canic has unrelated pending work; IcyDB has independent release work. Do not
modify either as an incidental part of this Storage batch.

The authorized IcyDB layout preparation exists only in `/tmp/icydb-control-layout`,
branch `storage/control-allocation`, base `03c5713e`. Preserve its complete
uncommitted tracked/untracked work; do not clean or continue implementation there.
Its 685 focused tests and native/Wasm lint passed; installed composition is unqualified.
Approval is pending for durable retention at `/home/adam/projects/icydb-control-layout`
or an authorized draft PR. Do not modify the sibling or create an unsolicited PR.
