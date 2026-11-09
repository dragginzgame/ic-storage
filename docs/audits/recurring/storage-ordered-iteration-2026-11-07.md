# Ordered iterator costs versus ICSS

Reference: published **ic-stable-structures 0.7.2**, not an older Storage engine.
Entry-iterator shortcuts shipped in Storage 0.68.4 and remain intact. The key/value
projection wrapper and direct-call trials were rejected and reverted because of Wasm growth.

## Direct `next_back()`: rejected size tradeoff

Replacing only `keys().last()` and `values().last()` in the shared benchmark caller
with `next_back()` changes both Storage and published ICSS 0.7.2. No library code or
new generic wrapper is needed, but the existing typed reverse traversal still compiles.

| Full-catalogue Wasm | `.last()` bytes | `next_back()` bytes | Delta bytes |
| --- | ---: | ---: | ---: |
| Storage total | 14,360,722 | 14,750,150 | +389,428 |
| Storage code section | 12,137,165 | 12,445,023 | +307,858 |
| ICSS total | 11,125,785 | 11,505,770 | +379,985 |
| ICSS code section | 9,518,618 | 9,828,757 | +310,139 |

Code sizes are section-10 payload lengths parsed from the original uncompressed
modules. Storage saves only 5,473 bytes versus the rejected wrapper. Removing the
wrapper does not solve the size problem; exact function-level attribution remains open.
These harness sizes do not establish an individual application's increase.

Full-handler instructions, same 256-record/32-query scope as the wrapper trial:

| Case / operation | Storage `.last()` | Storage `next_back()` | ICSS `.last()` | ICSS `next_back()` |
| --- | ---: | ---: | ---: | ---: |
| u64 keys | 7,566,251 | 1,097,815 | 8,442,282 | 1,255,167 |
| u64 values | 12,688,445 | 1,161,441 | 13,415,727 | 1,318,140 |
| String keys | 28,731,462 | 1,612,247 | 30,877,638 | 1,648,569 |
| String values | 41,414,569 | 1,623,184 | 42,428,243 | 1,681,224 |
| u64/256-byte keys | 9,064,506 | 1,280,433 | 9,739,175 | 1,427,772 |
| u64/256-byte values | 27,300,590 | 2,621,389 | 26,111,518 | 2,764,876 |

All 126 projection cells improve for both libraries (Storage handlers 26–97%).
Storage's whole-matrix handler improve/match/regress counts are 193/1,722/47;
worst other handler +0.662%. ICSS counts are 160/1,599/203. Stable pages match.
Both ordinary and boundary runs validate 11,772 calls; all inner round counts match.

Baseline `g5v6Ay` and direct-call `CpMLNZ` use the same compiler, locks and driver.
Reproduce by changing those two calls in `benches/wasm.rs`, running the ordered
preset and boundary probe below, then restoring the calls; trial operation labels
remain `KeysLast`/`ValuesLast`. Raw results: `target/evidence/storage-next-back/`.
The sandbox refused the initial server bind; the admitted local-server retry passed.
Both call-site edits were restored and checked against the baseline source hashes.

## Key/value projections: rejected experiment

`keys()`, `keys_range()` and `values()` expose Rust's general mapping
iterator. Its `last()` walks every item. The candidate's private, allocation-free envelope
selected the last remaining item through the existing reverse cursor, forwarding
other iterator operations unchanged. Public return signatures, engines, layouts
and governed access remain unchanged. Regions retain their separate snapshot path.
Only the selected value is decoded; this is not validation of discarded records.
A subsequent caller-supplied mapping closure keeps standard Rust iteration semantics.

Matched full-handler instructions: 256 records, 32 fresh iterator queries,
shuffled population, uniform inputs, one seed, medians over three stateful rounds.
ICSS uses eight-page buckets. The handler includes request/reply codecs and cleanup,
but excludes population setup. These are synthetic handlers, not application endpoints.

| Case / operation | Storage 0.68.4 | Candidate | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| u64 Map keys last | 7,566,251 | 1,097,879 | 8,442,282 |
| u64 Map values last | 12,688,445 | 1,161,505 | 13,415,727 |
| String Map keys last | 28,731,462 | 1,612,311 | 30,877,638 |
| String Map values last | 41,414,569 | 1,623,248 | 42,428,243 |
| u64/256-byte Map keys last | 9,064,506 | 1,280,497 | 9,739,175 |
| u64/256-byte Map values last | 27,300,590 | 2,621,453 | 26,111,518 |

All 126 projection-last cells improve: 58–98% in-loop, 26–97% for full handlers.
Across all 1,962 cells, handler results improve/match/regress 193/1,722/47;
loop results 180/1,749/33. Worst handler regression: 256-byte U64Bytes Replace,
batch 5, 1,235,285 → 1,243,463 (+0.662%; ICSS 1,311,137). Its loop rises 1.657%.
The trace is stateful; these secondary changes are not isolated allocator attribution.

