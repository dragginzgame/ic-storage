# IC Storage

Owned stable allocation and collections for Internet Computer applications.
Storage owns the backing interface, codec trait, named allocation directory,
namespace admission, exclusive leases and collection implementations. It uses
independently implemented collection engines, with no production
`ic-stable-structures` dependency. Rust 1.100.0 or later is required.

## Supported collections

| Public type | Implementation | Known sibling use |
| --- | --- | --- |
| `StableCell` | Cached singleton with explicit create/load/reset | Canic, Blob |
| `StableBTreeMap` | Persistent AVL tree with reusable size-class blocks | Canic, `IcyDB`, Blob, Relay, Metrics and Logging Storage |
| `StableBTreeSet` | The same ordered-tree engine with unit values | Blob deletions |
| `StableVec` | Fixed-stride indexed slots with bounded values | Canic templates and authentication |
| `BTreeMapRegion` | Operation-local view of the same tree inside a leased page range | `IcyDB` control storage |

Map, Set and regions share the independent ordered tree and allocation controls.
Vector owns its bounded-slot format and direct iterator; Cell caches its decoded value. Stable Log and Min-heap
remain outside the scope.

There are two collection layers: private `ordered_map` contains the ordered algorithm;
public `adapter` supplies governed leases and the existing successful-call
APIs. Cell and Vector directly implement that boundary and own their framing.
All engines use Storage's `Storable` directly. The copied engines and codec
bridge are deleted. The previous competing engines, node cache,
record checksums and Vector scan-window implementation have been removed.

`ic_storage::adapter` exports these collections, `MapIter`, `SetIter`, `MapEntry`,
`RegionGrowError`, `CollectionError`, `CodecError`, and the owned `Memory`, `Storable`, `Bound` and `storable`
interfaces. Backing implementations and runtime types live at the crate root.
Collections require non-cloneable `RuntimeMemory` leases; implementing `Memory`
or `Storable` does not grant access to an allocation.

```rust
use ic_storage::{AllocationGrants, GenericAllocationPolicy, MemoryAuthority,
    MemoryRequest, MemoryRuntime, SealedDeclarationSnapshot, StableKey, VectorMemory};
use ic_storage::adapter::StableBTreeMap;

let mut host = MemoryRuntime::new(VectorMemory::default())?;
let requests = SealedDeclarationSnapshot::new(vec![
    MemoryRequest::new("app", "app.rows.v1")?,
])?;
let grants = AllocationGrants::new(vec![MemoryAuthority::new("app", "app.")?])?;
host.bootstrap(&requests, &grants, &GenericAllocationPolicy)?;
let key = StableKey::parse("app.rows.v1")?;
let mut rows = StableBTreeMap::<u64, u64>::create(host.open_memory(&key)?)?;
rows.insert(7, 11);
drop(rows);
let rows = StableBTreeMap::<u64, u64>::load(host.open_memory(&key)?)?;
assert_eq!(rows.get(&7), Some(11));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Current storage contract

The Storage 0.68 engine baseline was a hard cut. Earlier manager, ledger, map,
set, vector and region images are retired. The subsequent direct-result API cut
preserves the current images. There is no legacy decoder or automatic reset. Hosts must install
fresh backing or explicitly retire the complete old storage image; never reset
only allocation governance while retaining unmanaged application data. Rebuild
consumers against Storage's owned codec and backing traits. Primitive numeric
codecs are little-endian; application codecs remain application-owned.

The allocation directory uses STORNAM1 version 1. Map/Set/regions now use
`STORMAP1` version 1 with a 1 KiB header; Vector
uses `STORVECT` version 1 with a 32-byte header. The new independently implemented
Cell uses `STORCELL` version 1 and a 24-byte header. Previous collection images are
rejected, including ICSS images. Hosts must explicitly provide fresh/reset
backing for this hard cut; loading never converts or initializes incompatible
images. **Collections using the previous copied engine must be explicitly
rebuilt/reset or placed in fresh admitted allocations.** Hosts must update their collection framing checks, including
region admission. No reader converts or silently resets old images.

Collection payloads have no Storage integrity checksum. Valid-looking byte
changes can pass application decoding. Vector requires a finite `Storable::BOUND`
and reserves its maximum slot width, plus a four-byte length for variable records.
Cell has a u64 length envelope with host buffer limits; `StableCell::load(memory, maximum_bytes)`
admits a caller-selected ceiling before allocating its payload buffer.

The public codec trait remains fallible. Ordinary Map, Set, Vector and Cell
operations return their successful values directly; iteration yields entries or
values directly. **Inside collections, codec failures, malformed engine state
and engine allocation failures trap**, following ICSS behavior. Explicit framing
admission and page reservations can still return typed errors. Do not rely on
mutation failures returning `CollectionError`, iterators yielding an error, or
an unsuccessful operation leaving native backing intact. `MapEntry` keeps its
owned key/encoded-value API; decoding malformed bytes traps.

`create` requires empty allocation memory. `load` never initializes or repairs a
replacement. `reset` explicitly replaces contents in an admitted allocation.
Maps also support `clear`. IC execution rollback supplies atomicity for trapped
updates and upgrades. Native writes are not transactions: a mutation panic
poisons the shared allocation owner; discard the runtime and all leases, then
explicitly recover or replace the backing. Cached lengths and Cell values are
not recovery APIs. The native unwind guard is absent on Wasm.

The map caches no routing nodes and exposes no cache configuration. Vector
caches committed length under its exclusive lease, with owner-health checks.
It has no scan windows or mutation epochs. See [source provenance and deliberate
adaptations](../../docs/storage/upstreams.md) before changing this baseline.

`reserve_pages` takes an absolute virtual extent in 65,536-byte pages, not entry
capacity. `allocated_pages` observes that extent while retaining the lease.
Satisfied reservations do not grow or write. These APIs grant no raw-memory
extraction.

## Map regions

`BTreeMapRegion::new(Rc<RuntimeMemory<M>>, start..end)` selects a disjoint page
range within one admitted allocation. Construction performs no IO. `initialize`
requires an empty region. Operations load the current owned root, allowing
views to interleave without stale caches. The current collection marker is `BTR` version 2, with explicit page-size
metadata. Hosts such as `IcyDB` must update framing admission together with
fresh or explicitly rebuilt region storage for this hard cut.

`keys_range_limited(bounds, limit)` bounds the collected key count before
returning an owned snapshot. Resume using an excluded last key. Regions bound
all IO and growth to their selected extent; callers own disjoint application
frames and must not reenter collection access from codecs. Drop all views and
the last shared lease owner before reopening that allocation.

See [allocation and host integration](ALLOCATION.md).

Package metadata selects the repository's canonical `LICENSE.md`.

## Native memory metering

Enable `test-support` and use `ic_storage::testing::{MeteredMemory, MemoryIoCounts}`
for native backing IO observations. The wrapper implements Storage's `Memory`
and delegates `MemoryBacking` identity to the underlying backing, preserving
exclusive runtime custody across raw, wrapped and cloned aliases. Counters are
shared by clones; `reset()` clears observations without clearing bytes. Reads
and writes count attempts, while growth counts completed calls and successful
requested pages. Counters saturate and set `overflowed`; reject overflowed results
when reporting exact measurements. These observations are native IO counts,
not IC instruction or cycle costs. Allocation measurement remains in
`ic-testkit-native`.
