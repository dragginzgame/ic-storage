# Explicit discard-result mutations

This study qualifies additive `insert_discard` and `remove_discard` operations on
Storage's independently owned Map/region engine. Both return whether a prior
entry existed. Returning operations retain their previous-value behavior. The
single traversal, balancing, allocation format and governance remain shared;
there is no second collection engine or format migration.

Discard skips old payload reads and decoding, not stored codec lengths, allocation
identity/class/extent, key comparison or reclamation checks. Callers retain any
required old-envelope or business-state validation. Metrics and Logging use the
operations only after their existing admission; unchanged checkpoints remain
write-free. These measurements preceded publication in Storage 0.73.6,
Metrics Storage 0.76.3 and Logging Storage 0.72.3; see the
[delivery handoff](../status/current.md) for release evidence.

## Reproduction and controls

Rust 1.99.1, current locked root/qualification graphs, Testkit 0.101.0 and PocketIC
16.1.0. Performance uses the existing `comparison` profile: opt-level 3, fat LTO,
one codegen unit, abort panics, matching 8 MiB synthetic stack, no Binaryen pass.
Small applications use the existing `wasm-size` profile. These are library consumer
recipes, not a measurement of Auth, Canic or Blob endpoints.

```sh
make benchmark-storage BENCH_WORKLOAD=discard-mutations BENCH_SAMPLES=1
cargo run --locked --offline --manifest-path qualification/Cargo.toml \
  -p ic-storage-benchmarks --example collection-io --features owned,dfinity \
  --target-dir target/runtime-qualification -- NEW_OUTPUT.jsonl --discard-only
make benchmark-storage-size
SIZE_DISCARD_MUTATIONS=1 make benchmark-storage-size
```

The fixed workload covers 256 records, scalar/full identity keys, Map/regions,
8/256/4,096/65,536-byte values, batches 1/4, uniform/hot keys and three rounds.
It verifies replacement, reads after writes, deletion, insertion into holes,
missing removal and population restoration. Both backends return presence for
discard phases; ICSS uses its returning operation followed by `is_some()`.

For a matched Storage control, build the same comparison canister with
`--features returning-mutations` instead of `owned`, retaining the exact profile
and `RUSTFLAGS='-C link-arg=-zstack-size=8388608'`. Pass that artifact to the existing
driver's `--dfinity-wasm` input with
`--reference-revision storage-returning-same-engine`. The reference columns in
that run represent **Storage returning APIs**, not ICSS. This measures the caller's
choice in the same new engine, not a historical-engine before/after comparison.

## Native IO attribution

One selected 256-record Map replacement, scalar keys. These backing observations
are not IC instruction counts; default native raw reads may split payload IO.

| Value bytes | Returning read bytes | Discard read bytes | Returning/discard old decodes |
| --- | ---: | ---: | ---: |
| 8 | 112 | 104 | 1 / 0 |
| 256 | 392 | 136 | 1 / 0 |
| 4,096 | 4,232 | 136 | 1 / 0 |
| 65,536 | 65,672 | 136 | 1 / 0 |

The 160-row native run also covers regions, insertions, removal and misses,
returning controls and ICSS. All Storage discard rows decode zero old values;
resulting values and presence agree with the model. Raw output:
`target/storage-discard-io.jsonl`.

## Installed mutation results

Both comparisons complete 2,880 observations and 480 three-round medians, with
all response and final-state checks passing. Of the 96 replacement/deletion
discard medians, all improve against Storage's returning control, by 0.245–36.160%.
The sum of the ten phase medians improves in each of the 48 matched workload
configurations. This is a per-configuration synthetic trace comparison, not a
fleet-weighted score. Other phases can move slightly with code generation:
the largest returning/control regression is 83 instructions. No old-engine
instruction regression claim follows from these same-engine controls.

Selected scalar-key Map medians, 256 records, uniform single-operation calls:

