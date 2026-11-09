# B-tree save and root-search experiments against ICSS 0.7.2

Keep the root-search shortcut; reject overflow-list transfer. Both trials use
the same pending baseline `sGL0uw` from the
[empty-value study](storage-empty-values-2026-11-07.md). Reference means published
**ic-stable-structures 0.7.2**, not an earlier Storage engine. Public APIs and
persisted geometry remain fixed; no migration or new tuning parameter is added.

## Matched scope

The `depth` preset covers all five collection forms with u64 and 32-byte values,
256/4,096 records, batches 1/32, uniform/hot-16 access and three stateful rounds.
Cell has one value. Each run checks 4,272 calls and 712 aggregate cells. Whole
handlers include request/reply codecs and cleanup; population setup is excluded.
These are synthetic collection measurements, not application speedups.

Keep Rust 1.99.1, Runtime 0.19.5, benchmark Wire 0.17.6, Testkit 0.79.0,
PocketIC 16.1.0, ICSS eight-page buckets and the comparison profile fixed.
The full type harness uses the existing eight-MiB stack. Root Wire independently
advanced from 0.17.9 to 0.17.10 during this study; that edit is preserved. Earlier
transfer native tests used 0.17.9; final root-search native tests/lint use 0.17.10.
The benchmark graph did not change, and no dependency adoption is claimed here.

Both trials retain byte-identical ICSS modules and benchmark drivers. All 4,272
ordinary/probe responses match per trial, including inner instruction counts;
boundary probes add no linear memory. Stable pages and ICSS costs stay fixed.
All 480 native IO rows match the baseline for each trial. Neither change reduces
measured stable payload IO; native range counts are not heap-fault measurements.

## Overflow-list transfer: rejected

Trial `6AbJuM` transfers the loaded node's overflow-address vector into its writer
instead of cloning it. All entries are loaded first, the original first pointer
is saved, and the completed writer returns the list. This removes an allocation
and copy for nonempty lists without changing the image. The source was reverted.

Loop cells improve/match/regress **46/600/66**, with maximum saving/regression
0.295%/4.07%. Full-handler cells are **50/591/71**, with maxima 0.157%/2.95%.
The worst handler is the populated hot-16 Set RemoveInserted trace. Native and
installed correctness checks pass, but neither fewer allocations nor reduced
Wasm size justifies those regressions. The exact heap-layout contribution was
not isolated; allocation elimination alone is not a throughput measurement.

An additional matched `bulk` preset uses eight records (Cell one), 64-KiB/1-MiB
payloads, uniform access, batch one and one round/sample. It checks 228 calls
and 114 cells per run across Map, bounded Vector, Cell and regions. The retained
frozen pre-change module supplies the baseline. These are **inner-loop**, not full-handler costs:
the current execution-probe selector does not admit this workload.

Bulk cells improve/match/regress **54/40/20**. Maximum gain is 16.42% on Map
MissAbove at 64 KiB, after population and earlier stateful operations; it does
not isolate a saving in the save routine. Worst increase is 53 instructions
(9.40%) on a Region control; excluding controls, Region RemoveMiss increases
8.53%. Stable pages and ICSS costs match. Selected mutation results:

| Bulk loop, one operation | Baseline | Rejected transfer | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| 64-KiB Map Replace | 4,215,361 | 4,206,273 | 4,301,440 |
| 64-KiB Map Shrink | 5,934,196 | 6,087,137 | 6,020,820 |
| 64-KiB Region Grow | 6,559,645 | 6,710,474 | 6,026,040 |
| 1-MiB Map Replace | 65,670,611 | 65,441,968 | 64,234,852 |
| 1-MiB Region Shrink | 78,549,061 | 77,988,765 | 75,818,494 |

The larger-payload study does not reverse the rejection. Both frozen modules
and raw results remain available; the final library still clones the list.

## Non-full root search: retained

ICSS checks every existing root before testing fullness, then searches a
non-full root again in `insert_nonfull` when the first lookup misses. Storage's
[insert path](../../../crates/ic-storage/src/engine/btreemap.rs) now keeps the
pre-split lookup only for full roots. Non-full roots use the existing insertion
lookup once. Updating an entry already in a full root still avoids a split;
root misses, deeper updates and insertions retain their original algorithms.

Trial `eP5Nni` improves/matches/regresses **104/608/0** cells in both timing
scopes. Maximum loop saving is 0.404%; maximum full-handler saving is 0.263%.
This is a small comparison-overhead saving. The bulk trial above qualifies only
the rejected transfer; the root-search candidate was not timed on that preset.

Full-handler instructions, 4,096 records, batch 32, uniform unless labelled hot;
Cell has one value. Both alternatives are compared to the same baseline:

| Case | Prior pending baseline | Rejected transfer | Retained root search | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| u64 Map Replace | 2,122,433 | 2,122,134 | 2,117,729 | 2,251,444 |
| 32-byte Map Replace | 3,198,558 | 3,198,570 | 3,193,822 | 3,342,919 |
| u64 Set Reinsert | 1,791,373 | 1,791,954 | 1,786,669 | 2,065,794 |
| u64 Set RemoveInserted, hot | 2,044,013 | 2,104,356 | 2,044,013 | 2,361,759 |
| u64 Region Replace | 2,259,888 | 2,259,286 | 2,255,184 | 2,274,181 |
| u64 Vector Set | 695,155 | 695,155 | 695,155 | 772,509 |
| 32-byte Cell Replace | 870,826 | 870,826 | 870,826 | 955,587 |

This is not universal ICSS parity: Region Reinsert still costs 2,304,751 versus
2,301,196 here. ICSS's region reference reopens a map per operation.

| Full-catalogue Wasm | Total bytes | Code-section payload |
| --- | ---: | ---: |
| Prior pending baseline | 14,351,042 | 12,127,485 |
| Rejected transfer | 14,344,750 | 12,121,193 |
| Retained root search | 14,351,702 | 12,125,697 |

Root search adds **660 total bytes**, while code payload falls by 1,788 bytes.
The complete pending batch is **8,358 bytes smaller** than deployed 0.68.5,
improves/matches/regresses **368/344/0** handler cells and saves up to **11.35%**.

## Qualification and evidence

Each trial passes 80 core tests, 28 benchmark/model/codec/interchange tests,
strict core lint, formatting and installed original Storage IO/upgrade/rollback
across 1,031 allocations. Existing interchange histories exercise lazy large
keys/values, splits, replacement, overflow shrink/regrowth and freed-page reuse.
All final source/input hashes match the retained root-search attempt.

Reproduce depth with `BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage`
and the [boundary-probe guide](../../../qualification/ic-storage-benchmarks/README.md#iterator-coverage).
Bulk used the same frozen driver/modules with `--workload bulk --samples 1`.
Raw evidence: `target/evidence/storage-overflow-transfer/` and
`target/evidence/storage-root-search/`; frozen sources/modules remain under
`target/evidence/storage-comparison/attempt.*`. Other optimizer profiles, the
full installed 73-type timing sweep and release gates were not rerun. Package
versions, Git history and publication remain unchanged.
