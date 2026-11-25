# IC Storage

Named stable-memory allocations with a Storage-owned degree-six B-tree and Cell.
The crate owns allocation names, namespace grants, exclusive leases, record
traits and collection formats.

The default allocation limit is 4,096. `StableKey` accepts up to 128 bytes of
lowercase ASCII dot-separated names ending in a nonzero `.vN`, for example
`app.users.v1`. Names remain bound to opaque IDs across ordinary IC upgrades.
Applications do not assign numeric memory IDs.

```rust
use ic_storage::{AllocationGrants, GenericAllocationPolicy, MemoryAuthority,
    MemoryRequest, MemoryRuntime, SealedDeclarationSnapshot, StableKey, VectorMemory};
use ic_storage::adapter::StableBTreeMap;
let mut host = MemoryRuntime::new(VectorMemory::default()).unwrap();
let declarations = SealedDeclarationSnapshot::new(vec![
    MemoryRequest::new("app", "app.users.v1").unwrap(),
]).unwrap();
let grants = AllocationGrants::new(vec![MemoryAuthority::new("app", "app.").unwrap()]).unwrap();
host.bootstrap(&declarations, &grants, &GenericAllocationPolicy).unwrap();
let memory = host.open_memory(&StableKey::parse("app.users.v1").unwrap()).unwrap();
let mut users = StableBTreeMap::<u64, String>::create(memory);
users.insert(1, "Alice".into());
assert_eq!(users.get(&1), Some("Alice".into()));
```

On the IC, use `DefaultMemoryImpl` at the host integration boundary. In
post-upgrade, reconstruct the runtime and declarations, then `load` existing
collections. Stable memory holds the records; no whole-heap retention or save/load
pass is required. `create` requires empty allocation memory; `load` refuses
foreign data. `reset` explicitly replaces a collection and must be a host decision.

Each collection consumes a non-cloneable allocation lease. Drop every collection
before reopening the same allocation. Raw backing aliases are infrastructure-only;
applications must not bypass the runtime's custody checks. Ordinary errors are
not transactions; installed IC traps roll back the message. Native writes are
not atomic.

The current B-tree uses `ICPMAP001` and the Cell uses `STORCELL` version 1.
Both require fresh allocations when moving from 0.79. Loading an old map or cell
refuses its framing before writing. The new V1 allocation directory requires fresh backing. To retain records, export them with the previous reader and import them into
a fresh V1 backing before switching applications. Previous `ICMAP003` and
`ICMAP004` headers are refused on load.

### Per-map allocation options

Use `create_with_options` when a named map has a distinct workload. For
example, a chunk index that stores 1 MiB values can use a 257-byte allocation
unit. Its 1,048,596-byte framed value fits in a 1,052,672-byte block. The
default 256-byte unit rounds the same frame up to 2 MiB.

```rust
use ic_storage::{Memory, RuntimeMemory};
use ic_storage::adapter::{BTreeMapOptions, StableBTreeMap};

fn create_chunks<M: Memory>(lease: RuntimeMemory<M>) -> StableBTreeMap<[u8; 32], Vec<u8>, M> {
    StableBTreeMap::create_with_options(lease, BTreeMapOptions {
        allocation_unit_bytes: 257,
        ..Default::default()
    })
}
```

`node_target_bytes` controls the initial node reservation (default 1,024).
`allocation_unit_bytes` controls the minimum block size for nodes and external
values (default 256). Both accept 256 through 65,535 bytes. Each allocation has
its own settings. Call `load(lease)` after upgrade and `options()` to inspect
the recovered settings. Changing the layout of a populated map requires
copying into a fresh map. Use `reset_with_options` only to discard its contents.