| Value bytes | Operation | Storage returning control | Storage discard | ICSS returning/discarded |
| --- | --- | ---: | ---: | ---: |
| 8 | Replace | 173,482 | 172,738 | 182,470 |
| 256 | Replace | 174,842 | 173,853 | 191,112 |
| 4,096 | Replace | 182,522 | 177,693 | 735,142 |
| 65,536 | Replace | 625,309 | 399,200 | 4,393,630 |
| 8 | Remove | 177,500 | 176,834 | 197,093 |
| 256 | Remove | 338,893 | 338,064 | 292,223 |
| 4,096 | Remove | 982,733 | 978,064 | 1,380,261 |
| 65,536 | Remove | 1,764,431 | 1,538,064 | 8,572,952 |

Storage wins 87/96 selected discard medians against ICSS and loses nine. The
losses are 256-byte cases, including full-identity replacement (263,287 versus
205,609 instructions). This API does not solve cold routing/locality or prove
Storage universally faster. At 64 KiB the selected same-engine replacement saves
36.16% and deletion 12.83%; smaller values save less because traversal and first
access charges remain. No complete Metrics, Auth or Blob endpoint gain is claimed.

Compact data: [returning control](storage-discard-returning-2026-11-12.csv) and
[ICSS comparison](storage-discard-icss-2026-11-12.csv). The compact control file
renames the driver's reference columns to `returning_*` and candidate columns
to `discard_*`; it is not a second ICSS run. Full phases and round samples remain
at the raw evidence paths below.

## Qualification and evidence

- 43 selected ordered-engine tests, nine public ordered contracts, 29 Metrics
  and 13 Logging tests pass. Discard regressions cover forbidden payload reads,
  corrupted block headers/classes/lengths, inline/external transitions, reuse,
  mixed returning/discard mutation, cold reopening and interleaved region views.
- Product native Clippy, benchmark native/Wasm Clippy and 45 driver/library tests
  pass. The fixed workload's Wire/reply budgets and model trace are tested.
- Installed Storage qualifies discard mutations after upgrade from the retained
  earlier fixture, without resetting images, plus update/upgrade rollback.
  Logging's text limits, query non-persistence, update/upgrade rollback, reopen
  and empty-history watermarks pass. Logs: `target/storage-discard-installed.log`
  and `target/storage-discard-logging-installed.log`.
- Existing returning-API size baseline: `target/evidence/storage-consumer-size/attempt.hCBXfV`;
  candidate: `attempt.bdc4dR`. Each qualifies 16 modules, 131 calls and two upgrades
  per module. Map grows 135 total/code bytes (103,224/87,744 → 103,359/87,879);
  all collections grow 198 (117,030/99,741 → 117,228/99,939). Vector/Cell and ICSS
  controls are unchanged. These default applications use the returning APIs.
- Adopting applications: `target/evidence/storage-consumer-size/attempt.ISaHX4`.
  The optional size mode makes Map/region mutations return presence on both
  libraries; Storage uses discard operations and ICSS discards returned values.
  All 16 modules pass 131 calls and two upgrades each. Map is 103,337 total /
  87,840 code bytes versus ICSS 90,673 / 49,650. All collections are 117,302 /
  99,989 versus ICSS 119,781 / 77,531. Compared with the old returning applications,
  Storage grows 113 total / 96 code bytes for Map and 272 / 248 for all collections;
  that adoption comparison changes result semantics as well as engine code.
  The unchanged returning application comparison above isolates engine growth.
- Mutation/ICSS evidence: `target/evidence/storage-comparison/attempt.CHrpkc`.
  Returning-control evidence: `target/evidence/discard-mutations-20261112/`.
  Frozen input checks pass; ICSS comparison source/artifact checks pass.

The initial sandboxed size attempt `attempt.cUKCAs` built but could not bind
PocketIC's localhost listener. Its approved retry is the baseline above. This
failure is not a runtime pass. No full release gate or new macOS result is claimed.

