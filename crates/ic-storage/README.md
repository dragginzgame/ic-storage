# IC Storage

Named stable-memory allocations with upstream `ic-stable-structures` collections.
One crate owns allocation names, namespace grants, exclusive leases and checked
reopening. DFINITY's library owns the B-tree, set, cell, vector, heap and log engines.

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
not atomic. Upstream codec, growth and panic contracts apply.

This version replaces the experimental OP API. It is not stored-format compatible
with OP or the 0.64 custom collection engines. The directory uses a new identity
and refuses earlier backing. Existing deployments need an explicit migration or
fresh storage; installing this crate does not authorize resetting application data.
