# Storage-owned bucket backing — 2026-11-09

## Decision and scope

The maintainer authorized copying ICSS's allocation backing and adapting it by
hard cut. Storage now owns an ICSS 0.7.2-derived bucket allocator in
`crates/ic-storage/src/runtime/directory_model/persistence.rs`; provenance and
Apache notices remain in `docs/storage/icss-source.json`. There is no production
ICSS dependency. This is pending working-tree work, not a published release.

Delete the whole-directory snapshot encoder/decoder, candidate construction and
snapshot recovery path. Keep named opaque u64 allocation identities, namespace
grants, admission, backing custody, exclusive leases and logical bounds. ICSS's
bucket allocation/translation and last-span cache are adapted to these contracts.
Storage additionally coalesces adjacent physical buckets within a requested IO;
it does not scan unrelated buckets or maintain another index.

Growth writes new owner rows, a 16-byte checked size prefix and the root. It does
not reserialize names or reread all allocation metadata. Names have a separate
checksum. Explicit admission/verification still rereads active controls. Native
interruption fails closed; Wasm relies on IC execution rollback and omits the
native dirty-root write. Refused physical growth precedes metadata writes and is
retryable. No second transaction journal or fallback image exists.

## Breaking contract and costs

Old whole-store images are rejected without modification. Hosts must explicitly
retire/reset backing or supply fresh backing; no automatic migration/reset is
provided. Collection framing remains unchanged, but that does not make an old
whole-store image attachable. Remove `StorageConfig::wire`; core Storage's Wire
edge is now development-only. No sibling source was changed.

Buckets contain eight 64-KiB Wasm pages. `extents` now limits bucket count.
Creation-time control capacities are fixed: 160 bytes per possible name, 16 per
possible bucket, plus a 64-byte root, rounded to pages. Defaults reserve 15 pages
(960 KiB) of controls and permit 8 GiB of bucket data. Unused rows are not
explicitly written. Logical size excludes reserved bucket slack; no reclamation
or capacity enlargement is provided. Default control overhead is 14 pages more
than the eight-page ICSS manager reference in these one-memory fixtures.

This trades snapshot rewriting for fixed control reservation and bucket slack.
Cold-open IO remains proportional to active allocation/bucket rows and has not
been profiled here. Do not claim a cold-open improvement. Checksums detect
accidental corruption; they are not authentication of an adversarial image.

## Matched instruction results

References use published ICSS 0.7.2 with eight-page buckets. The before baseline
already includes the pending Cell length cache, combined B-tree overflow write,
Build/Host selection and ordinary size-consumer modules. Counts below time the
collection loop, not the full endpoint. All logical results match after excluding
instruction and physical-page counters, which the backing deliberately changes.

- Depth: 720 cases / 4,320 samples; 430 medians improve, 240 regress, 50 equal.
  Worst regression is 3.24% in Vector Control; excluding Control, 2.91% in a Map
  bytes terminal iterator. These are measured tradeoffs, not universal parity.
- Bulk: 116 cases / 232 samples; 41 improve, 68 regress, seven equal. The largest
  regression is Vector 64-KiB Chunk Get: 307,021 to 467,180 (+52.17%). Vector Pop
  rises from 387,215 to 547,625. Vector Push improves from 1,081,555 to 871,082.
- ICSS reference instruction rows are unchanged between the matched runs.
  No whole-application speedup is established.

Selected depth rows, uniform 4,096 population and batch 32 (Cell has one value):

| Operation | Before | Bucket backing | ICSS |
| --- | ---: | ---: | ---: |
| Map u64 Get | 850,384 | 805,400 | 914,502 |
| Map u64 Insert | 1,967,605 | 1,891,814 | 2,002,760 |
| Map u64 Replace | 1,568,616 | 1,494,919 | 1,596,676 |
| Set Get | 704,115 | 706,539 | 826,477 |
| Vector Get | 94,903 | 95,060 | 102,932 |
| Vector Push | 190,327 | 190,420 | 186,644 |
| Cell 32-byte Replace | 172,859 | 172,923 | 183,037 |
| Region Get | 933,524 | 888,689 | 933,384 |
| Region Insert | 2,173,012 | 2,093,367 | 2,020,396 |

