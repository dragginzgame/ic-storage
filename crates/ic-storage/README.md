# IC Storage

```toml
[dependencies]
ic-storage = { version = "0.76.1", features = ["map", "derive"] }
```

Experimental `no_std` + `alloc` library for operations over an
already valid `Vec<u64, A>`. It forbids unsafe code. The default vector core has
no runtime dependencies; the optional `map` feature uses the root-owned SHA-256
dependency for portable checkpoint integrity.
Enable `derive` for the re-exported `Store` macro from `ic-storage-derive`.
This is the native successor API; it is not API- or stored-format-compatible
with the legacy `ic-storage` 0.74 series. Runtime admission and custody remain
the caller's responsibility; publication does not establish production readiness.
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
Enable `checkpoint` for bounded byte records and fixed-width pages over explicitly
owned backing memory. Exact layout admission rejects missing or foreign storage
without writes; no legacy Storage dependency or legacy-format reader is involved.
The [checkpoint backing contract](https://github.com/dragginzgame/ic-storage/blob/main/docs/design/checkpoint-storage.md)
defines ownership, framing, capacity and the enclosing transaction requirement.
Installed scope cases cover normal callback errors, full-capacity refusal and
forgotten views across compatible upgrades. They check settlement before reply
and distinguish these outcomes from whole-message trap rollback.
The separate [provider-host fixture](https://github.com/dragginzgame/ic-storage/blob/main/qualification/ic-storage-native-feasibility/provider-region.md#host-poisoning-and-explicit-replacement)
tests caller-owned poisoning across caught unwinds and explicit checkpoint
replacement using the same core and region controller. The core itself does not
own that failure policy or make unwinding transactional.

The optional `derive` feature re-exports the experimental
[`Store` derive](https://github.com/dragginzgame/ic-storage/blob/main/crates/ic-storage-derive/README.md) for bounded u32/u64 records.
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

The optional `ordered` feature adds
[`ordered::Map<K, V>`](https://github.com/dragginzgame/ic-storage/blob/main/docs/design/ordered-storage.md)
for bounded encoded byte keys and values. It provides lookup, replacement,
deletion/reuse, double-ended ranges, prefix scans, preflighted batches and portable
recovery. Compact nodes refer to a separately owned byte arena; short indexes
do not reserve a full row's capacity. `new_in_allocators` accepts separate node
and byte authorities, and admission validates both allocations. Free blocks are
scrubbed, reused and split; coalescing and large-scale churn qualification remain
open. `Map<512, 4096>` covers the reviewed IC DB row/index byte bounds;
`Map<1, 16384>` covers its separate aggregate-job slots. DB retains field encoding
and schema policy. Both have host/provider-pool evidence; a closed installed
row/index profile additionally covers compatible upgrades, rollback, pagination
and two-domain recovery. Its generation-owned service reuses the map and both
allocators across messages, closes before upgrade and freshly admits after
controlled startup. `allocation_usage` observes current sizes without scanning
records. This is bounded fixture evidence, not a general runtime, DB adoption or
installed qualification of the separate aggregate-job profile.

Enable `features = ["map"]` for `map::Map`: a reusable `u64 → u64` native hash
table with scoped lookup, insertion/replacement, deletion, reservation and clear.
It accepts an explicit allocator, grows geometrically, and reclaims deleted
slots without tombstones. A configured live-entry limit bounds growth. Expected
allocation/limit refusals preserve earlier writes; callback unwinds poison the
owner until explicit checkpoint replacement succeeds.

```rust
use ic_storage::map::{Error, Map};

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
[closed installed map profile](https://github.com/dragginzgame/ic-storage/blob/main/qualification/ic-storage-native-feasibility/native-map-provider.md)
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
[page contract and governed host evidence](https://github.com/dragginzgame/ic-storage/blob/main/docs/design/paged-map-checkpoint.md)
cover the hash chain, reverse export order and failure rules. The reader can
checkpoint/resume a 144-byte verifier continuation under trusted custody. The
governed host adapter binds it to job progress and native-prefix fingerprints;
this is not authentication of a caller-supplied cursor. Export still sorts the
complete source. The installed paged map job now uses this format for
3,072 + 1,536-record recovery, including private-target pool growth and combined
native/progress rollback. Installed entrypoint costs are now measured in the
linked contract. Applying each page in one admitted scope removes per-record
readmission. Source cancellation and repeated recovery epochs now have installed
evidence, including reverse-domain reconstruction and stale retries. Live
allocation peaks remain work before larger sizes or production service budgets
are qualified.

Run `make native-core-test` for behavior, compile-fail and host/Wasm lint checks.
Run `make native-map-firehose` for one million live keys, 256 million mixed
operations, native readmission and full portable recovery. The workload uses
the public API and verifies every operation. See the
[native map contract and results](https://github.com/dragginzgame/ic-storage/blob/main/docs/design/native-map.md) for retained
representation, memory cost, performance scope and remaining integration work.
