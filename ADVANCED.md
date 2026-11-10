# Advanced Storage allocation

Storage owns the named directory, backing custody, exclusive leases and all
supported collections. Read [the allocation contract](crates/ic-storage/ALLOCATION.md)
for namespace admission, persistence and resource budgets, and
[the collection guide](crates/ic-storage/README.md) for typed stores.

## Runtime ownership

The host constructs one `MemoryRuntime` over an exclusively held backing,
composes a `SealedDeclarationSnapshot` and `AllocationGrants`, and bootstraps with
its `RuntimeBootstrapPolicy`. Only successful admission and durable commitment
allow `open_memory` to issue a non-cloneable handle. A detached handle retains the
backing claim after the runtime is dropped.

`verify_bootstrap` compares the complete established host binding without IO or
policy callbacks. `verify_authority` checks a component's declared subset against
that binding. Neither grants access independently or releases existing leases.
See the [composed host example](crates/ic-storage/examples/composed_host.rs).

## Recovery and persistence

Construction accepts empty backing or a valid current directory. There is no
legacy reader or automatic reset. Bootstrap retains omitted names and IDs;
opening them requires current grants and policy. Historical selection is explicit
in `prepare_bootstrap`; see the
[recovered admission example](crates/ic-storage/examples/recovered_admission.rs).

Directory commits reuse inactive snapshot arenas and publish a protected root
last. New names and allocation growth advance its generation. Reopening unchanged
names does not. Native backing writes are not transactional; an interrupted write
poisons the owner. IC message rollback is the installed atomicity boundary.

## Stable key rules

Keys are permanent logical names, at most 128 ASCII bytes, ending in a nonzero
`.vN` suffix. Dot-separated segments start with a lowercase letter and contain
only lowercase letters, digits and underscores. Changing a key declares a new
allocation. ID assignment is opaque, durable and uses `u64`; applications do not
choose numeric slots or reserve unmanaged ranges.

## Host responsibilities

Hosts own backing identity, namespace grants, component composition, schema
admission, clocks, lifecycle, authorization and recovery decisions. Allocation
metadata does not prove application data validity or schema compatibility.
Custom codecs and comparators must remain consistent across retained data.
See [safety](SAFETY.md), [operations](docs/operations/operations.md) and
[the current format](docs/design/current-ledger.md).
