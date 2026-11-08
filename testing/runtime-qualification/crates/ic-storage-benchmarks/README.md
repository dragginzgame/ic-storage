# Installed collection comparison

Compare IC Storage and DFINITY `ic-stable-structures` 0.7.2 using matched concrete
Rust workloads compiled into separate original Wasm canisters. This explicitly
selected, unpublished benchmark retains raw results and fails on mismatches.
It is not a release gate or a claim about complete application calls.

```sh
# Explicit preparation only when needed:
make install-runtime-server
cargo fetch --locked --manifest-path testing/runtime-qualification/Cargo.toml

make benchmark-storage                             # 256 rows, three seeds
make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=3
make benchmark-storage BENCH_ROWS=64 BENCH_SAMPLES=1 # quick complete smoke matrix
```

Rows accept 1–1,000,000, samples 1–20. Cell always has one value, regardless of the
selected population. Builds are locked/offline on Rust 1.99.1; the command never
installs tools or changes locks. Testkit 0.79.0 manages the prepared PocketIC
16.1.0 server through Platform 0.19's owned native client. Platform 0.18 removed
its stable-structures facade, so only the DFINITY backend retains the named
`ic-platform-stable-benchmark` alias for Platform 0.17. Both backends use Runtime
0.19 for system calls. Client setup and installation failures stop the comparison;
they are not retried or treated as collection results. The production graph has
no DFINITY dependency. Both Wasm backends use the same comparison profile:
optimization level 3, fat LTO, one codegen unit, abort panics, no Binaryen pass.

### Governed-backing attribution

`BENCH_REFERENCE=dfinity-governed` runs published ICSS 0.7.2 over the same admitted
Storage allocation as the owned backend. This separates much of the backing cost
from the copied collection engine. The ordinary `dfinity` reference continues
to use ICSS's own manager; its default benchmark bucket is eight Wasm pages.

```sh
BENCH_REFERENCE=dfinity-governed BENCH_CASES=U64U64,Region-U64-U64 \
  BENCH_PATTERNS=uniform BENCH_ROUNDS=3 make benchmark-storage
```

The governed control retains an Rc wrapper for region reopening and each
backend's primitive codecs. It is not an exact algorithm-only measurement.
ICSS has no bounded region API; the reference reopens its map per operation.
Bucket selection has no effect on governed backing. Existing CSV `dfinity_`
columns identify the chosen reference through configuration/revision metadata.

The direct-read path uses published Runtime >=0.19.4's
`stable64_read_uninit` export. Both selected locks provide it; no local Platform
patch or alternate lock is needed. The [backing study](../../../../docs/audits/recurring/storage-icss-backing-parity-2026-11-06.md)
distinguishes the original local-package trials from registry qualification.

### Complete type-by-collection matrix

```sh
BENCH_WORKLOAD=types make benchmark-storage
```

One catalogue owns **73 concrete types** from the published ICSS test suite and
Storage's additional comparisons. It generates every type on Map, Set, Vector,
Cell and map regions for both backends. Maps and regions cover each type as a
value with u64 keys, and as an ordered key with u64 values. They additionally run
ICSS's complete four-key/five-value grid and Storage's existing paired-type cases.
This is type-by-collection/role coverage, not every possible key/value pairing.

The catalogue includes primitives and both float widths; unit/bool; arrays;
bounded Blobs; ICSS's eight-byte `UnfixedU64` with 255/65,000/68,000 maxima;
fixed/variable/unbounded pairs and triples; all eight fixed/Blob8 triple-bound
arrangements and seven Blob8/String arrangements; optional scalars/strings/tuples;
actual Principal values; and Storage's signed scalar, Wire/session records,
chunk/bounded bytes and application tuple/optional codecs. See
[`catalogue.rs`](src/catalogue.rs) for exact types, origins, bounds and payloads.

Every case is a real generic instantiation. Composite/Principal types that do
not implement Storage's trait use a local `ReferenceValue<T>` codec bridge to
published ICSS codecs in **this unpublished benchmark only**. No production
collection or codec dependency changes. This avoids inventing another tuple
encoding while comparing the same concrete application type on both engines.
Decoded results normalize every field independently of the storage codec.

The preset owns 32 population rows (Cell one; finite key domains are capped),
shuffled Map/Set/Region insertion, uniform accesses, batches 1/8, one round and
one sample. Full-width bounded payloads join smaller controls. `payload_scope`
distinguishes whole values from variable-component/optional-inner size caps;
requested payload is not a claim about encoded bytes. Cell traces read before
and after replacement to cover both optional variants and scalar states.

The Make runner builds both original Wasm modules, then executes five automatic
collection shards with separate prepared Testkit server lifetimes, each bounded
at 30 minutes for this full-width matrix. It writes the
aggregate only after every shard completes. Ordinary defaults retain their
curated selection and runtime. No manual type/size choices are needed. Deterministic gzip reduces install
transport while retaining original modules. The synthetic modules select an
8 MiB stack identically on both backends and record the exact Rust flags: the
multi-operation handler plus a node-load frame exceeds the ordinary 1 MiB stack
for a 64 KiB inline key. This benchmark setting does not alter production stacks.
Raw module sizes and compressed install-payload sizes are reported separately.

New comparison attempts live under `target/evidence/storage-comparison/`.
Keep full CSV/JSON logs, original modules and archives there; Git retains compact
topic summaries and selected results within Shared's evidence budgets. Earlier
attempt paths remain valid historical locations and are not moved or deleted.

Compatibility is part of the output:

- Unbounded values are instantiated on Map/Region/Cell, while their Vector
  initialization refusals are checked on both backends.
