# Native map versus ic-stable-structures

This installed comparison measures the current guarded native-map fixture
against `StableBTreeMap<u64, u64>` from **ic-stable-structures 0.7.2**. It uses
neither the legacy Storage backend nor the transitional checkpoint provider.
The older M0/Storage benchmark remains historical evidence and is not part of
this comparison.

The latest [scoped-loan and batch results](#scoped-loan-and-batch-reuse-17-november-2026)
substantially improve ordinary batches. The initial results below remain as the
baseline; neither cohort claims that native admission across messages is solved.

Run `make benchmark-map` from the repository root with the existing prepared
Testkit server. `make benchmark-map-build` performs strict linting and builds
the stable backend without starting a server. All validation/build commands
are locked and offline. The exact stable-structures dependency is selected only
by the qualification workspace's `stable-map` feature; product dependencies
and the raw native-map executable are unchanged.

## Workload and measurement boundary

- Populations: 64, 192 and 1,536 records; batches: 1, 8 and 64; three key schedules.
  Both backends have a populated first map and an empty second map.
- Initial keys are `0..rows`, with value `key + 1000`. Each fresh instance runs
  control, successful lookup, replacement, deletion, reinsertion and growing
  insertion in that order. Existing-key operations use distinct keys selected
  by `(17 * iteration + 13 * seed) % rows`. Growing insertion adds `rows..rows+batch`.
- An independent host BTreeMap oracle checks every record and the empty second
  root after setup and after every measured update. Installed module hashes are
  checked. Backend order alternates between schedules. Failures stop the run;
  no sample is silently dropped or retried.
- The native backend uses the existing admitted Rust executable, access guards,
  two-root admission and settlement. Its pool is expanded to 192 KiB during
  unmeasured setup. Rehashing during measured insertion is included.
- In the initial cohort, native lookup/replace/delete/reinsert commands reopen
  both roots for **each operation**, including inside a multi-operation update.
  The latest cohort uses one admitted scope for the complete batch of each kind.
  Existing range insertion also executes its complete batch in **one native scope**. The stable
  backend borrows its held maps once per update and performs the whole batch.
  The baseline difference was part of that executable interface, not a claim that
  per-record reopening is necessary for the intended native architecture.
- Stable maps use two MemoryManager regions with the library's default bucket
  size. Platform Runtime supplied the fixture's raw IC bindings at 0.23.16 in the
  baseline and 0.23.17 in the latest cohort. Its Wasm
  graph contains the benchmark, Runtime, stable-structures and ic_principal;
  the collector rejects a selected graph containing legacy Storage.
- Both Rust artifacts use 1.100.0, release optimization, LTO, one codegen unit and
  panic abort. The native workload wrapper is WAT with a fixed scenario; the
  stable wrapper is Rust with a small argument parser. Their controls are
  reported without subtraction. This measures current implementation costs,
  not isolated table algorithms or raw heap versus stable-memory access.

The same outer Wasm transformation records IC performance counter zero after
each original update returns, including request handling, guards, operations and
reply. The final meter store/return and host RPC latency are excluded. A separate
query retrieves the sample; repeated state/measurement queries must leave it
unchanged. Installed calibration verifies nonzero empty-call cost, additional
loop cost and counter rollback after a trapped update. Reinstrumentation refuses.

## Initial results, 17 November 2026

`target/evidence/native-map-provider/attempt.KsgxpY` contains all **324 validated
samples**, `MAP_COMPARISON` and `COMPLETE`. Five artifact checks and the installed
matrix pass; the installed test takes 7.67 seconds on the local PocketIC runner.
Strict Wasm/backend and host-harness Clippy pass.

Below are whole-update IC instructions at 1,536 initial records. Each cell is
the median of the three deterministic key schedules, not a wall-clock statistical
estimate. Ratios divide native instructions by stable-structures instructions.

| Operation | Batch | Native | Stable structures | Native / stable |
| --- | ---: | ---: | ---: | ---: |
| Lookup | 1 | 7,793,948 | 419,527 | 18.58× |
| Replace | 1 | 7,874,723 | 509,237 | 15.46× |
| Delete | 1 | 7,883,266 | 810,279 | 9.73× |
| Reinsert | 1 | 7,875,673 | 608,455 | 12.94× |
| Growing insert | 1 | 21,933,336 | 674,663 | 32.51× |
| Lookup | 64 | 468,586,540 | 1,567,659 | 298.91× |
| Replace | 64 | 468,796,140 | 2,273,171 | 206.23× |
| Delete | 64 | 450,589,944 | 8,348,435 | 53.97× |
| Reinsert | 64 | 450,487,260 | 3,141,215 | 143.41× |
| Growing insert | 64 | 22,250,030 | 3,551,680 | 6.26× |

The table shows a large current disadvantage, especially when the fixture repeats
admission for each operation. Even the single-scope insertion batch loses at
1,536 records. The boundary matters: at 64 initial records, inserting 64 records
costs 2,070,757 native instructions versus 2,988,036 for stable structures, a ratio
of 0.69×; at 192 records it costs 3,372,536 versus 3,053,685, or 1.10×. One small
winning case is not a general throughput claim.

Growing insertion crosses a native table capacity boundary for both batches
shown at 1,536 records; it includes allocating and rehashing a 4,096-slot candidate.
Existing-key single lookups rise from 717,648 native instructions at 64 records
to 7,793,948 at 1,536, while stable lookups remain near 412,000–420,000. This is
consistent with the full-table admission path dominating the native operation,
but this matrix does not separately attribute time to admission, guards and
table logic.

## Scoped-loan and batch reuse, 17 November 2026

The provider now validates root geometry and allocation bounds once when it
issues an exclusive loan. Per-access guards enforce that loan; settlement
verifies the unchanged provider metadata and revokes it. No native access can
write provider metadata or extend the admitted bounds. Full table validation
remains at native owner reconstruction. The fixture's batch interface now holds
both admitted map owners and their pool through 1–64 lookups, replacements,
deletions or insert-only operations before releasing and publishing them once.

Two successive cohorts isolate these changes:

- `attempt.0NQ03L` qualifies loan-guard reuse while retaining per-operation owner
  reconstruction. A single lookup at 1,536 records drops from 7,793,948 to
  1,698,486 instructions; growing insertion of 64 records drops from 22,250,030
  to 4,462,344. The new admission-only endpoint costs 1,698,067 instructions at
  that population, identifying the remaining fixed cost of reopening both owners.
- `attempt.TnPFUR` additionally holds the owners across each batch. All 324
  comparison samples and 54 admission-only samples pass full-state checks.
  Four batch modes pass late acknowledgement, wrong-key and metadata-write
  failures, invalid mode/count refusal, full rollback and subsequent clean retry.
  `BATCH_CUSTODY`, `MAP_COMPARISON` and `COMPLETE` are present. Five artifact
  checks pass; the installed comparison takes 7.08 seconds.

For 64-operation updates at 1,536 initial records, medians across the same three
key schedules are:

| Operation | Initial native | Current native | Stable structures | Current native / stable |
| --- | ---: | ---: | ---: | ---: |
| Lookup | 468,586,540 | 1,748,696 | 1,567,659 | 1.115× |
| Replace | 468,796,140 | 1,928,472 | 2,273,171 | 0.848× |
| Delete | 450,589,944 | 2,190,428 | 8,348,435 | 0.262× |
| Reinsert | 450,487,260 | 1,986,070 | 3,141,215 | 0.632× |
| Growing insert | 22,250,030 | 4,462,344 | 3,551,680 | 1.256× |

Every stable-structures CSV row is identical to the baseline despite the
separately selected Runtime 0.23.17 / Testkit 0.108.1 lock updates. The stable
backend was rebuilt against the current lock before this comparison; its new
artifact hash is preserved. No dependency update is credited for the native
improvement. The initial `attempt.RzHDx1` stopped at missing dependency sources
before measurement and has no completion marker; explicit locked preparation
preceded the successful offline builds.

Single-operation lookup still costs 1,698,756 instructions versus 419,527 for
stable structures (4.05×), and admission alone still costs 1,698,067. Holding an
owner for an entire request fixes repeated admission within that request; it
does not establish reusable trusted ownership across ordinary IC messages.
That needs its own runtime custody/lifecycle contract, not an unchecked public
constructor or a flag that skips table validation. Existing unknown/corrupt-data
refusal remains intact. Batches are homogeneous operation groups, not a new
application transaction API or a claim of ordered-map equivalence.

The changed executable also passes standalone upgrade/growth/corruption checks
in `attempt.jCOnL0` (7.97 seconds) and all eleven paged lifecycle/recovery checks
in `attempt.7Y46uE` (32.02 seconds). Recovery still selects insert-only batch mode.
The older bounded recovery-job profile also passes all four selected checks in
`attempt.MtlilZ` (11.65 seconds), with `COMPLETE`.
The raw compiler shape now has nine defined functions, including factored
removal, while retaining the proved 2,688-byte stack bound and complete memory
access rewriting. Retained formats and product Map APIs are unchanged.

## Evidence and limits

Each attempt preserves compiler identity, source snapshots, manifests/locks,
selected stable dependency graph, raw native artifact, raw stable artifact,
all composed/metered artifacts, configuration, logs and `map-comparison.csv`.
`MAP_COMPARISON` is written only after all 324 state-checked samples and server
instance cleanup succeed. `COMPLETE` additionally requires the artifact checks.
The completion markers do not certify a clean Git revision or a production subnet.

The CSV's `stable_bytes` is allocated stable backing only. The stable fixture
reserves 16,842,752 bytes for two default manager buckets plus metadata; native
reports zero because its data lives in Wasm memory. This is **not** a total-memory
efficiency comparison. Native pool slack, Rust heaps, stacks, checkpoint copies,
physical allocator overhead and rehash peaks need their own measurements.

The comparison cohort does not measure upgrades, portable recovery, large application
records, ordered ranges, random misses, adversarial keys or production latency.
It does not establish a replacement for stable structures. The next performance
work is reusable admitted ownership between ordinary messages, with explicit
invalidation/revalidation at incompatible or externally changed state boundaries.
Preserve corruption refusal, ownership, publication and rollback guarantees while
changing that boundary. R1's installed memory
and lifecycle gates remain required before increasing the supported size.
