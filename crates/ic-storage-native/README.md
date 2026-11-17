# Native collection core

Unpublished experimental `no_std` + `alloc` library for operations over an
already valid `Vec<u64, A>`. It forbids unsafe code. The default vector core has
no runtime dependencies; the optional `map` feature uses the root-owned SHA-256
dependency for portable checkpoint integrity.
The caller retains the owning vector; `with_vector` lends a restricted scoped
view for scalar reads/writes, append, ordered removal, clear and shrink.

The view exposes neither `&mut Vec`, allocator access, owning iteration nor
Clone. References, views and capturing futures cannot escape the scope. Forgetting
a view does not forget the vector, so no guard destructor is needed for safety.
Ordinary returned errors preserve earlier mutations. The caller owns panic/trap
handling, native-host poisoning and successful-frame publication.

The installed single-vector provider uses this library after its own admission,
foreign-allocation claim and fresh allocator issuance. The library does not
validate retained bytes, reopen pointers, certify private layouts or implement
a persistence runtime. Its safe constructor accepts live Rust vectors, not saved
descriptors. Native owners and scalar record conversion remain runtime work.

Run `make native-core-test` for host behavior, ownership compile-fail tests and
the optional derive's application-facing codec/layout tests, plus strict host and
Wasm lint checks with and without that feature;
`make native-provider-test` exercises the library through the installed provider.
The existing checkpoint/job gates additionally exercise portable reconstruction.
Installed scope cases cover normal callback errors, full-capacity refusal and
forgotten views across compatible upgrades. They check settlement before reply
and distinguish these outcomes from whole-message trap rollback.
The separate [provider-host fixture](../../qualification/ic-storage-native-feasibility/provider-region.md#host-poisoning-and-explicit-replacement)
tests caller-owned poisoning across caught unwinds and explicit checkpoint
replacement using the same core and region controller. The core itself does not
own that failure policy or make unwinding transactional.

The optional `derive` feature re-exports the experimental
[`Store` derive](../ic-storage-native-derive/README.md) for bounded u32/u64 records.
Its support types describe scalar fields and codec errors; the macro generates
metadata, logical codecs and scoped getters. These are not retained-memory
admission or a general persistence runtime. The macro runs at compile time and
is disabled for the standalone raw-vector diagnostic build.
The generated API also runs in a `no_std` consumer with no implicit prelude and
with application names shadowing primitive aliases and result constructors.
The `Store` API's compile-fail examples check owning/borrowed fields, custom Drop,
misleading scalar widths, escaped views and attempts to obtain a mutable record.
These declaration rules are tested here; qualification fixtures own their
application-specific retained layouts, conversion, rollback and recovery cases.

## Keyed collections

Enable `features = ["map"]` for `map::Map`: a reusable `u64 → u64` native hash
table with scoped lookup, insertion/replacement, deletion, reservation and clear.
It accepts an explicit allocator, grows geometrically, and reclaims deleted
slots without tombstones. A configured live-entry limit bounds growth. Expected
allocation/limit refusals preserve earlier writes; callback unwinds poison the
owner until explicit checkpoint replacement succeeds.

```rust
use ic_storage_native::map::{Error, Map};

let mut records = Map::new(1_000_000);
records.with(|mut table| {
    table.reserve(10_000)?;
    table.insert(42, 100)?;
    assert_eq!(table.get(42), Some(100));
    table.remove(42);
    Ok::<_, Error>(())
}).unwrap().unwrap();
```

The map owns a native array of controlled scalar slots. It never persists private
Vec headers or allocator objects. A provider can release/admit the live slot
allocation; admission validates the complete table and preserves rejected input.
The existing region controller now exercises this path on the host, including
fresh issuance and growth refusal. A separate
[closed installed map profile](../../qualification/ic-storage-native-feasibility/native-map-provider.md)
now grows from 8 to 512 slots/384 entries across compatible IC upgrades and tests
custody, malformed-data refusal and rollback. `Map::rebuild_in` constructs a
candidate under another allocator while preserving the source. The installed
provider uses two disjoint retained banks and publishes the candidate only after
construction and owner release; tests trap during construction and on both sides
of publication. General allocation pools and installed portable map recovery
remain outside this bounded profile.
`checkpoint`/`export` and `from_checkpoint`/`recover` provide a bounded logical
path into fresh storage. `from_reader`/`from_reader_in` additionally recover from
an exact-length stream using a 4 KiB record buffer, returning a candidate only
after complete validation and integrity checks. The streaming encoder needs
constant scratch space; its caller owns buffering, complete-publication,
checkpoint custody and freshness. These synchronous APIs do not implement
resumable cross-message recovery.
`Map::records_from_checkpoint` validates the same complete portable bytes and
returns unique `(key, value)` pairs in ascending key order for logical staging.
It allocates records directly without rebuilding hash buckets; it still reads
the whole checkpoint and is not a chunked or resumable validator.
`Map::export_pages` and `map::paged::Manifest` provide a separate byte-defined
sequential page format with at most 64 records per verified page. The
[page contract and governed host evidence](../../docs/design/paged-map-checkpoint.md)
cover the hash chain, reverse export order and failure rules. Export still sorts
the complete source, and the reader has no durable cross-message cursor. Current
installed map jobs do not yet use this format.

Run `make native-core-test` for behavior, compile-fail and host/Wasm lint checks.
Run `make native-map-firehose` for one million live keys, 256 million mixed
operations, native readmission and full portable recovery. The workload uses
the public API and verifies every operation. See the
[native map contract and results](../../docs/design/native-map.md) for retained
representation, memory cost, performance scope and remaining integration work.
