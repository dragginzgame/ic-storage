# Sharing Cell writes across codecs

Storage 0.68.9's comparison Wasm contains 68 `Cell::flush_value` specializations,
occupying 47,125 encoded function bytes. These are compiler specializations, not
68 independently maintained implementations. Published **ic-stable-structures
0.7.2** uses the same type-dependent organization; its matched module contains
63 specializations occupying 32,986 bytes. Counts alone are not a defect.

## Smallest useful separation

Keep encoding dependent on `T`, with the existing borrowed/owned `Cow` alive
through the write. Extract capacity checks, growth, the eight-byte SCL1 header
and the payload write into a private `flush_bytes<M>` helper. Prevent inlining
so those operations are compiled once per memory backing, rather than per value
type. No trait object, extra allocation, public API or stored-format change.
Cached values still change only after a successful write. Native poisoning and
IC rollback contracts remain unchanged.

The candidate has one 474-byte writer and 29 remaining type-specific
`flush_value` wrappers, together 9,201 encoded bytes. Other wrappers are inlined
into their callers. Distinct codecs still need distinct code; eliminating every
specialization is not the objective. Whole-module code reduction is smaller
than this symbol-group reduction because callers and inlining also change.

## Matched size results

| Artifact | Before bytes | After bytes | Delta |
| --- | ---: | ---: | ---: |
| Full comparison Wasm | 14,350,841 | 14,308,124 | −42,717 |
| Comparison code section | 12,118,858 | 12,088,524 | −30,334 |
| Small Storage consumer fixture | 191,692 | 191,499 | −193 |
| Consumer fixture code section | 168,166 | 167,980 | −186 |

These are uncompressed bytes. The broad comparison includes many codecs and
retains function names; its total reduction includes non-code sections. The
size-oriented consumer fixture strips symbols and uses far fewer types. A real
application must measure its own benefit; the 42.7-KB reduction is not a promise
for every consumer.

## Instruction tradeoff against ICSS

Full benchmark handlers, including request/reply work and cleanup:

| Fixed-32-byte Cell, uniform | Storage before | Shared writer | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Get, batch 32 | 686,808 | 686,808 | 767,943 |
| Replace, batch 1 | 653,824 | 653,846 | 733,996 |
| Replace, batch 32 | 871,027 | 871,731 | 955,587 |

The shared call costs 22 instructions per replacement: +0.081% for the batch-32
handler, or +0.394% for its measured inner collection loop. This is a size
optimization, not a speedup. Storage remains below ICSS in these cases.

Across the five-form depth workload, 704 aggregate cells are unchanged and eight
Cell replacement cells regress by that fixed overhead. All 2,136 individual
round comparisons show the same pattern: 2,112 unchanged, 24 regressed, none
improved. Map, Set, Vector, regions and cached Cell reads are unchanged. Stable
page counts and ICSS instruction counts match. Ordinary and instrumented
responses, including inner instruction counts, match exactly for each build.

## Full codec catalogue and adverse cases

The matched 73-type catalogue validates 2,400 calls per build with no refusals,
covering 1,200 inner-loop comparison cells: 100 improve, 750 are unchanged and
350 regress. Stable pages and ICSS instructions remain identical. Mutating
loops range from −0.123% to +0.353%; the largest absolute increase is 576
instructions for eight unit-value replacements. These are inner-loop timings,
not complete handlers, and the 22-instruction figure is specific to Fixed32.

Compiler changes also affect some tuple control and read loops: the worst
relative increase is 46 instructions on a 1,387-instruction TripleFixed control
loop (+3.32%). Follow-up boundary probes cover TripleFixed, TripleSBB and
TripleBytes at batches 1/32, all default payloads, three rounds. Across 56
handler cells, 18 improve, 12 match and 26 regress; the range is −0.103% to
+0.077%. The worst tuple control/read handler increase is +0.033%. Individual
rounds retain that conclusion (54 improved, 36 equal, 78 regressed).

| Tuple replacement handler, batch 32 | Before | Shared writer | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| TripleFixed | 1,131,094 | 1,131,964 | 1,213,796 |
| TripleBytes, empty components | 810,570 | 809,738 | 896,938 |

Retain the shared writer for the measured size benefit with this small, explicit
runtime tradeoff. Do not describe it as universally faster or claim all Cell
reads are unaffected. Other application footprints and full-handler timings
for the remaining catalogue types are unqualified.

## Reproduction and scope

Base source is `6efd86d` (Storage 0.68.9), with the unrelated root Wire 0.17.12
edit preserved. Baseline frozen run: `attempt.xOASTZ`; candidate: `attempt.LRun1o`
under `target/evidence/storage-comparison/`. The earlier `attempt.qp9hEg` built
identical baseline modules but could not bind PocketIC inside the sandbox;
its saved modules supply the baseline probes. The host rerun completes normally.

Raw analysis and probes live in `target/evidence/storage-cell-outline/`.
Use `BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage`, then the existing
`trace_stable_reads.pl --execution-only` boundary probe and `--trace-execution-cost`
driver. Depth covers 256/4,096 population contexts, uniform/hot-16 access,
batches 1/32 and three stateful rounds. Cell itself holds one value.
Use the same frozen modules with `--workload types --collection cell` for codec
breadth. Both suppliers use identical logical workloads; ICSS uses eight-page
buckets. The reference Wasm is byte-identical between builds.

Rust 1.99.1, Runtime 0.20.0, benchmark Wire 0.17.6, ICSS 0.7.2 and PocketIC 16.1.0
remain fixed. Comparison uses the existing optimized profile and eight-MiB
synthetic stack; the small fixture uses `wasm-size`. No production stack change.
Native adapter tests (35), benchmark/interchange tests (28 library + 21 driver),
strict library lint and installed Storage IO/upgrade/rollback pass, including
1,031 named allocations. Full release validation and other optimizer recipes
are outside this development experiment. No publication is performed.
