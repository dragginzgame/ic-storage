# Current handoff — 2026-11-08

## Prepared: architectural hard cut

The approved reduction is implemented locally; no commit, version bump, release
or publication was performed. Pending notes select Storage 0.69.0, Metrics Storage
0.71.0 and Logging Storage 0.67.0. Both adapters and all workspace callers migrate
to direct collection results. External consumers require a coordinated adoption
of these breaking APIs; siblings remain untouched.

The [implementation and evidence](../audits/recurring/architectural-reduction-2026-11-08.md)
records each retained cut, validation and the rejected iterator experiment.
Current BTR2/BTN2, SVC1/SCL1, directory and checkpoint images are unchanged;
this batch requires no reset of current images. Older pre-0.68 collection images
remain retired as documented in the owning release notes.

All-five application Wasm is 147,911 bytes versus ICSS 119,378, saving 2,151
bytes against the retained delivered fixture. All 16 size artifacts pass installed
calls/upgrades. The inline-node iterator trial grew broad Wasm by 1,134,630
bytes and was reverted; the original shared-node iterator remains authoritative.

Native, compile-fail, adapter and benchmark tests, strict native lint, dependency
checks, formatting and final installed lifecycle qualification pass. The complete
565-case/51,768-call type comparison passes, including source/artifact checks.
Evidence: `target/evidence/storage-comparison/attempt.FAkSnk`. Release/archive
validation remains separate and has not been run for this pending batch.

Incoming root Cargo.lock edits are preserved exactly; neither lockfile was
changed by this implementation. Shared Tooling 0.31.12 (`eb3c8e0`) is adopted through the canonical exporter;
see the [adoption record](../shared-tooling-adoption.md). No dependency selection
changed during the refresh.

## Delivered: Storage 0.68.11

Published `40752edd62489cdafb57a8f54b7cbdcec9057cc0`, tag `storage-v0.68.11`, from `ba36b9572c8ca4f481c8403abc37977cfe0e6adb`; registry/download/receipt checksum `e1b2afe4d180c2f20dd05dd1f7505dc280ce6985e63d743da1afa141f1da842d` and clean archive VCS identity match.
Batch `batch-6VCf977D` passed all 12 stages and final archive qualification; publication and guarded local reconciliation passed. Evidence: `target/evidence/storage-06811-deploy/`; captured receipts under `target/release-frozen/batch-6VCf977D/target/release-validation/`, durable recovery under `.git/release-state/frozen/batch-6VCf977D/`.
Shared tree backing and redundant directory-index removal preserve API/images; consumer Map 134,314 bytes, all-five 150,062, with mixed instruction tradeoffs recorded in the owning release notes. Installed original/O3/Os/Oz lifecycle: `attempt.dZEIjv`. No adapter publication. Deferred Build 0.79.1, Host 0.75.1, Runtime 0.20.1 and Wire 0.17.14 selections are preserved at `preserve/storage-06811-deferred-dependencies`; release selects Build 0.79.0, Host 0.75.0, Runtime 0.20.0 and Wire 0.17.11. Restore deferred lock with only local Storage projected to 0.68.11.

## Delivered adapters and prior history

Metrics Storage 0.70.0 (`388c366`) and Logging Storage 0.66.0 (`7ac245c`)
remain the latest delivered adapter groups. Their publication and readback passed;
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