Full-catalogue benchmark Wasm grows 14,360,722 → 14,755,623 bytes (+2.75%);
section-10 payload grows 12,137,165 → 12,450,496 bytes (+313,331).
This includes all harness type instantiations and is not a production-canister size
estimate. Total growth is 394,901 bytes (395 KB / 386 KiB). The maintainer rejected
this tradeoff and requested reverting the wrapper and its dedicated tests. Code
is restored to the deployed 0.68.4 baseline; these results describe the rejected
candidate, not current behaviour. Future changes should stay within 5–10 KB of
Wasm growth unless an exceptional benefit justifies more. Additional generic
iterator instantiations are a possible explanation, not a function-level attribution.

## Candidate qualification and reproduction

Baseline `g5v6Ay` uses released 0.68.4 at source `3af68a5`; candidate `KaX5nl`
changes only the adapter and native tests. Each ordinary run and frozen boundary
probe validates 11,772 calls. ICSS modules and driver match byte-for-byte; both
probes preserve every ordinary inner count, and median stable-page counts match.
Both use Rust 1.99.1, Runtime 0.19.5 and PocketIC 16.1.0 via Testkit 0.79.0.

All 24 adapter tests, seven ICSS image-interchange tests, strict Clippy and focused
installed Storage IO/upgrade/rollback pass. Native tests cover empty and consumed
ranges, wide keys, values up to 64 KiB, selected-value corruption, read counts,
one selected-value decode, and execution of caller-supplied mapping callbacks.
The full type timing catalogue, other optimizer profiles and release gates were
not rerun; they are separate from this focused development qualification.

Run `BENCH_WORKLOAD=ordered BENCH_SAMPLES=1 make benchmark-storage`; apply the
[benchmark guide's boundary probe](../../../qualification/ic-storage-benchmarks/README.md)
to frozen original modules for handler results. Current raw evidence lives under
`target/evidence/storage-projection-last/` and `target/evidence/storage-comparison/`.
The shared [selected matrix](../../measurements/storage-icss-backing-steps.csv)
owns `projection-last-handler-*` and historical `iterator-last-handler-*` rows.
Historical raw iterator-last runs are no longer present in the current target;
their compact results below are records, not proof of retained raw artifacts.

## Historical entry shortcut: released 0.68.4

Map iterator `last()` now calls its existing reverse cursor; Set inherits it.
Ranges and partial consumption retain their results. ICSS 0.7.2's iterator
still scans forward, though its direct Map/Set last-entry APIs are already fast.
Key/value projections were unchanged in this step. No new cache, public API or layout.

The expanded ordered harness separates `IterLast`, `KeysLast` and `ValuesLast`
from direct `Last`. Baseline `ksmFDM` includes step 11; candidate `w0luRK` differs
only in this shortcut and native tests. Each validates 11,772 calls / 1,962 cells.
Full-handler instructions below: 256 records, 32 fresh iterator-last queries,
shuffled setup, uniform inputs, one seed and medians over three stateful rounds;
ICSS uses eight-page buckets. CSV rows: `iterator-last-handler-*`.

| Case | Before | After | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| u64 Map | 7,757,897 | 1,182,226 | 8,623,678 |
| String Map | 25,413,172 | 1,833,104 | 27,747,470 |
| u64 Set | 7,688,250 | 1,107,900 | 8,746,399 |
| String Set | 25,346,909 | 1,519,380 | 30,710,448 |

All 90 `IterLast` cells improve: 58–98% in-loop and 26–97% for full handlers.
Across the whole matrix, handler cells improve/match/regress 139/1,757/66;
loop cells 126/1,783/53. Worst handler regression: U64Bytes ValuesLast, 256-byte
values, batch 32, 26,702,298 → 27,300,590 (+2.24%; ICSS 26,111,518).
Outside key/value-last controls the worst handler increase is 0.282%.
Do not attribute those stateful-trace differences to a specific allocator cause
without further isolation. Ordinary lookup and scan algorithms are unchanged.

ICSS modules and driver are byte-identical; every boundary probe preserves all
ordinary inner counts and all median stable-page counts match. Benchmark Wasm
grows 14,064,129 → 14,360,735 bytes; section-10 payload grows
11,868,259 → 12,137,165. These are full-catalogue harness sizes, not application sizes.
All 23 adapter and 49 benchmark/model/image tests, strict lint and focused
installed IO/upgrade/rollback pass. Evidence: `target/evidence/storage-iterator-last/`.
Reproduce with `BENCH_WORKLOAD=ordered BENCH_SAMPLES=1 make benchmark-storage`
and the benchmark guide's boundary probes. Full type timing and all optimizer
profiles were not rerun; this is a terminal-iterator gain, not universal parity.
