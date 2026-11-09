# Current handoff — 2026-11-09

## Delivered: Storage 0.71 and Toko Tooling 0.36.1

The authorized supplier-first batch `batch-JGNkyhL2` completed from source
`14c7342629aa2f7d1539dafe3be5b3a21250d16a`. GitHub main/tags and crates.io
publication are verified for all three groups:

| Package | Version | Release commit |
| --- | --- | --- |
| Storage | 0.71.0 | `8ae045a4f2dff37261cda0ae99095fd4c032e936` |
| Metrics Storage | 0.73.0 | `306fefc8253a831a82566df9a1cfa2eb0d169181` |
| Logging Storage | 0.69.0 | `7e73cf3718fdb6a3b9c605f51b794528207ccc8e` |

All 12 release stages passed for each group, followed by final archive integration
and publication qualification. Independent crates.io index checks found all
versions non-yanked with checksums matching retained release receipts. Remote
annotated tags resolve to the expected commits; both adapters publish Storage
`^0.71.0` requirements. The clean normal checkout was fast-forwarded through the
supported batch reconciliation flow. No publication needs replaying. Hosted CI
remains separate from these local release proofs.

Toko Tooling 0.36.1 is installed from crates.io in both maintained workspaces.
Exact pins, all ten inherited package declarations, Cargo locks and the 170-file
snapshot agree. The adoption changed only Toko Tooling's lock selection and
updated its existing exact-pin exceptions. Its executable patch corrects CLOC
test attribution inside call-argument closures; local engine CLOC, snapshot,
pin, formatter and documentation checks passed. Rust 1.99.1 remains selected.
Previously pending Build 0.81.0, Host 0.76.1 and incoming lock selections are
included in the delivered source. No sibling repository was changed or deployed.

Evidence: `target/evidence/deploy-storage-071/`, including `release.log`, batch
status/reconciliation, independent registry/ref/checksum records and all 36
stage results under `verification/`. The managed checkout and durable recovery
state remain at `target/release-frozen/batch-JGNkyhL2` and
`.git/release-state/frozen/batch-JGNkyhL2`; they have not been retired or cleaned.

## Delivered contract and measured tradeoffs

Storage owns the ICSS 0.7.2-derived bucket backing, names, opaque IDs, grants,
custody, exclusive leases and collection engines. No production ICSS dependency
is introduced; core Storage no longer has a normal IC Wire edge. Existing
snapshot-directory whole-store images are rejected without writes. Hosts must
explicitly retire/reset backing or supply fresh backing, and remove
`StorageConfig::wire` overrides. Eight-page buckets and fixed creation-time
capacities replace snapshot extents. Default controls reserve 960 KiB and permit
8 GiB of bucket data. See [the allocation contract](../../crates/ic-storage/ALLOCATION.md).
Adapter checkpoint encodings are unchanged; that does not make old whole-store
backing attachable. Installed upgrade proofs reopen the current format, not the
retired image.

The [bucket report](../audits/recurring/storage-bucket-backing-2026-11-09.md) retains
720 depth cases, 116 bulk cases and whole-handler/allocator attribution. Small
Cell Wasm falls 105,377 → 90,537 bytes and all-collection 148,075 → 133,445;
both remain larger than ICSS. Gains are workload-dependent. Cell shrink and
Vector Get/Pop inner-loop regressions mostly move costs across the timer boundary;
64-KiB iterator handlers retain genuine allocator-placement costs. Do not claim
universal parity or throughput improvement.

The delivered [terminal-buffer change](../audits/recurring/storage-vector-last-2026-11-09.md)
transfers consumed Vector `last()` bytes. Large-value inner counts improve
32.6–41.7%, confirmed by whole-handler savings; scalar/array controls add 5/12
instructions per result. Other measured operations are unchanged. Broad Wasm
shrinks 2,647 bytes; small-consumer controls are byte-identical. General iterator
buffer reuse remains unchanged. Earlier isolated [Cell cache](../audits/recurring/storage-cell-length-2026-11-09.md)
and [overflow initialization](../audits/recurring/storage-overflow-init-2026-11-09.md)
experiments are included; their pre-backing comparisons are historical controls.
Both maintained size consumers now use ordinary modules and retain their source
equality check.

## Next work

- Investigate general Vector iterator buffer ownership with multi-item/multi-type
  controls; do not infer a blanket buffer-removal policy from consuming `last()`.
- Measure allocation admission/reopen scaling with real active rows and buckets.
- Rerun the complete type catalogue against the new backing; earlier 73-type Cell
  results predate it. Sibling endpoint costs remain outside these measurements.
- Consumer adoption requires explicit backing retirement/fresh-state decisions;
  no automatic migration or sibling deployment was performed here.

The previous Wire 0.18 delivery (`batch-gI8lVjJG`, Storage 0.70.0 / Metrics 0.72.0 /
Logging 0.68.0) remains in canonical release history and its retained evidence.
Do not replay or retire that batch as part of this completed delivery.

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
