# Storage / ICSS type and collection comparison

Current study: 2026-11-05; evidence compacted 2026-11-06. This records the
pending copied-ICSS engine baseline, not a published Storage release.

## Coverage contract

The [single catalogue](../../../qualification/ic-storage-benchmarks/src/catalogue/mod.rs)
owns 73 concrete types and 565 unique collection/role cases. Its callback roster
drives metadata, normalization and concrete Wasm generic dispatch.
Each type appears as a Map/Region value under u64, an ordered Map/Region key
with u64 values, a Set element, a Vector value and a Cell value. This is not
all 73 × 73 possible key/value pairings. Non-Ord roles remain visible as unavailable.

Map and Region also run ICSS's actual 20-pair grid: u32 / Blob10 / Vec / String
keys crossed with unit / u32 / Blob20 / Vec / String values, plus Storage's eight
additional paired cases. These supplement the ordered, iterator, boundary and
population-scaled Auth presets; the type matrix does not replace their histories.

The reference is published ICSS 0.7.2, source
`72b02ab65b6c6fbf53fbabf752732d2726539377`. The selected Git `b5d1650` reference
has compile/lint proof only, not this full installed run.
[ICSS sources](https://github.com/dfinity/stable-structures/tree/72b02ab65b6c6fbf53fbabf752732d2726539377/src)
include the B-tree grid, Vector controls and Storable tests.

The union covers unsigned/signed scalars, exact float bits, bool/unit, fixed
arrays, bounded Blob/byte types, binary/string keys, two/three-field tuples,
all fixed-u64/Blob8 triple-bound arrangements, all Blob8/String triple orders,
optional scalars/strings/tuples, Principal, loose-bound eight-byte UnfixedU64
values (255/65,000/68,000 maxima), Wire/session records and bounded chunks.
Large actual blobs and small values with large declared bounds are distinct cases.
ICSS tuple/Blob/Principal codecs are shared through a benchmark-only wrapper;
decoded fields are normalized independently of the storage codec. Production
dependencies, codecs and public APIs are unchanged by these benchmark additions.

## Results and compatibility

| Collection | Cases | Measured calls | Verified refusals |
| --- | ---: | ---: | ---: |
| Map | 173 | 24,832 | 7 |
| Set | 73 | 2,704 | 3 |
| Vector | 73 | 5,044 | 36 |
| Cell | 73 | 2,400 | 0 |
| Map regions | 173 | 11,624 | 7 |
| Total | 565 | 46,604 | 53 |

Every timed result and final state passed the independent model. Reconciliation
matched configured cases, backends, populations, payloads, batches and phases,
with no duplicate/missing measurements. Of 1,130 backend-case entries, 1,059
are supported, 53 are verified runtime refusals and 18 are non-Ord roles.
Storage has 540 supported cases versus ICSS's 519. The complete run produced
22,996 paired timing rows and 612 Storage-only rows with blank ICSS costs/ratios.

- Unbounded Vector initialization is attempted and refused on both backends.
- Native ICSS fixed Option bounds reject one-byte None during checked
  Map/Set/Vector/Region setup. Storage's successful paths remain measurable.
  Cell's native Option path succeeds on both libraries.
- Floats/optional floats have no Ord; ordered-key/Set roles are not applicable.
- Unit/bool keys use their real finite populations and omit impossible absent-key
  operations. Narrow integer populations leave room for representable extra keys.
- Unexpected traps fail the run; they are never relabeled as expected refusals.

## Selected data and scope

[Selected results](../../measurements/storage-comparison-selected.csv) retain
one row for every case: batch one, available payload closest to 32 bytes (lower
breaks ties), Get and the named mutation, plus stable pages observed at Get.
Set uses Insert, or Reinsert for finite domains. Unavailable timings are blank.
The scope column distinguishes whole-value bytes from component/optional caps.
These selected rows do not reproduce every phase, payload or tail in the bulk run.
[Compact identities](../../measurements/storage-comparison-identities.json)
retain compiler, module/source identities, configuration, counts and limits.

The recipe uses 32 rows (Cell one; finite key caps), shuffled ordered population,
uniform access, batches 1/8, one sample/round, ICSS eight-page research buckets,
Rust 1.99.1, Testkit 0.79.0 and PocketIC 16.1.0. Five collection shards use
independent server lifetimes; aggregate output appears only after all finish.
Setup, trace generation, normalization and response encoding are outside the
operation counter. Stable pages include each backend's allocation infrastructure.
ICSS has no governed region API: its control reloads a map per operation.
This bounded synthetic run establishes no statistical or universal speed winner.

The synthetic large-inline-key handler needs a common 8 MiB stack. Its original
917,712-byte frame plus a 131,136-byte node-load frame exceeds the default
1 MiB stack; 88 targeted calls passed after the benchmark-only stack selection.
This does not establish an intrinsic Get bound or a production stack guarantee.
Gzip changes installation transport, not the retained original module.

## Reproduction and evidence lifecycle

```sh
BENCH_WORKLOAD=types make benchmark-storage
```

Prepare tools/caches separately. The current runner writes new attempts beneath
`target/evidence/storage-comparison/` and allows 30 minutes per collection.
An earlier 15-minute deadline stopped after 24,277 valid Map calls; it was not
a complete result. Other partial runs exposed range-key preparation, diagnostic
panic text and stack limits; their partial results are not counted above.
All 42 focused native tests and selected strict native/Wasm lint passed.

The completed original run is locally at `target/storage-comparison/attempt.tSHDxo/`;
its raw logs, full tables, source archive and original Wasm were left untouched.
The earlier 58-case/6,208-call study is superseded by this catalogue.
Bulk CSV/JSON exports and repeated per-run reports were retired from documentation
under the authorized cleanup. Git keeps selected results, not a raw archive.
Local target artifacts may be removed by explicit cargo clean; their hashes
identify inputs but do not make missing raw output independently verifiable.
No durable raw-retention exception is selected. For engine/safety details and the
separate Auth baseline, see the [engine baseline](icss-engine-baseline-2026-11-05.md).
