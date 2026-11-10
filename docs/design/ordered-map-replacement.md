# Independent ordered-map implementation contract

## Restricted implementation inputs

This section specifies public behavior and newly selected private integration
interfaces. It supplies no predecessor algorithms, source, storage layout or
expressive tests. The coordinating session has seen predecessor code and must
not implement the replacement engine itself. Use a separate unexposed implementer.

The implementer may read this document, AGENTS.md, TOKO-TOOLING.md and their
reviewed engineering rules, Cargo manifests, and its own newly authored files.
Do not read existing engine, adapter, codec, test or benchmark implementations,
Git history, registry ICSS source, or sibling implementations. Do not read the
broader replacement inventory as an implementation blueprint. Record actual
inputs and any accidental exposure. Coordination and acceptance use public
contracts; performance comparison uses external ICSS as a black box.

## Behavior

Implement one persistent ordered key/value engine supporting Map, unit-valued
Set, and operation-local map-region views. Public ordering is Rust `K: Ord`,
not lexicographic encoded bytes. `K` is cloneable; values need not be cloneable.
Support zero-byte records, finite fixed/variable codecs and unbounded codecs,
including values exceeding a Wasm page and keys exceeding a small node.

- Insert returns the previous decoded value, or none for a new key. Remove
  returns the removed value, or none for an absent key. Missing removal is
  read-only. Get returns the matching value. Membership never decodes values.
- Length and emptiness reflect committed mutations. First/last return the
  minimum/maximum pair. Clear removes everything and reuses existing backing.
- Ordered iteration and included/excluded/unbounded ranges are bidirectional;
  alternating ends never repeats or skips entries. Iteration remains exhausted
  after either end meets the other. A key-only scan never reads value payloads
  or invokes their decoder. Count/nth may skip values without decoding them.
  Entry iteration returns owned decoded keys plus owned encoded value bytes;
  consuming just the key must not decode the value. Invalid ranges may panic.
- Reopening uses saved state without initialization, repair, a full collection
  scan or an in-heap copy of all entries. Region callers reopen each operation;
  interleaved views must observe current state. Ordinary maps retain exclusive
  leases and can retain their current root metadata.
- Native failed mutations are poisoned by the caller's guard. Do not add a
  transaction journal or promise rollback of native partial writes. IC trap
  rollback is external. Mutations must not silently continue after failed IO.
- Bounds, offsets, sizes and stored identities must be checked before reads,
  allocations or writes. Malformed storage must refuse or panic, never cause
  UB or unbounded traversal through cycles. It need not globally scan every
  record during a lookup or load. Deterministic behavior is required.
- Repeated update/delete/insert churn must reuse freed space; do not retain an
  append-only history. Clear must not grow memory to release the collection.
  Search/update must scale with the search path rather than total population.
  Avoid one Wasm page per small record and full-value reads during routing.

## Fresh-format boundary

Choose and document an independently designed format with a distinct magic
and version. Only that format is admitted. Any other image is rejected without
writes. Fresh/reset backing is explicitly authorized; no predecessor reader,
automatic reset, fallback or retained-format compatibility is wanted. Explicit
initialization can replace a caller-authorized allocation image. It must not
depend on unused capacity already containing zeroes.

## Available interfaces (behavior, not implementation)

`crate::Memory` has `size(&self) -> u64` in 65,536-byte pages,
`grow(&self, pages: u64) -> i64` returning previous size or -1 without change,
`read(&self, offset: u64, dst: &mut [u8])`, and
`write(&self, offset: u64, src: &[u8])`. Full byte ranges must be valid.
`grow` adds zero-filled pages. Memory does not grant authority; the parent
passes only a checked lease or bounded view. Backing IO may panic.

