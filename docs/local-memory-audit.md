# Local consumer memory audit — 2026-11-03

This is a source audit of local working trees, not a measurement of deployed
canisters. Siblings were read-only. Many have pending migrations; their current
declarations must not be confused with their published versions or selected
lockfiles. No consumer layout, public API or retained image was changed.

Some inspected files were uncommitted and have since moved or disappeared.
Unavailable source paths are recorded as text, not live links. Inventory hashes
identify the inspected bytes but cannot verify an absent snapshot; re-inspect
consumers before making current layout or sizing decisions.

## What a memory count means

The inspected current consumers use Storage-issued named allocations. These
share a canister's physical stable backing; they are not separate Wasm heaps
or ICSS MemoryManager buckets. Storage records new names with no data extents
and grows by the requested number of 64 KiB pages. Directory metadata also
occupies real pages. See [directory construction](../crates/ic-storage/src/runtime/directory_model.rs)
and [physical accounting](../crates/ic-storage/src/runtime/directory_model/persistence.rs).

A declaration can remain at zero data pages. Opening/initializing a collection
and writing application data are separate events. Consequently, multiplying
these inventories by ICSS's default 128-page (8 MiB) bucket is incorrect.
Likewise, summing components without considering canister roles double-counts
mutually exclusive deployments.

## Verified inventories

| Component | Named allocation inventory | Implementation and actual use |
| --- | --- | --- |
| Canic core | 27 unconditional, plus up to 7 feature-selected | [Key/request factory](../../canic/crates/canic-core/src/memory/keys/mod.rs): three optional auth-state keys, two portable enrollment keys and two sharding keys. [Memory bootstrap](../../canic/crates/canic-core/src/memory/mod.rs) admits the complete inventory; intent stores (`canic/crates/canic-core/src/storage/stable/intent.rs`) initialize lazily. |
| Canic control plane | Root adds 21; fleet coordinator adds 3; Wasm store adds 6 | [Role-gated keys](../../canic/crates/canic-control-plane/src/memory/keys/mod.rs): 17 root-only, 3 fleet-only, 2 Wasm-only, 4 shared root/Wasm. [Composition](../../canic/crates/canic-control-plane/src/memory/mod.rs) adds these to core. Combined feature builds use the union, not the sum. |
| Canic metrics | 0 or 1 additional allocation per role | [Provider host](../../canic/crates/canic-core/src/ops/runtime/metric_providers/host.rs) declares one shared metrics map only when a compiled role provider needs stable storage. It is not one memory per metric. |
| Blob Storage | 14 per installation | [Thirteen service stores](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs): ten upload, two funding, one gateway; [installation](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/installation/mod.rs) adds configuration. [Canic adapter](../../canic/crates/canic-blob-service/src/ops/memory/mod.rs) composes these with its framework inventory. |
| Relay | 3 per channel; +1 per responder, dispatch, mailbox or durable allocator instance | [Channel](../../ic-relay/crates/ic-relay/src/channel/mod.rs) creates stream/outbox/inbox maps; [responder](../../ic-relay/crates/ic-relay/src/responder/mod.rs), [dispatch](../../ic-relay/crates/ic-relay/src/dispatch/mod.rs), [mailbox](../../ic-relay/crates/ic-relay/src/mailbox/mod.rs), and [resources](../../ic-relay/crates/ic-relay/src/resources/mod.rs) own the additions. The actual two-peer qualification host (`ic-relay/crates/ic-relay-persistence-probe/examples/mailbox_peers/installed/storage.rs`) declares 18: four shared plus seven per peer. This is a fixture count, not a universal Relay count. |
| IcyDB | `3 + 4 × journaled stores` per generated canister | [Generator](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) adds commit, integrity progress and startup, then data/index/schema/journal per journaled store. Heap stores add none. [Facade fixture](../../icydb/testing/model-facade-only/src/design/mod.rs) has one journaled store (7); [logical-memory fixture](../../icydb/testing/logical-memory/src/schema.rs) has two (11) unless its retiring-store omission feature is enabled. |
| Metrics Storage / Logging Storage | One host-supplied map per store instance | [Metrics](../crates/ic-metrics-storage/src/lib.rs) and [Logging](../crates/ic-logging-storage/src/lib.rs) wrap supplied governed maps; they do not construct independent managers. Hosts decide how many instances exist. |
| Auth / Scheduling | No fixed production allocation count imposed by the library | Their qualification consumers use explicit host memories; those fixtures are not allocations automatically created by linking the libraries. |