Selected bulk Cell rows:

| Operation | Before | Bucket backing | ICSS |
| --- | ---: | ---: | ---: |
| Grow 64 to 256 KiB | 1,490,396 | 1,304,583 | 1,227,657 |
| Grow 1 to 1.5 MiB | 6,001,116 | 5,975,950 | 5,739,124 |
| Shrink from 256 KiB | 160,834 | 240,793 | 161,001 |
| Shrink from 1.5 MiB | 160,834 | 240,793 | 161,103 |

Skipping shrink capacity queries, avoiding name rewrites and coalescing IO did
not remove the approximately 80,000-instruction shrink difference. A 256-byte
heap-state alignment trial also failed that goal and was discarded. Do not
attribute the difference to a specific memory access without further evidence.

## Cell whole-handler follow-through

The existing execution-only WAT probe preserves linear-memory/data/global layout.
Both uninstrumented round trips reproduce all 24 original Cell samples exactly;
both probed runs preserve the inner counts/results, and the ICSS reference is
unchanged. The outer scope includes decode, preparation, loop, response encoding,
reply and cleanup, with probe overhead; it excludes setup population. Large
synthetic replies dominate these totals, so this is not an Auth endpoint result.

| Operation | Before whole handler | Current whole handler | ICSS whole handler |
| --- | ---: | ---: | ---: |
| Grow 64 to 256 KiB | 16,024,459 | 15,678,827 | 15,921,525 |
| Grow 1 to 1.5 MiB | 193,796,692 | 193,611,707 | 193,694,505 |
| Shrink from 256 KiB | 42,984,060 | 42,983,969 | 43,064,236 |
| Shrink from 1.5 MiB | 252,256,454 | 252,256,363 | 252,176,253 |

Control and Get totals are identical before/after. Replace increases by two
instructions at both sizes; GetAfterReplace decreases by 31. Shrink decreases
by 91 despite its approximately 80,000-instruction inner-loop increase: cost
moves across the inner timer boundary, not into the complete handler. This
resolves the Cell timing-window question without identifying the exact faulting
load/store. Do not generalize this result to Vector; the separate follow-through below
finds genuine iterator regressions alongside timer-boundary shifts.

Evidence: `target/evidence/storage-bucket-cut/execution/`, including roundtrip
and traced before/current Cell shards, paired ICSS controls, module hashes and
`whole-handler.csv`. Reproduce with the README's execution-only injector,
`--workload bulk --collection cell --samples 1 --trace-execution-cost`; run a
plain WAT roundtrip first and compare it with the ordinary bulk Cell samples.
No production instrumentation or alignment change is retained.

## Vector whole-handler follow-through

Reuse the same frozen before/current/ICSS modules and execution-only injector.
Run `--workload bulk --collection vector --samples 1` first with plain WAT
roundtrip modules, then with traced modules and `--trace-execution-cost`.
Both plain runs reproduce all 52 original samples exactly, including instruction
and stable-page counts. Both traced runs preserve the inner counts and logical
results for all 52 samples, and all ICSS outer counts remain identical. The four
runs pass final-state validation. This step changes no production or harness
source and adds no application Wasm bytes.

There are 26 cases: 13 operations at 64 KiB and 1 MiB, eight populated slots,
uniform access, batch/iterator limit one. A “scan” here returns one item; it is
not a full eight-item scan. Full-handler scope includes input preparation and
potentially large response encoding, as in the Cell study above.