- Floats and optional floats do not implement Ord, so ordered-key/Set entries are
  marked not applicable, with no invented timing.
- Native ICSS fixed optional bounds reject None during checked Map/Set/Vector/
  Region setup. Those traps are verified. Storage timings remain visible where
  its native Option codec succeeds; absent ICSS costs/ratios stay blank. Cached
  Cell uses the actual native codec on both sides.
- Unit/bool keys use their real finite populations. Inapplicable absent-key and
  out-of-domain range operations are omitted rather than fabricated.

`coverage.csv` records each case/backend status and reason; `refusals.jsonl`
retains verified runtime failures. Raw samples and complete retained-state checks
remain mandatory for every supported case. CSV tuple labels are quoted correctly.
Configuration and source/module hashes retain exact measured inputs.
The [current coverage summary](../../../../docs/audits/recurring/benchmark-full-catalogue-2026-11-05.md)
retains selected results and compatibility counts. The earlier 58-case selection
is superseded. Full per-run tables and artifacts belong in transient target
evidence, rather than tracked per-run reports.

### Iterator coverage

Use `BENCH_WORKLOAD=vector-reads make benchmark-storage` to isolate read/update
history from the full iterator mix. It fixes 256 rows, u64/fixed-32-byte/raw-chunk/
WireRecord values, uniform/hot-16 queries, batches 1/32 and three rounds of Get,
Set, Push and Pop. Chunk payloads are 31/4096 bytes; the other cases retain their
declared payload sizes. Push/Pop restores the population while Set's
updates remain visible to the next round. Results and final contents are checked
for both libraries. Additional GetAfterSet/GetAfterPush/GetAfterPop phases locate
changes within each round; each phase is a separate physical execution.
This complements the full suite; it is not an Auth endpoint
benchmark. Report per-round results as well as the aggregate because earlier
operations can change allocation history and later instruction charges.

For address/cost attribution, `benches/trace_stable_reads.pl` instruments frozen
comparison modules using Binaryen 133 folded WAT. It appends bounded Wasm globals
and functions without changing existing memory/data/global declarations. It wraps
`stable64_read` and the selected dlmalloc malloc/free functions; the latter are
counted only between the two existing measurement counters. A separate query
exports records through idle stack scratch, without committing query writes.
The driver rejects incomplete/overflowed Get traces or unexpected counter counts.
This is a diagnostic, not a general allocator profiler or production timing mode.

Prepare both modules under a target evidence directory, with no optimization passes:

```sh
# Select an existing attempt containing frozen owned.wasm and dfinity.wasm.
source_attempt=target/evidence/storage-comparison/attempt.REPLACE
trace_output=target/evidence/vector-read-trace
mkdir -p "$trace_output"
for backend in owned dfinity; do
    .tools/ic/bin/wasm-opt "$source_attempt/$backend.wasm" -S -o "$trace_output/$backend.wat"
    perl testing/runtime-qualification/crates/ic-storage-benchmarks/benches/trace_stable_reads.pl \
        "$trace_output/$backend.wat" "$trace_output/$backend-traced.wat"
    .tools/ic/bin/wasm-opt "$trace_output/$backend-traced.wat" --all-features \
        -o "$trace_output/$backend-traced.wasm"
    gzip -n -c "$trace_output/$backend-traced.wasm" > "$trace_output/$backend-traced.wasm.gz"
done
```

Through the same explicitly admitted Testkit server runner, invoke the native
driver with those gzip modules, `--workload vector-reads --samples 1
--trace-stable-reads` and an explicit `--upstream-bucket-pages`. Repeat at 1/8/128.
`samples.jsonl` records `[destination, source, length, instructions]` per read;
`heap` records measurement-counter calls, malloc calls/cost and free calls/cost.
Read records include setup/cleanup reads outside the timer. Costs include probe
overhead and are not additive to ordinary results. Keep the normal run as the
headline comparison. First qualify an uninstrumented WAT round trip against the
original module's instructions, results and final state; verify existing layout
declarations and retain module/driver hashes. Installed IC validation still applies.

For the full benchmark handler, run the same injector on the **original** WAT
with `--execution-only` as its third argument. Assemble/install those modules
with the same commands and use driver `--trace-execution-cost` instead of the
read-trace flag. It measures request decoding, input preparation, the collection
loop, result conversion, response encoding/reply and cleanup. It excludes setup
population and host execution setup, and includes boundary probe overhead.
The ordinary inner-loop columns remain; `execution-summary.csv` and
`execution-round-summary.csv` report the outer scope separately, with raw
`execution_instructions` in each sample. The two diagnostic modes cannot be mixed.
Both modes also admit `--workload depth`, covering Map, Set, Vector, Cell and
regions at 256/4,096 records. Full-handler counters retain all batches/operations
and aggregate across populations. Depth read traces capture only batch-one Get
calls: multiple tree traversals can exceed the bounded 160-read trace. Other
ordinary samples remain present with null read traces; overflow still fails closed.
The existing Vector read-history diagnostic retains both batch sizes. This large
synthetic reply is not a measurement of Auth's endpoint. Keep frozen ordinary
modules as the reference, verify a plain WAT round trip, then compare boundary
probe inner counts against ordinary counts before interpreting the outer scope.
Boundary-only probes also admit `--workload ordered` to check larger scans and
string/fixed-byte key controls, or custom `--cases` selections with the same
rows, rounds and patterns as their ordinary runs. Bounded read probes remain
restricted as above.