The scan also covered ic-backup, ic-crates, ic-observability, ic-profiler,
ic-query, ic-replica-watch, ic-tooling, ic-wire and plex. No additional active
ICSS manager/bucket factory was found in the scanned production sources.
This does not establish that every host has no durable state. Plex has no root
Rust workspace. Toko and Toko Miner were excluded under their scope restrictions.

The local [ic-crates registry](../../ic-crates/apps/ic_crates/registry/src/design/mod.rs)
declares one journaled store, but its root lock
still selects Storage 0.56.4 while pending catalogs select newer Canic/IcyDB.
Do not present the current IcyDB generator's seven-allocation result as a
qualified ic-crates build. The refreshed locks select Storage 0.65.5 for Canic, 0.65.4 for Relay, and
0.66.0 for Auth, Blob, Scheduling and IcyDB. Their working trees may contain later
work; no lock selection alone qualifies those consumer builds.

## 1. Remove IcyDB's forced commit-buffer prefix on a future layout change

**Verified:** [schema control](../../icydb/crates/icydb-core/src/db/schema/control_store.rs)
places its single shared control map at page `MAX_COMMIT_BYTES / 65_536 + 1`,
which is 257. [Commit marker limits](../../icydb/crates/icydb-core/src/db/commit/marker.rs)
set `MAX_COMMIT_BYTES` to 16 MiB. `control_memory()` returns the commit allocation.
Initializing the map reserves space at that offset; Storage's
[region growth](../crates/ic-storage/src/adapter/region.rs) grows the underlying
allocation through the entire prefix. It is not a sparse virtual reservation.

**Consequence derived from the code:** once this map initializes, the commit
allocation must contain at least 258 pages (16.125 MiB), even if its commit
payload and control records are tiny. Actual records can require more pages.
The region's much larger end bound is only a limit, not an eager allocation.
[Schema application](../../icydb/crates/icydb-core/src/db/schema/application_store.rs)
and [live checkpoints](../../icydb/crates/icydb-core/src/db/schema/live_schema_checkpoint.rs)
open this map; read-only `open_existing` alone does not initialize it.

The smallest credible change belongs in **IcyDB**: give this already-consolidated
control map a small separate governed allocation, leaving the commit buffer to
grow on demand. Do not split its record families into more maps. This removes
the fixed prefix coupling and can avoid roughly 16 MiB of early capacity when
the commit buffer is small. A smaller ICSS bucket cannot solve it.

This requires a new generated request and an explicit retained-layout migration
or fresh-image policy. Existing generated consumers and upgrade/recovery tests
must be qualified; no Storage public API change is needed. Moving the map must
not overwrite commit bytes or silently reset control records. Already-grown
stable memory cannot be shrunk by this change. Keeping the current layout is
simpler for existing deployed images, and a full commit buffer would erase most
of the saving. Confirm actual commit/control occupancy before prioritizing a
deployment migration. No sibling implementation or migration was attempted.

## 2. Requalify Canic's capacity reservation against its actual engine

**Verified:** application receipt reservation (`canic/crates/canic-core/src/storage/stable/intent.rs`)
computes capacity using explicitly ICSS 0.7.2-specific constants: five entries
per node, a 2,378-byte chunk and 116 fixed bytes. The active map imports Storage,
and [receipt admission](../../canic/crates/canic-core/src/ops/storage/intent/mod.rs)
calls this reservation before admitting application receipts. Storage's current
framed B-tree and power-of-two block allocator have different geometry.

This is an active stale sizing assumption, **not a demonstrated capacity
failure or measured over-allocation**. The current
[capacity tests](../../canic/crates/canic-core/src/storage/stable/receipt_capacity_tests.rs)
already compare reserved and actual pages; they were inspected, not run here.

The smallest change belongs in **Canic**: retain the pre-admission reservation
guarantee, derive its bound for the selected Storage format and qualify it with
maximum-sized records, low occupancy, deletion/reinsertion and allocation reuse.
Replace the ICSS constants only when that bound is proven. No new shared package,
consumer API or retained layout is required. It could reduce excess reservation
or expose an under-reservation; neither outcome is established yet. Keeping an
unqualified old-engine bound invites drift as Storage evolves.

## Measure actual usage before consolidating more stores

