# Byte and Boolean codec experiment

Keep borrowed `u8` and `bool` encodings in Storage's
[codec implementation](../../../crates/ic-storage/src/storable.rs). Published
**ic-stable-structures 0.7.2** allocates a vector for these borrowed encodings;
that is the ICSS reference here, not an older Storage release. The baseline is
deployed Storage 0.68.7. Public APIs, encoded bytes, decoders, collection images
and governed allocation rules remain unchanged. There is no unsafe code or new
generic helper. Owned encodings still return owned vectors.

## Inputs and coverage

Rust 1.99.1, Runtime 0.20.0, benchmark Wire 0.17.6, Testkit 0.79.0 and PocketIC
16.1.0 are fixed. ICSS uses eight-page buckets. The comparison profile and
eight-MiB synthetic stack are unchanged. The unrelated incoming root-lock Wire
0.17.12 selection is preserved; this experiment does not adopt it.

Each selection uses seed 1, one sample, three stateful rounds, uniform/hot-16
access and batches 1/32. Counts below are aggregate comparison cells, not calls.

| Selection | Coverage | Cells | Calls per ordinary/probe run |
| --- | --- | ---: | ---: |
| Focused | `VectorU8`, `VectorBool`, `CellBool` | 116 | 696 |
| Affected roles | 14 byte/Boolean key and value roles across all five forms | 716 | 4,296 |
| Depth controls | Map, Set, Vector, Cell and regions; u64/32-byte values | 712 | 4,272 |

The affected-role selection uses 4,096 rows for values and u64 keys. Byte-key
populations cap at 128, Boolean-key populations at two, and Cells at one. Map
and region coverage includes each primitive as key and value; Set covers keys.
Depth controls sweep 256 and 4,096 records. These are collection histories, not
Auth/Canic endpoints or a claim about every application workload.

Whole-handler probes include request decoding, input preparation, collection
work, reply encoding and cleanup; population is excluded. All ordinary/probe
responses and inner instruction counters match exactly. ICSS modules, reference
costs and stable-page counts are unchanged between revisions. The diagnostic
driver now admits custom case selections; bounded read probes retain their
existing restrictions and cannot be mixed with whole-handler probes.

## Two revisions and the decision

The first trial also rewrote the u8 decoder as a slice match. Broader coverage
caught an extra seven instructions per returned byte in some map operations.
Restoring the original decoder expression removes that penalty. Keep only the
encoding change; the final trial accepts small measured allocation-history
tradeoffs rather than claiming every execution improves.

| Selection / revision | Loop improved / equal / regressed | Handler improved / equal / regressed |
| --- | --- | --- |
| Focused, initial | 58 / 58 / 0 | 58 / 58 / 0 |
| Focused, final | 32 / 84 / 0 | 32 / 84 / 0 |
| Affected roles, initial | 358 / 338 / 20 | 362 / 333 / 21 |
| Affected roles, final | 178 / 536 / 2 | 183 / 531 / 2 |
| Depth controls, either revision | 0 / 712 / 0 | 0 / 712 / 0 |

Selected **whole-handler instructions** from the affected-role matrix:

| Case / population / batch / pattern | Baseline | Initial | Final | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| u8 Vector Set / 4,096 / 32 / uniform | 695,875 | 689,283 | 689,283 | 771,918 |
| bool Vector Set / 4,096 / 32 / uniform | 695,339 | 688,747 | 688,747 | 771,389 |
| bool Cell Replace / 1 / 32 / uniform | 708,051 | 702,259 | 702,259 | 793,428 |
| u64→u8 Map AlternatingScan / 4,096 / 32 / uniform | 797,567 | 797,791 | 797,567 | 873,793 |
| u8 Set RemoveInserted / 128 / 32 / hot-16 | 2,063,171 | 1,902,558 | 1,938,414 | 2,335,566 |
| u8→u64 Region Get / 128 / 1 / hot-16 | 585,385 | 584,969 | 585,785 | 665,451 |

The final focused Vector writes save about 3.5–3.6% in the collection loop and
0.95% in the full handler; Boolean Cell replacement saves 3.2% / 0.82%. Selected
byte-key Set churn saves 6.05% of its handler cost. This larger history-dependent
result is not an isolated codec-instruction speedup or a guarantee for other seeds.

The two regressed handler medians are byte-key Region Control (+19 instructions)
and hot-16 Get (+400, 0.068%). Across all 2,148 affected-role round cells,
handlers improve/match/regress 540/1,596/12; the worst individual regression is
778 instructions (0.133%) on Region Get. Loop medians also include a +25 scan
regression; the worst individual loop regression is 699 instructions (0.741%).
Fewer temporary allocations can change later allocator state; these observations
do not identify a particular heap fault. Retain the small change for its write
savings and reduced Wasm size, with these narrow costs explicitly recorded.

| Revision | Benchmark Wasm bytes | Delta | Code-section payload bytes |
| --- | ---: | ---: | ---: |
| Baseline | 14,351,702 | — | 12,125,697 |
| Initial | 14,349,684 | −2,018 | 12,124,005 |
| Final | 14,350,182 | −1,520 | 12,124,503 |

These are the full type-matrix harness modules, not application Wasm sizes.
No claim follows about production endpoint throughput or complete heap footprints.

## Validation and reproduction

Two codec tests cover every u8 encoding, both Boolean encodings, invalid Boolean
bytes and wrong widths, and borrowed/owned compatibility. All 28 benchmark library
tests pass, including catalogue coverage and ICSS image interchange. The focused
driver admission test, strict Storage Clippy and formatting pass. Installed
comparison runs validate results and final state; full release and lifecycle
qualification were not rerun for this development batch. No deployment occurred.

Frozen ordinary modules and sources are under `target/evidence/storage-comparison/`:
`attempt.5MG9NM` (baseline focused; byte-identical to prior depth baseline
`attempt.zIwiCf`), `attempt.LAF83K` (initial), `attempt.peWKLJ` (initial depth),
`attempt.Ki7QOT` (final) and `attempt.nPqHnW` (final depth). Diagnostics, comparison
JSON, driver/lock hashes and custom reports are under
`target/evidence/storage-byte-codecs/`. These raw artifacts are transient;
this summary is the retained record.

```sh
BENCH_CASES=VectorU8,VectorBool,CellBool BENCH_ROWS=4096 \
  BENCH_ROUNDS=3 BENCH_SAMPLES=1 make benchmark-storage
BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage
```

For affected roles, substitute the existing case labels
`Map-U64-U8,Map-U64-Bool,Map-U8-U64,Map-Bool-U64,Region-U64-U8,Region-U64-Bool,Region-U8-U64,Region-Bool-U64,Set-U8-Unit,Set-Bool-Unit,Vector-U64-U8,Vector-U64-Bool,Cell-U64-U8,Cell-U64-Bool`.
Use the [boundary-probe instructions](../../../qualification/ic-storage-benchmarks/README.md#iterator-coverage)
with identical custom arguments and `--trace-execution-cost`; preserve ordinary
modules and verify response/inner-counter equivalence before interpreting results.
