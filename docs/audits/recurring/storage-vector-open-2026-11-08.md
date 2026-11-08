# StableVec opening and initialization

This study compares the deployed Storage 0.68.8 engine, the retained change that
consolidates its header IO, and published ICSS 0.7.2. The benchmark harness is
identical between baseline and candidate. Production APIs, SVC1 framing,
fixed-stride slots and allocation governance must remain unchanged.

## Measurement boundary

`BENCH_WORKLOAD=vector-open BENCH_SAMPLES=1 make benchmark-storage` retains the
allocation owner but opens and drops a vector for each request. Storage obtains
a fresh governed lease; ICSS clones its existing virtual-memory handle. This
includes Storage's lease overhead without repeatedly constructing either
allocation manager. The ordinary retained-handle workloads remain controls.

The timed interval includes opening, the existing operation's preparation and
result conversion, and vector/lease destruction. `OpenControl` supplies the
same request/result machinery without a collection operation. Whole-handler
instrumentation additionally includes request decoding and response encoding.
The bounded stable-read tracer is not qualified for these nested timers and is
refused for this workload.

The matrix covers u64, fixed 32-byte arrays, bounded byte records with maximum
255 and 256 bytes, variable byte payloads and Wire records. It sweeps 256 and
4,096 live entries, uniform/hot-16 access, batches of 1/32, and three successive
rounds of read, replace, push and pop with intervening reads. Both backends must
produce the same values and final state.

Each population subreport retains `initialization.jsonl`. Its counter covers
the complete initialization body, including runtime/manager setup, vector
creation and destruction; it does not isolate a header write. Initialization
precedes population, so its `rows` field is context, not live initialization
population. The counter is stored in a Wasm global and queried separately.

The comparison recipe uses Rust 1.99.1, opt-level 3, fat LTO and the existing
8-MiB synthetic stack. The full benchmark harness size is not an application
size prediction. One matched sample and three stateful rounds do not establish
application throughput or explain Canic configuration/Auth session costs.

Raw evidence lives under `target/evidence/storage-vector-open/` and the frozen
attempt directories recorded there. It remains removable by explicit cleanup.

## Implementation and native IO

Storage combines the active 17-byte vector header into one read/write range.
The adapter uses the engine's framing result instead of reading the four-byte
marker again. Header helpers depend on the backing type, not the element type.
Empty memory and foreign magic/version retain typed `Format` errors; incompatible
element bounds still trap. Any nonzero fixed-size flag remains true, as in ICSS.
Length is still read from memory on every `len()`; no cache or new layout is added.

Native measurements exclude acquiring the governed lease and begin with an
already admitted header region. ICSS counts describe its published 0.7.2
BaseVec implementation, without a Storage admission wrapper.

| Operation | Storage 0.68.8 | Candidate | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Valid header load: reads / bytes | 6 / 21 | 1 / 17 | 5 / 17 |
| Header reset: writes / bytes | 5 / 17 | 1 / 17 | 5 / 17 |

Three new native tests first passed on the unchanged engine, then with the
candidate IO expectations. They preserve loaded values, uncached length reads,
reserved/payload bytes on reset, typed errors versus bounds traps, zero writes
on failed admission and lease release after failure. Existing image cross-read
tests additionally cover fixed/variable vectors against ICSS.

## Benchmark reporting correction

The first ordinary and instrumented attempts completed both populations, but
the server exited while unbuffered aggregate JSON serialization was still
running. Those attempts are retained as failures. Buffering the aggregate file
and explicitly flushing it allowed both complete reruns to succeed with
byte-identical canister Wasm. This changes native report output efficiency,
not measured canister work or report semantics.

## Results and decision

Keep the header consolidation. All **704 reopen cells** and **2,112 individual
rounds** save exactly **689 instructions**, both inside the reopen-inclusive
timer and across the complete handler. The whole-handler saving is only
0.0028–0.1174%; this is a modest fixed-cost improvement, not a large throughput
gain. All 712 retained-handle depth cells and 2,136 individual rounds are
unchanged, including their whole-handler costs, across all five collection forms.
ICSS results and both backends' stable-page counts remain identical between runs.

Selected complete-handler instructions, uniform access, 4,096 live entries;
medians over three stateful rounds, one matched sample:

| Vector value / operation | Batch | Storage 0.68.8 | Retained | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| u64 / open control | 1 | 587,499 | 586,810 | 650,646 |
| u64 / get | 1 | 589,696 | 589,007 | 653,109 |
| u64 / set | 1 | 668,136 | 667,447 | 731,244 |
| fixed 32 bytes / get | 1 | 593,845 | 593,156 | 656,965 |
| bounded 255, full payload / get | 1 | 708,724 | 708,035 | 772,191 |
| bounded 256, full payload / get | 1 | 709,002 | 708,313 | 772,349 |
| 4,096-byte payload / get | 32 | 24,438,917 | 24,438,228 | 24,503,658 |
| Wire record, 60 bytes / get | 32 | 2,246,783 | 2,246,094 | 2,314,961 |

This table includes request/response work and heap/stable-memory charging. It
does not isolate a faster vector engine than ICSS. For example, the u64 get's
reopen-inclusive inner interval at 256 entries changes from 100,669 to 99,980,
versus ICSS's 84,115. Storage's governed lease remains a real extra cost there.
Likewise, ICSS's larger whole-handler cost is not all collection traversal.

Selected retained-handle controls, complete-handler get instructions, uniform
single-record access and 4,096-entry context (Cell itself holds one value):

