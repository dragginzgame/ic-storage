# Storage backing parity with ICSS

Reference: published **ic-stable-structures 0.7.2**, not earlier Storage. Updated
2026-11-07: routing-read improvements preserve layouts, APIs and governance.
Steps 1–8 shipped in Storage 0.68.1; steps 9–10 shipped in Storage 0.68.3.
Steps 11–12 target 0.68.4; the current handoff records delivery state.

The [step/round matrices](../../measurements/storage-icss-backing-steps.csv) and
[payload matrix](../../measurements/storage-icss-backing-payloads.csv) retain both costs.
Raw attempts: `target/evidence/storage-comparison/`; ICSS buckets: eight pages unless specified, default 128.

## Steps 1–2: isolate backing and remove repeated admission

`dfinity-governed` puts ICSS on Storage memory; differing Rc/codec wrappers prevent
an exact algorithm-only control. Regions reopen per operation; ICSS has no region
API. Retained directory indices, inline cache hits and static region checks reduce
repeated work. Bounds, custody, poisoning, later bindings and fragmented growth
are tested. CSV runs: `WQ6DQa`, `J3enEn`, `ChfB4h`; individual effects are not isolated.

## Step 3: restore ICSS's direct reads

Forwarding `read_unsafe` through leases and regions removes an inherited zeroing
fallback. Custom public backings retain a default; private views must forward it.
Successful reads initialize every byte; interrupted native IO still poisons leases.

Matched depth results below use **shuffled** insertion, 4,096 records, uniform
batches of 32 and three samples/rounds. Cell retains one value. Sources are
`j0xUKb`, `uZ0Imy`, `Zny95C`; each depth run checks 11,808 calls. Both populations
and additional types are in the selected step matrix.

| Operation | Before backing work | Index/bounds | Direct read | ICSS |
| --- | ---: | ---: | ---: | ---: |
| u64 Map Get | 1,235,681 | 1,231,303 | 995,006 | 930,341 |
| u64 Map Replace | 2,122,543 | 2,039,588 | 1,769,297 | 1,650,756 |
| Set Get | 1,136,425 | 1,132,173 | 892,936 | 838,132 |
| Vector Get | 101,880 | 101,752 | 101,432 | 102,933 |
| Cell Get | 3,639 | 3,643 | 3,643 | 4,349 |
| Region Get | 1,766,548 | 1,365,035 | 1,134,372 | 954,675 |
| Region Replace | 2,890,110 | 2,223,279 | 1,960,260 | 1,674,410 |

Ordinary-insertion control `yh1wyH` checks 1,152 calls: Storage/ICSS-governed Map Get
565,640/579,637; region Get 648,418/601,845. Payload runs `l0wLHL`/`Suq31G` check
7,776 calls with 31-byte/4-KiB values. Region overhead persists on shared backing.

Step 4 (`3P6tEd`, 11,808 comparisons) regresses Map/Set and saves under 0.5% on
region Get by passing indices through every IO and storing bounds. Reverted
byte-for-byte; the CSV retains both sides.

## Step 5: locate the wider Vector gap

`vector-reads` fixes 256 rows, scalar/fixed/chunk/Wire values, uniform/hot-16 access,
batches 1/32 and three rounds. ICSS's eight-page timed loop becomes 160k cheaper
after Push; Storage stays flat. Runs `VtOuHy`/`mUd1uV`/`2wi3e4` each check 2,520 calls.
The whole-handler attribution below changes the interpretation of that gap.

## Steps 6–7: trace allocation, then correct the measurement boundary

Frozen-Wasm probes append globals/functions without moving linear memory/data.
Binaryen 133 uses no optimization passes; a plain WAT round trip reproduces all
840 sample-0 counts. In dlmalloc 0.2.14, growth changes chunk selection: a footer
write at heap offset 8,659,336 touches region 132 **before** the loop timer,
moving one costly header write outside it. The two allocator probes each check
840 calls. Probe costs are diagnostic, not additive production instructions.
A boundary-only probe includes request decoding, preparation, reply and cleanup,
but excludes population/host setup. All inner counts match across 2,520 calls at
buckets 1/8/128; a small common boundary-probe overhead remains.