For deeper attribution, apply `benches/trace_allocator_accesses.pl` to the
**read-instrumented** WAT, then assemble it identically. Add driver
`--trace-allocator-accesses` alongside `--trace-stable-reads`. It instruments the
six selected dlmalloc functions and records up to 64 loads/stores costing at
least 10,000 instructions, including accesses before/after the inner timer.
Records are `[address, phase * 16 + kind, instructions, site]`, where phases
0/1/2 mean before/during/after the loop and kinds 0–5 mean i32 load/store,
u8 load/store and i64 load/store. The adjacent generated `.sites.tsv` identifies
original functions, operations and offsets. Unknown memory operations or function
rosters refuse instrumentation. These are costly-access observations, not a
complete memory profile; neither these costs nor probe timings are throughput.

Run the fixed, bounded iterator preset without choosing window parameters:

```sh
BENCH_WORKLOAD=iterators make benchmark-storage
```

It owns 256 population rows, seven Vector types, eleven batch/scan lengths
(`1, 2, 31, 32, 63, 64, 65, 127, 128, 129, 256`), and three stateful rounds.
Make defaults this heavier preset to one sample; ordinary comparisons retain
three, and explicit `BENCH_SAMPLES` still overrides either. Types cover u64,
i64, u128, f64, fixed 32-byte arrays, Wire records and variable bytes. Variable
values use logical lengths 0/32/256. The three byte lengths give nine case/payload
combinations: 1,287 aggregate rows and 7,722 timed backend calls per sample.

All Vector selections, including the ordinary matrix, now cover thirteen
operations. Existing Get/Set/Push/Pop and forward/reverse scans are joined by:

| Operation | Timed behavior |
| --- | --- |
| `Nth` / `NthBack` | Fresh iterator, direct skip to each generated forward/reverse index |
| `Last` | Fresh iterator's `last()` for each query |
| `AlternatingScan` | One iterator, alternating front/back until the requested limit |
| `ScanSet` | One iterator, replace each next slot before reading it, leaving the first slot untouched |

Interleaved replacement inputs are built before timing, as for ordinary Set;
collection encoding and writes remain timed. Results are checked against an
independent native model, then stored contents are verified after all rounds.
Each batch gets a fresh collection; rounds share its mutation history. Scan
length, query count and population are recorded separately. Setup/final checking,
RPC codec work and result canonicalization remain outside collection timing.

The preset bounds reply volume. Use the existing selection for wide-value
controls at the ordinary 1/32-item limits:

```sh
BENCH_CASES=VectorBytes BENCH_PAYLOADS=4096 BENCH_ROUNDS=3 \
  BENCH_PATTERNS=uniform make benchmark-storage BENCH_ROWS=256 BENCH_SAMPLES=1
```

Crossing all 256 items with 4 KiB values exhausted the 900-second server budget
in qualification; its partial evidence remains a failed run, not a pass. Wide
fallback behavior is covered separately instead of inflating every iterator run.
Correctness refusals, corrupt slots, empty collections and poisoned ownership
remain library tests; successful-operation timing does not qualify those paths.
The new private operation schema requires rebuilding both Wasm backends and the
driver together, which the Make command does. Older frozen results retain their
original binaries and cannot be relabeled as this expanded matrix.

### Ordered collection coverage

```sh
BENCH_WORKLOAD=ordered make benchmark-storage
```

This preset uses one sample, three stateful rounds and shuffled population of
256 entries. Seven Map/Set cases cover integer, fixed-byte and string keys;
variable Map values use 31/32/33/256 bytes. These sizes retain coverage of the
previous engine's inline threshold; the copied engine has ICSS geometry.
Batch/scan lengths are `1, 5, 6, 11, 12, 31, 32, 64, 128`. The ten case/payload
combinations produce 1,962 aggregate rows and 11,772 measured backend calls.

The normal Map matrix also includes direct `Nth`/`NthBack`, `AlternatingScan`,
`ValuesScan`, `ReverseKeysScan` and `BoundedKeysScan`. Value-only responses use
ordinal ranks as labels, so preparing keys does not warm the measured traversal.
Bounded key scans use the same lower-inclusive, upper-exclusive interval as
bounded pair scans. Key-only paths call each library's public key iterator;
values are not fetched merely to normalize results. Skipping uses each public
iterator API, including any traversal needed by that implementation.

`IterLast`, `KeysLast` and `ValuesLast` repeatedly call a fresh iterator's
`last()` for Map entries, keys and values. Set also measures `IterLast`.
These are distinct from Map's existing `Last` operation, which calls
`last_key_value()` directly. The native model checks every result, empty
collections and unchanged state. Rebuild both backends and the driver together
for this expanded private operation schema.

Paired Set measurements include forward scans and `Nth`. Published ICSS's Set
iterator and the pinned Git control are forward-only; reverse/mixed-end Set
traversals are not approximated by collecting or repeatedly rescanning keys.
Storage's bidirectional Set iterator remains covered by its native tests.

The private request envelope admits Map/Set batches through 128, Vector through
256 and other structures through 32. Inserted and missing-removal IDs use
adjacent disjoint ranges derived from the batch length; a former fixed offset
of 64 overlapped at larger batches and was caught by model admission. The new
regression checks every admitted Map/Set/Region batch size. Historical traces
retain their original identities; these binaries establish a new baseline.

All operations still check results, length and complete persisted state against
an independent model. This preset studies one population size and one shuffled
insertion history per cell, not production-scale trees or every cache policy.

### Session lookup coverage

```sh
BENCH_WORKLOAD=auth-lookups make benchmark-storage
```

This instrumented preset isolates the outer stored record used by Canic's Auth
adapter: a Wire byte-vector envelope with `CSW\x01` framing and a variable
4,096-byte Storable bound. Inner bytes are synthetic opaque data, not valid Auth
sessions. It fully populates 1/512/1,024/2,048 records with 256/2,048-byte inner
payloads, then reads first/middle/last indices in separate query and update
executions. Each cell has one round and defaults to one sample, yielding 480
timed backend calls and 240 paired rows. Explicit samples still scale the run.