## Complete Metrics checkpoint calls

The maintained `metrics-checkpoints` / `metrics-checkpoints-wasm` examples now
exercise the actual Metrics Storage adapter, including its existing envelope,
numeric restoration and continuity admission. They use 1/16/256 histogram bounds,
1/64 retained rows and three rounds of changed/unchanged/rejected saves, load,
removal, missing removal and reinsertion. State assertions run outside timing.
The first four operations execute twice per IC message; report the first-call
and same-execution second-call costs separately. These are complete adapter API
calls, not whole host endpoints; setup, histogram recording and reply encoding
are outside the counters. First calls start fresh executions, but this is not a
claim that no preceding harness access warmed heap regions.

The control changes only Metrics Storage's two discard call sites back to
returning operations. Both use the same new Storage engine and locked suppliers.
The qualification lock adds existing product selections Metrics 0.29.11 and the
local adapter; no previously selected dependency changed. Production source was
restored after building the control. The snapshots, exact two-line patch, native
IO, 126 installed observations per variant, medians, input hashes and modules
are retained under `target/evidence/metrics-discard-20261112/`.
The final feature-isolated run is recorded in `final-*-installed.csv`,
`final-summary.csv` and `final-inputs.sha256`. Metrics dependencies are optional
behind the benchmark-only `metrics-checkpoints` feature; the ordinary ICSS Wasm
graph contains neither Metrics nor Storage. Both final installed variants,
native/Wasm Clippy, pin checks and frozen-input verification pass. Earlier runs
are retained; the final graph reproduces their instruction and size results.

Selected three-round medians (first operation in an execution):

| Bounds | Rows | Operation | Returning adapter | Discard adapter | Saving |
| --- | ---: | --- | ---: | ---: | ---: |
| 1 | 1 | Changed save | 179,090 | 178,258 | 0.46% |
| 16 | 1 | Changed save | 208,451 | 207,558 | 0.43% |
| 256 | 1 | Changed save | 669,432 | 667,217 | 0.33% |
| 256 | 64 | Changed save | 698,271 | 696,056 | 0.32% |
| 256 | 1 | Remove | 335,036 | 333,103 | 0.58% |
| 256 | 64 | Remove | 525,818 | 523,857 | 0.37% |

All six changed-save and six removal medians improve. Maximum-bound second-save
costs fall 509,432 → 507,217 (one row) and 538,271 → 536,056 (64 rows).
Unchanged/rejected/load controls differ by -35 instructions; missing removal
differs by +1 and insertion by -1, rather than a meaningful claimed improvement.
All state assertions pass. Native no-op/rejected/load/missing operations retain
zero writes/growth; matched mutation writes/bytes are unchanged. At 256 bounds,
discard avoids rereading a 1,310-byte encoded checkpoint in the selected round.
Backing size remains 26 Wasm pages in all installed cases.

This is a smaller benefit than the generic 64 KiB Map result: Metrics caps records
at 6,144 bytes, and validation must still decode the prior histogram. The study
uses maximum supported bound count, not maximally encoded counts in every bucket.
No claim about actual Auth/Canic costs follows from this benchmark.

Matched O3 benchmark modules shrink 241,102 → 240,366 total bytes and
191,419 → 190,837 code-section bytes. These include three histogram shapes and
fixture/model assertions; they are **benchmark harness sizes**, not application
size replacements for the earlier minimal consumers.

A separate rejected trial inserted the record magic into Wire's output buffer
instead of allocating a second buffer. All 29 Metrics tests passed, but it added
1,284 total / 1,036 code bytes, slowed selected small saves/inserts by 414
instructions and saved only about 106 on maximum-bound ones. Of 126 first-call
observations, 12 improved, 24 worsened and 90 were unchanged. Its production edit
is removed byte-for-byte; raw `prefix-*` evidence and the tested source remain
in the same target evidence directory. Do not retain that optimization.