4-KiB chunks (1-MiB declared maximum), 256 rows, uniform batch 32, round 0:

| Scope / checkpoint | Storage | ICSS, eight-page buckets |
| --- | ---: | ---: |
| Inner loop after Set | 3,040,020 | 3,042,082 |
| Inner loop after Push | 3,040,020 | 2,882,183 |
| Full handler after Set | 24,337,272 | 24,418,902 |
| Full handler after Push | 24,337,272 | 24,419,160 |

The apparent 159,899 saving disappears at the handler boundary (+258 instead).
ICSS bucket-1/128 handler counts stay 24,418,970/24,418,923 across growth. The large
reply dominates: this does not justify changing geometry or the Vector algorithm.
`vector-execution-boundary` CSV rows retain both scopes. Raw probes are under
`target/evidence/storage-icss-parity/`; reproduce via
[the benchmark guide](../../../qualification/ic-storage-benchmarks/README.md#iterator-coverage).

## Step 8: remove small fixed Vector read allocations

[BaseVec](https://github.com/dragginzgame/ic-storage/blob/3c4734d9bd413b8a902f23458ebdc222255cd9e7/crates/ic-storage/src/engine/base_vec.rs) uses a separate stack helper
for direct Get/Pop of fixed-width values ≤32 bytes without destructors. Other
values keep owned input; iterators reuse buffers. Isolating the helper removes
the initial inline trial's miss regression. APIs, formats and policy are unchanged.

256 rows, uniform batch 32, round-0 GetAfterSet, eight-page ICSS buckets:

| Type / scope | Storage before | Storage final | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| u64 / loop | 101,432 | 95,032 | 102,933 |
| Fixed 32 / loop | 104,970 | 98,570 | 104,577 |
| u64 / full handler | 670,749 | 664,349 | 752,611 |
| Fixed 32 / full handler | 796,572 | 790,172 | 876,746 |

Allocations fall from 32 to zero; the same 6,400 saving survives outside the loop.
Fixed 33/64/256, variable chunks and Wire controls stay unchanged; 84 u128 cells
rise six instructions, all other typed cells improve/match. Wasm shrinks 3,735 bytes.
The 7,200 checked calls include 4,680 typed and 840 each ordinary/allocation/boundary;
all boundary inner counts match. CSV: `vector-fixed-*`; raw: `vector-inline/` under
`target/evidence/storage-icss-parity/`; final build `mxipc1`.

## Steps 9–10: routing allocations and contiguous key reads

The fresh baseline uses published Runtime 0.19.4/Wire 0.17.6. Child addresses now
read into their final vector, eliminating ICSS's temporary byte buffer/copy.
Already-eager fixed-width keys ≤16 bytes read as one range, with unchanged
per-key decoding and overflow translation. Larger/variable keys retain their path.
For a representative single Map get: malloc calls 15 → 12; stable reads 70 → 45.
All 160 native comparisons reduce IO calls with unchanged bytes, codecs, stable
range sets, writes and capacity against ICSS over governed backing.

Full-handler instructions, 4,096 records, uniform batch 32, eight-page ICSS buckets;
one sample, three stateful rounds, shuffled setup. These are synthetic handlers.

| Operation | Before | Child buffer | Plus key batching | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| u64 Map Get | 1,569,403 | 1,540,592 | 1,417,427 | 1,568,879 |
| u64 Map Replace | 2,284,742 | 2,256,861 | 2,133,696 | 2,251,437 |
| Set Get | 1,430,919 | 1,390,684 | 1,240,334 | 1,440,540 |
| Region Get | 1,686,035 | 1,633,772 | 1,488,348 | 1,585,791 |
| Region Replace | 2,469,188 | 2,417,888 | 2,274,684 | 2,274,181 |

Baseline Map Get was already effectively equal to ICSS at the full boundary;
its 8.8% loop gap was misleading. Combined changes improve 499/656 loop cells,
leave 156 equal and add 69 instructions to one no-op control. Full handlers improve
511 cells, match 141 and regress four controls by at most 639 instructions (0.13%).
Child-only depth regressions disappear; Vector/Cell loops are unchanged.

Ordinary runs `qKBykW`/`SpnBn9`/`LLDlVi` each check 3,936 calls. Frozen probes live
under `target/evidence/storage-map-routing/{before,children,keys}/`; `routing-*`
CSV rows retain both scopes. The plain round trip and every boundary probe preserve
all ordinary inner counts. Read probes capture batch-one Gets only; overflow fails.
The wider `ordered` control checks 10,476 calls per run and 1,746 cells. Combined
changes improve 1,654 full-handler cells; 54 match and 38 regress. Worst: a 128-entry
string scan rises 1.63% (3,595,044 → 3,653,675; ICSS 3,626,366). This is a retained
tradeoff, not universal parity. Both wider boundary probes preserve all loop counts.

## Step 11: transfer lazy key buffers into owning decoders

ICSS 0.7.2 and the copied engine borrow a newly allocated lazy-key buffer for
decoding, forcing String/byte-vector codecs to copy it. Transfer it as owned
bytes when the key has a destructor; a compile-time check keeps fixed arrays on
their existing borrowed path. Eager keys still use their shared buffer.

The unconditional trial improved strings but regressed fixed-array iterator
handlers up to 1.07%. The retained refinement removes those regressions.
Full-handler instructions, 256 rows, uniform access, shuffled setup, one sample
over three stateful rounds, ICSS eight-page buckets:

| String-key operation | 0.68.3 | Unconditional | Retained | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| Map Get, batch 32 | 2,158,247 | 2,073,573 | 2,073,573 | 2,178,251 |
| Map Replace, batch 32 | 2,993,507 | 2,859,809 | 2,859,809 | 2,970,587 |
| Scan, 128 entries | 3,653,675 | 3,600,612 | 3,600,612 | 3,626,366 |
| Set Get, batch 32 | 1,677,730 | 1,590,373 | 1,590,373 | 1,727,726 |

Ordinary attempts `qEcOwW`/`tKETst`/`SmqSbx` each check 10,476 calls, as do their
frozen boundary probes; every probe preserves all ordinary inner counts. ICSS
modules/driver and all median stable-page counts match. Of 1,746 refined handler
cells, 289 improve, 1,452 match and only five no-op controls regress (≤0.095%).
Loop cells: 287 improve, 1,457 match, two regress: a no-op and single-key scan
(+113 instructions, 0.116%). Integer/fixed-byte measurements are unchanged.

The complete type-harness module falls from 13,793,677 to 13,789,967 bytes;
its code section falls from 11,603,300 to 11,599,590 bytes (both −3,710).
Code size is the Wasm section-10 payload length; no post-link optimization passes.
These are benchmark modules, not minimal production modules.
Raw evidence: `target/evidence/storage-owned-key/`; selected CSV rows: `owned-key-*`.
Seven native ICSS image tests cover all five collection forms, including new
4–65,536-byte String/byte-vector key histories. All 21 adapter tests, strict core
and benchmark lint, and original installed Storage IO/upgrade/rollback pass.
Other optimizer profiles and the full 73-type timing catalogue were not rerun.

## Step 12: select the last remaining entry through the reverse cursor

The [ordered-iterator study](storage-ordered-iteration-2026-11-07.md) owns the
released 0.68.4 entry shortcut and rejected key/value projection comparisons.

## Qualification, reproduction and delivery boundary

Focused engine/collection, ICSS cross-image, driver tests and strict Clippy pass.
Installed original/O3/Os/Oz IO, custody, upgrade and rollback pass in `attempt.UjEWo6`
under `target/qualification/runtime/`. Historical trials patched Platform `998764c`;
current proof uses published Runtime 0.19.4, Rust 1.99.1 and PocketIC 16.1.0.
Reproduce with `BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage` and
the benchmark guide's frozen probes. Raw outputs remain transient target evidence;
these comparisons do not qualify Auth endpoints, all populations or a 2× speedup.