`crate::storable::Storable` has associated `BOUND: Bound`,
`to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError>`,
`into_bytes(self) -> Result<Vec<u8>, CodecError>`, and
`from_bytes(Cow<'_, [u8]>) -> Result<Self, CodecError>`.
`Bound` is `Unbounded` or `Bounded { max_size: u32, is_fixed_size: bool }`.
Its crate-visible `check(&[u8]) -> Result<(), CodecError>` independently verifies
maximum/exact width. Do not trust overrideable codec convenience checks.
Encoding/decoding/bound failures panic. These are application-owned encodings;
schema identity/admission remains the caller's responsibility.

## New private integration interface

Author `crates/ic-storage/src/ordered_map.rs` and child modules only. The parent
integrates it into the crate and deletes the predecessor after acceptance.
Use `OrderedMap<K: Storable + Ord + Clone, V: Storable, M: Memory>` with:

- `new(memory: M) -> Self` for explicit initialization;
  `load(memory: M) -> Option<Self>` for read-only framing admission;
  `memory(&self) -> &M` for the private governed adapter.
- `const len(&self) -> u64`, `const is_empty(&self) -> bool`, `clear(&mut self)`.
- `insert(&mut self, K, V) -> Option<V>`, `remove(&mut self, &K) -> Option<V>`,
  `get(&self, &K) -> Option<V>`, `contains_key(&self, &K) -> bool`.
- `first_key_value/last_key_value(&self) -> Option<(K,V)>`.
- `iter(&self)` and `range(&self, impl RangeBounds<K>)` return
  `Entries<'_, K, V, M>`, implementing double-ended iteration of `(K, Vec<u8>)`.
- `keys(&self)` and `keys_range(&self, impl RangeBounds<K>)` return a
  double-ended iterator of K; `values(&self)` returns one of V.

The engine owns its codec validation. Do not recreate a second codec trait or
an upstream codec wrapper. Iterator internals and physical data design are the
implementer's choice. Prefer few modules, no dependency additions and no broad
lint suppressions. Add independently authored format/algorithm tests under the
new module. Coordinate Cargo invocations with the parent to avoid overlapping
builds. Do not inspect old tests to reproduce them.

## Acceptance and comparison

The parent writes separate public model tests, runs existing supported API
contracts, checks corruption/growth/custody and native/Wasm lint, then runs
installed IO/upgrades/rollback, old-image refusal and fresh installation.
External ICSS remains a benchmark dependency only. Report matched Map, Set,
region and all-collection application Wasm sizes and instruction measurements;
label regressions explicitly. A functional rewrite alone is not a performance
claim or release qualification. Do not delete remaining notices/provenance
until the covered material and its derived helpers are actually removed and
the final source/package review is complete.

## Implementation record — 2026-11-10

The separate implementation agent read only this specification, AGENTS.md,
TOKO-TOOLING.md, the two product manifests, and Rust hygiene/engineering rules
at immutable upstream revision `626da7cd9626cc2e152594862a2d82ea82ef51da`.
It authored `src/ordered_map.rs`, `ordered_map/storage.rs` and
`ordered_map/tests.rs` without predecessor source or test inputs. The parent
integrated public adapters and wrote separate public acceptance tests, which
passed on the predecessor before being run against the replacement.

The first candidate uses STORMAP1 version 1: a 1 KiB header, 128-byte AVL nodes,
separate encoded keys/values and reusable power-of-two blocks. Subtree counts
support rank cursors and value-free count/skipping. Each yielded iterator item
still traverses a logarithmic path. Free blocks reuse their exact size class;
there is no splitting/coalescing. Clear resets the allocation frontier and
free lists without growing backing. Reopen reads the fixed header and root
prefix, without scanning the collection.

The common ordered engine serves Map, Set and regions. The entire 3,541-line
predecessor engine directory (including private tests) is deleted. The replacement
engine and its own tests total 1,286 lines: 2,255 fewer in that slice, before
counting adapter simplification, new public acceptance tests and documentation.
The copied codec trait/wrapper, address helpers and import inventory are removed.
No predecessor algorithm remains as an alternate implementation or fallback.

### First candidate: matched application size