| Value / operation | Before handler | Current handler | ICSS handler | Change from before |
| --- | ---: | ---: | ---: | ---: |
| 64 KiB Get | 11,340,255 | 11,340,692 | 11,419,817 | +437 (+0.0039%) |
| 64 KiB Set | 1,918,972 | 1,919,470 | 1,998,854 | +498 (+0.0260%) |
| 64 KiB forward Scan | 11,326,588 | 11,646,814 | 11,405,610 | +320,226 (+2.8272%) |
| 64 KiB reverse Scan | 11,406,593 | 11,566,815 | 11,485,833 | +160,222 (+1.4046%) |
| 64 KiB alternating Scan | 11,326,586 | 11,486,333 | 11,405,605 | +159,747 (+1.4104%) |
| 64 KiB Push | 2,533,650 | 2,323,188 | 2,249,010 | -210,462 (-8.3067%) |
| 64 KiB Pop | 11,420,584 | 11,420,891 | 11,500,309 | +307 (+0.0027%) |
| 1 MiB Get | 171,837,864 | 171,838,847 | 171,198,223 | +983 (+0.0006%) |
| 1 MiB forward Scan | 175,367,181 | 175,367,604 | 174,727,435 | +423 (+0.0002%) |
| 1 MiB Push | 20,171,629 | 20,147,026 | 20,069,783 | -24,603 (-0.1220%) |
| 1 MiB Pop | 171,757,941 | 171,758,567 | 171,117,880 | +626 (+0.0004%) |

At 64 KiB, Nth, NthBack and Last each add 160,222 whole-handler instructions;
ScanSet adds 159,747. Control/MissAbove fall by about 159,879. At 1 MiB, all
non-Push changes lie between -29 and +1,034 instructions. Last remains cheaper
than ICSS at both sizes because of the existing terminal-iterator optimization;
this is not a new bucket-backing gain.

The apparent 52.17% inner-loop Get regression is primarily a timer-boundary
shift: +160,159 inside the loop, only +437 in the complete handler. The Pop
increase similarly falls from +160,410 inside to +307 overall. The iterator
regressions do **not** disappear: forward Scan gains roughly 160k inside and
another 160k outside the loop. Differences near multiples of 80k are consistent
with memory tracking charges, but this probe does not identify the faulting
load/store or distinguish allocator metadata from payload buffers. Do not claim
that exact cause is proved, or tune alignment merely to hide the observed row.

Several 1 MiB full-handler reads remain approximately 640k above ICSS, with
nearly the same gap before the hard cut. That is a preexisting opportunity,
separate from the new 64-KiB iterator regression. Next attribution should trace
allocation/read destinations and costly heap accesses across the complete
64-KiB iterator handler, holding the logical trace and frozen baselines fixed.
Cold-open/admission scaling and the full type catalogue remain later gates.

Evidence: `target/evidence/storage-bucket-cut/execution/vector-{before,current}-{roundtrip,traced}/vector/`
and `vector-whole-handler.csv`. The source/provenance and ordinary Wasm sizes
remain those recorded below: Vector 108,314 → 93,523 bytes, ICSS 64,564.
Diagnostic module growth is instrumentation, not a product-size change.

## Vector allocator/read attribution

The existing read and allocator WAT injectors now run through the driver for
`--workload bulk --collection vector --trace-stable-reads
--trace-allocator-accesses`. Only this bounded Bulk shard is admitted; all its
operations are captured. Two focused driver tests and strict driver Clippy pass.
Both installed diagnostic runs pass 26 cases / 52 samples, preserving all
uninstrumented logical results and physical-page counts. Both ICSS diagnostic
runs are identical. Overflow and unexpected counter counts remain fatal.

This instrumentation adds globals/functions without adding linear-memory trace
buffers. It instruments 521 load/store sites in six dlmalloc functions. It
changes instruction counts significantly, so the preceding ordinary and
boundary-only tables remain authoritative for performance. Source/memory layout
and original module identities are retained; no production change is made.

