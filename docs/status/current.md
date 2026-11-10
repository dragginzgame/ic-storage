# Current handoff — 2026-11-10

## Active work and readiness

Toko Tooling 0.39.7 is adopted in both Cargo catalogs/locks and the authenticated
snapshot. Only Toko changed in those locks. The 39 selected entrypoints install
102 files; reference docs remain upstream. Both workspace catalogs own
`license-file`, inherited by all ten packages. Snapshot/member admission, pins,
production graphs, boundary, canonical licence and documentation checks pass.
See [the adoption record](../adoption/storage-tooling.md#current-cargo-owned-0397-selection).

The independent implementation now covers Memory, named backing, Cell, Vector,
Map, Set and regions. The copied ordered engine and its private codec/address
helpers are deleted. ICSS remains an external qualification comparison only.
The source-exposed coordinator supplied behavioral contracts and integration;
separate unexposed implementers authored replacement algorithms. Actual inputs
and limitations are recorded in the [replacement inventory](../design/independent-storage-replacement.md).

The maintainer has requested deployment of the independent replacement with its
recorded performance tradeoffs. The refined implementation
passes correctness checks and reduces application Wasm, but larger reads/scans
and small-record capacity still regress. It packs small payloads inline, retains
iterator paths and checks cached child metadata during traversal. Preserve all
candidate measurements; do not remove integrity checks to recover performance.
The current deployment covers Storage 0.73.0 and its Metrics Storage 0.75.0 and
Logging Storage 0.71.0 adapters only. Release gates and publication are pending;
no sibling edits or deployed-canister resets are included.

The earlier `target/` artifacts are no longer present at release preparation.
The figures below are recorded historical results; required release validation
and archive proofs must be regenerated. The current root lock additionally
selects IC Metrics 0.29.11 and IC Logging 0.24.16.

## Qualification and measured limits

The final ordered refinement passes 146 workspace library tests, 35 selected
public collection/backing contracts, 28 comparison-library tests and strict
native/Wasm Clippy. Eight paired application consumers pass 118 calls and two
upgrades each. Installed original/O3/Os/Oz IO, custody, logging/checkpoints,
upgrade and rollback pass at `target/qualification/runtime/attempt.Dh67gD`.
An incompatible ordered-image upgrade traps, preserves the previous application,
and permits a fresh installation. First-candidate archive qualification passed at
`target/package-qualification/attempt.n5kPft` for all three product packages.
Those archives predate the refinement; release archive gates remain separate.

| Final ordered refinement | Previous Storage | Candidate | ICSS |
| --- | ---: | ---: | ---: |
| All-collection raw Wasm bytes | 141,342 | 126,406 | 119,318 |
| All-collection code-section bytes | 121,771 | 108,523 | 77,127 |
| 4,096-record Map Get×32 instructions | 934,522 | 1,145,264 | 914,982 |
| 4,096-record Map Scan×32 instructions | 326,047 | 773,764 | 309,902 |

These are matched application sizes and uniform u64 metered inner-operation
scopes, not whole-application throughput. The 752-row depth comparison passes
state checks; 265 rows regress versus preceding Storage. All ICSS instruction
controls match. Small-record Map capacity rises from 26 to 34 pages. See the
[ordered-engine record](../design/ordered-map-replacement.md) for identities,
full size matrix, scope and retained failures.

Earlier [Cell](../design/cell-replacement.md),
[Vector](../design/vector-replacement.md),
[Memory](../design/memory-replacement.md) and
[backing](../design/backing-replacement.md) records retain their independent
qualification, size/performance tradeoffs and reproduction commands. Their
historical results do not qualify changed ordered-engine inputs. The final frozen
modules pass the complete 565-case catalogue: 54,318 sample observations across
all five collection forms, with expected refusals and unpaired results recorded
separately. Source/artifact hashes pass after the runs. Native attribution records
fewer Map reads but twice the touched stable-address intervals versus ICSS in the
selected 4,096-record case; exact IC fee attribution remains unproven.

## Persistence and release boundaries

Hosts must explicitly provide fresh/reset whole backing and update application
framing admission. Current formats are STORNAM1, STORCELL, STORVECT and STORMAP1,
version 1. Previous images refuse without writes; no migration reader, automatic
reset or production ICSS fallback exists. The destructive reset is authorized
for this implementation; no deployed consumer has been migrated here.

Small-record density and traversal footprint remain follow-up optimization work;
the measured regressions are retained in the release notes. Apply the authorized
release-group gates to current inputs. Current version authorities remain the three product
packages and their group ledgers. Hosted CI and publication are separate from
local checks. Earlier pre-rewrite optimization measurements remain historical
in the [measurement index](../measurements/README.md), not active engine claims.

## Preservation and external boundaries

Keep failed attempts, frozen comparison/release artifacts, archives, receipts,
`.git/release-state/`, historical tool caches and all existing preservation
stashes. Raw experiment evidence remains under `target/evidence/`; ordinary
`cargo clean` can remove it. Compact decisions and reproduction belong in the
linked records. No cleanup or sibling work was performed.

Canic/Auth complete endpoint costs remain outside Storage qualification. Canic
and IcyDB have unrelated pending work. The authorized IcyDB preparation remains
at `/tmp/icydb-control-layout`, branch `storage/control-allocation`, base
`03c5713e`; preserve it unchanged. Its native checks passed, installed composition
is unqualified, and durable relocation or draft-PR approval remains pending.
Do not modify that sibling or treat its pending delivery as part of this batch.