Raw uncompressed Wasm bytes, using the existing `wasm-size` profile and matched
production-shaped consumers. Previous source is the final backing-replacement
baseline, not the earlier pre-backing version. All eight pairs pass 118 calls
and two upgrades per backend. Full source/archive/hash evidence is at
`target/evidence/storage-consumer-size/attempt.B96yda`; the first server attempt
failed to bind localhost inside the sandbox, then the same frozen modules passed
with local-server execution enabled. This is distinct from benchmark-handler size.

| Application | Previous Storage | Replacement | ICSS | Storage delta |
| --- | ---: | ---: | ---: | ---: |
| Base | 16,942 | 16,942 | 16,938 | 0 |
| Map | 125,524 | 107,925 | 90,208 | -17,599 |
| Set | 125,299 | 108,063 | 90,008 | -17,236 |
| Vector | 99,790 | 99,954 | 64,560 | +164 |
| Cell | 97,513 | 97,685 | 63,135 | +172 |
| Region | 126,607 | 109,305 | 89,933 | -17,302 |
| Map + region | 129,249 | 110,885 | 110,858 | -18,364 |
| All | 141,342 | 123,075 | 119,318 | -18,267 |

The small Cell/Vector changes are included even though their algorithms were not
changed in this cut. No specific attribution for those linked-artifact deltas is
established. The new ordered algorithm's runtime tradeoffs require instruction
measurements independently of this size reduction.

### First candidate: correctness and depth measurements

The first candidate passes 141 workspace library tests, 35 selected public
contracts, 28 comparison-library tests and strict native/Wasm Clippy. Installed
original/O3/Os/Oz IO, custody, checkpoints, upgrade and rollback pass at
`target/qualification/runtime/attempt.rYKLIm`. Frozen preceding Map images refuse
upgrade while preserving the previous application; fresh installation passes.
All three product package archives qualify at
`target/package-qualification/attempt.n5kPft`.

The matched depth run at `target/evidence/storage-comparison/attempt.sznDp1`
contains 752 summary rows at 256 and 4,096 records. The preceding control is
`target/evidence/backing-replacement/direct-depth`. Every row has matching
logical configuration and identical ICSS reference instruction counts. There
are 443 instruction regressions against preceding Storage; functional success
and smaller Wasm do not qualify this candidate for shipping.

Selected 4,096-record uniform u64 inner-operation measurements, batches of 32:

| Operation | Previous Storage | First candidate | ICSS |
| --- | ---: | ---: | ---: |
| Map Get | 934,522 | 1,730,012 | 914,982 |
| Map Insert | 2,197,103 | 5,110,199 | 2,002,760 |
| Map Remove | 2,189,451 | 5,235,843 | 1,986,372 |
| Map Scan | 326,047 | 1,748,294 | 309,902 |
| Region Get | 1,020,606 | 2,167,954 | 933,864 |
| Set Scan | 218,329 | 1,403,641 | 212,181 |

Map/Set/region stable capacity in these cases rises from 26 to 34 Wasm pages.
The repeated rank traversal per scan result is an implementation-level cost;
the share attributable to page charging has not been isolated. These counters
are not complete application throughput or a promise about every key type.

The independent implementer receives these black-box measurements to refine its
own design without predecessor source. Preserve this failed performance candidate
and compare refinements with both it and preceding Storage. The parent has not
implemented engine optimizations. Complete type-catalogue qualification was
outstanding at this first-candidate stage; the final refinement below supplies it.

### Independent refinement

The same restricted implementer received only black-box measurement feedback and
its own first-candidate source. It added `ordered_map/iter.rs` and refined the
design without reading predecessor or benchmark implementations:

- Nodes keep keys up to 16 bytes and values up to 24 bytes inline within the
  existing 128-byte block. Routing reads exclude the inline-value tail; key-only
  operations still do not read value payloads.
- Parent links retain child heights/counts, avoiding repeated sibling reads
  during mutation. Insertion performs one search. Existing-child consistency is
  checked when traversed, without another IO call or a whole-tree scan.