The five phases are Control, Get, WarmGet, Decode and MissAbove. Get includes the
collection's IO, validation, allocation and outer Wire decoding. WarmGet performs
one untimed get in the same execution before measuring another. Decode measures
the outer decoder over bytes prepared before timing, without collection IO.
Preparation can already warm memory; Get is not a guaranteed cold whole endpoint.
Query dispatch accepts only these read operations for this case.

WarmGet is an untimed pre-read followed by a timed read, not a guarantee that
all memory charges are already paid. Stable-read dispatch can defer a native
mapping charge until the second read. In the checksummed baseline's 1,024-row
last-record case, Get costs 25,613 instructions while WarmGet costs 105,613.
Do not treat their difference as an ordinary cache benefit or instruction-only
decoder cost. See the [confirmed source-page diagnosis](../../../../docs/audits/recurring/vector-stable-read-charges-2026-11-05.md).

`lookup-stages.csv` records total instructions, nested outer-decoder instructions,
decoder calls and decoder input bytes. Decoder cost is included in the total;
do not add it again. The driver checks counts/bytes, every returned value and the
entire final population. Aggregated reports carry `execution_mode`; per-population
and mode subdirectories retain their full configurations and raw samples.

The library's native regression separately asserts bounded backing reads, exact
requested byte volume, one decode per successful get and one length read with no decode on a miss,
including reopened vectors. Native IO counters are not IC page charges. Neither
test supplies malloc counts or a page-fault oracle. Canic configuration projection,
caller-to-index lookup and Auth's inner `Session::decode()` remain outside scope.
See the [qualified results](../../../../docs/audits/recurring/icss-engine-baseline-2026-11-05.md).

The original results above use the historical checksummed Vector. The following
ICVEC002 experiment is also superseded. The current copied ICSS SVC1 baseline
reads the durable eight-byte length and, for a successful lookup at this bound,
two length bytes plus the payload. New runs retain their own binary/source
identities. Neither historical format's measurements describe the current engine.

### Stable-read alignment diagnostic

The independent examples below reproduce actual source addresses from saved
original Vector Wasm. They fully populate each vector, capture its stable image,
upgrade to a passive read-only probe, then time prefix/payload/repeated reads
and a combined-read control. The raw IO is confined to this diagnostic fixture;
production integrations continue to use governed leases.

```sh
cargo build --locked --offline --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --example stable-read-probe-wasm --features owned \
  --target wasm32-unknown-unknown --profile comparison --target-dir target/storage-comparison-build/owned
cargo build --locked --offline --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --example stable-read-probe --target-dir target/runtime-qualification
probe_cli=$(bash scripts/dev/install-rust-tools.sh --consumer "$PWD" \
  --package ic-testkit --version 0.79.0 --bin ic-testkit-server \
  --profile release --features cli --check)
"$probe_cli" run --directory .tools/ic-testkit-server --ttl 900 -- \
target/runtime-qualification/debug/examples/stable-read-probe \
  --baseline-wasm target/storage-comparison/attempt.c6YVkt/owned.wasm \
  --candidate-wasm target/storage-comparison/attempt.xIWOoI/owned.wasm \
  --probe-wasm target/storage-comparison-build/owned/wasm32-unknown-unknown/comparison/examples/stable_read_probe_wasm.wasm \
  --output target/vector-read-probe-report
```

The output directory must be new. Frozen input Wasm and source archives remain
part of the earlier studies; this command neither reconstructs nor relabels them.
The selected matrix covers 512/1,024/2,048 rows, both payload sizes, first and
last indices with neighbours, and compact-layout index 2,015 with its neighbours.
It checks all returned bytes against the captured image. All heap destinations
are filled before timing; no source stable pages are read before the measured
sequence. Each sequence is a fresh query execution. The CSV's region counts
are physical range intersections, not independently observed fault counters.
The raw system-call experiment excludes Vector traversal, decoding and normal
host state. Its result is not a whole-Get or whole-endpoint benchmark.

### Extended controls

The independent workspace also pins upstream Git revision
`b5d1650463f477affb3d358e86b1113953fc9937` as an explicitly optional research
control. `BENCH_REFERENCE=dfinity-head` selects it; the default `dfinity` control
remains published 0.7.2. The Git control does not enter production qualification
graphs. Prepare its locked source with the existing explicit fetch command.

```sh
BENCH_REFERENCE=dfinity-head BENCH_CASES=U64U64,VectorU64 \
BENCH_PATTERNS=uniform,hot16,zipf BENCH_SHUFFLED=1 BENCH_ROUNDS=3 \
BENCH_UPSTREAM_CACHE_SLOTS=32 \
make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=1
```

Case selection uses the exact case names from `types.rs`. Storage's copied
0.7.2 engine has no node cache. Positive `BENCH_CACHE_BYTES` now refuses, so an
inert ceiling cannot be mislabeled as an active cache. Published ICSS 0.7.2 also
has no cache; only the optional Git reference supports upstream slot counts.
`BENCH_COLD_CACHE=1` affects that Git reference's ordinary maps; it adds no
Storage cache or cold-reopen measurement. Historical cached-engine runs remain
valid only for their recorded source and binaries.

`BENCH_UPSTREAM_BUCKET_PAGES=1..65535` selects the ICSS MemoryManager bucket size
for every structure; the comparison default is eight pages (512 KiB); stock ICSS defaults to 128 (8 MiB). One Wasm page is 64 KiB.
This changes only the DFINITY backend's backing allocation policy, not its node
size, Storage's growth policy or cache settings. Each fresh comparison canister
receives the selection during initialization. Existing managers cannot be resized
by reopening with a different setting. Smaller buckets reduce reservation slack,
but consume the fixed 32,768-bucket manager limit sooner and can change IO costs.