Use existing [runtime diagnostics](../crates/ic-storage/src/runtime/mod.rs):
`memory_allocations()` supplies each key's pages; `diagnostics()` supplies data,
metadata, physical and unassigned pages, extents and live leases. Capture after
bootstrap, role initialization, representative work, churn and upgrade. Pair
these with collection lengths and payload bounds: reserved pages are not live
payload occupancy. This audit made no deployed-canister queries.

Preserve Canic's lazy initialization. Do not merge authority/lifecycle domains
just to reduce the count: saving one small collection's first 64 KiB page is
quite different from saving an ICSS 8 MiB bucket. Blob's fourteen declarations,
for example, do not imply 112 MiB of reserved buckets.

The existing [bucket experiment](../qualification/ic-storage-benchmarks/README.md)
supports 1/8/128-page controls for **DFINITY ICSS**, not a Storage default switch.
Eight pages matched 128-page instructions across the selected integer trace,
but byte workloads had different tradeoffs. Storage's own 4 KiB payload block
rounding remains a separate profiling opportunity; changing its allocator format
is not justified merely by a memory-count audit.

## Reproduction and provenance

Evidence is retained locally in `target/platform-019-adoption/`: consumer search
hits, sibling HEAD/dirty-state inventory, incoming locks and patch, Platform
adoption checks and installed lifecycle logs. Main source snapshots: Canic
`0509ec870891` (399 changed paths), Blob `fd4649ebe5eb` (67), Relay
`db3746404f09` (20), IcyDB `b1d88ba9b939` (26), ic-crates `0851757145a7` (107).
Links above resolve into the sibling working trees and may change as their
pending work completes. Counts are source-derived; no sibling tests or release
qualification were run and no pending sibling work was modified.

## Complete declaration inventory and repeatable workloads

Refreshed from the local sources on 2026-11-03. The
[machine-readable inventory](measurements/consumer-memory-inventory.json) retains
exact keys, conditional declarations, source locations, source hashes, repository
HEADs and dirty-path counts. Those snapshots supersede the older provenance
counts above for this follow-up. No sibling was edited or compiled. Qualification
hosts and host-selected dynamic roles are identified separately. Declarations do
not establish live population, occupied bytes, or physical page allocation.

Workload labels below are representative benchmark families, not runtime sizing
presets, admission ceilings or a claim that every record fits the profile sizes.
Existing Canic, IcyDB, Relay and Blob production searches found no use of
`set_node_cache_budget`; the baseline therefore leaves the routing cache disabled.
The complete Canic union is 34 core + 26 control-plane keys + one optional metrics
key; actual canisters select role and feature subsets. Root adds 21 control-plane
keys, coordinator adds three, and Wasm-store adds six; shared keys count once.

