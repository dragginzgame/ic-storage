# IC Metrics Storage

See [Storage naming adoption](../../docs/migration/storage-naming.md) for the repository and adapter rename.

This package is maintained and released by IC Storage alongside
Storage. The `ic-metrics` arithmetic crate remains independently
maintained in [IC Metrics](https://github.com/dragginzgame/ic-metrics).
The [migration guide](https://github.com/dragginzgame/ic-storage/blob/main/docs/migration/ic-metrics-stable.md) records source
provenance and earlier version selections; the current format cut is described below.

Optional durable counter, aggregate, summary and histogram snapshots in **one stable
B-tree map per canister**. Multiple components share that map with permanent
namespace/name keys. Adding this package is the opt-in: `ic-metrics` alone stays
allocation-free, `no_std` and volatile, with validated Serde support.
This adapter uses `std` and IC Storage’s owned governed ordered map,
allocates bounded records and supports native hosts and Wasm with Rust 1.100.0.

The host owns one `ic-storage` namespace grant set and bootstrap. It declares one
permanent metrics-store key, such as `app.metrics.v1`, and admits that allocation
before opening the map. Follow the
[Storage allocation and bootstrap guide](../ic-storage/ALLOCATION.md)
for the selected dependency's declaration and lifecycle APIs.

Storage owns the core `storage` release identity. This adapter
owns its independent `metrics-storage` release group; hosts select compatible core
requirements declared by its package metadata. Hosts exchanging arithmetic values
also select `ic-metrics = "0.30"` so Rust type identities agree. The adapter
selects Metrics 0.30, which exposes canonical restoration through `ic_metrics::persistence`, and owns its stored
Wire 0.21 codec. Services own frontend/report DTOs and their Wire schemas; this
package does not require or supply Wire implementations for Metrics types.
See [Storage #154](https://github.com/dragginzgame/ic-storage/issues/154) and the adapter's
[release ledger](../../releases/metrics-storage/CHANGELOG.md) and [group commands](../../RELEASING.md#release-groups).
Stored formats and governed handle requirements are unchanged by release grouping.

After bootstrap, the host opens `ic_storage::adapter::StableBTreeMap` with one
admitted memory handle and passes that public collection to `MetricsStore::new`.
Components share access to that store;
namespace/name keys distinguish rows without allocating an ID per metric or
component. `ic-storage`'s governance allocations are separate. This package never
allocates memory IDs, registers keys, creates a global store or installs lifecycle
hooks.

## Upgrade disposition

Metrics 0.30 and Wire 0.21 change public Rust type identities; align direct
dependencies when exchanging those types. Existing Storage-owned backing and
metric rows require no reset or migration for this adoption.

Upgrading from the retired manager/collection implementation to Storage's owned
allocator and ordered maps still requires fresh/reset whole backing. The previous manager and collection images have no reader. Do not clear
only governance while retaining application bytes. Increment the host's reset
epoch when restarting metric history.

The adapter's `ICMW` row schema and semantic restoration rules remain unchanged.
Unknown or malformed rows still fail closed on load/save/remove; there is no
legacy decoder or automatic reset. The format cut is in the backing allocator
and map beneath those records.

## Store operations

Use the concrete operations for each metric family. Loads need only the row key,
identity and, for histograms, finite bounds; they do not take a dummy observation.

| Arithmetic type | Load | Checkpoint |
| --- | --- | --- |
| `MeasurementCounter` | `load_counter(key, identity)` | `save_counter(key, identity, value)` |
| `MeasurementAggregate` | `load_aggregate(key, identity)` | `save_aggregate(key, identity, value)` |
| `MeasurementSummary` | `load_summary(key, identity)` | `save_summary(key, identity, value)` |
| `MeasurementHistogram<N>` | `load_histogram(key, identity, bounds)` | `save_histogram(key, identity, value)` |

Histogram loads admit at most 256 strictly increasing bounds before reading a row;
invalid expected bounds return `StoreError::InvalidState(InvalidBounds)`.
Serialization, kind selection and continuity dispatch are private implementation.

`MetricsMap<M>` names the exact `ic_storage::adapter::StableBTreeMap<Vec<u8>,
Vec<u8>, M>` type. Hosts may use either spelling; this is a type alias, with no
collection wrapper or independent initialization policy. `MetricsStore::new(map)`
and `store.into_map()` transfer that same collection and its allocation lease.
Hosts select `StableBTreeMap::create(handle)` for known fresh storage or
`StableBTreeMap::load(handle)` for a known retained map. The store constructor
wraps the supplied collection without touching storage. The generic parameter
remains the runtime's backing memory. Storage owns the physical tree format;
rows use this adapter's current Wire contract. No additional allocation is introduced.

`into_map` transfers the collection and its allocation lease to the host for
administration. Drop the store or returned map before reopening the allocation.
Direct map changes are host-owned and must preserve valid Metrics rows; subsequent
store operations still validate stored records at their ordinary boundaries.

This complete native example owns bootstrap once, opens only admitted handles,
checkpoints all four families and reopens the same map. It proves storage/codec
behavior, not IC upgrade execution or instruction/cycle costs:

```rust
use ic_storage::{
    GenericAllocationPolicy, AllocationGrants, MemoryAuthority, MemoryRequest,
    MemoryRuntime, RuntimeOpenError, SealedDeclarationSnapshot, VectorMemory,
};
use ic_metrics::{MeasurementAggregate, MeasurementCounter, MeasurementHistogram, MeasurementSummary};
use ic_metrics_storage::{MetricIdentity, MetricKey, MetricsStore};
use ic_storage::adapter::StableBTreeMap;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
const STORE_KEY: &str = "app.metrics.v1";
let pool = AllocationGrants::new(
    vec![MemoryAuthority::new("app", "app.")?],
)?;
let declarations = SealedDeclarationSnapshot::new(vec![MemoryRequest::new("app", STORE_KEY)?])?;
let mut runtime = MemoryRuntime::new(VectorMemory::default())?;
runtime.bootstrap(&declarations, &pool, &GenericAllocationPolicy)?;
let store_key = ic_storage::StableKey::parse(STORE_KEY)?;
let mut store = MetricsStore::new(StableBTreeMap::create(runtime.open_memory(&store_key)?)?);
assert!(matches!(runtime.open_memory(&store_key), Err(RuntimeOpenError::AlreadyOpen { .. })));
let identity = MetricIdentity::new("instructions", 0)?;
let key = MetricKey::new("canic", "endpoint.create")?;
let mut summary = store.load_summary(&key, &identity)?.unwrap_or(MeasurementSummary::EMPTY);
summary.record(42);
store.save_summary(&key, &identity, &summary)?;

let bytes_key = MetricKey::new("app", "processed.bytes")?;
let bytes_identity = MetricIdentity::new("bytes", 0)?;
let mut bytes = store.load_counter(&bytes_key, &bytes_identity)?.unwrap_or(MeasurementCounter::ZERO);
bytes.add(32_768);
store.save_counter(&bytes_key, &bytes_identity, &bytes)?;
let aggregate_key = MetricKey::new("app", "batch.size")?;
let items = MetricIdentity::new("items", 0)?;
let mut aggregate = store.load_aggregate(&aggregate_key, &items)?.unwrap_or(MeasurementAggregate::EMPTY);
aggregate.record(3);
store.save_aggregate(&aggregate_key, &items, &aggregate)?;
let histogram_key = MetricKey::new("app", "batch.distribution")?;
let bounds = [0, 10];
let mut histogram = store.load_histogram(&histogram_key, &items, &bounds)?
    .unwrap_or(MeasurementHistogram::new(bounds)?);
histogram.record(3);
store.save_histogram(&histogram_key, &items, &histogram)?;
let map = store.into_map();
assert!(matches!(runtime.open_memory(&store_key), Err(RuntimeOpenError::AlreadyOpen { .. })));
drop(map);

let reopened = MetricsStore::new(StableBTreeMap::load(runtime.open_memory(&store_key)?)?);
assert_eq!(reopened.load_summary(&key, &identity)?, Some(summary));
assert_eq!(reopened.load_counter(&bytes_key, &bytes_identity)?, Some(bytes));
assert_eq!(reopened.load_aggregate(&aggregate_key, &items)?, Some(aggregate));
assert_eq!(reopened.load_histogram(&histogram_key, &items, &bounds)?, Some(histogram));
# Ok(())
# }
```

Production hosts retain one bootstrapped runtime and open the public IC Storage
collection for the permanent metrics key. IC Storage requires
`ic_storage::RuntimeMemory` when opening its map; raw backing cannot bypass
allocation admission. Runtime handles cannot be cloned, and another live open
through the same runtime returns `RuntimeOpenError::AlreadyOpen`. Hosts still
prevent aliased backing access across runtimes. The store and a map returned by
`into_map` retain that lease until dropped.

Hosts own the new-versus-retained format decision. Never use IC Storage's fresh
constructor as an unknown/corrupt-store recovery fallback; its retained `load`
constructor does not fall back to initialization. Allocation/collection corruption
and capacity traps remain Storage contracts; typed adapter errors cover
identifiers, values and numeric admission.

## Durable contract

- Namespace/name keys are stable ASCII identifiers, up to 64/128 bytes. Units
  use the same alphabet and have a 64-byte limit. These namespaces separate rows;
  the canister still owns authorization and label cardinality.
- Records begin with `ICMW`, followed by an IC Wire native Serde/Postcard
  envelope containing kind, unit, reset epoch and a borrowed bounded state byte
  string. The state is another bounded Wire payload. This separation lets explicit
  removal validate an envelope without interpreting corrupt numeric state.
  Kinds 1/2/3/4 select aggregate/summary/histogram/counter. Records retain `u128`
  totals, optional maxima/latest, sticky overflow flags and disjoint histogram
  buckets. Storage restores through Metrics' typed arithmetic admission.
  Histograms admit at most 256 finite bounds; complete records admit 6,144 bytes.
  Each Wire operation has explicit traversal allowances of 1 MiB allocation,
  16,384 nodes and depth 16; these are conservative admission charges, not measured
  heap sizes. Borrowed envelope fields and bounded lists constrain decoded data.
- Load checks kind, unit, epoch and exact histogram bounds, then restores through
  the core's typed numeric admission. Unknown formats, truncation, trailing bytes
  and malformed state are errors; they never silently create an empty metric.
- Save replaces a full snapshot rather than adding it again. Repeated identical
  saves validate the retained row and skip encoding/map writes when unchanged.
  Numeric regressions and clearing overflow flags are rejected before writing.
  With exact sample counts, the added total must fit the added observations'
  maximum, the summary's latest value and the histogram's added bucket ranges.
  A new maximum must be attainable among those additions. Impossible transitions
  return `StoreError::StateRegression` without writes. Overflowed sample counts
  retain the conservative monotonic checks because exact additions are unknown.
  Supply the latest retained state before recording; numeric checks do not
  establish sample attribution, ordering or provenance.
- Deliberate reset/retirement uses `remove` with the old identity. It verifies the
  envelope and discards the payload explicitly, including unreadable numeric
  state. The complete outer byte-string framing must still be valid.
  For a reset, save the replacement with a new epoch in the same update message,
  without an await between operations. It never retires/reuses the allocation ID.

Batch observations within an update message and checkpoint at an attribution
boundary chosen by the host. Stable writes during successful update execution
survive upgrades; query writes do not persist, and writes in a trapping message
roll back with that message. Async callbacks are separate message boundaries.
Checkpointing less frequently explicitly permits loss since the last checkpoint.
Neither persistence nor an upgrade establishes counter/call-context continuity;
unfinished spans must be re-established by the consumer.

Host-owned endpoints, resets, sample admission and synchronization remain local.

For diagnostic byte or event totals, use `load_counter`, start an absent row
with `MeasurementCounter::ZERO`, add only admitted exact quantities, then checkpoint at the host's
update-message boundary. Absent and stored zero remain distinct. Exact
`u128::MAX` and actual overflow remain distinct too; an overflowed counter cannot
later clear its flag or decrease without explicit removal and a new reset epoch.
These diagnostics never replace authoritative balances, quotas or ledger debt.

The native example verifies codec/storage behavior, not IC checkpoint costs or
upgrade execution. Hosts choose which completed work contributes and when to
write; persistence does not establish attribution or reset continuity.
Stable map reads/writes add allocation and IC costs. The frozen
[Memory 0.49 comparison](https://github.com/dragginzgame/ic-metrics/blob/main/docs/evidence/memory-0147.md)
checks retained upgrades from Memory 0.48, query discard, trap rollback, leases
and reset, with matched IC checkpoint and bootstrap measurements.
Batch observations before checkpointing at the
owning message boundary; measured costs depend on workload, map state and cadence.
The prior [IC store receipt](https://github.com/dragginzgame/ic-metrics/blob/main/docs/evidence/stable-current-0146.md)
retains its Memory 0.48.2 / Storage 0.5.6 identities. Neither fixture qualifies a
later graph or an application's workload, lifecycle and deployment decisions.
The arithmetic-only cost evidence and native reopen tests retain their separate
scope and do not qualify live consumer execution.

The record byte budget limits codec admission after IC Storage reads a row.
It is not a bound on allocation while reading a host-supplied oversized value.
Hosts administering the public byte map must preserve its Metrics-only row
contract; underlying collection and allocation failures retain Storage semantics.

Collection failures propagate as `StoreError::Storage`. Hosts must handle fallible
map construction and iteration under the current Storage API.
