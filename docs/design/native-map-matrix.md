# On-demand storage comparison matrix

This compares the current generation-owned native u64 map with
`ic-stable-structures` **0.7.2** `StableBTreeMap<u64, u64>`. It uses no legacy
Storage backend or checkpoint provider. It follows the sibling comparison
convention of an explicit measurement command with retained evidence, separate
from routine validation and release gates.

## Commands

From the repository root, with the normal tools and Testkit server already prepared:

```sh
make benchmark-matrix          # 972 samples; full matched matrix
make benchmark-matrix-smoke    # 18 samples; one paired scenario
make benchmark-matrix-build    # lint/build without starting a server
```

Commands use Rust 1.100.0 and locked offline dependencies. They do not fetch or
install tools. The existing explicit preparation command is `make retention-setup`;
the [bootstrap instructions](../../README.md) own compiler and dependency
preparation. The collector refuses unknown profiles, compiler overrides and a
stable-structures selection other than the reviewed 0.7.2. Dependency graph checks
reject a legacy Storage dependency in the selected stable backend.

Each invocation prints its unique directory under
`target/evidence/native-session/attempt.*/`, including:

- `summary.md`: paired medians and native/stable ratios for every matrix cell.
- `matrix.csv`: every sample, instruction/work-unit costs and separate Wasm,
  stable-memory and native-slot observations.
- `matrix-configuration.txt`: dimensions, compiler mode, backend identity, hashes
  and installed Wasm byte sizes.
- Compiler/lint/test logs, selected dependency graph, copied sources/manifests/
  lockfiles, native layout report and raw/transformed executables.
- `MATRIX` and `COMPLETE`: success only after every expected sample passes and
  the report is written. Partial CSV rows from a failed run are not a result.

Failures stop the run; samples are not silently retried or dropped. No existing
cohort is overwritten. The Markdown report is local evidence, not an automatically
published page. `make benchmark-map` remains the older guarded-loan comparison.

## Matrix and correctness

| Dimension | Full matrix | Smoke |
| --- | --- | --- |
| First-root records | 64, 192, 1,536 | 192 |
| Sibling-root records | 0, 192 | 192 |
| Records per batch | 1, 8, 64 | 8 |
| Key schedules | Seeds 1, 2, 3 | Seed 1 |
| Backends | Native session, stable structures | Both |
| Operations | Control, lookup hit, lookup miss, replace, delete, reinsert, growing insert, churn, full scan | All nine |

Every fresh canister starts with keys `0..rows` and values `key + 1000`. Operations
run in the listed order. Existing-key batches select distinct keys using
`(17 * index + 13 * seed) % rows`. Ingest adds `rows..rows+batch`. Churn removes
the selected existing keys and adds `rows+64..rows+64+batch`; its work unit is one
remove/insert pair. Scan visits the complete first root and returns an unordered
wrapping sum of values; its work-unit divisor is the actual number of records.
Batch size is not a scan limit. The sibling root is never mutated after setup.

An independent host BTreeMap oracle checks both complete roots after setup and
every measured operation, rejects duplicate returned keys, and verifies scan
sums. Native status must retain exactly one admission throughout all ordinary
updates. Installed module hashes must match the measured artifacts. State and
meter queries must leave the stored sample unchanged. Backend order alternates
between key schedules. Setup and verification queries are outside timing.

Both backends use a 16-byte scalar request and an eight-byte response. The same
outer Wasm meter reads IC counter zero after the entire update returns, including
argument handling, collection operations and reply. It excludes only its own
final store/return and host RPC latency. Native reuses its admitted counter import;
stable receives the same import through instrumentation. Controls are reported
without subtraction; installed calibration checks work, query isolation and trap
rollback. Medians across three deterministic key schedules are not statistical
confidence intervals or wall-clock throughput measurements.

The native backend uses the qualified generation-owned Map/Pool service, including
its phase checks and scoped access. Its source-reviewed access path differs from
the per-access Wasm guards in the earlier benchmark. Stable uses held BTreeMaps
through a RefCell and default MemoryManager buckets. Both builds use release
optimization, LTO, one codegen unit and panic abort. These are complete fixture
costs; the wrappers are not identical and the results do not isolate algorithms.

## Results: 17 November 2026

Full cohort `target/evidence/native-session/attempt.gN3f9D/` passed **972 samples**
in 13.41 seconds of installed-test wall time. The smoke cohort is
`attempt.bVTURb`. Both have complete reports and success markers. Raw fixture,
stable backend and harness strict Clippy pass; report/oracle/meter tests and
the existing installed owner lifecycle also pass.

Native used fewer instructions in **142 of 144 non-control matrix cells**.
The two losses are growing single-record inserts at 1,536 rows, with either
sibling shape: the native table allocates and rehashes to cross its load boundary.
This is a workload result, not general capability or production parity.

At 1,536 first-root records and 192 sibling-root records, ratios of native to
stable whole-update median instructions are:

| Operation | One record | 64-record batch |
| --- | ---: | ---: |
| Lookup hit | 0.958 | 0.262 |
| Lookup miss | 0.802 | 0.240 |
| Replace | 0.947 | 0.217 |
| Delete | 0.596 | 0.064 |
| Reinsert | 0.793 | 0.166 |
| Growing insert | **1.446** | 0.280 |
| Churn | 0.668 | 0.132 |
| Full scan after that workload | 0.202 | 0.203 |

A 64-record lookup uses 410,849 instructions versus 1,568,519; replacement uses
493,025 versus 2,275,247. Single lookup is much closer: 402,863 versus 420,450,
with controls around 323,000. Do not turn the batch advantage into a claim of
the same improvement for individual calls.

## Scope and next extensions

Memory columns are reservations/endpoints, not allocator peaks. Native reserves
8,650,752 Wasm bytes (including its conservative 8 MiB transient layout) and no
stable memory. Stable reserves 2,097,152 Wasm bytes and 16,842,752 stable bytes
using default 128-page buckets. Native slot bytes exclude transient/controller
overhead; zero in the stable backend's slot column means not applicable.

This matrix covers keyed u64 records and unordered full traversal. Ordered ranges,
arbitrary values, larger populations, allocator peaks, upgrades and portable
recovery are not measured here. In particular, native does not yet supply the
ordered-map contract that stable structures supplies. Extend the owning fixtures
and their qualification before adding larger or structurally different cells;
do not silently extrapolate beyond the current pool and representation limits.