- Bidirectional iterators retain bounded traversal paths; skipped ranks seek
  directly and sequential results no longer restart at the root each time.
- Read-only reopening reads a 64-byte header and 104-byte root metadata, rather
  than the full reserved 1 KiB header. Allocation bins are admitted when used.

The implementer's read-only review found that parent/child cached metadata could
disagree and misdirect `nth`. The final refinement checks root length and child
height/count consistency during existing reads, including searches, mutations,
rotations and cursor traversal. Independent regressions exercise count and height
corruption, mixed iterator skipping, inline/external value transitions, owned
entries across mutation and exclusion of inline-value reads from key scans.

The final engine and its own tests total 1,712 lines, versus 3,541 in the removed
engine slice: a net reduction of 1,829 lines there. This excludes adapter changes,
new public acceptance tests and documentation. It is a source-size comparison,
not a compiled-size or speed claim. The final format remains unpublished
STORMAP1 version 1; no reader is retained for earlier experimental candidates.

The intermediate refinement before the consistency fix remains frozen at
`target/evidence/storage-consumer-size/attempt.t9CbtW` and
`target/evidence/storage-comparison/attempt.r9WH01`. It reduces most of the first
candidate's costs, but still regresses large scans and small-record reservation.
Those intermediate results do not qualify the later consistency fix.

### Final refinement: matched size and depth

The final application run is
`target/evidence/storage-consumer-size/attempt.TaQUk6`. All eight pairs pass
118 calls and two upgrades per backend. Complete raw module and code-section
payload sizes are shown separately; no compression sizes are used.

| Application | Previous raw | Final raw | ICSS raw | Previous code | Final code | ICSS code |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Base | 16,942 | 16,942 | 16,938 | 14,818 | 14,818 | 14,818 |
| Map | 125,524 | 113,038 | 90,208 | 107,983 | 97,043 | 49,244 |
| Set | 125,299 | 112,860 | 90,008 | 107,752 | 96,866 | 48,993 |
| Vector | 99,790 | 99,954 | 64,560 | 85,798 | 85,944 | 27,627 |
| Cell | 97,513 | 97,685 | 63,135 | 84,162 | 84,309 | 26,115 |
| Region | 126,607 | 114,417 | 89,933 | 109,010 | 98,356 | 49,072 |
| Map + region | 129,249 | 115,957 | 110,858 | 111,567 | 99,832 | 69,759 |
| All | 141,342 | 126,406 | 119,318 | 121,771 | 108,523 | 77,127 |

The final depth run is `target/evidence/storage-comparison/attempt.HoJZud`.
All 752 summary rows pass; every external ICSS instruction control matches the
preceding Storage baseline's reference. Of these rows, 265 regress against
preceding Storage. Non-Control median row changes are Map -7.97%, Set -7.41%,
regions -10.18%, Cell unchanged and Vector +0.12%. These heterogeneous row
medians are not a weighted application speedup. Larger scans remain substantially
slower, and the replacement is **not release-ready on performance**.

Uniform u64 keys/values, batches of 32, metered inner-operation instructions:

| Rows | Operation | Previous Storage | Final Storage | ICSS |
| ---: | --- | ---: | ---: | ---: |
| 256 | Map Get | 543,270 | 421,469 | 538,219 |
| 256 | Map Insert | 1,649,703 | 1,002,988 | 1,458,963 |
| 256 | Map Remove | 2,111,854 | 725,914 | 1,839,263 |
| 256 | Map Scan | 163,168 | 201,371 | 146,922 |
| 4,096 | Map Get | 934,522 | 1,145,264 | 914,982 |
| 4,096 | Map Insert | 2,197,103 | 2,012,112 | 2,002,760 |
| 4,096 | Map Remove | 2,189,451 | 2,119,046 | 1,986,372 |
| 4,096 | Map Scan | 326,047 | 773,764 | 309,902 |
| 4,096 | Set Get | 830,455 | 1,039,750 | 826,957 |
| 4,096 | Set Scan | 218,329 | 419,660 | 212,181 |
| 4,096 | Region Get | 1,020,606 | 1,219,469 | 933,864 |