For 64-KiB reads/iterators, stable-read **counts, lengths and measured per-read
costs are identical** before/current. The payload destination moves from
8,455,328 to 8,455,192, but the 65,536-byte read costs remain 225,756 for Get and
reverse iteration, and 145,756 for forward/alternating iteration. Each iterator
makes six reads; direct Get makes five. Push is a different growth path and is
not included in that equality claim.

| 64-KiB operation | Before/current inner malloc calls | Before/current costly allocator records | Before/current whole handler |
| --- | --- | --- | --- |
| Get | 1 / 1 | 4 / 6 | 11,340,255 / 11,340,692 |
| Forward Scan | 2 / 2 | 4 / 9 | 11,326,588 / 11,646,814 |
| Reverse Scan | 2 / 2 | 4 / 7 | 11,406,593 / 11,566,815 |
| Alternating Scan | 2 / 2 | 4 / 8 | 11,326,586 / 11,486,333 |

Records are allocator accesses costing at least 10,000 probe instructions, not
all allocations or distinct page counts. Forward Scan before the cut has three
records before the timer, one during it and none afterward; current has four
before, three during and two afterward. The new post-loop stores at heap offsets
8,781,788 and 8,651,980 each cost 160,200 probe instructions, at malloc sites
217 and 307. During the loop, current accesses at 8,520,732 include an 80,200
load and an 80,200 store; the prior run has neither at that site. These are
concrete allocator metadata costs consistent with first-access/dirty charges.
Do not add probe-record costs to the ordinary totals: probes have overhead and
other accesses can charge the same regions or shift costs between phases.

The evidence localizes the change to heap allocation history/placement rather
than additional stable reads or a changed Vector traversal algorithm. It does
not establish which high-level post-loop allocation owns every store, nor fully
account for uninstrumented heap copies. `TypedVector::run` converts results after
its timer, and the surrounding benchmark endpoint encodes a large reply; these
costs are part of this handler, not a universal Vector operation cost.

Code inspection identifies a bounded next experiment. In
`crates/ic-storage/src/engine/vec.rs`, `Iter::last` consumes the iterator but
passes its buffer to `T::from_bytes` as `Cow::Borrowed`. The benchmark's
`ChunkBytes::from_bytes` calls `into_owned`, so it must allocate/copy even though
the iterator could transfer that buffer. Direct Get already supplies owned
bytes. Test an ownership transfer in the consuming `last()` path, with the
existing mutation/truncation semantics and matched size/instruction qualification.
Do not blanket-remove reusable buffers from `next`/`next_back`: repeated scalar
or structured decodes can benefit from reuse, and those paths need multi-item,
multi-type measurements. No production ownership or alignment change is retained
in this attribution pass.

Evidence is in `target/evidence/storage-bucket-cut/execution/`: diagnostic
`before/current/icss-allocator.wasm`, corresponding `.wat.sites.tsv`,
`allocator-artifacts.sha256`, and `vector-{before,current}-allocator/vector/`.
Reproduce with the README's two injectors over the frozen original WAT, then the
Bulk Vector flags above through the admitted local Testkit runner. The earlier
plain round trips already qualify those original inputs. This diagnostic does
not qualify admission scaling, the full type catalogue or application endpoints.

## Matched Wasm size

Raw module bytes, same compiler/profile and small consumer entrypoints. These
are application-shaped fixtures, not a deployed application's exact size.
Code-section payload sizes are reported separately; neither is compressed size.

| Selection | Before bytes | Current bytes | ICSS bytes | Current minus before |
| --- | ---: | ---: | ---: | ---: |
| Base | 16,950 | 16,950 | 16,946 | 0 |
| Map | 133,297 | 118,518 | 90,212 | -14,779 |
| Set | 132,741 | 117,962 | 90,012 | -14,779 |
| Vector | 108,314 | 93,523 | 64,564 | -14,791 |
| Cell | 105,377 | 90,537 | 63,143 | -14,840 |
| Region | 134,436 | 119,739 | 89,937 | -14,697 |
| Map + region | 137,018 | 122,333 | 110,862 | -14,685 |
| All | 148,075 | 133,445 | 119,326 | -14,630 |

