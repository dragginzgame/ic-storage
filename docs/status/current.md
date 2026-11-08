# Current handoff — 2026-11-08

## Delivery preparation: 0.69.1 / 0.71.1 / 0.67.1

The maintainer authorized this repository's deployment to origin/main and
crates.io. Select the supplier-first `storage,metrics-storage,logging-storage`
patch batch. Include the reviewed B-tree optimizations and naming cleanup,
Toko Tooling 0.36 and IC Host 0.76 / IC Build 0.80. Root lockfile selection also
includes the previously pending Runtime 0.20.2 and Wire 0.17.14 compatible patches;
full product validation applies. No sibling changes or destructive migration.
Final delivery identities and publication receipts must be verified after the
shared frozen-batch runner completes. Earlier pending sections below describe
pre-release development evidence, not a claim of completed publication.

## Pending: engine naming cleanup

Remove inherited version-labelled engine modules and methods: `node/v2` becomes
`node/layout`, load/save forwarding methods are removed, and `new_v2`,
`HeaderV1`, `HEADER_V1_SIZE` and `clear_new` become `new`, `Header`, `HEADER_SIZE`
and `clear`. Remove obsolete ICSS 1.0 TODOs and the unused old-layout diagram.
Storage's pre-1.0 API is not named after ICSS's format history. Keep persisted
format discriminators unchanged; no destructive migration was requested.
Upstream file names/checksums remain in provenance, and historical ICSS findings
retain the names of the implementation actually measured.

Native tests (91), strict Clippy, ICSS image interchange (7), formatting and
documentation checks pass. All eight small-consumer installed checks pass;
all-collection Wasm grows by 9 bytes (maximum individual selection +16 bytes).
No new timing claim; this is naming and module cleanup. Not deployed.
Evidence: `target/evidence/storage-engine-naming/`.

## Pending: Cargo-owned Toko Tooling 0.36.0

Both maintained workspace roots select `toko-tooling = "=0.36.0"`. All ten real
packages inherit it as an unconditional dev-dependency with a cargo-machete
exemption. Cargo added only that package and the ten member edges, preserving
all existing versions, checksums and unrelated pending lockfile work.
The obsolete metadata table and separate tooling lock are deleted; the previous
selection-only package was already absent. Real tools and qualification packages
remain. No artificial imports, aliases, source overrides or sibling edits.

The producer bootstrap installed crates.io payload SHA-256
`de0dd05b6437061622a76942baa25d40d2181497e7ba66d9fd809132a3082936`:
170 owned files, with existing entrypoints retained, and installed tools admitted.
`shared-tooling-check` admits root membership and snapshot identity before common
CI/release and custom deployment effects. Consumer `check-pins` also admits all
seven independent-workspace members and requires the two Cargo selections to
agree. Two documented pin exceptions satisfy the generic compatible-range checker
without modifying snapshot-owned policy or checkers.

Validation passes: installed snapshot/tools and all ten member declarations,
83 preserved entrypoints, both lockfiles' unrelated-selection preservation,
normal/build native and Wasm production graphs, dependency pins, documentation
links and formatting. Consumer release dispatch/failure propagation and all
three actual native/shared release file plans pass. A substituted mismatched
independent-workspace selection refuses before pin checks. The initial fixture
failure is retained; the independent admission belongs in `check-pins`, preserving
the canonical shared dispatch fixture unchanged. Full product release and
installed runtime qualification were not rerun for this tooling-only migration.

Adoption evidence: `target/evidence/toko-tooling-036/`. Product compiler, package
versions and production dependency graphs remain unchanged. No deployment or
publication is included; product release qualification remains required.

## Pending: IC Host 0.76 and compatible IC Build 0.80

Adopt published `ic-host-artifacts`, `ic-host-fs` and `ic-host-process` 0.76.0
with IC Build 0.80.0. The root catalog and lock select one Host family; the
release-metadata CLI selection in `ci/release-tools.env` is 0.80.0 and its
prepared executable is admitted. No compatibility adapters or aliases are needed.
The earlier IC Build 0.79.1 public-type blocker is resolved by 0.80.0.

The lock changes only these suppliers and IC Build's required HMAC 0.13 / SHA-2
0.11 dependency closure. Existing SHA-2 0.10 remains where required. Other pending
selections, Toko Tooling 0.36 and the independent runtime lock are preserved.
These are native test/tool dependencies, excluded from product normal/build
Wasm graphs. No product API or persisted-state change; package versions are unchanged.

Validation passes: all 88 native repository-helper tests, strict helper Clippy,
all three native/shared release-group file-plan comparisons, installed release
tool admission, shared snapshot/member admission, dependency pins, native/Wasm
production graphs and formatting. Full release qualification and installed runtime
benchmarks were not rerun for this native-tool-only dependency change.

Evidence: `target/evidence/ic-host-076-adoption/`; the failed earlier attempt
remains under `target/evidence/ic-host-076/`. No deployment or publication.

## Pending: shared B-tree prefix and child-array readers

Retain combined 15-byte node-prefix IO with key-independent outlined validation
and overflow traversal. No API, stored-image or allocation changes. Matched depth
comparison: 552/720 cells improve, 168 unchanged, none regress; broad harness
Wasm -16,278 bytes, all-collection small consumer +41 bytes. Native tests (91),
ICSS image interchange (7), strict Clippy and installed original/O3/Os/Oz
IO/upgrade/rollback pass. Two lease-index alternatives were measured and reverted.
See [the evidence and reproduction commands](../audits/recurring/storage-shared-prefix-2026-11-08.md).
Follow-through removes the inherited small-child threshold, using the existing
single-allocation address reader for every internal node. Relative to the prefix
step: 552/720 cells improve, none regress, up to 5.59%; broad Wasm -14,563 bytes,
small all-collection consumer -191 bytes. The same native, Clippy, interchange,
size and original/O3/Os/Oz lifecycle qualification passes for the combined code.
Combined broad Wasm saving is 30,841 bytes; small all-collection saving is 150 bytes.
Not committed or deployed; the unrelated incoming root lockfile remains unchanged.
Full release qualification remains required before publication.

## Historical delivery: Toko Tooling 0.34.0

Commit `4c1ae8be4111ea2edf25453614108cb2b63a0c04` delivered the previous
metadata/lock scheme to origin/main without publishing product crates. That
selection scheme is superseded by the pending 0.36 adoption above. Evidence
remains in `target/evidence/toko-tooling-034/` and
`target/evidence/toko-tooling-034-delivery/`; prior failed migration evidence is
also retained. Deferred unrelated dependency updates were excluded from delivery.

## Delivered: Storage 0.69 and adapters

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