At 4,096 small records, backing capacity remains 34 pages versus preceding
Storage's 26 and ICSS's nine. The Storage-versus-ICSS totals also include different
governance metadata. A separate native IO check uses identical owned codecs and
both ordinary and governed ICSS backing. Selected 4,096-record Map Get×32:

| Backend | Reads | Read bytes | Key decodes | Stable 64 KiB intervals |
| --- | ---: | ---: | ---: | ---: |
| Storage | 370 | 35,408 | 338 | 8 |
| ICSS on Storage backing | 1,997 | 17,117 | 727 | 4 |
| ICSS manager | 1,997 | 17,117 | 727 | 4 |

These interval sets describe native stable-byte ranges, not IC faults or exact
page fees. Fewer IO calls alone do not establish lower IC cost. The next focused
investigation is small-record density and ordered traversal footprint, preserving
codec and structural checks. Stronger locality is a hypothesis to test, not
permission to copy a reference layout or drop integrity checks.

Final focused qualification passes 146 workspace library tests, 35 public
contracts, 28 comparison-library tests, formatting and native/Wasm Clippy.
Installed original/O3/Os/Oz IO, custody, checkpoints, upgrade and rollback pass at
`target/qualification/runtime/attempt.Dh67gD`. The final old-image upgrade test
also passes. Product archive checks passed on the first candidate only; a later
authorized release must qualify the final source under its group gates.

### Complete type catalogue

The final frozen comparison modules also pass all five type-suite shards at
`target/evidence/ordered-replacement/final/types-{map,set,vector,cell,region}/`.
There are 565 selected cases and 54,318 sample observations. Each shard uses a
separate managed server and output directory; source and artifact hashes are
rechecked after completion. Both libraries receive the same logical inputs.

The catalogue includes expected admission refusals: both Vector backends reject
16 unbounded-type cases, and ICSS refuses additional fixed-Option combinations.
Storage-only measurements for those ICSS refusals are kept separate from paired
performance counts. A refused reference is never treated as zero instructions.

| Collection | Selected cases | Samples | Paired non-Control rows | Storage lower | Equal | Storage higher | Storage-only rows |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Map | 173 | 30,224 | 14,368 | 11,456 | 0 | 2,912 | 350 |
| Set | 73 | 3,252 | 1,456 | 1,343 | 0 | 113 | 66 |
| Vector | 73 | 5,200 | 2,352 | 1,016 | 3 | 1,333 | 96 |
| Cell | 73 | 2,472 | 962 | 421 | 319 | 222 | 0 |
| Region | 173 | 13,170 | 5,946 | 4,746 | 0 | 1,200 | 140 |

These are row counts against external ICSS, not weighted throughput or changes
against preceding Storage. Type profiles use their declared small populations;
they do not cancel the large-population regressions in the separate depth study.
Vector/Cell results include the already-pending independent backing and codec
work; do not attribute their differences to this ordered-engine cutover.

Reproduction uses prepared tools/caches and the maintained entrypoints:

```sh
make benchmark-storage-size
BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage
BENCH_WORKLOAD=types BENCH_SAMPLES=1 make benchmark-storage
make test-runtime
cargo run --locked --offline --manifest-path qualification/Cargo.toml \
  -p ic-storage-benchmarks --example collection-io --features owned,dfinity \
  --target-dir target/runtime-qualification -- target/evidence/collection-io.jsonl
```

These rebuild current inputs. Frozen predecessor/candidate modules, source
archives and hashes remain under the recorded `target/evidence/` paths; explicit
cleanup can remove them. The compact tables retain observations, not the ability
to reconstruct deleted uncommitted predecessor source from hashes alone.
