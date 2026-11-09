# B-tree node-load experiments against ICSS 0.7.2

Reject all three trials. Avoiding a redundant node load improved selected
operations, but each trial regressed other whole-handler cells. The final
ownership variant also exceeded the Wasm growth budget. Production source is
restored byte-for-byte; no API, image, dependency or release change is retained.

## Matched scope

The starting source is `697be6037365359f2319479d04e82b32db59531b` (Storage 0.68.7
delivery handoff). Preserve the incoming root Wire 0.17.12 lock edit; it is not
part of these experiments. The independent benchmark graph stays fixed at
Runtime 0.20.0, Wire 0.17.6, Rust 1.99.1, Testkit 0.79.0 and PocketIC 16.1.0.
Reference means published **ic-stable-structures 0.7.2**, with eight-page buckets,
not an older Storage release. Both libraries use the existing comparison profile
and synthetic eight-MiB stack. No new generic specialization is introduced.

Each ordinary depth run checks 4,272 calls and reports 712 aggregate cells:
Map, Set, Vector, Cell and map regions; u64 and 32-byte values; 256/4,096 records;
uniform/hot-16 access; batches 1/32; three stateful rounds with shuffled population.
Cell contains one value. One sample uses seed 1. Setup is excluded from timing,
but its allocation history remains. This is not a universal application speedup.

Each boundary-only probe measures the full benchmark handler, including codecs,
input preparation, collection work, response handling and cleanup. All 4,272
responses and inner instruction counts match the corresponding ordinary run.
The probe adds no linear memory. The ICSS modules and its measured costs are
identical across the trials, and no aggregate stable-page count changes.
Every trial is compared directly with the same baseline, not the previous trial.

## Trials and decisions

The copied [B-tree](../../../crates/ic-storage/src/engine/btreemap.rs) retains the
following ICSS 0.7.2 behaviors after this study:

1. Deletion loads both siblings before attempting a left rotation. Trial A
   delays the right-sibling load until after that early return. It improves
   256-record Set Remove by 2.39%, but 4,096-record Region RemoveMiss regresses
   1.74%. Reject despite smaller Wasm. Other handler regressions are at most 0.011%.
2. Insertion loads a full child, searches it, then `split_child` loads it again.
   Trial B passes the existing child by mutable reference. Insertion improves,
   but later Set Nth regresses 2.61%; read-side changes are not explained by
   insertion comparisons alone. Reject despite a 15,545-byte Wasm reduction.
3. Trial C passes that child by value, releasing it before the caller loads the
   selected child. It removes some of B's regressions but still regresses Map
   Reinsert by 1.80% and grows Wasm by 19,009 bytes. Reject under the size budget.

The split trials change temporary allocation lifetime during population and
subsequent insertions. Effects on later reads are consistent with that history,
but heap addresses and fault charges were not isolated. Fewer source-level loads
do not establish a lower whole-execution charge. Do not attribute these results
to a particular allocator/page mechanism without a separate trace.

| Trial | Loop improved/equal/regressed | Handler improved/equal/regressed | Best handler saving | Worst handler increase |
| --- | ---: | ---: | ---: | ---: |
| A: delayed right sibling | 22 / 614 / 76 | 22 / 615 / 75 | 2.386% | 1.744% |
| B: borrowed split child | 154 / 474 / 84 | 155 / 456 / 101 | 2.157% | 2.613% |
| C: owned split child | 169 / 488 / 55 | 168 / 479 / 65 | 2.157% | 1.798% |

## Per-step comparison with ICSS

Full-handler instructions. Unless labelled otherwise, rows use 4,096 records,
batch 32 and uniform keys. All variants are rejected; the baseline remains current.
The table includes every supported collection form and adverse cases.

| Case | Storage baseline | A | B | C | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Set Remove, 256 records | 1,966,863 | 1,919,939 | 1,966,863 | 1,966,863 | 2,285,374 |
| u64 Map Insert | 2,453,778 | 2,453,778 | 2,418,791 | 2,418,840 | 2,593,969 |
| 32-byte Map Insert | 2,724,529 | 2,724,529 | 2,683,276 | 2,680,840 | 2,877,150 |
| u64 Map RemoveMiss | 1,351,257 | 1,351,385 | 1,372,397 | 1,372,397 | 1,520,257 |
| Set Insert | 2,019,489 | 2,019,489 | 1,984,711 | 1,983,355 | 2,342,775 |
| Set Nth, batch 1 | 2,717,526 | 2,717,526 | 2,788,529 | 2,717,526 | 3,056,777 |
| Region Insert | 2,637,934 | 2,637,952 | 2,603,079 | 2,603,295 | 2,610,845 |
| Region RemoveMiss | 1,401,983 | 1,426,439 | 1,401,983 | 1,401,983 | 1,535,258 |
| Vector Set | 695,155 | 695,155 | 695,155 | 695,155 | 772,509 |
| 32-byte Cell Replace, one value | 870,826 | 870,826 | 870,826 | 870,826 | 955,587 |

| Full-type benchmark Wasm | Total bytes | Code-section payload | Total delta |
| --- | ---: | ---: | ---: |
| Baseline | 14,351,702 | 12,125,697 | — |
| A | 14,347,713 | 12,118,057 | −3,989 |
| B | 14,336,157 | 12,110,136 | −15,545 |
| C | 14,370,711 | 12,144,690 | +19,009 |

These are original full-catalogue harness bytes, not application Wasm sizes.
Smaller artifacts do not compensate for the measured instruction regressions.

## Qualification, evidence and continuation

All trials pass 80 native core tests and 28 benchmark/model/codec/interchange
tests. Trial B also passes strict core Clippy and formatting. The installed
ordinary/probe runs verify results and final state; no candidate was promoted to
the separate IO/upgrade/rollback suite or release qualification. The 73-type
installed timing sweep, additional seeds and other optimization profiles were
not run. Successful correctness tests do not qualify performance outside this scope.

Baseline and A/B/C frozen sources, modules, input hashes and ordinary reports are
under `target/evidence/storage-comparison/attempt.zIwiCf`, `attempt.BBBG4y`,
`attempt.jOKdyT` and `attempt.BCZkZg`, respectively. Additional evidence lives in
`target/evidence/storage-sibling-load/`, `storage-split-load/` and
`storage-split-owned/`: preserved trial sources, native logs, ordinary/probe
response checks, size tables and complete loop/handler comparisons. No evidence
or cache cleanup was performed; explicit target cleanup would remove these artifacts.

Reproduce ordinary runs with `BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage`.
Use the [boundary-probe procedure](../../../qualification/ic-storage-benchmarks/README.md#iterator-coverage)
on those frozen modules, `--execution-only` injection and driver
`--trace-execution-cost --workload depth --samples 1 --upstream-bucket-pages 8`.

Next useful investigation: isolate allocation and heap-page effects of population
before changing more node lifetimes. Compare unchanged read operations over these
frozen images; distinguish transient allocator state from persisted tree layout.
That evidence should precede another load-elimination or ownership rewrite.