| Owner | Allocation key / supplied role | Structure | Condition | Workload family | Implementation |
| --- | --- | --- | --- | --- | --- |
| canic-core | `canic.core.application_receipt.eligibility.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.async_job_recovery.v1` | Cell | always | control | [source](../../canic/crates/canic-core/src/storage/stable/async_job_recovery/mod.rs) |
| canic-core | `canic.core.auth.delegated_token_issuer.state.v1` | Cell | cfg(any(test, feature = "auth-delegated-token-issuer-state")) | control | [source](../../canic/crates/canic-core/src/storage/stable/auth/mod.rs) |
| canic-core | `canic.core.auth.local_application_authorization.replays.v1` | Vector | always | records | [source](../../canic/crates/canic-core/src/storage/stable/auth/mod.rs) |
| canic-core | `canic.core.auth.local_application_authorization.logins.v1` | Map | always | records | [source](../../canic/crates/canic-core/src/storage/stable/auth/mod.rs) |
| canic-core | `canic.core.auth.local_application_authorization.sessions.v1` | Vector | always | records | [source](../../canic/crates/canic-core/src/storage/stable/auth/mod.rs) |
| canic-core | `canic.core.auth.local_application_authorization.state.v1` | Cell | cfg(any(test, feature = "auth-local-application-authorization")) | control | [source](../../canic/crates/canic-core/src/storage/stable/auth/mod.rs) |
| canic-core | `canic.core.auth.root_delegation.state.v1` | Cell | cfg(any(test, feature = "auth-root-delegation-state")) | control | [source](../../canic/crates/canic-core/src/storage/stable/auth/mod.rs) |
| canic-core | `canic.core.auth.portable.root_enrollment.v1` | Cell | cfg(any(test, feature = "auth-root-delegation-state")) | control | [source](../../canic/crates/canic-core/src/storage/stable/auth/enrollment/mod.rs) |
| canic-core | `canic.core.auth.portable.receiver_enrollment.v1` | Cell | cfg(any(test, feature = "auth-local-application-authorization")) | control | [source](../../canic/crates/canic-core/src/storage/stable/auth/enrollment/mod.rs) |
| canic-core | `canic.core.authority_restore.fence.v1` | Cell | always | control | [source](../../canic/crates/canic-core/src/storage/stable/authority_restore/mod.rs) |
| canic-core | `canic.core.caller_authority.header.v1` | Cell | always | control | [source](../../canic/crates/canic-core/src/storage/stable/caller_authority/mod.rs) |
| canic-core | `canic.core.caller_authority.rows.v1` | Map | always | records | [source](../../canic/crates/canic-core/src/storage/stable/caller_authority/mod.rs) |
| canic-core | `canic.core.cycles.funding_ledger.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/cycles.rs`) |
| canic-core | `canic.core.cycles.icp_refill_records.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/icp_refill.rs`) |
| canic-core | `canic.core.cycles.topup_events.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/cycles.rs`) |
| canic-core | `canic.core.cycles.tracker.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/cycles.rs`) |
| canic-core | `canic.core.fleet.activation.v1` | Cell | always | bulk | [source](../../canic/crates/canic-core/src/storage/stable/fleet_activation/mod.rs) |
| canic-core | `canic.core.fleet_admission.projection.v1` | Cell | always | control | [source](../../canic/crates/canic-core/src/storage/stable/fleet_admission_projection/mod.rs) |
| canic-core | `canic.core.fleet.state.v1` | Cell | always | control | source (`canic/crates/canic-core/src/storage/stable/state/fleet.rs`) |
| canic-core | `canic.core.intent.expiry_index.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.intent.meta.v1` | Cell | always | control | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.intent.pending.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.intent.receipt_backed_records.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.intent.records.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.intent.totals.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.placement.acknowledgement_index.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/intent.rs`) |
| canic-core | `canic.core.placement.index_registry.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/placement_index.rs`) |
| canic-core | `canic.core.placement.scaling_registry.v1` | Map | always | records | [source](../../canic/crates/canic-core/src/storage/stable/scaling.rs) |
| canic-core | `canic.core.replay.receipts.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/replay.rs`) |
| canic-core | `canic.core.runtime.bindings.v1` | Cell | always | control | [source](../../canic/crates/canic-core/src/storage/stable/env.rs) |
| canic-core | `canic.core.runtime.canister_children.v1` | Map | always | records | source (`canic/crates/canic-core/src/storage/stable/children.rs`) |
| canic-core | `canic.core.sharding.assignments.v1` | Map | cfg(feature = "sharding") | records | [source](../../canic/crates/canic-core/src/storage/stable/sharding/mod.rs) |
| canic-core | `canic.core.sharding.registry.v1` | Map | cfg(feature = "sharding") | records | [source](../../canic/crates/canic-core/src/storage/stable/sharding/mod.rs) |
| canic-control-plane | `canic.control_plane.root.application_checkpoint.v1` | Cell | cfg(feature = "root-control-plane") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/application_checkpoint/mod.rs) |
| canic-control-plane | `canic.control_plane.fixture_store.v1` | Map | cfg(feature = "wasm-store-canister") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/fixture_store/mod.rs) |
| canic-control-plane | `canic.control_plane.fleet_admission.v1` | Cell | cfg(feature = "fleet-coordinator-canister") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/fleet_admission/mod.rs) |
| canic-control-plane | `canic.control_plane.fleet_coordinator.funding.v1` | Cell | cfg(feature = "fleet-coordinator-canister") | bulk | [source](../../canic/crates/canic-control-plane/src/storage/stable/fleet_coordinator/mod.rs) |
| canic-control-plane | `canic.control_plane.fleet_coordinator.registry.v1` | Cell | cfg(feature = "fleet-coordinator-canister") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/fleet_coordinator/mod.rs) |
| canic-control-plane | `canic.control_plane.root.admission.v1` | Cell | cfg(feature = "root-control-plane") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/root_admission/mod.rs) |
| canic-control-plane | `canic.control_plane.root.canister_inventory.assets.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/canister_pool/mod.rs) |
| canic-control-plane | `canic.control_plane.root.canister_pool.handoff_receipts.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/canister_pool/mod.rs) |
| canic-control-plane | `canic.control_plane.root.canister_pool.state.v1` | Cell | cfg(feature = "root-control-plane") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/canister_pool/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component.allocations.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_registry/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component.draining.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_registry/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component.principal_index.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_registry/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component_provisioning.operations.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_provisioning/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component_provisioning.placements.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_provisioning/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component_provisioning.state.v1` | Cell | cfg(feature = "root-control-plane") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_provisioning/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component.registry_entries.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_registry/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component.registry_state.v1` | Cell | cfg(feature = "root-control-plane") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_registry/mod.rs) |
| canic-control-plane | `canic.control_plane.root.component.subtree_removal_history.v1` | Map | cfg(feature = "root-control-plane") | records | [source](../../canic/crates/canic-control-plane/src/storage/stable/component_registry/mod.rs) |
| canic-control-plane | `canic.control_plane.root.fleet_registry_mirror.v1` | Cell | cfg(feature = "root-control-plane") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/fleet_registry_mirror/mod.rs) |
| canic-control-plane | `canic.control_plane.root.funding.v1` | Cell | cfg(feature = "root-control-plane") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/root_funding/mod.rs) |
| canic-control-plane | `canic.control_plane.root.wasm_store.state.v1` | Cell | cfg(feature = "root-control-plane") | control | source (`canic/crates/canic-control-plane/src/storage/stable/state/root_wasm_store.rs`) |
| canic-control-plane | `canic.control_plane.template.chunk_payloads.v1` | Vector | cfg(any(feature = "root-control-plane", feature = "wasm-store-canister")) | bulk | source (`canic/crates/canic-control-plane/src/storage/stable/template/chunked.rs`) |
| canic-control-plane | `canic.control_plane.template.chunk_refs.v1` | Map | cfg(any(feature = "root-control-plane", feature = "wasm-store-canister")) | records | source (`canic/crates/canic-control-plane/src/storage/stable/template/chunked.rs`) |
| canic-control-plane | `canic.control_plane.template.chunk_sets.v1` | Map | cfg(any(feature = "root-control-plane", feature = "wasm-store-canister")) | records | source (`canic/crates/canic-control-plane/src/storage/stable/template/chunked.rs`) |
| canic-control-plane | `canic.control_plane.template.manifests.v1` | Map | cfg(any(feature = "root-control-plane", feature = "wasm-store-canister")) | records | source (`canic/crates/canic-control-plane/src/storage/stable/template/manifest.rs`) |
| canic-control-plane | `canic.control_plane.wasm_store.gc_state.v1` | Cell | cfg(feature = "wasm-store-canister") | control | [source](../../canic/crates/canic-control-plane/src/storage/stable/template/gc/mod.rs) |
| metrics host | `canic.core.metrics.providers.v1` | Map | role provider needs stable metrics | records | [source](../../canic/crates/canic-core/src/ops/runtime/metric_providers/host.rs) |
| service | `blob.tenants.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.roots.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.root_objects.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.permissions.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.usage.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.manifests.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.confirmed.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.references.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.receipts.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.pending_deletions.v1` | Set | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.funding_accounting.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.funding_intents.v1` | Map | per installation | records | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| service | `blob.gateways.v1` | Cell | per installation | control | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/stores/grants/mod.rs) |
| installation | `blob.configuration.v1` | Cell | per installation | control | [source](../../ic-blob-storage/crates/ic-blob-storage/src/ops/service/installation/mod.rs) |
| channel | `<host-selected channel.stream>` | Map | per component instance; channel has three | control | [source](../../ic-relay/crates/ic-relay/src/channel/mod.rs) |
| channel | `<host-selected channel.outbox>` | Map | per component instance; channel has three | bulk | [source](../../ic-relay/crates/ic-relay/src/channel/mod.rs) |
| channel | `<host-selected channel.inbox>` | Map | per component instance; channel has three | bulk | [source](../../ic-relay/crates/ic-relay/src/channel/mod.rs) |
| responder | `<host-selected responder.state>` | Map | per component instance; channel has three | bulk | [source](../../ic-relay/crates/ic-relay/src/responder/mod.rs) |
| dispatch | `<host-selected dispatch.state>` | Map | per component instance; channel has three | control | [source](../../ic-relay/crates/ic-relay/src/dispatch/mod.rs) |
| mailbox | `<host-selected mailbox.state>` | Map | per component instance; channel has three | bulk | [source](../../ic-relay/crates/ic-relay/src/mailbox/mod.rs) |
| resources | `<host-selected resources.state>` | Map | per component instance; channel has three | bulk | [source](../../ic-relay/crates/ic-relay/src/resources/mod.rs) |
| generated database | `icydb.{namespace}.commit.control.v1` | raw buffer + schema region | per generated canister | bulk | [source](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) |
| generated database | `icydb.{namespace}.integrity.progress.v1` | Map | per generated canister | records | [source](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) |
| generated database | `icydb.{namespace}.startup.control.v1` | raw fixed receipt | per generated canister | control | [source](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) |
| generated database | `icydb.{namespace}.store.{store}.data.v1` | Map | per journaled store | records | [source](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) |
| generated database | `icydb.{namespace}.store.{store}.index.v1` | Map | per journaled store | records | [source](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) |
| generated database | `icydb.{namespace}.store.{store}.schema.v1` | Map | per journaled store | records | [source](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) |
| generated database | `icydb.{namespace}.store.{store}.journal.v1` | Map | per journaled store | bulk | [source](../../icydb/crates/icydb-model/src/build/actor/db/store.rs) |
| ic-metrics-storage | `<host-selected>` | Map | per adapter instance | records | [source](../crates/ic-metrics-storage/src/lib.rs) |
| ic-logging-storage | `<host-selected>` | Map | per adapter instance | bulk | [source](../crates/ic-logging-storage/src/lib.rs) |
| qualification host | `ic_auth_qualification.checkpoint.v1` | Cell | fixture only, not imposed by library | control | [source](../../ic-auth/apps/qualification/canister/src/storage.rs) |
| qualification host | `test.jobs.consumer.management.v1` | Cell | fixture only, not imposed by library | control | [source](../../ic-scheduling/apps/ic-jobs-test-consumer/src/queue.rs) |