Cell code-section payload: 92,341 to 77,854 bytes (ICSS 26,115).
All-collection code-section payload: 129,647 to 115,364 (ICSS 77,127).
The broad benchmark harness shrinks from 11,497,579 to 11,462,663 bytes
(-34,916); do not confuse this harness with the small consumers. Storage remains
larger than ICSS: 27,394 bytes for Cell and 14,119 for all collections.

## Qualification and reproduction

Passed 83 core library tests, five runtime integration tests, 42 adapter tests,
eight ICSS
collection-image/IO attribution tests, strict core Clippy, production graph
checks, eight installed small-consumer selections and original/O3/Os/Oz installed
IO/upgrade/rollback. Tests cover opaque IDs beyond 255 names, stable name binding,
interleaved bucket translation, contiguous IO, slack exclusion, physical refusal,
budget/overflow refusal, native partial writes, active-row corruption and
incompatible images. Installed upgrade checks cover the new format reopening
itself, not migration of the retired snapshot format.

Current frozen evidence:

- Depth: `target/evidence/storage-comparison/attempt.F7Ba3c`;
  before `attempt.B7ATSm`.
- Bulk: `target/evidence/storage-comparison/attempt.hAmH7F`;
  before `target/evidence/storage-cell-length/candidate-bulk-matched`.
- Size: `target/evidence/storage-consumer-size/attempt.73dNho`;
  before `attempt.JcCuvj`.
- Lifecycle: `target/qualification/runtime/attempt.8Uya9I`.
- Focused logs/source checkpoints: `target/evidence/storage-bucket-cut/`.

Use the maintained comparison commands in
[the benchmark README](../../../testing/runtime-qualification/crates/ic-storage-benchmarks/README.md)
for depth, bulk and size; the scripts freeze inputs, modules and configuration.
`make test-runtime` retains installed lifecycle qualification separately. The
benchmark driver now supports `--workload bulk --collection cell` and full-handler
execution probes for Bulk, using the existing WAT injector. No benchmark hooks
are added to production Storage. Raw target evidence is transient; committed
source and these commands recreate the experiment, not the old transient files.

The 73-type Cell catalogue was measured before this backing cut, not rerun as
current backing qualification. Full sibling endpoints and cold-open scaling are
also unqualified. Product release gates, consumer reset decisions and publication
remain separate work. Keep the explicit regressions visible during review.

## Follow-up: shared size and Map/region attribution

This follow-up includes the pending admission optimizations and compact name
index described in [the admission report](storage-admission-2026-11-09.md).
It does not change collection algorithms or the data-IO path. The index wins
cold-reopen instructions at a 2.7–2.8 KB application-size cost, so it does not
solve the separate shared-code-size gap. Current-format upgrade qualification
still passes for all eight size-consumer pairs.

The existing native `collection-io` example supplies three controls: Storage,
ICSS 0.7.2 on Storage's governed backing, and ICSS on its own eight-page bucket
manager. All 480 observations pass their value/model checks. Across all 160
Map/region cases, Storage makes fewer stable reads than the governed ICSS
control, with identical total read bytes and key/value decode counts. This
covers fixed 8-byte values and variable 32/4,096-byte payloads, 256/4,096 rows,
single/batched operations, uniform/hot patterns, replacement and fourfold growth.

| 4,096 rows, 4-KiB value, single uniform operation | Storage reads | ICSS governed reads | Read bytes, both |
| --- | ---: | ---: | ---: |
| Map Get | 129 | 153 | 5,571 |
| Map Replace | 149 | 173 | 21,955 |
| Map Grow to 16 KiB | 161 | 185 | 22,147 |
| Region Get | 131 | 155 | 5,647 |
| Region Replace | 151 | 175 | 22,031 |
| Region Grow to 16 KiB | 163 | 187 | 22,223 |