| Collection / type | Storage 0.68.8 | Retained | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Map u64/u64 | 672,583 | 672,583 | 755,590 |
| Map u64/32-byte payload | 754,963 | 754,963 | 836,752 |
| Set u64 | 668,892 | 668,892 | 751,677 |
| Vector u64 | 571,361 | 571,361 | 651,757 |
| Cell fixed 32 bytes | 493,004 | 493,004 | 573,049 |
| Map region u64/u64 | 676,018 | 676,018 | 756,876 |

Initialization saves **768 instructions** in every Storage configuration;
ICSS is unchanged. For u64, complete initialization changes from 1,213,618 to
1,212,850, versus ICSS's 936,319. These values include each allocation owner and
do not justify attributing the entire difference to vector creation.

| Matched Wasm artifact | Before total bytes | After total bytes | Delta | Before code bytes | After code bytes | Delta |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Full comparison harness | 14,373,053 | 14,350,854 | −22,199 | 12,141,113 | 12,118,858 | −22,255 |
| Storage integration fixture | 191,596 | 191,692 | +96 | 168,224 | 168,166 | −58 |

Code bytes are the Wasm section-10 payload length, parsed from each final
uncompressed module. Both sides of the harness comparison contain the new
workload; its addition is not credited as a production optimization. The
fixture uses the existing `wasm-size` recipe and is application-shaped evidence,
not a measurement of an actual downstream application. No optimizer recipe changes.

### Why the benchmark saves 22 KB

Inspection of the frozen binaries accounts for the entire reduction. Both have
7,376 defined functions. Matching demangled symbols identifies 58 smaller
functions; every other function has unchanged encoded size. The Wasm name
section is unchanged in size, so symbol stripping does not explain the saving.

| Emitted code owner | Changed functions | Code bytes saved |
| --- | ---: | ---: |
| Catalogue `matrix::by_value<V>` dispatchers containing vector creation | 38 | 14,967 |
| `TypedVector<V>::new` constructors | 10 | 3,921 |
| `StableVec<T>::create` constructors | 5 | 1,972 |
| `ReopenedVector<V>::open` | 5 | 1,395 |
| Total code-section reduction | 58 | 22,255 |

The data section grows by 56 bytes; all other section sizes are unchanged.
Thus the complete-module reduction is exactly 22,255 − 56 = **22,199 bytes**.
Function accounting includes each body's length encoding and local declarations;
the code-section function-count encoding is unchanged.

Construction supplies 20,860 bytes (93.7%) of the saving. Its 53 compiled copies
each shrink by 391–398 bytes. In the u64 constructor, the former header path
contains three `safe_write` calls and two direct governed writes; the combined
header uses one `safe_write`. This removes repeated call setup, result/error
branches and panic formatting setup. Its encoded function shrinks from 885 to
491 bytes and its explicit stack frame from 112 to 48 bytes. The u64 reopen
function shrinks from 1,046 to 767 bytes; its six governed reads become one.

The new header helpers are still inlined in this artifact: no separate
`HeaderV1::read/write` function appears. This is smaller repeated code, rather
than an observed conversion of all specializations to one shared implementation.
The broad catalogue amplifies a roughly 394-byte constructor simplification
dozens of times. Much of that code runs only at initialization, explaining the
large size saving alongside small per-request instruction improvements.

Do not project 22 KB onto ordinary consumers. The integration fixture uses only
u64 and `SessionBytes` vectors and the separate size-oriented `opt-level="z"`
recipe, whereas the comparison harness uses `opt-level=3`. Its observed result
remains **+96 total bytes / −58 code bytes**. Its symbols are stripped, so this
inspection does not attribute its individual function changes. Raw section,
function and representative instruction dumps are under
`target/evidence/storage-vector-open/size-attribution/`; no new runtime tests or
builds were needed for this binary inspection.

## Validation and reproduction

- Baseline native admission/IO: three new tests pass before the change.
- Candidate: 35 affected adapter tests; 28 benchmark library tests (including
  fixed/variable ICSS image interoperability) and 21 driver tests pass.
- Storage library, native benchmark driver and both owned/ICSS Wasm benchmark
  backends pass Clippy with warnings denied.
- Installed original Storage fixture passes collection IO/admission, upgrade
  across 1,031 allocations and decoder/update/upgrade rollback. Broad release
  and optimizer matrices are not rerun for this development change.
- Instrumented and original runs have identical responses, inner instruction
  counts and stable-page counts. Every response and final state is validated.
- Removing the dead helper after the frozen candidate run changes exactly four
  panic source-line numbers in the final benchmark data section; all executable
  bytes, addresses, section sizes and other data are identical. This is verified
  by complete binary comparison after accounting for those four locations.
  Final fixture size and installed qualification are rechecked after that cleanup.

Baseline: `target/evidence/storage-comparison/attempt.LcZri5/`; candidate:
`attempt.ClisQS/`. Both freeze source, locks, compiler identity and Wasm. Failed
first baseline `attempt.djK3tT/` and the initial probe remain preserved.
`target/evidence/storage-vector-open/` contains matched `baseline`/`candidate`
depth runs, whole-handler probes, initialization comparisons, size tables and
test logs. The successful baseline reopen probe uses `baseline-probe/buffered-report/`.
The unrelated root Wire 0.17.12 lock edit is preserved; the independent comparison
lock remains unchanged, including Wire 0.17.6 and ICSS 0.7.2.

Reproduce ordinary runs with the command above and `BENCH_WORKLOAD=depth` for
controls. Follow the benchmark README's execution-only instrumentation procedure
with `--workload vector-open` or `--workload depth` for complete-handler results.
The private benchmark initialization protocol changed; rebuild the driver and
both modules together. Production public APIs and stored bytes require no migration.
