# Scalar Map scan attribution — 2026-11-11

The initial investigation left Storage's independent engine unchanged and adopted published Wire
0.20.0, Build 0.82.0, Platform 0.29.3, Runtime 0.21.1 and Toko Tooling 0.39.9,
then investigates the scalar scan deficit from the
[previous complete suite](storage-full-suite-2026-11-10.md). ICSS 0.7.2 remains
an external benchmark dependency. No upstream implementation text was used and
no replacement engine code was authored in that investigation. The separately
qualified [range-path experiment](#retained-range-path-experiment) below is now
retained; its independently authored refinement does not change the format.

## Native attribution

The [collection diagnostic](../../qualification/ic-storage-benchmarks/benches/collection_io.rs)
now supports `--scans-only`. Its 324 checked observations cover 256/4096 records,
fixed eight-byte and variable 32/4096-byte values, starts at zero/middle/last,
limits 1/32/256, and entry/key-only scans. Every result is checked against a
separate `BTreeMap` model. Each backend receives the same sequential population;
there are no intervening mutations between scans.

For 4,096 fixed-eight-byte records, start 2,048, limit 32:

| Backend and operation | Reads | Read bytes | Key decodes | Value decodes |
| --- | ---: | ---: | ---: | ---: |
| Storage entries | 87 | 5,976 | 44 | 32 |
| ICSS, governed Storage backing, entries | 165 | 1,261 | 52 | 32 |
| ICSS, its own manager, entries | 165 | 1,261 | 52 | 32 |
| Storage keys only | 55 | 5,720 | 44 | 0 |
| ICSS, either backing, keys only | 133 | 1,005 | 52 | 0 |

All six observations perform zero writes or growth. Both ICSS controls match
despite different backing capacities. Storage touches three stable 64 KiB ranges
versus two for ICSS. These are byte-range counts, **not heap faults or IC charges**.
Native read counts alone cannot explain the instruction ratio.

## Verified costs and next experiments

1. [Range admission](../../crates/ic-storage/src/ordered_map.rs) computes a lower
   rank with `rank`, then [Cursor::seek](../../crates/ic-storage/src/ordered_map/iter.rs)
   walks from the root again to construct a path. A first/middle one-record range
   in the 4,096-row tree reads 24 routing nodes and one value, with 13 key decodes.
   A 32-record range reads 55 routing nodes and 32 values. Reusing the admission
   path is the first candidate: preserve rank skipping, double-ended consumption,
   range bounds and corruption checks while avoiding the second descent.
2. [Node admission](../../crates/ic-storage/src/ordered_map/storage.rs) reads
   104 routing bytes per node, including child identity/height/count and payload
   metadata. This accounts for all 5,720 bytes in the selected key-only scan.
   The footprint is verified; instruction attribution between copying, checks
   and page charging is unmeasured. Removing checks or changing the persisted
   layout is not justified by these results.
3. `Store::value_bytes` allocates an owned `Vec` even for an eight-byte inline
   value. [MapEntry](../../crates/ic-storage/src/adapter/iteration.rs) retains
   those bytes so entries remain usable independently of the iterator. The
   implementation therefore creates 32 value buffers in the selected entry
   scan, besides cursor/output buffers. The diagnostic does not measure heap
   allocation. Small inline owned storage is a candidate, not a demonstrated
   win; generic specialization could exceed the 5–10 KB Wasm-growth budget.

`Cursor::take` also prepares a successor path before yielding the current record.
A caller stopping with `take(limit)` can pay for unused lookahead. This varies
with tree shape and stopping point and has not been isolated. Investigate it
after the repeated descent; it does not justify removing rank support or
replacing the tree. Any engine refinement must preserve the independent-authoring
boundary in the [replacement record](../design/ordered-map-replacement.md).

## Pre-experiment application size and qualification

Two fresh matched runs validate 1,764 observations each (3,528 total): scalar
Map/Set/region, 4,096 records, three seeds, three stateful rounds, uniform access,
batches 1/32 and stock ICSS 128-page buckets. Only insertion order changes
between the two runs; both use identical Wasm hashes.

Selected 32-result medians, in IC instructions:

| Operation | Sequential Storage | Sequential ICSS | Shuffled Storage | Shuffled ICSS |
| --- | ---: | ---: | ---: | ---: |
| Map Get | 1,146,803 | 1,058,394 | 1,159,867 | 930,340 |
| Map Replace | 1,921,515 | 1,716,023 | 1,937,599 | 1,650,755 |
| Map Scan | 372,747 | 239,784 | 773,363 | 312,054 |
| Map BoundedScan | 413,698 | 217,368 | 659,869 | 219,051 |
| Map KeysScan | 167,341 | 211,919 | 322,122 | 289,750 |
| Set Scan | 181,974 | 214,870 | 336,755 | 212,181 |
| Region KeysScan | 345,598 | 300,334 | 504,528 | 293,258 |

The shuffled Map Get/Replace/Scan and Set Scan results exactly reproduce the
prior depth study. Map Scan is 1.55 times ICSS for sequential insertion and
2.48 times for shuffled insertion. **This is population-history sensitivity,
not a dependency-update speedup.** The native diagnostic above uses sequential
insertion and cannot by itself attribute the additional shuffled cost.
Installed Map KeysScan traverses the unbounded key iterator; Map Scan performs
a lower-bounded range and decodes values. Their difference is not an isolated
value-decoding cost. The native key-only control uses the same range bound as
the native entry scan to avoid that confound.

All 16 application artifacts retain the previous suite's byte sizes. The size
runner validates 118 calls and two upgrades per selection/backend. These are matched small
applications, not the broad benchmark handler Wasms.

| Selection | Storage bytes | ICSS bytes | Storage change |
| --- | ---: | ---: | ---: |
| Base | 16,942 | 16,938 | 0 |
| Map | 113,038 | 90,208 | 0 |
| Set | 112,860 | 90,008 | 0 |
| Vector | 99,954 | 64,560 | 0 |
| Cell | 97,685 | 63,135 | 0 |
| Region | 114,417 | 89,933 | 0 |
| Map and region | 115,957 | 110,858 | 0 |
| All | 126,406 | 119,318 | 0 |

Installed original-Wasm qualification passes IO, directory fault rollback,
collection upgrades across 1,031 allocations, backing custody and Logging
checkpoint/retained-text/upgrade cases. This focused adoption does not repeat
Binaryen O3/Os/Oz variants or the full type catalogue.

Wire 0.20 accepts redundant varints. Metrics' mutation test previously required
byte canonicality for every admitted record; it now checks semantic re-encoding,
zero-write admission and original-byte preservation for unchanged saves. All
29 Metrics and 13 Logging unit tests, Build's 88 helper tests and the Platform
composition test pass. Public Wire error identities require future minor adapter
releases. No package version or stored format changes here.

## Retained range-path experiment

`Cursor::range` now retains the checked successor/predecessor stacks found
during lower/upper-bound admission. A bounded end starts from its retained path
instead of walking from the root again. Unbounded ends and nonzero `nth`/`nth_back`
jumps retain the existing lazy rank seek. There is no second engine, cache,
storage-format change, public API change or removed integrity check.

Native sequential attribution for 4,096 fixed-eight-byte records, middle start,
32 results changes from 87 reads / 5,976 bytes to 75 reads / 4,728 bytes. Key-only
reads change from 55 / 5,720 to 43 / 4,472. Key/value decode counts, writes,
capacity and touched stable-range counts are unchanged; both ICSS controls are
unchanged. This verifies the avoided 12-node routing descent without treating
native range counts as IC page charges.

Matched IC medians for 4,096 records and 32 results:

| Insertion history / operation | Storage before | Storage after | ICSS | Storage reduction |
| --- | ---: | ---: | ---: | ---: |
| Sequential Map Scan | 372,747 | 201,440 | 239,784 | 46.0% |
| Shuffled Map Scan | 773,363 | 760,735 | 312,054 | 1.6% |
| Sequential Map BoundedScan | 413,698 | 402,764 | 217,368 | 2.6% |
| Shuffled Map BoundedScan | 659,869 | 648,083 | 219,051 | 1.8% |
| Sequential region KeysScan | 345,598 | 252,402 | 300,334 | 27.0% |
| Shuffled region KeysScan | 504,528 | 335,248 | 293,258 | 33.6% |
| Shuffled Set Scan | 336,755 | 336,755 | 212,181 | 0% |

Each scalar candidate run validates 1,764 observations against the same earlier
baseline selection. The wider `ordered` recipe validates 12,312 observations
per version across scalar, array, string and variable payload cases. Of its
2,052 median rows, 252 improve and 1,800 are unchanged; none regress. ICSS medians
are identical between versions. For example, 256-row scalar Map Scan falls
from 201,371 to 194,794 versus ICSS 146,922. These are selected workloads, not
a universal speedup. Shuffled 4,096-row Map Scan still costs 2.44 times ICSS.
The larger instruction deltas are not explained by native IO savings alone;
this experiment does not isolate allocator/heap page-charge contributions.

### Size fixture correction and matched footprint

The previous size consumer used unbounded scans, which did not instantiate the
changed range-admission path. Its eight paired sizes remained unchanged, but
that was insufficient size evidence for this optimization. The maintained
`scan` request now takes an inclusive lower bound for Map/region; the runner
checks 0/17/80/999 through mutation and two upgrades. Set/Vector/Cell semantics
are unchanged. Both backend endpoint files remain identical.

These **range-aware** fixtures are identical between the baseline and candidate;
do not compare this table directly with the old unbounded-only size table:

| Selection | Storage before | Storage after | Change | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Base | 16,893 | 16,893 | 0 | 16,889 |
| Map | 113,513 | 113,710 | +197 | 90,195 |
| Set | 112,813 | 112,813 | 0 | 89,955 |
| Vector | 99,918 | 99,918 | 0 | 64,506 |
| Cell | 97,643 | 97,643 | 0 | 63,093 |
| Region | 114,888 | 115,077 | +189 | 89,904 |
| Map and region | 116,518 | 116,631 | +113 | 110,859 |
| All | 126,969 | 127,027 | +58 | 119,324 |

The archived baseline engine files match Git commit
`87cf55e4ca58db415da1639d72ed4cef04eeeb94`. The baseline and candidate are built
with current locks and identical updated fixtures in the same retained source
directory. Both use `--remap-path-prefix` from that directory to the normal
checkout. Initial unremapped archive builds enlarged source-path strings;
those artifacts are retained but excluded from the attributed size delta.
Remapped candidate sizes match normal-checkout sizes, though the artifact bytes
differ; both remapped versions were installed and validated separately.
Each selection/backend passes 127 calls and two upgrades.

### Qualification, tradeoff and decision

All 30 selected native tests pass: 16 engine, three adapter iteration, five
region and six public ordered-contract tests. New coverage exercises all bound
kinds, missing/empty/invalid endpoints, mixed-end rank skips, reuse without a
second routing read, child-metadata corruption and key-only payload exclusion.
Native/Wasm lint and installed Storage IO, upgrade and rollback pass. No
dependency or schema changes are made by this experiment.

Each bounded endpoint now eagerly allocates a 16-node-capacity vector and copies
its retained path. Count-only ranges, empty ranges and unused opposite endpoints
incur that work; their dedicated IC instruction costs are not measured. Count
and overskip still perform no extra IO after admission. Unbounded iteration is
unchanged. The measured scan gains and sub-200-byte application growth justify
retaining this step with that explicit limitation. Per-entry value buffers and
eager successor preparation remain separate experiments.

The unexposed refinement implementer read only the current independent engine
and its own tests, AGENTS, the installed baseline, the ordered-engine contract,
pinned engineering/hygiene/contribution rules and supplied black-box counts.
It read no upstream/predecessor implementation, Git history, sibling source,
benchmark implementation or public adapter tests. The coordinator extended the
separate public model test and owns measurement/qualification.

Experiment evidence is under `target/evidence/range-cursor-20261111/`:
`baseline-ordered`, `scalar-sequential`, native attribution, comparison JSON,
compiler/test logs, candidate source archive, baseline source identities and
both `remapped-*-size` directories. Canonical candidate runs are
`target/evidence/storage-comparison/attempt.McM2tt/` (shuffled scalar),
`attempt.tz63xE/` (ordered), and
`target/evidence/storage-consumer-size/attempt.ORcC78/` (range-aware size).
The first unbounded-size control is `attempt.wNM2K6`. Prior evidence is intact.

## Reproduction and evidence

```sh
cargo run --locked --offline --manifest-path qualification/Cargo.toml \
  -p ic-storage-benchmarks --example collection-io --features owned,dfinity \
  --target-dir target/runtime-qualification -- NEW_OUTPUT.jsonl --scans-only

BENCH_CASES=Map-U64-U64,Set-U64-Unit,Region-U64-U64 \
  BENCH_PATTERNS=uniform BENCH_ROUNDS=3 BENCH_UPSTREAM_BUCKET_PAGES=128 \
  make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=3
# Repeat with BENCH_SHUFFLED=1 for the populated-depth insertion history.
make benchmark-storage-size
# Wider iterator comparison:
BENCH_WORKLOAD=ordered BENCH_UPSTREAM_BUCKET_PAGES=128 make benchmark-storage BENCH_SAMPLES=1
```

`NEW_OUTPUT.jsonl` must not exist. Native evidence, before-locks, pending diff,
adoption logs and original runtime qualification are retained under
`target/evidence/adoption-scalar-20261111/`. Paired size evidence is
`target/evidence/storage-consumer-size/attempt.rqGBG7/`; the fresh scalar run is
`target/evidence/storage-comparison/attempt.EWiC5H/` (sequential) and
`target/evidence/storage-comparison/attempt.cvMl16/` (shuffled). Source archives, input hashes,
graphs and compiler identities are retained with the runs. Previous evidence
remains intact. Those initial adoption runs precede the separately recorded
range-path experiment above.

## Data-structure investigation — 2026-11-11

This is a design comparison, not a new performance result or a selected format.
No alternative engine was implemented, no production source changed, and no
new benchmark ran in this investigation. The current AVL and external ICSS are
the measured controls. The coordinator inspected current owned code, public
research and consumer call sites; a future implementation must retain the
separate authoring boundary in the [replacement contract](../design/ordered-map-replacement.md).

### Requirements established by consumers

The [Map adapter](../../crates/ic-storage/src/adapter/map.rs) promises `Ord`
ordering, bidirectional iteration and bounded ranges. The
[region adapter](../../crates/ic-storage/src/adapter/region.rs) additionally
offers bounded key snapshots resumed after an excluded last key. Hashing cannot
preserve those costs by merely changing the lookup routine.

Read-only sibling inspection verified concrete dependencies on those contracts:

| Consumer and inspected path | Required behavior | Implication |
| --- | --- | --- |
| IcyDB `crates/icydb-core/src/db/index/store.rs`, `visit_journaled_entries_in_range`, canonical branch | Ascending/descending stable-map ranges, merged with an ordered live overlay | Ordered access is an active dependency, not an unused export. |
| IcyDB `crates/icydb-core/src/db/schema/control_store.rs`, `load`, `keys` | Region namespace validation and key enumeration within family bounds | Point lookup alone cannot replace this use. |
| Blob Storage `crates/ic-blob-storage/src/ops/service/uploads/read/mod.rs`, permission scan around line 338 | Resume after a compound tenant/request key, with scan/result limits | Sorting the complete hash table for each page would defeat bounded work. |
| Blob Storage `crates/ic-blob-storage/src/ops/service/uploads/recovery/mod.rs`, reference range around line 240 | Enumerate references belonging to one owner | Requires an ordered range or a separately maintained owner index. |
| Canic `crates/canic-auth/src/storage/sessions/mod.rs`, `application_login_record` | Caller-identity map lookup; session/replay records separately use Vector | A point-map candidate exists, but changing the map would not remove Vector or session decoding costs. |
| [Metrics](../../crates/ic-metrics-storage/src/lib.rs) `load_admitted`/`save` and [Logging](../../crates/ic-logging-storage/src/lib.rs) `load`/`save` | Checkpoint get, insert and remove | Hashing is semantically plausible for these paths; actual deployed cardinalities and benefit remain unmeasured. |

Inspected IcyDB HEAD was `addc98269fb9c8a42c8ecd9fdee438ed558482ef` and
Blob Storage HEAD `b5da63b0da5d3018737136a6b914f7817aa91b70`; those cited
files were clean. Canic HEAD was `3700c6e1bf78cb04a60dcfaf0f789d4dd992c9e0`
with pending session-module edits: its observation describes that working tree,
not a deployed release. Its `canic-core/src/cdk/structures.rs` re-exports Storage
adapters. No sibling was changed or rebuilt. These examples are not a complete
fleet inventory or a measured application workload distribution.

### Representation matters independently of balancing

Current [node storage](../../crates/ic-storage/src/ordered_map/storage.rs) reserves
128 bytes per entry: 104 routing bytes (including 16 inline key bytes), followed
by 24 inline value bytes. A `u64 -> u64` record has 16 logical payload bytes, so its node slot
is eight times its payload. At 4,096 live entries that is 512 KiB of node slots
for 64 KiB of payload, before the collection header, free capacity and backing
metadata. This arithmetic is not the complete allocation footprint, and does
not imply an eightfold achievable reduction or speedup. The full suite's
34-page reservation includes more than live node bytes.

Replacing AVL balancing with red-black balancing alone leaves one record per
node and scattered traversal. The stronger hypothesis is to amortize routing
and allocation metadata over multiple entries. Rust's public
[B-tree discussion](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html#background)
explains this locality/comparison tradeoff; its native cache benefits are not
measurements of IC instructions.

### Candidate families and disposition

The assessments below are engineering hypotheses against the current contract.
They are not measured rankings among unimplemented candidates.

| Family | Potential advantage | Cost or constraint here | Disposition |
| --- | --- | --- | --- |
| Current AVL with smaller decode/iterator costs | Preserves the selected format; limits rewrite risk | Does not remove per-entry routing overhead or insertion-history sensitivity | Keep as control; test small-buffer and unused-lookahead costs separately. |
| Packed B-tree | Several keys per node; arbitrary `Ord`; some successful lookups stop at an internal record | Splits, merges and within-node moves; scans cross both internal and leaf records | First-tier ordered candidate. |
| Packed B+tree | Compact key-only routing and dense leaf scans; leaf links can avoid repeated ascent | Separator storage, split/merge/link updates, long-key handling; links do not guarantee physical adjacency | First-tier ordered candidate, especially for the measured scan deficit. |
| Sorted flat array | Very compact scans and binary search; simple small-map baseline | Middle insertion/deletion shifts a suffix; variable records need offsets or movement | Include for tiny populations and read-mostly controls, not the unrestricted replacement. |
| Blocked sorted array / packed-memory array | Dense ordered scans while leaving insertion space | Rebalancing can move a large interval; a scalable block directory starts to resemble a tree | Secondary candidate if packed trees leave a demonstrated scan problem. |
| Flat open-addressed hash table | Dense point lookup with little routing metadata | No ordered ranges; probe clusters, deletion policy and whole-table resize costs | Point-only control; include resizing in the evidence. |
| Bucketed dynamic hashing | Growth can redistribute one bucket instead of the whole table | Split work, overflow chains or a directory remain paid costs; no ordered ranges | Stronger growing point-map candidate if flat-table resize tails fail. |
| Separate-chaining hash table | Direct bucket access and simple deletion | Per-record allocation and pointer chains risk reproducing the current locality problem | Lower priority than packed buckets. |
| Cuckoo hashing | Bounded candidate locations for lookup in the classic scheme | Inserts can displace many records and force rehashing | Defer until ordinary hashing establishes an unmet lookup requirement. |
| Hash-array mapped trie | Incremental branching without flat-table doubling | Extra routing/bitmap levels and allocations; persistent-functional path copying offers no required benefit | Defer; compare only if dynamic hash growth remains problematic. |
| Radix / Patricia / adaptive radix tree | Prefix sharing and ordered byte traversal, potentially fast byte-key lookup | Requires an order-preserving key representation absent from the generic API; adaptive node forms add code | Specialized future candidate, not a transparent Map replacement. |
| Skip list | Ordered traversal with simpler local linking than balanced-tree rotations | Pointer-rich nodes; reverse traversal/rank support adds metadata; probabilistic variants need an explicit deterministic policy | No demonstrated advantage over dense leaves for this target. |
| Other binary trees: red-black, treap, splay | Different balance/update tradeoffs | Retain per-record traversal; splaying would mutate reads; randomized priorities need a policy | Do not spend the next experiment on balancing alone. |
| LSM / buffered tree | Batch mutations and sequential merge work | Read reconciliation, tombstones, deferred work and temporary capacity; maintenance consumes real executions | Poor fit for the current synchronous collection contract without a write-heavy workload proving need. |
| Direct-index vector / bitmap | Avoid comparison and hashing for dense numeric domains | Cannot represent arbitrary sparse/composite keys efficiently | Use existing Vector where the application already has dense indexes; not a generic Map engine. |
| Static/perfect or learned index | Compact routing for a known population/distribution | Construction or model maintenance under churn; generic comparisons and arbitrary inserts remain requirements | No demonstrated current workload justifies this complexity. |

Supporting primary sources establish mechanisms, not IC performance:
[PostgreSQL hash-index documentation](https://www.postgresql.org/docs/18/hash-index.html)
describes incremental bucket splitting, foreground expansion and overflow-chain
costs. It is an index, not our full key/value map: storing only a hash is not
sufficient for Storage key equality. The original
[cuckoo-hashing paper](https://www.brics.dk/RS/01/32/BRICS-RS-01-32.pdf)
separates lookup guarantees from insertion/rehash costs.
[ART's authors](https://portal.fis.tum.de/en/publications/the-adaptive-radix-tree-artful-indexing-for-main-memory-databases/)
report ordered traversal and adaptive node representation for their main-memory
setting. The [packed-memory-array study, section 2.3](https://erikdemaine.org/papers/CacheObliviousBTrees_SICOMP/paper.pdf)
describes dense scans and interval redistribution with amortized update bounds.
[RocksDB's compaction documentation](https://github.com/meta/rocksdb/wiki/Compaction)
explains the read/write/space tradeoffs of multiple sorted runs.
[Learned-index research](https://arxiv.org/abs/1712.01208)
uses key-distribution models; it establishes no benefit for this crate.
No third-party implementation was copied or adapted in this investigation.

### Constraints that eliminate misleading shortcuts

1. **Byte order is not `Ord`.** [Integer codecs](../../crates/ic-storage/src/storable.rs)
   use little-endian bytes: numeric 255 precedes 256, but their encoded byte
   strings sort in the reverse order. Arbitrary user-defined ordering cannot
   be inferred from `Storable`. Radix indexing would need a separately specified
   order-preserving representation, including signed, compound and variable keys.
2. **Encoded bytes are not a generic hash contract.** Equal keys may have
   distinct encodings. A point-map proposal needs consistent equality/hashing
   and a versioned persisted hash policy; deriving `Hash` alone does not promise
   stability of the input byte stream across future implementations. Rust's
   [HashMap contract](https://doc.rust-lang.org/std/collections/struct.HashMap.html)
   requires equal keys to hash equally and does not freeze its default algorithm.
   Hash collision resistance, seed ownership and full-key comparison must be
   accounted for; a fingerprint is not proof of identity.
3. **Packing must preserve large-value wins.** The earlier full suite has
   substantial get/replace wins for 4-KiB to 1-MiB values. A candidate should
   avoid routing through or moving those bodies during ordinary node splits.
   Inline-small/overflow-large is a candidate policy, not a selected threshold.
   Key-only scans must still avoid value payload reads and decoding.
4. **A node is not an IC page.** Do not choose a 64-KiB node simply because Wasm
   pages have that size. Compare several internal geometries, with exact copied
   bytes, occupancy, padding and boundary crossings. Stable ranges and heap
   charge regions are different measurements. Linked leaves need placement and
   churn tests; logical adjacency alone is insufficient.
5. **Keep supported semantics.** Zero-sized and unbounded records, current
   `Ord`, reverse/mixed-end iteration, rank skipping, read-only misses and reopen,
   independent owned iterator entries, current corruption checks and native
   poison/IC rollback boundaries remain qualification requirements. A new layout
   cannot reuse the existing format identity to reinterpret deployed bytes.
   Production adoption needs an explicit format/reset decision; this study makes none.

### What the next comparison must add

The current [type catalogue](../../qualification/ic-storage-benchmarks/src/catalogue.rs)
covers many shapes, but shape breadth is not key-distribution breadth.
[Array and principal generators](../../qualification/ic-storage-benchmarks/src/catalogue/models.rs)
embed a big-endian counter in otherwise constant bytes;
[bounded and unbounded byte generators](../../qualification/ic-storage-benchmarks/src/types/breadth.rs)
also use an identity prefix and a constant suffix. Shuffled insertion changes
history, not these bytes. A tree-family decision needs independent controls for:

- Full-width random byte keys, long shared prefixes, sparse integers, compound
  tenant/key prefixes, custom `Ord` differing from encoded order, and adversarial
  collisions for hashing. Preserve a separate correctness oracle and identical
  logical traces for the backends; no test-only key bypass inside a candidate.
- Tiny through larger live populations (initially 1, 16, 256, 4,096 and 65,536
  where the type domain and payload budget permit), sequential/reverse/shuffled
  population and repeated delete/reinsert histories. Large-payload cases remain
  separately bounded rather than multiplying every population by 1 MiB.
- Successful lookups and missing keys below, between and above present keys;
  replacements, new inserts, removals, forward/reverse/bounded/key-only scans,
  `nth`, terminal access, clear/reuse and reopening. Hash-only candidates mark
  unsupported ordered operations as unavailable, never as zero-cost successes.
- Explicit split/merge/resize-triggering calls, collision tails, occupancy
  sweeps and temporary source/target coexistence. Report worst observed call
  separately from medians; aggregate throughput cannot hide unfinishable growth.
- Single-operation and batched executions over the same governed backing,
  codecs and validation. Include setup/growth costs separately, native read/write
  bytes and codec/heap-allocation counts, and installed IC instruction counters.
  Repeat ICSS's own backing at 1/8/128-page buckets as a distinct capacity control.
- Matched application Wasm, both total bytes and code-section bytes, across Map,
  Set, regions and all collections. The 5–10 KB per-change budget still applies.
  Hash-only consumer size needs an equivalent point-only ICSS/AVL consumer;
  comparing fewer instantiated APIs with the full ordered consumer would mislead.

The resulting shortlist is packed B-tree/B+tree for the existing ordered API,
flat and incrementally growing bucketed hashing for the point-only comparison,
and a sorted-array small-population control. These are research candidates,
not a plan to ship several production engines or expose tuning knobs. Measure
the common-layout costs before selecting a winner or opening a format migration.

## Existing tree reuse and executable attribution — 2026-11-11

The public names do not identify the current physical algorithm:
[StableBTreeMap](../../crates/ic-storage/src/adapter/map.rs) wraps the owned AVL,
[StableBTreeSet](../../crates/ic-storage/src/adapter/set.rs) wraps that Map with
unit values, and [regions](../../crates/ic-storage/src/adapter/region.rs) load the
same engine through [TreeMemory](../../crates/ic-storage/src/adapter/bridge.rs).
Map and region use the same concrete memory type, avoiding a separate generic
engine instantiation solely for the region view. There is no second owned
persistent B-tree available to substitute for Map.

The other tree family is Rust's heap `BTreeSet`, used for
[admitted StableKeys](../../crates/ic-storage/src/runtime/admission.rs),
[live allocation leases](../../crates/ic-storage/src/runtime/directory_model/leases.rs)
and [backing custody](../../crates/ic-storage/src/runtime/custody.rs). These sets
track transient runtime authority; they do not implement persistent collection
records. Substituting a heap map and rebuilding it from storage would change
reopen cost and memory use. Conversely, running these transient sets through a
persistent codec/allocator would add machinery they do not require.

The existing qualified size run `attempt.ORcC78` already measures combined use:

| Application | Storage total bytes | Storage code payload bytes | ICSS total bytes | ICSS code payload bytes |
| --- | ---: | ---: | ---: | ---: |
| Map | 113,710 | 97,634 | 90,195 | 49,231 |
| Map + region | 116,631 | 100,424 | 110,859 | 69,760 |
| All collections | 127,027 | 109,063 | 119,324 | 77,133 |

Adding region to Map therefore adds 2,921 total / 2,790 code bytes for Storage,
versus 20,664 / 20,529 for ICSS in these fixtures. These are incremental
application sizes, not isolated engine sizes or a fresh implementation change.
Map/region sharing is supported by both the concrete types and these results.

For attribution, the installed Rust/Cargo 1.99.1 rebuilt the all-collection
consumer with its existing `wasm-size` profile and only stripping disabled:

```sh
CARGO_PROFILE_WASM_SIZE_STRIP=none cargo build --locked --offline \
  --manifest-path qualification/Cargo.toml -p storage-size-consumer --lib \
  --target wasm32-unknown-unknown --profile wasm-size --no-default-features \
  --features map,set,vector,cell,region --target-dir target/storage-size-build
twiggy top -f json PATH_TO_NAMED_WASM
```

No dependency/profile/source selection was edited. A section reader checks the
Wasm header, decodes section lengths as unsigned LEB128, hashes section 10's
payload, and separately hashes the header plus every non-custom section. Both
hashes match the existing qualified artifact exactly:

- Code payload: 109,063 bytes,
  SHA-256 `74051356e51fc25a203ca237c56590279e1d659de39c233748616ab32625f8c5`.
- All non-custom sections with header:
  SHA-256 `608afc5b70842a61983cd7f365fea68950787b97360b4a6c05ab399e64dbc3f9`.

The diagnostic module totals 318,393 bytes because it retains custom debug/name
sections; it is not a deployable size regression or the application-size metric.
The previous source manifest matches all recorded inputs except the benchmark
README, which has subsequent documentation edits. This is not a claim that the
entire source archive is unchanged. Executable/data identity is checked directly.

Installed Twiggy 0.8.0 attributes these shallow symbol bytes:

| Named function group | Functions | Shallow bytes |
| --- | ---: | ---: |
| Rust B-tree functions naming AllocationId | 20 | 3,935 |
| Rust B-tree functions naming BackingIdentity | 19 | 3,995 |
| Rust B-tree functions naming StableKey | 6 | 1,113 |
| Other Rust B-tree functions | 1 | 35 |
| All Rust B-tree groups above | 46 | 9,078 |
| Owned ordered-map functions | 57 | 13,274 |

Groups use the `::collections::btree::` and `::ordered_map::` name substrings.
These are named function sizes, not complete subsystem costs or achievable
deletion savings: inlining places work in callers, shared helpers have other
names, and replacement code also occupies bytes. The binary has one named
`balance`, `rotate_left`, `rotate_right` and `detach_first` body, despite the
Map/Set value-type difference. Separate `insert_at` and `remove_at` bodies exist
for `u64` and unit values. Source generics alone therefore do not prove duplicate
machine code, and moving balancing helpers cannot be credited with saving a
second body that is already absent.

This narrows the follow-up: preserve one ordered engine for Map/Set/regions;
evaluate a packed replacement in those same consumers rather than adding a
second production engine. Independently investigate whether small transient
custody/lease sets warrant their B-tree code. Sorted vectors or directory-local
lease flags are hypotheses, not selected changes: admission order, duplicate
rejection, detached-handle custody, drop behavior, supported large populations
and sparse live-lease memory use must remain correct. Keep strong identity types.
No claim is made that replacing these sets would save the full 9,078 bytes.

Build log, named artifact and Twiggy JSON remain at
`target/evidence/ordered-engine-sharing-20261111/`. Only attribution was run;
no new instruction benchmark, engine replacement or release occurred.

## Retained custody-registry simplification — 2026-11-11

Only the transient physical-backing claim registry changes, from `BTreeSet` to
a sorted `Vec<BackingIdentity>` in
[custody.rs](../../crates/ic-storage/src/runtime/custody.rs). It keeps binary
search for duplicate detection and removal, retains the native mutex/Wasm
thread-local boundary, and releases vector capacity after the last claim drops.
No identity type, public API, lease ownership, admission order, stored format,
collection engine or dependency changes. StableKey admission and AllocationId
lease sets still use their existing trees. The first-party custody integration
is outside the restricted independent-engine implementation slice.

The vector removes a tree specialization from applications that use Storage,
including Cell/Vector-only consumers. It tracks physical backings rather than
individual allocations: DefaultMemoryImpl always identifies the same canister
backing. The implementation still accepts arbitrarily many distinct custom
backings. Insertion/removal now move O(n) identities; lookup stays O(log n).
Capacity can remain at its high-water mark while any claims survive. These
tradeoffs are not hidden behind a new configuration limit or backend option.

### Matched application sizes

Before is the qualified `storage-consumer-size/attempt.ORcC78`; after is
`storage-consumer-size/attempt.3MVuA5`. Both use the same range-aware application
and Rust 1.99.1 `wasm-size` profile. Values are bytes; code columns are the
decoded section-10 payload, not compression or named-symbol totals.

| Application | Storage total before | Total after | ICSS total | Storage code before | Code after | ICSS code |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Base | 16,893 | 16,893 | 16,889 | 14,773 | 14,773 | 14,773 |
| Map | 113,710 | 108,822 | 90,195 | 97,634 | 92,690 | 49,231 |
| Set | 112,813 | 107,925 | 89,955 | 96,819 | 91,875 | 48,940 |
| Vector | 99,918 | 95,026 | 64,506 | 85,912 | 80,968 | 27,577 |
| Cell | 97,643 | 92,755 | 63,093 | 84,271 | 79,327 | 26,077 |
| Region | 115,077 | 110,189 | 89,904 | 98,935 | 93,991 | 49,043 |
| Map + region | 116,631 | 111,743 | 110,859 | 100,424 | 95,480 | 69,760 |
| All collections | 127,027 | 122,139 | 119,324 | 109,063 | 104,119 | 77,133 |

All Storage selections except Base lose 4,944 code bytes. Total reduction is
4,888 bytes, or 4,892 for Vector; Base and all ICSS sizes are unchanged.
All-collection total size remains 2,815 bytes above ICSS, and code payload
remains 26,986 bytes above it. This does not establish overall size parity.
The reduction exceeds the earlier 3,995-byte BackingIdentity symbol group;
shallow symbol attribution was explicitly not a predicted deletion saving.

### Installed instruction comparison

The unchanged admission fixture and driver compare fresh before/after modules
over 1/2/4/8/16/64/255/256/1,024 names, 0/1/32 buckets, and existing open,
held-lease, read, drop, verification, cold reopen and append operations. Each
Storage run validates 864 calls; ICSS validates 600 within its supported ID
limit and operation set. Results and pages agree between Storage runs.

Selected medians below use zero buckets; read rows exercise the empty-memory
branch, not a populated payload read. The complete comparison also includes
the populated one- and 32-bucket cases.

| Names / operation | Storage before | After | ICSS |
| --- | ---: | ---: | ---: |
| 1 / cold reopen | 21,729 | 21,943 | 805,171 |
| 16 / cold reopen | 166,133 | 166,292 | 805,171 |
| 255 / cold reopen | 2,592,912 | 2,593,071 | 805,171 |
| 1,024 / cold reopen | 10,354,517 | 10,354,676 | Not supported |
| 1 / open one | 162,257 | 162,257 | 160,391 |
| 16 / open all held | 262,916 | 262,821 | 164,337 |
| 255 / open all held | 3,367,625 | 3,366,263 | 219,307 |
| 1,024 / open all held | 15,981,749 | 15,976,183 | Not supported |
| 16 / held read x32 | 160,352 | 160,352 | 160,311 |

Of 864 paired samples, 45 improve, 657 are unchanged and 162 increase. The
324 median groups have 15 improvements, 255 unchanged and 54 increases. All
increases are in reopen/append. Maximum absolute increase is 420 instructions;
maximum relative increase is 0.985%, the one-name zero-bucket reopen above.
The baseline and candidate each own one backing per canister, regardless of
the number of names. These runs do not benchmark many custom physical backings.
Whole endpoint effects and allocator/page-charge attribution are not isolated.
ICSS admission semantics differ: it does not supply Storage's namespace,
exclusive-lease and durable-directory checks. These are boundary comparisons,
not equivalent security-contract claims or new Map traversal measurements.

Retain this as a size improvement with a small measured admission cost, not as
an instruction speedup. The collection algorithms and live-lease lookup policy
are unchanged; the full collection instruction catalogue was not rerun.

### Qualification and reproduction

- 23 runtime library tests and five public runtime tests pass. The new custody
  test interleaves 257 independent backings, refuses aliases, reclaims dropped
  identities and checks surviving claims without backing IO. Existing tests
  cover constructor failure/unwind and detached-handle custody.
- Strict native library/public-runtime Clippy and Wasm library Clippy pass.
- All 16 size-consumer modules pass 127 calls and two upgrades each.
- The focused installed Storage fixture passes IO, non-initializing admission,
  upgrade across 1,031 allocations and decoder/update/upgrade trap rollback.
- Both Cargo locks and retained artifact/source hashes are unchanged by
  qualification. Formatting, documentation links and scoped diff checks pass.
  Full release gates, publication and sibling qualification were not run.

Raw evidence is `target/evidence/custody-set-20261111/`: before source/diff,
native checks, independently frozen baseline/candidate/reference admission
modules, three CSVs, `comparison.json`, `size-comparison.json`, installed logs
and input hashes. Application source/artifact archives remain with the two size
runs. Current source reproduces the candidate via `make benchmark-storage-size`;
instruction reproduction uses the maintained `admission-wasm` example built
separately with `owned` and `dfinity`, and the native `admission` driver against
the prepared Testkit 0.95/PocketIC 16.1.0 server. Pass `--icss` for that control.
The build inputs use `qualification/Cargo.toml`, `--locked --offline`, target
`wasm32-unknown-unknown` and profile `wasm-size`. Preserve each module before
building the other backend. No deployment is included.

## Map hot-path experiments — 2026-11-11

The maintainer requested five focused experiments after the audit. Steps 1–3
and 5 below are retained. Step 4 was measured and removed. Step 5 adds a Cell
reopen workload and qualifies direct buffer initialization. No stored format,
public collection API, ownership check or dependency selection changes.

The separate ordered-engine implementer used its current independently authored
engine/tests, the replacement contract, pinned rules and black-box results.
The coordinator changed the public adapter integration and acceptance tests.
No ICSS implementation source or predecessor patches entered that implementer.

1. **Replacement writes:** omit routing serialization when the saved value
   address and length are unchanged. Payload writes, extent/height assertions,
   old-value decoding and resizing publication remain. The scalar matrix has
   12 improved and 282 unchanged Storage samples. Variable-payload controls have
   45 improved, 549 unchanged and 18 regressed samples; the regressions are only
   8–9 instructions on growing/shrinking replacements. All ICSS controls match.
2. **Owned inline values:** retain encoded values up to the existing 24-byte
   inline limit without a heap buffer. Point reads decode from that buffer;
   scan entries remain owned and defer value decoding. Larger values retain
   their owned vectors. Scalar samples: 144 improve, 114 are unchanged and 36
   regress, with missing-key median increases up to 0.166%. The variable-value
   matrix has 144 improved, 48 unchanged and 12 regressed medians; the largest
   median increase is 0.276%. ICSS controls match in both matrices.
3. **Deferred cursor advancement:** retain the yielded path top and advance only
   when another item is requested from that end. Scalar samples: 102 improve,
   192 are unchanged, none regress. The wider ordered study validates 12,312
   observations per version: 900 medians improve, 1,152 are unchanged and none
   regress. All ICSS medians match. Unvisited corrupt children are now rejected
   when traversal requests them, rather than during unused lookahead; affected
   nodes are never yielded without their structural checks.
4. **Rejected root handoff:** scoped region admission supplied its validated
   root to the first traversal. Native tests demonstrate one fewer root read,
   but the extra state/copying/branching did not produce an IC improvement.
   Of 294 Storage samples, 276 regress, 12 are unchanged and only six Control
   samples improve by six instructions. The extra `Option<Node>` occupies 112
   native bytes per Store. This implementation and its specific tests were
   removed; the files match the retained step-3 source archive exactly.

### Matched scalar instructions

These fresh runs use 4,096 shuffled entries, uniform access, batches/limits 32,
one sample and three stateful rounds with the same seed sequence. They are not
the earlier three-sample scalar runs. The ICSS 0.7.2 control uses 128-page buckets;
Storage retains its governed backing. Each scalar matrix has 98 median rows and
588 observations across both libraries. Counts below are measured inner-operation
instructions, not whole application calls or page-fault counts.

| Operation | Before | Step 1 | Step 2 | Step 3 retained | Step 4 rejected | ICSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Map get | 1,145,264 | 1,145,264 | 1,135,600 | 1,135,600 | 1,142,804 | 914,982 |
| Map replace | 1,919,674 | 1,907,674 | 1,898,010 | 1,898,010 | 1,909,566 | 1,599,094 |
| Map scan | 761,401 | 761,401 | 740,694 | 734,856 | 735,484 | 309,902 |
| Map keys scan | 405,027 | 405,027 | 405,027 | 314,977 | 315,493 | 289,750 |
| Set get | 1,039,750 | 1,039,750 | 1,039,750 | 1,039,750 | 1,046,958 | 826,957 |
| Set scan | 419,660 | 419,660 | 404,428 | 314,378 | 314,894 | 212,181 |
| Region get | 1,219,469 | 1,219,469 | 1,209,456 | 1,209,456 | 1,210,478 | 933,864 |
| Region replace | 1,995,671 | 1,983,031 | 1,973,111 | 1,973,111 | 1,974,229 | 1,621,923 |
| Region keys scan | 335,248 | 335,248 | 335,248 | 329,571 | 330,022 | 291,731 |

Scalar stable capacity is unchanged: 34 Storage pages versus 129 ICSS pages
under this explicit bucket selection. This is not a payload-density comparison:
the ICSS control reserves a larger bucket. No native interval count is priced
as an IC surcharge.

Variable-payload controls use 256 entries and 31/32/4096-byte Vec values, with
204 median rows and 1,224 observations per version. For 32 replacements of
4-KiB values, step 1 falls from 4,807,508 to 4,634,676 instructions; ICSS remains
7,532,524. The initial combined scalar/variable selection was refused because
the requested payloads were not admitted by every type. An eight-byte Vec
selection was also refused by the benchmark catalogue. The corrected matrices
use the same frozen modules; refusal logs are retained, not counted as passes.

### Matched application sizes

The unchanged production-shaped consumers use Rust 1.99.1, `wasm-size`, matched
features and no post-link optimizer. Each candidate qualifies 16 modules with
127 calls and two upgrades per module. Values are **uncompressed total / code
section payload bytes**, read from the final Wasm section framing. These are
application fixtures, not the broad benchmark dispatch modules.

| Selection | Before | Step 1 | Step 2 | Step 3 retained | Step 4 rejected | ICSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Base | 16,893 / 14,773 | 16,893 / 14,773 | 16,893 / 14,773 | 16,893 / 14,773 | 16,893 / 14,773 | 16,889 / 14,773 |
| Map | 108,822 / 92,690 | 108,188 / 92,055 | 108,576 / 92,394 | 108,636 / 92,438 | 108,796 / 92,598 | 90,195 / 49,231 |
| Set | 107,925 / 91,875 | 107,292 / 91,241 | 107,721 / 91,620 | 107,748 / 91,631 | 107,936 / 91,819 | 89,955 / 48,940 |
| Vector | 95,026 / 80,968 | 95,026 / 80,968 | 95,026 / 80,968 | 95,026 / 80,968 | 95,026 / 80,968 | 64,506 / 27,577 |
| Cell | 92,755 / 79,327 | 92,755 / 79,327 | 92,755 / 79,327 | 92,755 / 79,327 | 92,755 / 79,327 | 63,093 / 26,077 |
| Region | 110,189 / 93,991 | 109,555 / 93,356 | 109,852 / 93,605 | 109,897 / 93,634 | 109,856 / 93,592 | 89,904 / 49,043 |
| Map + region | 111,743 / 95,480 | 111,109 / 94,845 | 111,518 / 95,205 | 111,578 / 95,249 | 111,583 / 95,253 | 110,859 / 69,760 |
| All | 122,139 / 104,119 | 122,220 / 104,199 | 122,957 / 104,884 | 123,066 / 104,976 | 123,171 / 104,856 | 119,324 / 77,133 |

Step 1 changes all-collection total/code by +81/+80 bytes, step 2 by +737/+685,
and step 3 by +109/+92. Combined, the retained three steps add 927 total and
857 code bytes. Map-only combined size falls by 186 total and 252 code bytes.
Step 4 adds another 105 total bytes while removing 120 code bytes; that code
shrink does not justify its instruction and per-map state regressions.

### Evidence and reproduction

Evidence under `target/evidence/map-hot-paths-20261111/` includes native test/lint
logs, retained baseline source, frozen driver, scalar/variable/ordered reports,
comparison summaries and lock hashes. The installed Testkit 0.95/PocketIC 16.1.0
pair is unchanged. Source archives/modules and the eight-selection size matrix
are also retained at these attempt paths:

| Stage | `storage-comparison/attempt.*` | `storage-consumer-size/attempt.*` |
| --- | --- | --- |
| Before | `nHBWzs` (modules; corrected reports under the evidence directory) | `3MVuA5` |
| Step 1 | `pbmoGU` | `UkKgRb` |
| Step 2 | `f9K07W` | `wEz7L2` |
| Step 3 | `TtQm8V` | `wKTs3e` |
| Rejected step 4 | `1bqvko` | `FG0aJD` |

All attempt roots are under `target/evidence/`. The successful current-source
scalar command is:

```sh
BENCH_CASES=Map-U64-U64,Set-U64-Unit,Region-U64-U64 \
  BENCH_PATTERNS=uniform BENCH_ROUNDS=3 BENCH_SHUFFLED=1 \
  BENCH_UPSTREAM_BUCKET_PAGES=128 \
  make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=1
BENCH_WORKLOAD=ordered BENCH_UPSTREAM_BUCKET_PAGES=128 \
  make benchmark-storage BENCH_SAMPLES=1
make benchmark-storage-size
```

Frozen-module replays use the retained native driver, `--owned-wasm` and
`--dfinity-wasm`, through the installed Testkit server runner. Variable controls
select `--cases Map-U64-Bytes,Cell-U64-Bytes --payloads 31,32,4096 --rows 256
--patterns uniform --rounds 3 --shuffled --samples 1 --upstream-bucket-pages 128`.
Current-source commands reproduce the current implementation, not a removed
candidate. Archives are necessary to recover the unpublished intermediate
sources after edits; hashes alone cannot do that after explicit cleanup.

### Direct buffer initialization (step 5)

External Map blobs (keys and values) and Cell loading now read directly into
fresh vector capacity instead of zero-filling that buffer first. Bounds, block
capacity and Cell length admission still precede payload reads. The vector's
length remains zero until IO succeeds, so partial-read unwinding cannot expose
uninitialized bytes to a decoder. Empty Cell payloads perform no IO. The separate
Cell implementer used only current independent Cell code/tests, its contract,
the owned Memory interface and pinned rules; the ordered implementer retained
its established restricted inputs. No upstream implementation was supplied.

Both production native and Wasm backings provide direct raw reads through the
governed handle. A custom backend that only implements `Memory::read` instead
uses Memory's bounded 1-KiB scratch fallback, with extra reads/copies. These IC
results are not a claim of improved performance for that fallback. No new cache,
public unsafe API, persisted format or configuration is introduced.

All comparisons below use the same workload and ICSS 0.7.2 with 128-page buckets
before/after. The reference Wasm is byte-identical. Logical outputs, final state
and reported stable capacities are unchanged. The new broad benchmark modules
include Cell reopening; its baseline is rebuilt from the retained step-3 engine,
so these deltas do not compare different benchmark dispatch implementations.

| Selected operation | Population / batch | Before step 5 | Retained step 5 | ICSS |
| --- | --- | ---: | ---: | ---: |
| Map Get, 31-byte value | 256 / 32 | 439,453 | 434,365 | 514,465 |
| Map Get, 4-KiB value | 256 / 32 | 3,194,405 | 3,059,237 | 3,137,119 |
| Map Get, 64-KiB value | 8 / 1 | 455,510 | 389,846 | 1,058,286 |
| Map Get, 1-MiB value | 8 / 1 | 6,021,870 | 4,973,166 | 16,151,418 |
| Map Replace, 1-MiB value | 8 / 1 | 8,431,812 | 7,383,108 | 64,546,506 |
| Map Get, 32-byte array key/value | 256 / 32 | 760,252 | 721,212 | 659,668 |
| Set Get, 64-byte array key | 256 / 32 | 1,010,534 | 969,830 | 1,125,216 |
| Cell OpenControl, u64 | 1 / 1 | 83,462 | 83,450 | 81,919 |
| Cell OpenControl, 4-KiB bytes | 1 / 1 | 91,850 | 87,638 | 86,157 |
| Cell OpenControl, 64-KiB bytes | 1 / 1 | 294,730 | 229,078 | 387,597 |
| Cell OpenControl, 1-MiB bytes | 1 / 1 | 5,860,899 | 4,812,207 | 4,970,637 |

Cell OpenControl includes lease acquisition, read-only admission, decode,
surrounding request machinery and drop, with the allocation owner retained.
It is neither cached `get()` nor an isolated stable-read measurement. ICSS's
unused default is constructed before timing. Every round restores the declared
payload before OpenControl/Get, then checks growth and shrinkage with reads
after each. The initial generic Cell phase order left later-round controls
reading a previous shrink; that methodology run remains in `vRaalm/report` but
is not used in this table. The corrected driver is frozen as `driver-reopen`,
with its corrected source in the evidence directory and final source archive.

The 1-MiB Cell OpenControl improves 17.89%; 64 KiB improves 22.28%. A reopened
Get after growing 1 MiB to 1.5 MiB falls from 14,402,586 to 12,829,606; ICSS is
12,827,999. That phase includes cloning/result construction and is not the same
measurement as OpenControl. The scalar Map/Set/region control is exactly
unchanged from step 3, so the remaining scalar scan gap in the earlier table
is not eliminated by this change.

| Matched matrix | Observations per version, both libraries | Improved Storage medians | Equal | Regressed |
| --- | ---: | ---: | ---: | ---: |
| Scalar, 4,096 shuffled entries | 588 | 0 | 98 | 0 |
| Map byte values 31/32/4096 | 1,008 | 99 | 69 | 0 |
| Map array32 / Set array64 keys | 456 | 72 | 4 | 0 |
| Map bulk, 64 KiB / 1 MiB | 112 | 32 | 24 | 0 |
| Reopened Cell, u64 and five byte sizes | 228 | 25 | 13 | 0 |

All ICSS medians are unchanged. Scalar samples also match individually (294
per backend). Other rows report medians from one sample with three stateful
rounds, except bulk's single round; these are deterministic matched traces,
not statistical throughput estimates or a complete rerun of the type catalogue.

The unchanged eight application pairs again pass 127 calls and two upgrades
each. Values are uncompressed total / code-section payload bytes:

| Selection | Before step 5 | Retained step 5 | ICSS |
| --- | ---: | ---: | ---: |
| Base | 16,893 / 14,773 | 16,893 / 14,773 | 16,889 / 14,773 |
| Map | 108,636 / 92,438 | 109,076 / 92,844 | 90,195 / 49,231 |
| Set | 107,748 / 91,631 | 108,188 / 92,037 | 89,955 / 48,940 |
| Vector | 95,026 / 80,968 | 95,026 / 80,968 | 64,506 / 27,577 |
| Cell | 92,755 / 79,327 | 93,021 / 79,561 | 63,093 / 26,077 |
| Region | 109,897 / 93,634 | 110,337 / 94,040 | 89,904 / 49,043 |
| Map + region | 111,578 / 95,249 | 112,018 / 95,655 | 110,859 / 69,760 |
| All | 123,066 / 104,976 | 123,076 / 104,985 | 119,324 / 77,133 |

Step 5 adds only 10 total / 9 code bytes to all collections, though standalone
Map and Cell fixtures add 440/406 and 266/234 respectively. Across retained steps
1–3 and 5, all collections add 937/866 bytes; Map alone adds 254/154. These
figures exclude the preceding custody/range batches and the benchmark handlers.

Final qualification: 122 Storage library tests and 26 public Cell/ordered/runtime
tests pass, including direct/default read failures, no decode after partial
initialization, corruption refusal, ownership, growth and mixed iterator behavior.
The benchmark library/driver passes 42 tests. Native/Wasm strict lint, targeted
formatting, links and scoped diff checks pass. Installed Storage IO/admission,
upgrade across 1,031 named allocations, foreign/malformed snapshot refusal and
decoder/update/upgrade rollback pass. An initial test command named the nonexistent
`runtime_contract` target; its failure is retained and the correct `runtime`
target passed. Both Cargo lockfiles remain unchanged. No release gate or deployment
is included.

Evidence continues under `target/evidence/map-hot-paths-20261111/`: `step5-*`
reports, logs, comparisons, corrected frozen driver and qualification module.
Baseline modules/source are `storage-comparison/attempt.vRaalm`; final modules,
corrected sources and Cell report are `storage-comparison/attempt.eCLz3Y`.
The corrected baseline Cell report is `step5-before-cell-corrected`.
Application source/modules are `storage-consumer-size/attempt.YBMpeR`, paired
with step 3's `wKTs3e`. Baseline/candidate scalar, small-value, key and bulk
replays use the same frozen driver and the two module sets.

Reproduce Cell with `BENCH_WORKLOAD=cell-open BENCH_UPSTREAM_BUCKET_PAGES=128
make benchmark-storage BENCH_SAMPLES=1`. For frozen driver controls, use
`--samples 1 --upstream-bucket-pages 128` plus:

- Small values: `--cases Map-U64-Bytes --payloads 31,32,4096 --rows 256
  --patterns uniform --rounds 3 --shuffled`.
- External keys: `--cases Map-Array32-Array32,Set-Array64-Unit --rows 256
  --patterns uniform --rounds 3 --shuffled`.
- Bulk: `--workload bulk --collection map`.

Run through the explicitly prepared Testkit server as above. Current application
size reproduction remains `make benchmark-storage-size`. Retain the frozen
intermediate sources for historical comparison; current commands run current code.

## Lease and Vector experiments — 2026-11-11

Fresh comparisons on source `256157b7` plus the preserved qualification-rustix
fix select Wire 0.21.1, Runtime 0.21.1, Build 0.83.0 and Toko 0.39.10.
Rust/Cargo 1.99.1, Testkit 0.95 and PocketIC 16.1.0 are unchanged.
Application consumers use `wasm-size`; instruction harnesses use `comparison`
with the existing 8-MiB stack flag. These are original compiler modules, with
no post-link optimizer. Both backends receive matched settings and logical work.

Three isolated trials retain the live-lease bitmap and Vector checked-read
simplification. The combined Map read is rejected and removed completely.
Formats, public APIs, allocation admission, exclusive custody and native owner
poisoning remain unchanged. No new dependency or deployment is included.

### Application Wasm after every step

Each cell is **total uncompressed module bytes / code-section payload bytes**,
measured by the maintained size runner's Wasm section parser. These eight
application consumers are separate from the broad performance harness.
The Map trial includes the retained lease change; final includes lease and
Vector changes with the Map trial removed.

| Consumer | Before | Lease bitmap | Map trial, rejected | Final | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: | ---: |
| base | 16,893 / 14,773 | 16,893 / 14,773 | 16,893 / 14,773 | 16,893 / 14,773 | 16,889 / 14,773 |
| map | 109,076 / 92,844 | 102,426 / 87,111 | 103,972 / 88,358 | 102,426 / 87,111 | 90,195 / 49,231 |
| set | 108,188 / 92,037 | 101,538 / 86,304 | 101,534 / 86,304 | 101,538 / 86,304 | 89,955 / 48,940 |
| vector | 95,026 / 80,968 | 88,382 / 75,242 | 88,382 / 75,242 | 88,243 / 75,159 | 64,506 / 27,577 |
| cell | 93,021 / 79,561 | 86,355 / 73,817 | 86,355 / 73,817 | 86,355 / 73,817 | 63,093 / 26,077 |
| region | 110,337 / 94,040 | 103,734 / 88,355 | 105,270 / 89,600 | 103,734 / 88,355 | 89,904 / 49,043 |
| map-region | 112,018 / 95,655 | 105,397 / 89,952 | 106,953 / 91,209 | 105,397 / 89,952 | 110,859 / 69,760 |
| all | 123,076 / 104,985 | 116,408 / 99,236 | 117,933 / 100,471 | 116,288 / 99,172 | 119,324 / 77,133 |

Net all-collection reduction is **6,788 total / 5,813 code bytes**.
Final total size is 3,036 bytes below this ICSS consumer, while its code section
remains 22,039 bytes larger. Individual structures are still larger than ICSS;
the all-collection result is not a universal size claim.

### Matched scalar instructions after every step

These are three-round medians for 4,096 shuffled records, uniform access and
32 operations/results per call and one sample. ICSS uses its explicit
128-page bucket control; Storage
keeps its eight-page allocation units. Equal logical work does not imply equal
bucket reservation. IC memory charges are included; these are not complete
application endpoint measurements.

| Operation | Before | Lease bitmap | Map trial, rejected | Final | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Map Get | 1,135,600 | 1,135,600 | 1,096,615 | 1,135,600 | 914,982 |
| Map MissAbove | 835,099 | 835,099 | 900,329 | 835,099 | 895,291 |
| Map Scan | 734,856 | 734,856 | 735,920 | 734,856 | 309,902 |
| Set Get | 1,039,750 | 1,039,750 | 1,049,073 | 1,039,750 | 826,957 |
| Region Get | 1,209,456 | 1,209,648 | 1,173,581 | 1,209,648 | 933,864 |
| Vector Get | 103,413 | 103,413 | 103,413 | 99,543 | 102,932 |
| Vector Scan | 102,565 | 102,565 | 102,565 | 98,697 | 95,015 |
| Vector Push | 201,716 | 201,685 | 201,685 | 198,871 | 186,644 |

The 124-row scalar matrix also includes single operations, misses, updates,
removals, endpoints, reverse/bounded/key scans and rank operations.

### Step 1: retain indexed lease bookkeeping

The live-ID search tree becomes a lazily grown bitmap indexed by already stable
internal directory positions. IDs remain opaque to callers. Dropping a lease
clears only its bit; detached handles retain backing custody. Diagnostics count
set bits rather than maintaining a second counter. The bitmap remains allocated
to the highest opened index until the owner dies: at most 512 logical bytes for
the default 4,096-entry directory, excluding Vec capacity rounding/header.

Installed admission validates 864 samples per Storage version and 600 ICSS
controls. With zero data buckets, representative later-call samples are:

| Operation | Before | Bitmap | ICSS |
| --- | ---: | ---: | ---: |
| Open/drop one of 16 names | 164,440 | 164,067 | 160,391 |
| Hold/drop all 16 names | 262,821 | 247,891 | 164,337 |
| Hold/drop all 64 names | 737,432 | 649,759 | 175,377 |
| Hold/drop all 1,024 names, stress only | 15,976,183 | 13,539,921 | unavailable |

ICSS's control supplies no equivalent namespace/admission/exclusive-lease
guarantees and admits at most 255 names in this harness. Across Storage admission
samples, 393 improve, 216 are equal and 255 rise. The largest relative increase
is 66 instructions on a 21,943-instruction reopen (0.30%); retained 32-read calls
rise by 352 instructions (0.14%). The scalar matrix has 15 improvements,
84 unchanged medians and 25 increases, including a 55-instruction increase in
the single-operation Vector control and several short operations.
This is a size/open-drop win, not universal instruction improvement.
Sparse leases, adjacent bit positions, directory extension, duplicate refusal,
drop/reopen, detached custody and poisoned owners have explicit tests.

### Step 2: reject combined Map routing/value reads

The trial read routing plus the declared inline value bound (at most 24 bytes)
at each node for point gets. It kept key-only reads and iterator nodes unchanged,
and shared physical-node/root/child checks. Tests validated complete extent
admission and codec checks before decoding, including malformed lengths.

The 4,096-row scalar Map hit improves 3.43% at batch 32, but misses regress
7.81%; the single-hit result improves 44.8%, illustrating sensitivity to tracked
memory regions rather than a general algorithmic gain. At 256 rows, bounded
U8/U128/20-byte values and variable Blob20 hits regress about 4–8%, with misses
roughly 9–11% worse. The eight-case type control has 491 regressed, 25 improved
and four equal Storage medians. ICSS refuses the OptionU64 setup because its
fixed-width expectation rejects the shorter None encoding: 52 rows retain
Storage-only comparisons, with ICSS unavailable rather than zero cost.
The remaining 468 rows are paired reference comparisons.

The trial adds 1,525 total / 1,235 code bytes to the all-collection consumer.
Reading/copying tails on routing nodes and the changed shared parsing path do
not earn their cost across the controls. All three trial source/test files are
restored exactly to their pretrial state; no alternate path or option remains.

### Step 3: retain Vector checked-fact reuse

Successful get/pop/iterator selection uses the cached committed length and the
selected slot's existing live size/bounds check, instead of checking size twice.
Misses, empty pop, exhausted/skipping iterators, count and size hints still
observe poisoned ownership. Checked complete slot ends also determine starting
offsets. Set retains its pre-encoding health check, and writes still verify
actual capacity after growth. No persisted size cache or safety bypass is added.

Against the retained lease baseline, the scalar matrix has 23 improvements,
99 unchanged medians and two misses each increasing by two instructions.
All 140 fixed/variable read-mutation medians improve at 256 rows. Representative
32-read batches are 105,219 → 101,411 instructions for Array32 (ICSS 104,576),
239,720 → 236,072 for WireRecord (ICSS 232,212), and
2,887,697 → 2,884,047 for 4-KiB values in the 1-MiB-bound Chunk type
(ICSS 2,961,934). Follow-up reads after set/push/pop remain correct.

Auth-shaped SessionEnvelope sweeps cover 1/512/1,024/2,048 records, first/middle/
last positions, 256/2,048-byte records and query/update executions. Of 240
medians, 192 improve and 48 are unchanged. At 2,048 records, first-position
2-KiB update reads improve 86,452 → 86,336 instructions (ICSS 86,135);
the in-execution warm read improves 6,452 → 6,336 (ICSS 6,135).
The 116-instruction saving does not explain Canic's configuration preparation
cost or qualify a whole Auth endpoint speedup.

Vector-only Wasm saves another 139 total / 83 code bytes; all-collection saves
120 total / 64 code bytes. Eleven format tests, twelve public Vector tests,
twelve lease tests and five public runtime tests pass. New checks exercise
poisoning from a neighbor on selected/missing/exhausted paths and refuse missing
or overflowing read/write slots before IO. Native and Wasm Clippy pass.
All sixteen size modules pass 127 calls and two upgrades each at each stage.
Final installed Storage qualification passes IO, admission, upgrade across
1,031 named allocations, and decoder/update/upgrade trap rollback.
Both selected lockfiles are unchanged by the experiments. Full release gates,
the complete type catalogue, fresh macOS qualification and publication were not run.

### Evidence and reproduction

Raw inputs, logs, comparisons and frozen admission modules are under
`target/evidence/lease-map-vector-20261111/`. Source archives, hashes and
collection/application Wasms are retained in:

| Stage | storage-comparison attempt | storage-consumer-size attempt |
| --- | --- | --- |
| Before | qtVwLH | PmNhZK |
| Lease | EjHVZO | kklrbB |
| Rejected Map | NcaltQ | SySUqX |
| Final lease + Vector | HUpsEh | gSebAh |

All attempt roots are under `target/evidence/`. Current-source reproduction:

```sh
make benchmark-storage-size
BENCH_CASES=Map-U64-U64,Set-U64-Unit,Region-U64-U64,Vector-U64-U64 \
  BENCH_PATTERNS=uniform BENCH_ROUNDS=3 BENCH_SHUFFLED=1 \
  BENCH_UPSTREAM_BUCKET_PAGES=128 \
  make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=1
BENCH_WORKLOAD=vector-reads BENCH_UPSTREAM_BUCKET_PAGES=128 \
  make benchmark-storage BENCH_SAMPLES=1
BENCH_WORKLOAD=auth-lookups BENCH_UPSTREAM_BUCKET_PAGES=128 \
  make benchmark-storage BENCH_SAMPLES=1
```

Frozen-module replays use the existing native driver with `--owned-wasm`,
`--dfinity-wasm`, `--output`, `--samples 1` and `--upstream-bucket-pages 128`,
through the installed Testkit runner. The rejected Map control additionally uses
`--cases Map-U64-Unit,Map-U64-U8,Map-U64-U128,Map-U64-Array20,Map-U64-Blob20,Map-U64-OptionU64,Map-U64-Array32,Map-Array64-U128 --rows 256 --rounds 3 --patterns uniform --shuffled`.
Admission uses the maintained `admission-wasm`/native `admission` examples,
as described above. Retained source archives reproduce unpublished intermediate
trials; current commands reproduce only retained code.

Vector implementation work used a separate implementer reading the current
independently authored Vector files, current specification (including its later
integrator notes), manifests and pinned policy. The Map implementer read only
the current independent ordered files and its approved specification context.
Neither used predecessor or registry ICSS implementation text. The parent
authored separate public poison tests and performed qualification.