```sh
for pages in 1 8 128; do
    BENCH_UPSTREAM_BUCKET_PAGES=$pages BENCH_CASES=U64U64 \
    BENCH_PATTERNS=uniform,hot16,zipf BENCH_SHUFFLED=1 BENCH_ROUNDS=3 \
    make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=1
done
```

Compare runs made with the same sources, Wasm files and workload settings.
`configuration.json` and the appended `upstream_bucket_pages` CSV column record
the choice. Setup/growth during population remains outside instruction timing;
reported stable pages include that reservation. This unpublished initialization
protocol changed, so rebuild both Wasm backends and the driver together.

Shuffling uses a deterministic Fisher–Yates permutation for Map, Set and Region
population; Vector/Cell setup retains its semantics. Zipf uses a discrete
harmonic CDF with exponent one, generated outside timing on both backends.
The opt-in `sequential` pattern starts at `seed % rows`, visits adjacent IDs,
and wraps at the population boundary. Missing/insertion keys remain outside
the population. This exercises locality without changing operation order.
Trace generation can warm heap regions. `BENCH_ROUNDS=1..20` repeats the complete
operation sequence on existing state with different access seeds. Raw samples
retain the round; summaries take the upper median across samples and rounds.
Configuration records every selection. Larger supported populations are opt-in
and must be qualified separately; use selected cases to bound runtime and memory.

### Focused payloads and repeated growth

`BENCH_PAYLOADS=33,44,45,80,108,109,132` selects explicit logical value sizes for
selected cases. Every selected case must admit every supplied size; invalid
combinations refuse before contacting the server. Omitting the variable preserves
the ordinary payload matrix. It cannot be combined with `BENCH_WORKLOAD`.

`BENCH_GROWTH_CYCLES=1` requires `BENCH_CASES=U64Bytes` and replaces the normal
operation sequence with SmallReplace → SmallGet → Grow → LargeGet. Each round
uses `sample + 1` as its seed, so the same existing keys repeatedly change size;
no empty-value, removal or reinsertion phase intervenes. For 33/80-byte bases,
Grow writes 132/320 bytes. Count 32 may revisit hot keys within one call. Normal
runs retain their changing per-round seeds and complete operation sequences.
Raw records retain both the phase label and actual request operation; summaries
use the phase label to distinguish small and large Get. Upper medians over all
rounds can hide first-growth costs; use `round-summary.csv` or raw per-sample
trace totals when comparing recurring work.

```sh
BENCH_CASES=U64Bytes BENCH_PAYLOADS=33,80 BENCH_PATTERNS=uniform,hot16 \
BENCH_SHUFFLED=1 BENCH_ROUNDS=12 BENCH_GROWTH_CYCLES=1 \
make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=2
# Optional: select the Git reference and BENCH_UPSTREAM_CACHE_SLOTS to study its cache.
```

These controls change only the research workload. They do not change production
allocation policy. A comparison with published ICSS 0.7.2 does not give both
engines equal cache budgets; that release has no selectable node cache.

## Consumer workloads

### Vector opening

`BENCH_WORKLOAD=vector-open BENCH_SAMPLES=1 make benchmark-storage` measures a
fresh vector view for each request while retaining its allocation owner. Storage
acquires/releases a governed lease; ICSS clones its virtual-memory handle.
The interval includes opening, operation preparation/result conversion and
closing. `OpenControl` performs only the surrounding request machinery.
Existing workloads retain their vector handles.

Six vector types cover fixed scalars/arrays, the 255/256-byte length-prefix
boundary, larger byte payloads and Wire records. Populations are 256/4,096,
batches 1/32, uniform/hot-16 access and three mutation/read rounds. Each
population subdirectory also records `initialization.jsonl`: the complete
initialization body before population, including allocator setup, vector creation
and destruction. It is not an isolated header-write measurement.

The whole-handler `--trace-execution-cost` probe supports this workload; the
bounded stable-read probe refuses it because opening and nested timers are
outside that tracer's scope. Compare identical logical responses between
instrumented and original modules. See the
[opening study](../../../../docs/audits/recurring/storage-vector-open-2026-11-08.md)
for matched Storage/ICSS results and measurement limits.

### Population depth and IO attribution

`BENCH_WORKLOAD=depth make benchmark-storage` compares 256 and 4,096 populated
records across Map (u64 and 32-byte variable values), Set, Vector, fixed Cell
and u64 map regions. Cell has one value at either population. The preset uses
shuffled population, uniform/hot-16 access, batches 1/32 and three stateful rounds;
the default three samples use independent seeds and alternate backend order.
All ordinary operations remain, including insertion/removal/reinsertion and
variable-map growth/shrink. It supplements the 73-type catalogue rather than
repeating its shallow population. Per-population reports and an aggregate matrix
retain every phase, instruction median and stable-memory capacity. Aggregate CSV
and raw JSONL include `population_context_rows` separately from actual collection
rows, so the singleton Cell observations remain distinguishable.

```sh
for pages in 1 8 128; do
    make benchmark-storage BENCH_WORKLOAD=depth BENCH_UPSTREAM_BUCKET_PAGES=$pages
done
# Repeated growth of the same keys, without the intervening removal phases:
BENCH_CASES=U64Bytes BENCH_PAYLOADS=33,80 BENCH_PATTERNS=uniform,hot16 \
BENCH_SHUFFLED=1 BENCH_ROUNDS=6 BENCH_GROWTH_CYCLES=1 \
make benchmark-storage BENCH_ROWS=4096 BENCH_SAMPLES=3
```