## Three benchmark recipes, no new production knobs

The consumer inventory selects three fixed experiments in
[`workloads.rs`](../qualification/ic-storage-benchmarks/src/workloads.rs).
These are repeatable test points, not measured typical consumer sizes or promises
that a database fits a given byte/page allowance. See the
[benchmark commands and boundaries](../qualification/ic-storage-benchmarks/README.md#consumer-workloads).

| Recipe | Selected population and values | Operations / repeats | Consumers represented |
| --- | --- | --- | --- |
| `control` | One Cell or 16 Map records; 256 B / 4 KiB values (growth reaches 16 KiB) | Single operations, uniform, three rounds | Small recovery records, dispatch state, configuration; not every Cell's maximum |
| `records` | 4,096 records; byte values 31/32/33/256/1,024/4,096 B, 32-byte keys/values, 64-byte set keys and Wire record vectors | Batches 1/32, uniform/hot16/Zipf, shuffled Map/Set population, three rounds | Receipts, permissions, indexes, metrics, auth history and ordinary DB records |
| `bulk` | Eight records (one for Cell); 64 KiB / 1 MiB values, Map/Vector/Cell/Region | Single operations, uniform, one round | Relay payloads, IcyDB journal chunks, logging snapshots, Canic Wasm chunk vectors |

Both sides receive identical logical operations and payloads. ICSS uses an
explicit 1/8/128-page bucket control; Storage retains its current allocation
policy. Neither side has an ordinary-map cache enabled. Setup/verification batch
sizes are bounded by bytes; those operations remain outside instruction timing.
The bulk sizes are already at or above the trace's 64 KiB growth ceiling, so
`Grow` is an equal-sized replacement there, never a mislabeled shrink.

Useful source bounds, not observed occupancy:

- [Canic recovery](../../canic/crates/canic-core/src/storage/stable/async_job_recovery/mod.rs)
  is bounded at 241 bytes, while [activation snapshots](../../canic/crates/canic-core/src/storage/stable/fleet_activation/mod.rs)
  permit 2 MiB and [coordinator funding snapshots](../../canic/crates/canic-control-plane/src/storage/stable/fleet_coordinator/mod.rs)
  permit 32 MiB. A Cell is not automatically small. The 2/32 MiB maxima are outside this first matrix.
- [Relay](../../ic-relay/crates/ic-relay/src/channel/mod.rs) has 512-byte stream
  records and payloads up to 64 KiB; mailbox/responder records permit 72 KiB,
  dispatch 4 KiB and durable resource state 16 KiB. Snapshot framing and actual
  occupied entries need separate accounting from wire payload bytes.
- [IcyDB journal chunks](../../icydb/crates/icydb-core/src/db/journal/store.rs)
  are bounded at 64 KiB. Its 16 MiB commit buffer and schema map at page 257 are
  separate layout concerns; the bulk recipe does not reproduce that reserved prefix.
- Canic chunk payloads (`canic/crates/canic-control-plane/src/storage/stable/template/chunked.rs`)
  are Vector records up to 1 MiB, with a separate Map of 12-byte references.
  Blob's fourteen service stores hold metadata/accounting, not those Wasm bodies.
- Metrics Storage records are bounded at 6 KiB and Logging Storage records at
  64 KiB. The records recipe brackets small metric records; it does not claim
  exact metric snapshot or log-restoration behavior.

For now, retain one Storage runtime per backing, lazy allocation, exact requested
page growth and disabled caches by default. `StorageConfig` holds resource
ceilings, not eager reservations or bucket geometry; lowering it does not itself
save pages. `reserve_pages` is an explicit capacity reservation, not an entry-count
promise. Do not add per-consumer bucket settings or copy ICSS's 8-page result into
Storage. Keep the three recipes as the shared comparison vocabulary, then change
one internal default only when matched results establish a benefit.

The completed study below adds interleaved growth and churn across many
allocations. Simultaneous-store IC working sets, actual deployed cardinalities,
multi-MiB control snapshots and large DB populations remain unmeasured. Use
`memory_allocations()` and `diagnostics()` at bootstrap, initialization,
representative work, churn and upgrade before selecting production geometry.
No production default or sibling layout has changed in this pass.

### Initial qualification

The historical initial qualification recorded
all three profiles with the same binaries, ICSS 0.7.2, eight-page buckets and one
sample: 276 control calls, 5,076 record calls and 172 bulk calls. All results and
final contents matched the independent model. Twelve native tests, strict native
and all three Wasm backend lint checks, formatting, ShellCheck and documentation
links pass. An initial unbounded-vector fixture was corrected to the actual
1 MiB bounded chunk contract; its failed attempt is retained separately.

This qualifies the recipes, not an optimal default. In the record profile's
uniform single-read case, 4 KiB values used 519 Storage pages versus 273 ICSS
pages, while median measured reads cost 472,466 versus 1,738,322 instructions.
The stable-page totals include allocation history across the three rounds.
Do not generalize this speed/space tradeoff to other operations or sum distinct
profiles into a universal performance score. The following matched bucket and
multi-allocation controls extend this evidence without changing production geometry.

## Completed bucket and shared-backing study

Historical pre-0.68 results: the independent tree/block/record engines described
below were subsequently replaced. These tables do not measure the current copied
engines or select a current allocation policy.

[Selected historical results](measurements/storage-memory-study.json) retain
the bucket totals and native capacity table. Bulk per-run exports were retired
in the authorized cleanup; rebuilds measure current code, not the removed engines.
The IC runs used identical frozen driver and Wasm binaries, published
**ICSS 0.7.2**, Rust 1.99.1 and PocketIC 16.1.0. Each profile used one sample per
bucket with its existing rounds and population; every bucket was explicit.
All **16,572 calls** and final contents passed the independent ordered model.
Storage's per-call instructions, lengths and page counts were identical across
bucket controls. These are selected traces, not statistical workload coverage.

### ICSS bucket controls

The table compares each ICSS trace's total timed instructions with the same trace
at 128 pages. Positive means more instructions; totals include controls. Different
profiles perform different work and must not be combined into one score.

| Profile | 1-page versus 128 | 8-page versus 128 |
| --- | ---: | ---: |
| control | +0.23% | 0.00% |
| records | +1.15% | +0.76% |
| bulk | −0.64% | −0.44% |

Individual rows vary more: the records trace's largest increases are 7.61% at one
page and 7.05% at eight. Bulk 64 KiB vector reads improve by 51.38% at one page
and 34.27% at eight; the per-row extrema were historical observations, not retained current qualification. Neither smaller bucket is
universally faster. Timing excludes request/reply codecs and setup, as described
in the [benchmark boundaries](../qualification/ic-storage-benchmarks/README.md#comparison-boundaries).

The research command now defaults to **eight pages (512 KiB)**. ICSS 0.7.2's
32,768-bucket limit gives a 16 GiB data ceiling at eight pages, versus 2 GiB at
one page and 256 GiB at its stock 128 pages. One page cannot support the stated
2–4 GiB business-state target generally. Eight is a useful comparison setting,
not an optimum proven for every population. Pass 128 explicitly for stock-default
claims; retain one page as a low-reservation control. Production Storage has no
ICSS buckets and keeps its existing exact requested-page growth.

Reproduce the matched configurations from the repository root:

```sh
for workload in control records bulk; do
    for pages in 1 8 128; do
        BENCH_WORKLOAD="$workload" BENCH_UPSTREAM_BUCKET_PAGES="$pages" \
            make benchmark-storage BENCH_SAMPLES=1 || exit
    done
done
```

The retained run built once and reused frozen binaries; rebuilding now creates a
new source-bound experiment. The later CLI default change does not alter the
explicit arguments or saved configuration of any completed run.

### Many allocations sharing one backing

The [native executable](../qualification/ic-storage-benchmarks/benches/multi_allocations.rs)
uses 32 and 64 maps, interleaved growth from 32 to 96 records per map, 256-byte
values and 64 KiB values in every eighth map, then four delete/reinsert rounds.
It uses actual governed Storage leases or actual ICSS virtual memories on one
fresh backing per backend. Expected final pairs and displaced values are checked.
The count brackets Canic-sized compositions but does not execute Canic DTOs,
admission policy or scheduling. These are native capacity and backing-IO results,
**not IC instructions or endpoint speedups**.

| Configuration | 32 maps, final physical MiB | 64 maps, final physical MiB |
| --- | ---: | ---: |
| Storage | 50.1875 | 100.1875 |
| ICSS 1 page | 28.0625 | 56.0625 |
| ICSS 8 pages | 42.0625 | 84.0625 |
| ICSS 128 pages | 256.0625 | 512.0625 |

The 64-map logical value payload is 49.3125 MiB. Initializing empty maps alone
reaches 67 physical pages for Storage versus 65/513/8,193 for ICSS 1/8/128.
Both engines plateau under this bounded same-size churn: Storage remains at
1,603 physical pages and 832 extents after growth; ICSS expands during the first
reinsert and then stays at 897/1,345/8,193 pages. This is reuse evidence for this
history, not a general fragmentation or recovery proof.

For the 64-map growth phase, Storage writes 37,773,544 backing bytes, versus
416,622,512 / 416,622,032 / 416,621,968 bytes for ICSS 1/8/128. ICSS 0.7.2's
`btreemap/node/v2.rs::save_v2` serializes every entry's value when saving a node;
That historical Storage engine retained external-value references and saved
changed payloads separately.
This explains a concrete IO advantage, alongside Storage's larger capacity.
Native IO counts do not establish an equivalent IC instruction ratio.

Run the native experiment using the command in the
[benchmark README](../qualification/ic-storage-benchmarks/README.md#shared-backing-capacity-and-io).
Diagnostics are observed between phases and excluded from the next phase's IO.
The report separates physical pages, logical virtual pages, Storage metadata,
extents, per-allocation pages and expected value payload.

### External-value allocation boundary

[Record framing](https://github.com/dragginzgame/ic-storage/blob/4dea092f07e06749eb3f14aa24817c0962fbc0be/crates/ic-storage/src/adapter/record.rs) adds 20 bytes.
[Tree allocation](https://github.com/dragginzgame/ic-storage/blob/4dea092f07e06749eb3f14aa24817c0962fbc0be/crates/ic-storage/src/adapter/tree.rs) rounds external
allocations to a power of two with a 256-byte minimum. The geometry probe keeps
32 records, insertion history and tree shape fixed and checks adjacent sizes:

| Payload bytes | External block bytes | Actual allocation data pages |
| --- | ---: | ---: |
| 4,076 / 4,077 | 4,096 / 8,192 | 3 / 5 |
| 65,516 / 65,517 | 65,536 / 131,072 | 33 / 65 |
| 1,048,556 / 1,048,557 | 1,048,576 / 2,097,152 | 513 / 1,025 |

This verified the old implementation's source-derived slack. The copied-ICSS
reset retires that external-block format; these tables are historical and do not
select current allocator defaults.
Finer external-value allocation was a proposal for that retired engine, not a
current implementation plan. Measure the copied-ICSS baseline before selecting
any new class, threshold or persistent-format change. No production geometry,
public API, stored format, dependency or sibling release changed in this study.
