# Simple structure benchmarks

This opt-in experiment compares the current M0 retained-memory implementation
with published `ic-storage` **0.74.1**. It measures installed Wasm on local
PocketIC, not native Rust timings or RPC latency. The existing `../ic-storage`
checkout is not a dependency and is not modified.

## Run

Use the repository's pinned Rust 1.100.0 with the wasm32 target. Prepare locked
dependencies and the Testkit server explicitly, as described in
[qualification setup](../README.md#run). Then, from the repository root:

```sh
make benchmark-simple-lint
make benchmark-simple
```

The second command builds both release Wasm artifacts, admits the managed
artifact and runs the fixed matrix through the already prepared server. All
Cargo commands are locked and offline; missing inputs fail rather than install.
`make benchmark-simple-build` builds without starting a server. CI compiles and
lints the benchmark code; measurements are opt-in, with no performance threshold.

## Workloads and boundaries

| Logical structure | M0 backend | Storage backend |
| --- | --- | --- |
| Scalar `u64` | Root `CounterState` field | Held `StableCell<u64>` |
| Ordered `u64` sequence | Singly linked M0 `Node` objects | Held `StableVec<u64>` |

Storage uses `MemoryRuntime`, declared keys and governed memory handles. Its
cell getter reads the heap-cached value; cell writes persist through Storage.
This measures normal held-handle use, not reopening Storage on each operation.
M0 uses the same fixture-only custody kernel as the
[managed graph probe](../rust-startup-probe/managed-graph.md), including image
admission and validation. No product API or validation check is bypassed.

Each backend starts with the same scalar and ordered values. Populations are
1, 16 and 64 elements; batches are 1 and 8; seeds are 1, 2 and 3. A fresh canister
is installed for each population/batch/seed/backend combination. Each canister
then runs these operations in order, one update per batch:

1. **Control:** perform the shared loop/arithmetic without a structure operation.
2. **Scalar read:** read the scalar repeatedly and accumulate its value.
3. **Scalar write:** replace the scalar with `10000 + iteration`.
4. **Sequence scan:** sum every element, repeatedly for a batch.
5. **Sequence replace:** replace every element with `10000 + iteration + index`.
6. **Append:** append `10000 + iteration`, preserving order.

The runner independently calculates the expected return value and compares the
entire ordered state after every update, as well as after installation. It
checks installed module hashes and alternates backend order between seeds.
A trap, malformed reply or state mismatch aborts the run; failed measurements
are never silently omitted or retried.

Both backends use Runtime 0.22.5 and the same Rust release settings: optimization
level 3, LTO, one codegen unit, panic abort, no post-link optimizer. Linker settings
select a 1 MiB stack, 129 initial 64 KiB pages and a 256-page maximum. M0 grows one
arena page during setup. Its deterministic artifact transform reserves the
retained interval; Storage uses ordinary Wasm startup and stable memory.

Timing uses the delta of `ic0::performance_counter(0)` around the complete
backend operation batch. M0 includes opening/admitting the image, entering the
session, resolving its root, each operation and final validation. Storage
includes borrowing its held state and each operation. Request parsing, setup,
replies and correctness queries are outside the interval. The common workload
uses black-boxed inputs and observable results.

The metric is **IC metered instructions**, not CPU instructions, nanoseconds or
network latency. `instructions_per_iteration` divides by batch size: for a scan
or replacement, one iteration means the **whole sequence**, not one element.
The control is reported without subtraction. First-access and write charges,
admission and batching mean a subtraction is not a pure memory-access cost.
Seed runs check different data; they are not independent wall-clock samples.

The backends intentionally expose today's implementations of the same logical
workload. Their representations and algorithms differ: M0 append walks to the
tail, whereas Storage appends to a vector. M0 admission hashes its fixed 64 KiB
metadata page and checks the indexed objects. The initial implementation repeated
this on every public operation. The current session reuses admitted accounting
for reads under exclusive custody, refreshing it after every write; full entry,
post-write and exit checks remain. Writes still repeat whole-image work.
These measurements cannot establish raw heap versus stable-memory speed, an
optimized vector comparison, or production-scale behavior. There are no map,
upgrade-cost, large-payload, memory-usage or wall-clock benchmarks yet.

## Evidence

Each run creates a fresh directory under `target/evidence/simple-benchmarks/`
with:

- `samples.csv`: all validated measurements and returned checksums.
- `configuration.json`: matrix, compiler/profile and dependency versions,
  lockfile hash, source hashes and final artifact hashes.
- `managed-raw.wasm`, `managed.wasm`, `storage.wasm` and the admission report.
- `sources/`: the relevant local source, manifests, lockfiles and build recipe.
- `COMPLETE`: written only after all **216** samples pass and the instance closes.

The prepared server is selected and admitted by the canonical Testkit launcher.
An incomplete directory is diagnostic evidence, not a successful comparison.
Run commands without concurrent source edits; the snapshots describe the local
inputs at execution time and do not assert a clean Git revision.

### Initial result, 15 November 2026

All 216 samples passed on local PocketIC 16.1.0, x86_64 Linux. For a population of
64 and batch size 1, all three seeds gave these exact instruction counts:

| Operation | M0 retained image | Storage 0.74.1 |
| --- | ---: | ---: |
| Control | 23,783,606 | 160,297 |
| Scalar read | 29,649,587 | 160,303 |
| Scalar write | 35,595,452 | 320,841 |
| Scan 64 values | 1,161,785,382 | 275,159 |
| Replace 64 values | 1,537,280,930 | 360,305 |
| Append one value | 821,103,710 | 321,387 |

The initial M0 was substantially more expensive. Its expensive control and
repeated inspection paths identify validation as the first optimization target;
they do not imply that raw heap loads are slower than stable-memory access.
Batching amortized entry cost but left per-access validation intact.
An optimization must preserve corruption refusal and session safety, with these
same correctness checks retained for the next comparison.

The complete source-snapshotted run is
`target/evidence/simple-benchmarks/1794756714632/`; its CSV exactly matched the
first run. SHA-256 bindings:

```text
samples.csv   3abf1491d609c066710762af96ac4aefb2b72f01dc83a07653c1e13c96d0bf4b
managed.wasm  456fb34427359dacb1b9a21406c98c3a763eb84f7411ddda5f0659b1ff725428
storage.wasm  0f9f82c6d5c74348597b2cea68a94f0e7c9666995cb814b6e87da738899fb5c8
```

`make ci` passed, including strict Clippy for both backends and the host runner,
dependency boundaries, native M0 tests and documentation checks. Freshly rebuilt
scalar/startup and managed-graph artifacts also passed their existing five
admission/installed tests with unchanged artifact hashes. This is local
validation; hosted CI and production subnet measurements have not been run.

### Session read reuse, 15 November 2026

The next implementation retains the admitted summary inside one exclusive
session. Reads still check handles/references; every write refreshes the summary
only after full image validation. Entry and exit validation, failed-write poison,
the stored format, the workloads and Storage backend are unchanged.

All 216 correctness-checked samples passed again. With 64 elements and batch 1:

| Operation | Initial M0 | Session read reuse | Storage 0.74.1 |
| --- | ---: | ---: | ---: |
| Control | 23,783,606 | 17,917,873 | 160,297 |
| Scalar read | 29,649,587 | 17,917,977 | 160,303 |
| Scalar write | 35,595,452 | 23,863,919 | 320,841 |
| Scan 64 values | 1,161,785,382 | 17,938,869 | 275,159 |
| Replace 64 values | 1,537,280,930 | 393,444,974 | 360,305 |
| Append one value | 821,103,710 | 35,081,194 | 321,387 |

The scan uses about 65 times fewer metered instructions than the initial M0;
Storage remains cheaper. The high control cost shows that full entry/exit
validation still dominates small reads. Per-write inspection remains costly.
This result supports further validation-cost work, not a raw memory-speed claim.
Every Storage CSV row exactly matches the baseline.

Evidence is `target/evidence/simple-benchmarks/1794757137285/`, with `COMPLETE`,
source snapshots, configuration, admission report and both artifacts. Hashes:

```text
samples.csv   037af4b7ff680d691b33830d5d694ff45c9f8d17dd1d3c6d12714138c633f5f9
managed.wasm  03efa11fc733c89031a121e04e9c0815220c2a16b20e2e03ce7b3b8858db1a8f
storage.wasm  0f9f82c6d5c74348597b2cea68a94f0e7c9666995cb814b6e87da738899fb5c8
```

Validation passed: 25 native M0 tests, four Rustdoc examples (three compile-fail),
strict product/benchmark Clippy on the appropriate native/wasm32 targets,
Rustdoc, and both managed-graph artifact/installed tests. New native cases cover
summary refresh after growth/root replacement, refusal before entering a corrupt
image, independent exit validation and denial of cached reads after write faults.