For attribution, the native `collection-io` example compares Storage with
published ICSS 0.7.2 both on governed Storage memory and on its MemoryManager.
Identical counted codecs, insertion histories and requests isolate duplicated IO
and encoding/decoding work for Map/region Get, Replace and variable-value growth.
It checks every returned value and all final records against a separate model.

```sh
cargo run --locked --offline --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --example collection-io --features owned,dfinity \
  --target-dir target/runtime-qualification -- target/evidence/collection-io.jsonl
```

The output must be new and its parent must exist. This diagnostic records
physical read/write calls and bytes, codec calls, growth/capacity and distinct
4/64 KiB stable byte ranges. It does not measure native time, heap allocations,
heap fault sets or IC page charges. Zero-length IO and the stable-access helper
make range counts insufficient to price instructions. Use installed comparisons
for timings; instrumentation remains outside production and those timed modules.
The governed ICSS stack is an isolated research control, not a second production
collection integration or public allocation bypass.

The [local memory inventory](../../../../docs/local-memory-audit.md#complete-declaration-inventory-and-repeatable-workloads)
selects three consumer recipes plus a size-boundary experiment. Use a name instead of repeating a collection of
flags; these are benchmark selections, not new Storage runtime parameters.

| `BENCH_WORKLOAD` | Population | Values / structures | Access |
| --- | --- | --- | --- |
| `control` | 16 records; Cell has one | 256 B / 4 KiB, Map and Cell | Single, uniform, three rounds |
| `records` | 4,096 records | Bytes at 31/32/33/256/1,024/4,096 B; fixed 32-byte Map, 64-byte Set, WireRecord Vector | Batches 1/32, uniform/hot16/Zipf, shuffled Map/Set, three rounds |
| `bulk` | Eight records; Cell has one | 64 KiB / 1 MiB, Map/Vector/Cell/Region | Single, uniform, one round |
| `boundaries` | 64 records; Cell has one | 31/32/33, 235/236/237, 4,075/4,076/4,077 and 65,515/65,516/65,517 B; Map/Vector/Cell/Region | Single, sequential, shuffled population, three rounds |

```sh
make benchmark-storage BENCH_WORKLOAD=records BENCH_SAMPLES=1 BENCH_UPSTREAM_BUCKET_PAGES=8
# Same recipe and binaries, varying only the ICSS bucket control:
for pages in 1 8 128; do
    make benchmark-storage BENCH_WORKLOAD=bulk BENCH_SAMPLES=1 BENCH_UPSTREAM_BUCKET_PAGES=$pages
done
```

The boundary recipe probes adjacent payload sizes around candidate inline and
allocation-size transitions; these are not claims that both engines share layout
thresholds. It includes growth, shrink, removal and reinsertion where supported.
Use `BENCH_WORKLOAD=boundaries BENCH_SAMPLES=1 make benchmark-storage`.
For Set locality, use `BENCH_CASES=SetU64,SetString,SetI64,SetBytes64` with
`BENCH_PATTERNS=sequential BENCH_SHUFFLED=1 BENCH_ROUNDS=3`.

A recipe owns population (so `BENCH_ROWS` is unused), cases, payload sizes,
batches, patterns, ordering, rounds and disabled caches. Combining it with the
other detailed workload environment knobs refuses. Samples and ICSS bucket size
remain explicit experimental controls. Configuration records the recipe and all
resolved settings; original manual selections remain available without a recipe.

Large byte values are opt-in; the ordinary matrix retains its previous payload
sizes. Setup and final verification target at most 512 KiB per call, except for
one indivisible bulk value. Protocol tests qualify initial and grown values under
bounded Wire limits and the IC's 2 MiB reply ceiling, including framing. The
measured bulk call contains one operation; `Grow` takes 64 KiB to 256 KiB and
1 MiB to 1.5 MiB, while `Shrink` writes an empty payload and returns the old value.
Vector's bounded chunk fixture retains its 1 MiB bound and does not select Grow.
Encoding and setup remain outside the measured interval. Earlier retained bucket
results used same-size bulk Grow operations; those rows do not measure growth
and cannot be compared with the corrected trace as an implementation speedup.

These are synthetic collection traces informed by actual declarations. They do
not reproduce a complete canister, concurrently open allocations, actual live
populations or the 2–32 MiB upper bounds of some control snapshots. The inventory
explains which consumers and bounds are covered; no production default follows
from a successful smoke run.

## Supported structures and concrete types

| Structure | Types | Logical value bytes |
| --- | --- | --- |
| Map | u64 → bytes | 0, 31, 32, 33, 1024, 4096 |
| Map | String → String | 0, 32, 1024, 4096 |
| Map | u64 → u64/u128; u128 → u128 | 8; 16 |
| Map | Signed64 → Signed64 | 8 |
| Map | [u8;32] → [u8;32]; [u8;64] → u128 | 32; 16 |
| Map | u64 → [u8;31/32/33/256]; u64 → WireRecord | 31, 32, 33, 256; 60 |
| Set | u64; String; Signed64; [u8;64] | unit values |
| Vector | u64; Signed64; u128; f64; [u8;32]; WireRecord; ChunkBytes | 8; 8; 16; 8; 32; 60; 0/31/32/33/1024/4096 |
| Vector | SessionEnvelope (variable 4,096-byte bound) | 256, 2048 inner bytes |
| Cell | bytes; WireRecord; u128; [u8;32] | 0, 31, 32, 33, 1024, 4096; 60; 16; 32 |
| Map region | u64 → bytes | 0, 31, 32, 33, 1024, 4096 |

All are real generic instantiations. `WireRecord` has an account ID, u128 balance,
generation and 32-byte tag, using the same IC Wire codec on both backends. Its
60 field bytes are not its encoded size; the declared encoding ceiling is 256
bytes. Fixed array keys and zero-padded string keys preserve logical ID order.
String values use two-byte UTF-8 characters. Scalars exercise high bits.
There are 30 concrete cases and 54 type/size combinations in the ordinary matrix.
`VectorBytes` is included by default: both backends declare a variable-length
1 MiB maximum (`ChunkBytes`), matching Canic’s chunk vector. Both current Vector engines reserve
that maximum even for shorter values. The 31/32/33-byte
fixed and variable Map values preserve the earlier threshold controls; fixed
256-byte values and 4 KiB variable values extend the payload comparison. Both
current engines use the ICSS node layout.

`Signed64` wraps i64 because upstream has no primitive i64 Storable implementation.
Both backends use the same eight-byte little-endian benchmark codec. Signed keys
map logical ID 32 to zero, so populations above 32 cross the sign boundary. The
u128 keys vary their upper 64 bits, and 64-byte keys share a 56-byte prefix before
the ordered ID. Float vectors preserve exact IEEE bits, including signed zeros,
subnormals, infinities and a NaN payload; floats are never ordering keys.

By default each case uses uniform or hot-16 hit keys, one or 32 operations per execution, and
seeds 1 through the requested sample count. Backend order alternates by sample.
Default population is ascending, in batches of 128. Each complete case/sample/
backend gets fresh state; operations within it share the preceding operation's
committed state. This is a trace, not an independent reset per operation.

- Map: control, hit, miss, replacement, forward scan, insertion,
  membership, reverse scan, key-only scan, growth, shrink, removal, missing removal,
  reinsertion, inserted-key cleanup, first/last entry lookups and bounded forward ranges.
  Growth/shrink apply only to variable byte/string values.
- Set: control, membership hit/miss, forward iteration, insertion, removal,
  missing removal, reinsertion and inserted-key cleanup. DFINITY's Set iterator is not double-ended;
  reverse Set iteration remains native correctness coverage, not a fake comparison.
- Vector: control, indexed hit/miss, indexed set, forward/reverse iteration,
  push and pop. Pushes extend the current length; pops remove those appended slots.
- Cell: control, cached get and replacement; byte cells also grow/shrink. Gets
  clone the cached value on both sides to retain a result, so they include cloning.
- Region: control, hit/miss, replacement, bounded key snapshot, insertion,
  growth/shrink, removal, missing removal, reinsertion and inserted-key cleanup.

Growth replaces values with four times the initial size (minimum 128 bytes,
capped at 1.5 MiB to leave reply-framing space);
shrink uses empty bytes/strings. Growth/shrink run while selected original keys
still exist; removal follows, then reinsertion restores them. `RemoveInserted`
cleans up the insertion range so later rounds start with the original population.
`RemoveMiss` uses a separate never-inserted range. `MissAbove` probes beyond the
population; these misses and insertion/cleanup keys are sequential regardless of
the hit-pattern label. Below-population and interior misses are not represented.
Phase-entry assertions admit existence/absence and previous grow/shrink values
before either backend executes. Duplicate hot keys remain intentional: later
removals/reinsertions within a batch can be misses/replacements. This is still a
stateful mixed trace, not an independent reset per operation.
Map forward/reverse scans seek from the generated key; Map key-only scans and
Set/Vector iteration start at their respective ends. Region key snapshots seek
from the generated key and enforce the limit within the operation. Bounded Map
scans use `[start, start + ceil(count/2))` and also enforce the requested count;
this exercises an exclusive upper bound independently of the output limit.
First/last lookups run `count` times and retain every result. These three new
operations run after the existing mutation trace, so they observe its final values.

Call counts follow the operations and payload sizes recorded in each run's
configuration, multiplied by batches, patterns, samples, rounds and backends.
Controls are included. The extra cleanup phase changes counts from older runs.

## Comparison boundaries

Storage uses admitted governed leases. DFINITY uses its stock MemoryManager
and memory ID 0. This measures their configured stacks, not tree algorithms over
identical allocators. Storage now copies ICSS 0.7.2 engines privately, including
its node/page layout, rather than maintaining a different B-tree. Storage's
application codecs, checked backing, explicit load admission and owned MapEntry
API remain different. Its primitive unsigned codecs use little-endian bytes;
ICSS defaults use big-endian bytes. Shared Wire/byte codecs match. Native
same-image tests explicitly adapt codecs rather than assuming every default
encoding matches. No collection checksums or custom node cache remain.

DFINITY has no `BTreeMapRegion`: that control reloads a BTreeMap from the same
VirtualMemory for each operation. Storage uses its actual region API, which also
loads the retained root per operation. Region key results are bounded owned
snapshots on both sides. This is a documented wrapper comparison, not a claim
that upstream supplies the same governance or region bounds.

`performance_counter(0)` surrounds collection operations, their IO, codecs,
allocation, loops and result collection. Decoding the request, generating typed
inputs, reserving output capacity, normalizing results, verification and encoding
the reply are outside timing. Returned values stay live through the endpoint.
Preparation can warm heap regions before the counter. Each measured call is a
separate IC update, or a query in the explicit `auth-lookups` preset; batches
share that execution's tracking state. Controls are
reported without automatic subtraction.

The native ordered model checks every result and length, including displaced
values, booleans, indexed writes and append/pop behavior. Final bounded queries
compare all remaining contents. Raw samples are flushed before validation; a
mismatch or trap fails the run, and no successful summary is written. Typed
normalization happens after timing; Wire records normalize fields without their
storage codec. Physical stable pages include metadata and reservation, not just
payload bytes. The benchmark protocol admits eight million traversal nodes and
256 MiB of conservative allocation allowance so bounded responses including a single 1 MiB bulk value fit;
this allowance is not an actual allocation. Request/reply encoding is outside timing.
Native regression tests round-trip the largest verification and grown-value
responses under the same limits. Setup cost is excluded, but its allocation history remains.

This matrix does not measure cold reopening for ordinary collections, setup,
upgrades, rollback, GiB maps, retained heap or
complete application endpoints. Native B-tree tests additionally cover shuffled
insert/delete histories; installed lifecycle qualification is a separate suite.

## Retained results

Each `target/storage-comparison/attempt.*/` contains:

- `report/round-summary.csv`: per-round medians across samples, with zero-based
  `round`. Use this to separate the initial population from later mutated states.
  Cell Shrink leaves an empty value for the next round's Get; aggregate Get medians
  can therefore describe empty cached reads even when the initial payload is large.
- `report/summary.csv`: per-case upper-median instructions and
  `storage_over_dfinity` (below 1 means fewer Storage instructions), with explicit
  structure/type/payload/operation/batch/access-pattern labels. The two
  page columns report each backend's median physical stable pages after the operation.
  Each page is 64 KiB; these are whole-canister high-water sizes, including manager
  metadata and reserved capacity, not live payload bytes or allocation deltas.
- `report/samples.jsonl`: requests, counts, full results, lengths and stable pages.
- `report/configuration.json`: case operations, actual population and measurement
  scope; both Wasm files, source archive/hashes, compiler identities, both locked
  graphs, pending diff and build/server logs accompany the report.

New cases, endpoint/range operations and result handling change the harness from
the earlier map-only and 14-case experiments. Older results are historical evidence, not a controlled before/after
speedup for the library. Compare the two backends within a matched current run;
retain per-case regressions rather than averaging them into one score.

The phase-correctness repair also changes work ordering and cache history. Earlier
`RemoveMiss` rows actually removed the just-inserted keys; variable-value `Grow`
reinserted removed keys, and `Reinsert` replaced values restored by growth. Their
timings remain valid for those historical mixed traces, not for the old labels.
Use corrected baseline/candidate runs to attribute new implementation gains.

## Shared-backing capacity and IO

Pass `--small-records` after the output path in the command below to select the
focused Storage density study instead: four governed maps with 1,024 rows each,
payloads 33/44/45/80/108/109/235/236/237 bytes, six half-delete/reinsert rounds,
and full returned/final-value checks. It reports actual capacity and backing IO,
including metadata, without predicting a particular allocation minimum. This
selection changes the benchmark workload, not production allocation policy.

Alternatively, `--allocation-histories` selects nine Storage-only histories over
four governed 1,024-record maps: fixed 33/80-byte values, 33↔132 and 80↔320
alternation, one-time growth, 10%/50% growth, staircase sizes through 4,096 bytes,
and transitions across the previous inline threshold. Each history has 24 phases, full content checks
after cold reopening every fourth phase, complete deletion and small refill.
Mutation returns are checked against an independent map. Initial population uses
a deterministic permutation; final refill is sorted, so its capacity difference
is not by itself a fragmentation diagnosis. Diagnostic and verification reads
are excluded from subsequent mutation IO. `--allocation-histories` and
`--small-records` are mutually exclusive; neither reports native execution time.

The native `multi-allocations` example uses one backing per backend, 32 and 64
real governed allocations/ICSS virtual memories, 256-byte metadata records and
64 KiB records in every eighth store. It initializes all stores, interleaves
population and growth (32 then 96 rows per store), then performs four rounds of
deleting half the keys and reinserting in reverse order. Displaced and final
values are checked. Diagnostic reads are excluded from phase IO counters.
Six subsequent replacement rounds rotate values through 31/32/33,
236/237, 4076/4077 and 65516/65517 bytes. These retain historical allocation-boundary controls without changing the live key population. Every displaced
value and the final contents are checked; each phase reports its actual live
payload rather than reusing the original fixed-size estimate. The original
same-size phases remain directly comparable with retained runs.

```sh
cargo run --locked --offline --manifest-path testing/runtime-qualification/Cargo.toml \
    -p ic-storage-benchmarks --example multi-allocations --features owned,dfinity \
    --target-dir target/runtime-qualification -- target/multi-allocations.json
```

The output records logical payload, per-allocation capacity, physical pages,
Storage metadata/extents and backing IO per phase. ICSS's one metadata page and
bucket reservation must be distinguished from its logical virtual pages.
Counters include setup and mutations within each labelled phase, not diagnostics.
This is native memory accounting, not an IC instruction or fault-charge model.
It preserves the production custody boundary for Storage, with ICSS confined to
the independent research graph. Native helper dependencies are explicitly optional.

The native helper also retains a historical geometry probe with predictions
for the retired power-of-two allocator and its 20-byte record prefix. Those
predictions do not describe the copied ICSS allocator and cannot qualify current
layout efficiency. Actual capacity/IO fields remain observations; use the matched
current workload for present comparisons.

The completed matched study is recorded in
[the memory audit](../../../../docs/local-memory-audit.md#completed-bucket-and-shared-backing-study).
Eight pages is the tuned research default, not a change to ICSS or Storage.
The fixed 32,768-bucket ICSS limit gives 2 GiB at one page, 16 GiB at eight and
256 GiB at 128. Always name 128 explicitly when claiming a stock-default
comparison. All nine qualification runs passed an explicit bucket argument;
the subsequent default change does not reinterpret their configurations.
