# Write-path experiments against ICSS 0.7.2

This study compares deployed Storage 0.68.4 with published **ic-stable-structures
0.7.2**, not an older Storage engine. Public APIs and retained formats stay fixed.
An individual change should normally add no more than 5–10 KB of Wasm. Results
below are synthetic benchmark handlers, not complete application endpoints.

## Matched inputs and measurement scope

Baseline modules are frozen in `target/evidence/storage-comparison/attempt.g5v6Ay`
at source `3af68a5`. The independent workspace retains Rust 1.99.1, Runtime 0.19.5,
Wire 0.17.6, Testkit 0.79.0 and PocketIC 16.1.0. ICSS uses eight-page buckets.
Builds use the unchanged comparison profile and eight-MiB synthetic stack.

The depth workload covers Map, Set, Vector, Cell and map regions at 256/4,096
records, uniform/hot-16 accesses, batches 1/32 and three stateful rounds, one sample.
Cell retains one record. Each run validates 4,272 calls / 712 aggregate cells.
Boundary probes include request/reply codecs and cleanup; their inner round counts
must match the ordinary modules. Stable-page counts and the ICSS reference are controls.

Raw output, native counters and diagnostic scripts live in
`target/evidence/storage-write-paths/`; candidate source/module hashes and frozen
sources remain under `target/evidence/storage-comparison/`. Reproduce ordinary
runs with `BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage`, and use
the [benchmark guide's boundary probe](../../../qualification/ic-storage-benchmarks/README.md#iterator-coverage).
Size measurements parse section-10 payload lengths from uncompressed Wasm.

An independent edit advanced the root lock to Wire 0.17.7 during the first trial.
That edit is preserved. The independent benchmark lock, sources and artifacts
passed their hash checks, and the comparisons passed; the outer `NMLgPD` command
reported the unrelated root-lock drift. This is not a clean aggregate command pass.

## 1. Same-length replacements: rejected for size

The trial overwrites only a value's existing byte range when its encoded length
is unchanged. By-value entries without stored offsets and size changes fall back
to the original whole-node save. Tests covered empty through 64-KiB values,
overflow pages, neighbours, reopening, growth/shrink and ICSS image interchange.

For 32 uniform replacements of 32-byte values in a 4,096-record map, native
Storage writes fall from 580 calls / 7,968 bytes to 32 calls / 1,024 bytes. ICSS
over governed backing remains at 580 / 7,968. Native stable-range counts in this
example do not fall; IO-call reduction is not itself a page-charge measurement.

Full-handler instructions for the initial `NMLgPD` trial:

| Case, 4,096 records / batch 32 / uniform | Baseline | Trial | ICSS |
| --- | ---: | ---: | ---: |
| u64 Map Replace | 2,133,703 | 1,687,116 | 2,251,444 |
| 32-byte Map Replace | 3,209,558 | 2,743,267 | 3,342,919 |
| u64 Region Replace | 2,274,684 | 1,759,627 | 2,274,181 |

The initial handler matrix improves/matches/regresses 52/594/66 cells; the worst
regression is 0.075%. ICSS costs, stable pages and probe inner counts match.
Two refinements shared byte operations outside key/value-specialised callers,
then moved the update routine onto nodes. Both pass native and installed result
checks, but neither meets the size budget. Their full handlers were not profiled.

| Variant | Total Wasm bytes | Total delta | Code bytes | Code delta |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 14,360,722 | — | 12,137,165 | — |
| Initial `NMLgPD` | 14,533,192 | +172,470 | 12,284,318 | +147,153 |
| Byte helper `RuaaRD` | 14,456,784 | +96,062 | 12,189,445 | +52,280 |
| Node update `sGBxIZ` | 14,462,048 | +101,326 | 12,203,363 | +66,198 |

All three implementations and the trial-only write-count assertion were removed.
The additional same-length replacement history in the ICSS interchange test is
useful with either implementation and remains. The production update path is unchanged.

## 2. Bounded child-address encoding: rejected for runtime regressions

The first trial (`Ir8hxz`) encodes at most 12 child addresses in a 96-byte stack
buffer, in a helper shared across key types. It removes the temporary vector.
Total Wasm falls to 14,312,303 bytes (−48,419); code falls to 12,088,277 (−48,888).
However, full-handler cells improve/match/regress 129/338/245, with a 3.27% worst
regression. ICSS costs, stable pages and all probe inner counts match.

| Case, 4,096 records / batch 32 | Baseline | Shared helper | ICSS |
| --- | ---: | ---: | ---: |
| u64 Map Replace, uniform | 2,133,703 | 2,147,812 | 2,251,444 |
| u64 Region Replace, uniform | 2,274,684 | 2,270,260 | 2,274,181 |
| Set Reinsert, hot-16 | 1,993,559 | 2,058,731 | 2,151,367 |

The inline variant `z6R63l` reduces total Wasm to 14,351,584 (−9,138) and code to
12,128,011 (−9,154). Its handler counts remain 129/338/245; u64 Map Replace costs
2,147,043 (+0.625%) and hot Set Reinsert 2,057,740 (+3.219%). Both variants pass
native tests, ICSS image interchange and the 4,272-call ordinary/boundary runs.
Both were reverted. Removing an allocation can change later allocator history;
these traces do not isolate the cause of every secondary regression.

## 3. Cell header writes: retained

The candidate combines the existing magic, version and length into one eight-byte
write, retaining the payload write. A native test checks two writes / 2,056 bytes
for an existing Cell's 2,048-byte replacement, followed by header admission and
reopening checks. The encoded image and cached-value semantics are unchanged.

Trial `NsIPbc` reduces total Wasm from 14,360,722 to 14,360,060 bytes (−662), and
code from 12,137,165 to 12,136,503 (−662). All eight Cell replacement handler cells
improve; the other 704 depth cells are identical. ICSS costs, stable pages and
every boundary-probe inner count match their controls.

| Fixed-32-byte Cell / uniform / full handler | Baseline | Retained | ICSS |
| --- | ---: | ---: | ---: |
| Get, batch 32 | 686,607 | 686,607 | 767,943 |
| Replace, batch 1 | 654,102 | 653,847 | 733,996 |
| Replace, batch 32 | 878,986 | 870,826 | 955,587 |

The saving is 255 instructions per replacement in this workload, about 0.93% for
the batch-32 handler. This is a small improvement, not an application-wide speedup.
The additional full Cell catalogue checks 73 types / 1,200 cells / 2,400 calls
across both libraries, with no refusals. It validates the broader type surface;
no before/after full-handler claim is made for that separate catalogue.

All 23 adapter tests, seven ICSS image-interchange tests, focused Cell checks,
strict library/benchmark lint and installed original Storage IO/upgrade/rollback
pass. Installed proof covers 1,031 named allocations. Other optimizer profiles
and release gates were not run; this is development qualification, not delivery.

Strict lint flagged the initial array-size cast. The final literal-size correction
passes lint and rebuilds to byte-identical benchmark Wasm (SHA-256
`ab898a95b4a42030a90a2ed727cde8f761338ca16ce3857b96d2bcd45410c313`).
The measured artifacts therefore qualify the final source; final source/input
hashes and the rebuilt lifecycle fixture are retained with the raw evidence.