The [complete native counter matrix](storage-bulk-attribution-2026-11-09.csv)
retains all observations. Native range sets describe stable byte ranges, not
actual IC page faults or charges. Counts are not instruction measurements.
The governed reference retains its Rc wrapper and primitive codecs; it is a
useful common-backing control, not a perfectly isolated algorithm benchmark.
Reverting the current engine's combined reads would restore extra IO, not fix
the observed instruction gap. No change to production Map/region is justified
by these counts alone.

Reproduce the native attribution with:

```sh
cargo run --locked --offline \
  --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --example collection-io --features owned,dfinity \
  --target-dir target/runtime-qualification -- NEW-OUTPUT.jsonl
```

Use a new evidence path under the repository's `target/evidence/`; the example
refuses to overwrite an existing file. Raw results for this run are under
`target/evidence/storage-next/`.

### Installed bulk controls after the admission work

Both complete bulk runs pass 116 cases, with two samples per backend per case
(464 backend observations each). Current Storage medians are identical across
the independent ordinary-reference and governed-reference runs. ICSS is pinned
to published 0.7.2; no cache or bucket setting changed. Results are inner
collection-loop instruction deltas, not complete handler costs or throughput.

| Collection, excluding controls | Cases | Storage / ordinary ICSS, median | Range | Storage / governed ICSS, median | Range |
| --- | ---: | ---: | ---: | ---: | ---: |
| Map | 52 | 1.0130 | 1.0086–1.0338 | 1.00004 | 0.9949–1.0045 |
| Vector | 24 | 1.0917 | 0.1658–1.5232 | 1.00002 | 0.1452–1.0008 |
| Cell | 10 | 1.00005 | 0.5863–1.4956 | 0.99992 | 0.9987–1.00002 |
| Region | 22 | 1.0362 | 1.0221–1.0512 | 1.0214 | 0.9963–1.0259 |

A ratio below one favors Storage. Medians across different operations are
unweighted summaries, not an application speedup. The complete
[bulk control matrix](storage-bulk-controls-2026-11-09.csv) retains previous
Storage/ICSS (`attempt.hAmH7F`), current ordinary-reference and current governed
medians. The previous result predates later admission work and the terminal
Vector ownership change, so differences are not all attributable to the index.
Map and region ordinary-reference ratios remain effectively at their previous
levels. No new production Map/region optimization is retained in this pass.

Map's 1.30% median gap nearly disappears with common governed backing. Together
with the lower native IO counts, this points toward backing-call overhead and
associated allocation/layout effects rather than a need to revert the engine.
It does not isolate an exact instruction charge or prove any check removable.

**Region control limitation:** `benches/canister/collections.rs::Region` creates
Storage's bounded `BTreeMapRegion`, but the ICSS arms call `BTreeMap::load` on
`BenchMemory` directly. They do not wrap the memory in `RestrictedMemory`.
Likewise, the native region control measures operation-local root reload but
not the same bounded namespace. The small Wasm-size consumers are a separate
fixture and do use bounded ICSS regions. Thus the remaining 2.14% governed-region
median includes an unmatched semantic boundary, plus wrapper/codec/layout
choices; it is not evidence that the same checked engine operation is 2.14%
slower. A follow-up should qualify an equivalent bounded ICSS region before
changing production bounds checks. Do not cache the region root across calls:
interleaved views depend on operation-local reloads.

Reproduction uses the existing bulk runner with `BENCH_WORKLOAD=bulk` and two
samples, then the same driver and Storage Wasm with the reference example built
using `--features dfinity-governed`. Both modules use the `comparison` profile
and `-C link-arg=-zstack-size=8388608`. Current ordinary run:
`target/evidence/storage-comparison/attempt.WEni1B`; governed module/run and native
attribution: `target/evidence/storage-next/`. The existing scripts retain compiler,
source, input and artifact identities. No full release validation or publication
is implied by these focused experiments.
