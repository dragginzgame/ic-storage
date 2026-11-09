# Current handoff — 2026-11-09

## Authorized deployment in progress

The maintainer requested latest tooling adoption followed by deployment from this
repository. Toko Tooling 0.36.1 is installed from crates.io in both workspaces;
only its package selection changed during adoption. The selected supplier-first
batch is Storage 0.71.0, Metrics Storage 0.73.0 and Logging Storage 0.69.0, with
GitHub branch/tags and crates.io publication. Release requirements are unchanged.
Evidence: `target/evidence/deploy-storage-071/`. No sibling delivery is selected.
Do not report publication complete until the runner and registry/ref checks pass.

## Pending hard cut: Storage-owned bucket backing

The maintainer authorized copying ICSS allocation backing and adapting it to
Storage. Replace snapshot-directory persistence with the owned ICSS 0.7.2-derived
bucket implementation, retaining provenance, named opaque IDs, grants, custody,
exclusive leases and logical bounds. No production ICSS dependency is introduced.
Core Storage no longer has a normal IC Wire dependency. No commit, publication
or sibling edit has occurred; package versions remain unchanged. Pending release
notes are under Storage 0.71.0 because this is an incompatible allocation format.

Old whole-store images are rejected without writes. Hosts must explicitly retire
or reset them, or supply fresh backing. Remove `StorageConfig::wire` overrides.
Eight-page buckets replace snapshot extents; creation-time table capacities are
fixed. Defaults reserve 960 KiB of controls and permit 8 GiB of bucket data.
Growth writes owner/size/root controls without rereading the whole directory.
Explicit verification still admits active rows; IC rollback and native
fail-closed interruption retain distinct guarantees. See [the allocation
contract](../../crates/ic-storage/ALLOCATION.md).

Current qualification: 83 library tests, five runtime integration tests, 42 adapter tests, eight
ICSS image/IO tests, strict Clippy, production graph checks, eight installed size
selections and original/O3/Os/Oz installed IO/upgrade/rollback pass. Those upgrades
reopen the new image; they do not migrate a retired image. Depth covers 720 cases
and 4,320 samples, bulk 116 cases and 232 samples. All logical results match.

Small Cell Wasm falls 105,377 → 90,537 bytes; all-collection falls 148,075 →
133,445. Both remain larger than ICSS. Depth medians: 430 improve, 240 regress,
50 equal. Bulk: 41 improve, 68 regress, seven equal. Cell growth improves, but
shrink gains an approximately 80,000-instruction inner-loop charge; 64-KiB Vector
reads/scans also regress. Do not describe this as universally faster or ready
for deployment without reviewing these tradeoffs. Full-handler Cell probes show both growth cases improve and shrink is effectively
unchanged (-91 instructions); its extra inner-loop charge moves across the timer
boundary. Control/Get totals match and replacements differ by two instructions.
Vector bulk whole-handler attribution now passes 26 cases / 52 samples per run,
with exact plain-roundtrip and probe inner-count checks. Get/Pop are effectively
unchanged and Push improves, but 64-KiB iterator handlers genuinely regress by
1.4–2.8%. The 1-MiB read gap against ICSS predates this backing change. Read/allocator probes now confirm unchanged stable-read counts/lengths/costs
for those iterator cases, with additional costly dlmalloc metadata accesses
during and after the loop. Both 52-sample diagnostic runs match logical results
and pages; driver tests and Clippy pass. The attribution itself changes no production code.

The subsequent owned-buffer transfer in consuming `Iter::last` is retained: large
values improve by 32.6–41.7% inside the loop, with matching whole-handler gains;
scalar/array controls add 5/12 instructions per result. Other measured operations
are unchanged. Broad Wasm shrinks 2,647 bytes, small controls are byte-identical.
See [the terminal-buffer report](../audits/recurring/storage-vector-last-2026-11-09.md).
Fourteen native Vector tests, strict Clippy and installed size/call/upgrade checks
pass. General iterator buffers remain unchanged; forward/reverse scan attribution
is still separate work. Cold-open scaling, sibling endpoints and the full current type catalogue remain unqualified.
A state-alignment trial failed to remove the shrink charge and was discarded.

The [current report](../audits/recurring/storage-bucket-backing-2026-11-09.md)
contains the comparison matrix, frozen evidence and reproduction path. Keep all
failed trials and completed qualification attempts. No release command was run.

## Other preserved pending changes

- Cell encoded-length cache and combined B-tree overflow initialization remain.
  Their earlier isolated measurements are in the [Cell report](../audits/recurring/storage-cell-length-2026-11-09.md)
  and [overflow report](../audits/recurring/storage-overflow-init-2026-11-09.md).
  Those pre-cut results do not qualify the new backing or establish no regressions.
  The 73-type Cell catalogue has not been rerun after this backing cut.
- Both size consumers use ordinary crate-owned `src/app.rs` modules; their 49-line
  endpoints remain identical and the size script enforces equality. No path
  overrides, generated source or new helper crate remains. Current eight-pair
  installed size qualification includes this change.
- IC Build 0.81.0 and Host Artifacts/FS/Process 0.76.1 remain selected for native
  tooling, with the matching installed metadata editor. Earlier 88 helper tests,
  strict helper Clippy, Make dispatch and all three groups' offline bootstrap
  checks passed. Evidence: `target/evidence/build-081-host-0761/`.
  Incoming Logging 0.24.12, Runtime 0.20.3, itoa 1.0.19 and syn 3.0.10 lock choices
  are preserved. The backing cut removes only the core Storage → Wire edge from
  the independent lockfile; no dependency version was advanced by this cut.
- Bulk benchmark collection shards and existing full-handler probes now compose.
  Driver validation coverage, strict Clippy and formatting pass. This changes
  diagnostic admission only, not production endpoints.

## Delivered baseline

Storage 0.70.0, Metrics Storage 0.72.0 and Logging Storage 0.68.0 were published
from `batch-gI8lVjJG`. All required release gates, archive checks and registry/ref
verification passed. IC Wire 0.18 is selected in both workspaces; stored bytes
are unchanged. No publication needs replaying. Canonical release histories are
[Storage](../../releases/storage/CHANGELOG.md),
[Metrics Storage](../../releases/metrics-storage/CHANGELOG.md) and
[Logging Storage](../../releases/logging-storage/CHANGELOG.md).
Release evidence remains in `target/evidence/deploy-wire-018.log`,
`target/evidence/deploy-wire-018-verification/` and the retained frozen batch.
Earlier completed deliveries remain in Git history and their owning ledgers;
this handoff does not retire any recovery state or artifacts.

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
