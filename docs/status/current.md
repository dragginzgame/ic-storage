# Current handoff — 2026-11-09

## Pending: IC Wire 0.19 and Toko Tooling 0.38.2

Both product and qualification catalogs/locks select Wire 0.19.0 and exact
Toko Tooling 0.38.2. Only those two selections change from this adoption's input;
pre-existing Metrics/Logging updates and CI fixes are preserved. The seven
private packages live directly under `qualification/`, remain version 0.0.0,
and belong to no product release group. Product crate paths remain unchanged.
The authenticated snapshot contains 173 files. Pin checks now pass after scoped
intent-to-add for `qualification/Cargo.lock`; no contents are staged.

Wire's native codec code is unchanged; optional Platform schemas and packed
finite adapters are not selected. Public adapter codec errors use the new Wire
identity, so pending notes select Metrics Storage 0.74.0 and Logging Storage
0.70.0 for their eventual release. Product versions remain unchanged. No stored
checkpoint or collection format changes, reset or migration is needed.

Snapshot/pin/boundary/format/link checks, 42 adapter tests and five doctests,
44 native qualification tests, release-group recovery and substituted Make
callers pass. Installed IO, checkpoint, upgrade and rollback checks pass in
original/O3/Os/Oz modes. Evidence is retained at
`target/evidence/wire-019-toko-0382/` and
`target/qualification/runtime/attempt.922MAc/`. No commit, push or publication
is included. Earlier pending sections below record their original work and
qualification; the tooling selection and layout above supersede those values.

## Pending: Platform / Runtime adoption

Storage's Wasm backing selects Runtime 0.21 with `ic0` only; the independent
composition fixture selects Platform 0.28 / Types 0.19. Native production still
has no Platform/Runtime dependency. Platform removed its PocketIC facade, so
native qualification and benchmark callers now use Testkit 0.95's async client,
Tokio 1.54.1, explicit five-minute request budgets and awaited instance deletion.
The server remains PocketIC 16.1.0. Testkit's Platform 0.27 / Types 0.18 suppliers
remain native infrastructure; they are not the composition fixture's identities.
Root lock changes only Runtime; the independent lock removes the retired client
and resolves the required Testkit/Platform graph. Evidence is under
`target/evidence/platform-028/`; [qualification](../host-support.md#platform-028--runtime-021-adoption)
covers graph isolation, strict Clippy, 42 native tests, installed IO/upgrade/
rollback in four optimizer modes and eight size-consumer pairs. All-collection
Wasm remains 136,284 bytes; the frozen admission replay matches all 216 prior
observations. No product version, public Storage API or
persisted format changes; no commit, push or deployment is included.

## Pending: allocation admission optimization

Local changes remove temporary directory reconstruction during verification and
batch active metadata reads. No public API or persistent-format change. The
[admission report](../audits/recurring/storage-admission-2026-11-09.md) records
before/intermediate/final/ICSS results, small-case regressions, native integrity
checks and matched installed consumer qualification. Opening all 1,024 leases
with 32 buckets improves 54.2%. A follow-up reserves decoded rows and combines
name character scans: cold reopening improves another 0.33–1.99% in 17 cases,
with one case adding one instruction. It saves 113 bytes from the preceding
all-collection Wasm, leaving cumulative growth of 96 bytes versus the release.
A further compact index keeps names only in entries and sorts entry indices.
Matched cold reopen improves up to 48.3%; 1,024 names/32 buckets falls from
19.30M to 9.99M instructions. One-lease calls regress up to 0.81%; insertion
shifts indices, with measured reopen-plus-registration improving at larger
populations. The all-collection application grows 2,743 bytes to 136,284
(ICSS 119,326); this is a bounded speed–size tradeoff, not a size reduction.
All eight consumer pairs pass operations/two upgrades, 22 directory tests and
five runtime tests pass, and strict Clippy/formatting pass. No persisted-format
or public API change. The report records the expanded append matrix and limits.
Storage 0.71.1 notes are pending; no version bump, commit or publication occurred.

## Pending: Toko Tooling 0.37.5 adoption

Both workspace catalogs and lockfiles select published 0.37.5; this refresh
changes only Toko Tooling. The authenticated installer preserves 87 roots and
171 files, including active Candid checking and offline policy references.
The dependency policy/checker adds a narrow exception for unpublished Wire
benchmarks; Storage's Candid restrictions remain unchanged. Integrity, tool
admission, pins, Cargo/Candid boundaries, formatting and local links pass.
Rust 1.99.1,
Platform/Runtime adoption, product versions, release gates and pending
optimizations are preserved. See
[adoption evidence](../shared-tooling-adoption.md#current-cargo-owned-0375-selection)
and `target/evidence/toko-0375/`. No commit, push or deployment was performed.

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
- Admission/reopen scaling is now measured, including the compact name index;
  retaining a full check per lease still leaves quadratic all-lease admission.
  Any further cut must preserve its trust boundary.
- The native Map/region control confirms fewer reads with equal bytes/decodes
  than ICSS on the same backing. Target backing-call overhead and allocation
  effects rather than reverting the combined-read engine improvements. Both full
  bulk runs pass 116 cases. Map's 1.30% median gap becomes 0.004% with common
  backing. Region retains 2.14%, but its ICSS runtime control lacks equivalent
  namespace bounds; qualify a bounded reference before treating that residual
  as an engine regression.
- Shared Wasm remains larger than ICSS. The compact index trades 2.7–2.8 KB for
  substantial cold-open savings; no separate size reduction is qualified.
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
