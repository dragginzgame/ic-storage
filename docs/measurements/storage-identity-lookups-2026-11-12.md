# Full identity keys and cold Map lookup — 2026-11-12

## Decision and scope

Auth's indexed authentication has a reproducible cold-access problem. The pending
node-first allocation experiment does **not** solve it: the matched installed
100k-record Auth measurement is 1,223,890 instructions before and 1,224,146 after.
Both runs finish same-release upgrade checks, then fail Auth's existing <1M
ordinary-authentication budget. This is a measured failure, not a qualified fix.

The complete [198-row comparison](storage-identity-lookups-2026-11-12.csv) has
19 improvements, 151 unchanged medians and 28 regressions. All 18 initial cold-hit
medians are unchanged. Node-first helps some small churn cases but worsens two
100k/1KiB above-range misses by about 6.9%. ICSS reference medians and both
backends' reserved capacities are unchanged between the two runs.

Preserve the pre-existing pending placement edit for maintainer review; do not
promote it as the Auth fix. The review also corrected its node reservation:
mark an allocated node before allocating subordinate key/value blocks, so a
corrupt same-class free-list cycle cannot return that reserved node again.
Two new tests cover that refusal and valid same-class reuse. No public API or
persisted-format change, migration, Auth edit or publication is included.

The [subsequent batch-placement study](#small-allocation-batches--rejected)
rejects both fixed 4 KiB and population-scaled 4–64 KiB refills. Synthetic cold
hits can improve while mutations and the second same-execution read regress.
Neither solves Auth at 100k. Both trial implementations and their exclusive
tests are removed; the earlier pending changes described above remain.

## Reproducible workload

```sh
BENCH_WORKLOAD=identity-lookups BENCH_SAMPLES=1 \
  BENCH_UPSTREAM_BUCKET_PAGES=128 make benchmark-storage
```

The profile selects:

- Full 29-byte Principals, 31-byte tagged/length-prefixed Principal keys, and
  62-byte two-Principal/device-slot keys with a common 32-byte prefix.
- 1,000, 10,000 and 100,000 records, deterministic shuffled population, seed 1,
  one operation per update and three rounds with different sampled keys.
- 256-byte and 1KiB byte values. These bracket small records; they are not a
  claim about the exact serialized size of Auth's session records.
- Cold hit, repeated hit, above-range miss, replacement, removal, removed-key
  miss, reinsertion, hit after churn, insertion and removal of the inserted key,
  plus the surrounding control. The model verifies complete returned contents
  and final population independently for both backends.

This gives 1,188 ordinary observations per revision, summarized as 198 medians.
There are only three key paths per case: this is bounded churn and sampled
scaling, not broad lifetime histories, p99 or worst-case qualification. The
benchmark excludes population, request preparation and response encoding from
the timed collection interval. Every phase is a separate update execution.
`RepeatedGet` makes two full pre-reads in the same execution before timing a
third lookup; it is a separate control, not an Auth cache or a promise that
every heap/mapping cost is warm. No identity is shortened or hashed by the test.

The two new shared benchmark codecs also enter the 75-type catalogue. Existing
native catalogue/model tests and both comparison Wasm builds pass; the entire
75-type installed suite was not rerun for this focused investigation.

## Cold and repeated lookups

Instructions below are candidate medians for a 31-byte tagged key and a
256-byte value. Initial cold-hit medians are identical with the old allocation
order. Columns describe distinct executions/scopes; do not subtract Auth's
whole method from these synthetic collection operations.

| Records | Storage cold hit | ICSS cold hit | Storage repeated hit | ICSS repeated hit | Storage cold miss above | ICSS cold miss above |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 345,968 | 345,301 | 25,760 | 28,850 | 506,724 | 270,462 |
| 10,000 | 673,611 | 689,663 | 36,176 | 49,663 | 593,995 | 930,511 |
| 100,000 | 1,003,857 | 694,340 | 43,857 | 54,340 | 1,086,808 | 1,105,357 |

At 100k, every sampled tagged-key cold hit exceeds its repeated hit by exactly
960,000 instructions. The costly-read diagnostic finds twelve roughly 80k events
on each path, versus eight in these ICSS paths. This is strong evidence for a
cold working-set problem; it does not independently assign every event to the
stable mapping tracker versus destination heap tracking.

An additional frozen-module control uses inline `u64` keys, the same 100k
population/shuffle, seed-1 selected identity 65,933 and 256-byte values. Its
first cold Get costs 185,143 Storage / 432,192 ICSS instructions, compared with
1,006,229 / 688,033 for that tagged-Principal path. This is one path, not the
three-round medians above. Changing the key also changes decoding and allocation
geometry, so it supports the overflow/locality diagnosis without isolating the
inline threshold alone. It does not recommend shortening application identities.

Longer values change the ranking. At 100k:

| Key | Value bytes | Storage cold hit | ICSS cold hit | Storage pages | ICSS pages |
| --- | ---: | ---: | ---: | ---: | ---: |
| Principal, 29 bytes | 256 | 1,002,061 | 693,082 | 1,098 | 641 |
| Tagged Principal, 31 bytes | 256 | 1,003,857 | 694,340 | 1,098 | 641 |
| Composite, 62 bytes | 256 | 1,017,019 | 869,484 | 1,194 | 641 |
| Principal, 29 bytes | 1,024 | 1,162,780 | 1,426,628 | 3,442 | 1,921 |
| Tagged Principal, 31 bytes | 1,024 | 1,164,420 | 1,429,644 | 3,442 | 1,921 |
| Composite, 62 bytes | 1,024 | 1,177,582 | 1,448,205 | 3,538 | 1,921 |

Pages are physical stable-memory 64KiB pages, including backing capacity and
controls, not logical payload bytes. For tagged keys, 256-byte values reserve
68.625MiB in Storage versus 40.0625MiB in ICSS; 1KiB values reserve 215.125MiB
versus 120.0625MiB. Storage's power-of-two blob classes include a 16-byte header:
a 256-byte value needs a 512-byte block, a 1KiB value a 2KiB block. That explains
part of the density problem, separately from cold-read costs. The ICSS control
uses its 128-page manager bucket; Storage retains its own eight-page units.

Mutation controls for tagged keys/256-byte values at 100k are 1,091,581/1,047,062
Storage/ICSS instructions for replacement, 2,310,815/2,110,159 for insertion and
2,300,426/2,722,243 for removal. Node reservation normally adds 241 instructions
to an insertion when the changed placement does not alter access charges. See
the complete CSV for other populations, churn stages and regressions; no blended
score weights these unlike operations together.

## IO attribution and implementation

The owned engine's [node reader](../../crates/ic-storage/src/ordered_map/storage.rs)
reads 104 routing bytes from a 128-byte node block. It excludes the inline-value
tail. For a longer key, `blob` separately admits a 16-byte allocation header and
reads the complete key bytes. A successful lookup then reads the selected value;
large values are **not** fetched at every traversed node. Full typed equality,
ordering, bounds, child metadata and allocation custody checks remain active.

For the three tagged-key/256-byte/100k cold hits, the instrumented candidate
reports 50–53 stable read calls and 2,688–2,839 bytes. ICSS reports 99–115 calls
and 1,331–1,416 bytes. Storage reads fewer times but encounters more costly
events. Most costly Storage calls here are 16-byte blob-header reads costing
80,236 instructions; one path instead has an expensive 31-byte payload read.
Which call incurs a charge is not by itself proof of which tracker caused it.

These are separate diagnostic modules, built with `benches/trace_stable_reads.pl`
in its `--batch-reads` mode. Totals count all stable reads/bytes in the call; saved addresses
include only reads costing at least 10,000 instructions, up to 160 records.
They are not a complete address trace. Heap counters cover the timed loop.
Probe overhead is excluded from the ordinary headline tables. Both old and new
modules first pass a plain WAT round-trip comparison with identical ordinary
instructions/results. Identity diagnostics retain cold hits, removed-key misses
and hits after churn; other phases retain ordinary timings but no read trace.
All 324 matched old/new traced operations have identical total read calls and
bytes; their addresses and which calls pay access charges can differ.

Node-first places a new node before its key and value. It neither packs nodes
across records nor repacks existing allocations, and reuse may separate them.
The same logical read calls remain. A 31-byte key still requires 151 routing/key
bytes per compared node; a 62-byte key requires 182, excluding the final value.

## Actual Auth regression

Read-only consumer references: `ic-auth/crates/ic-auth/src/persistent/records.rs`
defines full keys and point-record access; `persistent/receiver/mod.rs::authenticate`
observes time, reads the caller record and checks the selected session/policy.
`observe` can update the metadata Cell. The qualification canister's
`profile_receiver` prepares context before timing authentication, then measures
a second read/decode of the same caller record through `SessionStore::read`.
Both reach `read_caller`, but this does not make the second read free of deferred
access charges. Those measurements are not an isolated cold Map/read-codec split.

| Retained records | Old order authenticate | Node-first authenticate | Old point read/decode | Node-first point read/decode |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 291,914 | 291,914 | 33,487 | 33,487 |
| 1,000 | 646,595 | 646,595 | 67,200 | 67,200 |
| 10,000 | 984,068 | 983,991 | 84,710 | 84,678 |
| 100,000 | 1,223,890 | 1,224,146 | 84,890 | 84,773 |

Both use Auth's existing
`persistence::ordinary_persistent_authentication_is_a_point_operation_at_scale`
test at 100,000 records, with the same native runner, unchanged Auth source/locks
and newly built canisters. The previously reported 1,223,140 is historical;
the matched old/new builds above are the allocation-order comparison. Both
failure logs retain the final budget assertion rather than loosening it.

Builds use Auth's manifest read-only, `--locked --offline`, its release profile
and `--cfg=cpubits="32"`, with CLI Cargo `paths` selecting local Storage and all
outputs in Storage's `target/auth-132-build`. Cargo warns that this override
differs in the tooling dev-dependency; retain that warning and use these only as
local qualification, not publication graph proof. Auth's existing runner SHA256 is
`b66b2233fed34a70eb408b9d9854137b4ec720d7ebf148287f50d4d553b3a189`.
No Auth files, locks, policy or deployments were changed.

## Wasm and checks

Matched uncompressed original application probes; numbers are total/code-section
bytes, not gzip sizes or the much larger benchmark dispatcher:

| Consumer | Old allocation order | Node-first with reservation fix | ICSS |
| --- | ---: | ---: | ---: |
| Map | 103,213 / 87,757 | 103,200 / 87,744 | 90,687 / 49,664 |
| All collections | 117,011 / 99,754 | 116,998 / 99,741 | 119,816 / 77,566 |
| Actual Auth qualification canister | 2,100,473 / 1,610,618 | 2,100,459 / 1,610,605 | Not an ICSS Auth implementation |

For completeness, the all-types comparison harness itself is 9,681,529 /
8,515,554 bytes before, 9,681,482 / 8,515,507 after, and 11,610,809 / 9,959,795
for ICSS. Its type dispatch, response machinery and generated instantiations
are qualification code; these totals are not application-size claims.

Forty engine tests, eight public ordered contracts and native/Wasm Storage
Clippy pass. Benchmark native Clippy and 56 library/driver tests pass; four
existing ignored tests were not run. Both size runs qualify all 16 modules,
each with 131 calls and two upgrades. Candidate installed Storage-only IO,
corruption, 1,031-allocation upgrade and decoder/update/upgrade rollback pass.
No full release gate, macOS run or publication is claimed.

## Next experiment and format boundary

The bounded small-class batch experiment below is complete and rejected. Do not
retry another batch size without better attribution. First measure the first,
second and third reads separately in one execution: `RepeatedGet` currently
times the third, whereas Auth exposes a costly second read. Keep ordinary timing
and instrumented access traces separate.

A format-preserving candidate is packing node/short-key pairs across records
while leaving large values elsewhere. This must preserve adjacency during
refill and account for mismatched class demand, reuse and churn. It is not
implemented or selected. A smaller alternative is bounded stack scratch for
short external-key decoding, preserving header admission before payload reads.
That may remove temporary allocations for identity codecs, but borrowed-input
codecs may copy again and it does not eliminate stable mapping charges. The
earlier rejected Cell stack-buffer trial is not evidence this will win.

Combining a blob header and payload read might remove a syscall while retaining
the same access charge. A speculative node-plus-key read also needs care:
adjacency is unknown until decoding the node, and the contiguous span crosses
the inline-value tail. Do not weaken key-only reads, corruption checks or lifetime
admission to manufacture a fast path.

Increasing the inline-key limit changes bytes 88–104, the adjacent value tail,
pointer-zero admission and potentially node size. It requires a distinct format
proposal and explicit migration/reset decision for affected consumers; Auth's
request does not authorize silently reusing STORMAP1 or retiring their records.
The current format and public APIs remain unchanged throughout this study.

## Small allocation batches — rejected

Both variants start from the node-first/reservation-fix baseline above. Only
32-, 64- and 128-byte allocation classes are batched; larger payloads keep
individual allocation. Unused slots use the existing FREE headers and class
heads. Prefixes are initialized before publishing the frontier/head. A shorter
existing capacity tail is consumed before growing, so optional batching never
refuses a requested block that fits. No new format, field, API, hint or cache
is introduced. Existing live objects are not moved.

The first variant reserves at most 4 KiB per refill. The second chooses a
population-scaled power-of-two slot count, bounded between 4 KiB and 64 KiB of
blocks, then caps that count by the existing tail. Rounding is bounded before
arithmetic, including at `u64::MAX`. These are allocation policies, not IC
charging units. The independent engine author implemented both from its own
format and black-box observations, without upstream implementation inputs.

### Actual Auth screen

Instructions from the same installed regression and frozen native runner:

| Records | Baseline authenticate | 4 KiB authenticate | Ramp authenticate | Baseline second read | 4 KiB second read | Ramp second read |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 291,914 | 291,914 | 291,914 | 33,487 | 33,487 | 33,487 |
| 1,000 | 646,595 | 726,595 | 726,595 | 67,200 | 67,200 | 227,200 |
| 10,000 | 983,841 | 983,617 | 823,623 | 84,580 | 164,416 | 644,697 |
| 100,000 | 1,224,106 | 1,304,126 | 1,223,928 | 84,773 | 324,753 | 964,890 |

Auth's native test was independently updated between studies: its guard is now
an interim 1.3M, and it additionally reports post-upgrade costs. Freeze that
runner (SHA256 `ea98e1bb36e549d4f34ca8397257b1135fa566ef50f846893e86d5057422a0aa`)
and replay the unchanged baseline Wasm for this table. Auth production/canister
sources and locks are unchanged between these builds; this investigation edits
no Auth files. Baseline and ramp pass the interim guard; fixed 4 KiB fails it.
**All three still miss the requested <1M authentication target.** Differences
of hundreds of instructions are not evidence of a meaningful placement win.

After upgrade at 100k, baseline/ramp authenticate both cost 1,223,273 and their
second reads cost 83,971/963,971. Fixed 4 KiB costs 1,303,273/323,971. This
confirms that same-release upgrades do not remove the observed problem.

Authentication and the second read use the same caller/index entry, not two
different maps. Separate node/key runs can defer materialization charges from
the first to a subsequent read; the approximately 80k increments are consistent
with that explanation. This is a working hypothesis, not a new per-access
tracker proof. Adding both timed intervals at 100k gives 1,308,879 baseline
versus 2,188,818 ramp; a cheaper first read alone is insufficient.

### Storage/ICSS comparison and mutation controls

The [198-row ramp comparison](storage-identity-batches-2026-11-12.csv) contains
81 improved, 53 equal and 64 regressed medians. ICSS reference medians and
reported stable capacities are unchanged throughout. Counts are not a weighted
application score. Selected 100k cold-hit medians:

| Full key / value bytes | Storage baseline | Storage ramp | ICSS |
| --- | ---: | ---: | ---: |
| 29 / 256 | 1,002,061 | 844,323 | 693,082 |
| 31 / 256 | 1,003,857 | 846,229 | 694,340 |
| 62 / 256 | 1,017,019 | 1,096,970 | 869,484 |
| 31 / 1,024 | 1,164,420 | 846,841 | 1,429,644 |
| 62 / 1,024 | 1,177,582 | 1,177,631 | 1,448,205 |

All 18 third-read medians are unchanged. At 100k with a 31-byte key/256-byte
value, insertion rises from 2,310,815 to 2,950,884 instructions (ICSS 2,110,159),
and reinsertion from 2,223,826 to 3,107,246 (ICSS 1,205,802). At 10k the cold
hit improves 673,611 → 516,240 while insertion regresses 1,332,857 → 1,812,926.
The changed tradeoff is present outside Auth too.

The independent `depth` profile covers Map, Set, Vector, Cell and regions at
256/4,096 rows: 27 of 768 medians improve, 588 are equal and 153 regress.
Vector/Cell are unchanged. Scalar-key byte-value shrink has a selected
173,485 → 333,485 regression; simple scalar Map/Set/region changes are mostly
small allocation overhead. Four refill controls at 16/31/32/64 rows use scalar,
31-byte and 62-byte keys with 256-byte values, shuffled uniform requests and
1/32-operation batches. Of 696 medians, two improve, 572 are equal and 122
regress. A 64-record composite-key single insertion costs 195,039 → 715,573
(ICSS 210,748). Neither control changes reference medians or reported capacity.

Capacity here is allocated stable pages, including governance and bucket slack;
it does not show that spare slots or refill writes are free. Population setup
is untimed, so this study makes no setup-cost improvement claim. No broad
identity matrix was run for the already-rejected fixed-4-KiB variant.

### Size, safety and retained qualification

Matched original application Wasm, total/code-section bytes:

| Consumer | Baseline | Fixed 4 KiB | Ramp | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Map | 103,200 / 87,744 | 103,660 / 88,100 | 103,753 / 88,168 | 90,687 / 49,664 |
| All collections | 116,998 / 99,741 | 117,450 / 100,097 | 117,543 / 100,165 | 119,816 / 77,566 |
| Auth fixture | 2,100,459 / 1,610,605 | 2,104,105 / 1,611,561 | 2,104,147 / 1,611,622 | Not an ICSS Auth implementation |

Both size runs qualify all 16 modules with 131 calls and two upgrades each.
Fixed/ramp engines pass 45/47 native tests, eight public contracts and native/
Wasm Clippy. Trial coverage checks dirty capacity, class reuse, short tails,
failed-prefix publication, corrupt free records and bounded population rounding.
Those trial-only tests are removed with the rejected allocator code.

Keep the qualification runner's new `--storage-install-wasm` option, restricted
to `--storage-only`. It installs/populates with an earlier compatible fixture,
then upgrades to the selected current fixture and checks successful mutation
and trapped-write rollback on the existing image. The baseline-image → fixed-
4-KiB run passes, including 1,031 named allocations, collection IO, framing
refusals and decoder/update/upgrade rollback. This proves the harness path and
that trial's compatibility; it is not a ramp upgrade proof or a new release
qualification. Runner Clippy and five native tests pass; two existing installed
tests remain ignored in the native invocation.

The original pre-trial engine/test files are restored byte-for-byte, preserving
node-first allocation, its reservation correction and the earlier removal
optimization. Both workspace lockfiles are unchanged by this experiment. No
new batching implementation or public configuration remains.

## Evidence and rerun

- Old/new ordinary matrices: `target/evidence/storage-comparison/attempt.l4cPQR`
  and `attempt.jkKNoi`; each retains sources, locks, modules, compiler settings,
  raw samples, per-round summaries and passing final input/source checks.
- Old/new application sizes: `target/evidence/storage-consumer-size/attempt.2NCFal`
  and `attempt.Z482WW`.
- Diagnostic round trips, read traces, Auth builds/logs/state, exact source/lock
  hashes and focused checks: `target/evidence/identity-map-20261112/`.
  `inline-control/` contains the additional frozen-module scalar-key control,
  run with `--cases Map-U64-Bytes --rows 100000 --payloads 256 --patterns uniform
  --shuffled --samples 1 --rounds 1 --upstream-bucket-pages 128`.
- Both revisions retain the earlier accepted removal-admission optimization.
  The only engine comparison is key/value/node versus node/key/value allocation,
  with immediate NODE reservation in the latter. Existing pending dependency
  work is held fixed; the Cell and Vector trials are absent from both builds.
- Batch follow-up baseline/ramp: `attempt.jkKNoi` / `attempt.vZJfQA` under
  `target/evidence/storage-comparison/`. Size baseline/fixed/ramp:
  `attempt.Z482WW` / `attempt.GqNaTo` / `attempt.3vH1m7` under
  `target/evidence/storage-consumer-size/`. Source/artifact checks pass.
- `target/evidence/routing-batches-20261112/` retains frozen Auth modules/runner,
  logs, rejected source, restoration inputs and `baseline-*`/`ramp-*` depth and
  refill reports. Depth uses `--workload depth --samples 1`; refill uses
  `--rows N --cases Map-U64-Bytes,Map-IdentityKey-Bytes,Map-CompositeIdentityKey-Bytes
  --payloads 256 --patterns uniform --shuffled --samples 1 --rounds 1`.
  Both use bucket 128 and the same frozen driver/reference modules. The initial
  refill invocation rejected the wrong `--payload-sizes` spelling before
  measurements; successful `*.retry.log` runs use `--payloads`.

To repeat the current candidate, run the workload command above and
`make benchmark-storage-size` with the prepared tools/caches. Read the benchmark
README's diagnostic recipe for frozen-module probes. Auth qualification uses
its manifest plus `--config 'paths=["/home/adam/projects/ic-storage/crates/ic-storage"]'`
and a Storage-owned `--target-dir`; set `POCKET_IC_BIN`,
`IC_AUTH_QUALIFICATION_WASM` and `IC_AUTH_QUALIFICATION_STATE_ROOT` when invoking
the exact existing regression. Keep Auth source and both lockfiles unchanged.
Raw target evidence is transient; this summary, CSV and maintained workload
survive an explicitly requested cargo clean.

## Short external-key scratch — rejected

After publishing Storage 0.73.6, a fresh matched experiment tested a bounded
128-byte stack buffer for external-key decoding. Inline keys were unchanged;
longer keys retained the existing owned buffer. Both external paths shared the
same block identity, class, extent and length admission before payload IO.
The independent engine implementer retained its restricted inputs; this was
not an import of an ICSS implementation. No format or public API changed.

The experiment did not improve the selected lookup workload. It added a copy
for codecs that consume `Cow` into an owned `Vec`, including the inspected Auth
key codec in `ic-auth/crates/ic-auth/src/persistent/records.rs`. Even the value-type
Principal control became slightly slower. The trial is rejected; its exclusive
production path and tests are removed, with source retained in the evidence.

The maintained identity workload now has twelve phases: `SecondGet` performs
one untimed pre-read, while `RepeatedGet` still performs two before timing the
third read. Each phase starts a separate execution and selects the same key
for all its reads. Neither result includes pre-read cost or represents a complete
Auth endpoint. This is a benchmark protocol change: use the new driver and
modules together, not old modules with the new operation selector.

Fresh controls use Rust 1.99.1, locked Wire 0.21.5, Testkit 0.101.0 and PocketIC
16.1.0. The pre-existing qualification-lock correction to Metrics Storage 0.76.3
is held equal. There are three rounds, 29/31/62-byte keys, 256/1,024-byte values,
1k/10k/100k records and ICSS buckets of 128 pages. The comparison retains the
same population, churn, ordinary update boundaries and independent result/model
checks. Earlier tables have different source/graph/protocol inputs; use this
fresh baseline for attribution.

The [complete 216-row comparison](storage-key-scratch-2026-11-12.csv) has
198 slower non-control medians, 18 unchanged controls and no improvements.
Each revision has 1,296 observations; final-state/source/input checks pass.
All ICSS medians and both engines' reserved capacities match between revisions;
the ICSS module is byte-identical. Selected 100k-record, 256-byte-value results:

| Key / operation | Storage baseline | Scratch trial | ICSS |
| --- | ---: | ---: | ---: |
| Principal / cold get | 1,002,076 | 1,002,441 | 693,085 |
| Tagged Principal / cold get | 1,003,869 | 1,008,394 | 694,340 |
| Tagged Principal / second get | 46,241 | 51,048 | 208,033 |
| Tagged Principal / third get | 43,869 | 48,394 | 54,340 |
| Composite Principal / cold get | 1,017,031 | 1,021,812 | 869,484 |
| Composite Principal / third get | 57,031 | 61,812 | 78,222 |
| Tagged Principal / replace | 1,092,078 | 1,096,603 | 1,047,062 |
| Tagged Principal / insert | 2,311,489 | 2,316,565 | 2,110,159 |
| Tagged Principal / remove | 2,300,648 | 2,305,292 | 2,722,243 |

At 1k tagged-key records, Storage's second/third costs are 25,772/25,979
versus ICSS's 108,850/28,850. At 100k, ICSS's second and third costs are
208,033/54,340. A third-read-only control therefore misses meaningful costs;
the extra phase remains even though the production trial is rejected.

Application size remains separate from the all-types benchmark dispatcher:

| Application | Baseline total / code | Trial total / code | ICSS total / code |
| --- | ---: | ---: | ---: |
| Map | 103,359 / 87,879 | 103,663 / 88,111 | 90,687 / 49,664 |
| All collections | 117,228 / 99,939 | 117,610 / 100,255 | 119,816 / 77,566 |

The trial's 304/382 additional total bytes meet the size budget, but do not
justify retaining slower operations. These small application probes use scalar
keys; they are not a full-identity Auth canister size measurement. Both size
passes qualify sixteen modules, with 131 calls and two upgrades per module.
The broad all-types benchmark grows from 10,137,182 to 10,258,931 bytes
(+121,749), while its ICSS control stays at 11,936,166; these include dispatch
and generic type instantiations and are not production application sizes.
All 48 candidate engine tests, nine public ordered contracts, native/Wasm
Storage Clippy, benchmark driver tests and native/owned-Wasm benchmark Clippy
pass. Functional qualification is not a performance win.

No actual Auth endpoint was rebuilt or timed for this rejected candidate, and
no sibling files were changed. No new stable-IO instrumentation or complete
allocation profile was collected: existing attribution remains separate evidence,
not a newly measured explanation of every instruction difference. The candidate
does not change block sizes or allocation order.

Evidence: ordinary baseline/candidate under
`target/evidence/storage-comparison/attempt.0QzmBQ` and `attempt.ojXCOs`;
application sizes under `target/evidence/storage-consumer-size/attempt.oxT55t`
and `attempt.kiXWT9`. Both comparison directories retain exact source archives,
locks, modules, build settings and per-round observations. Native/lint logs use
`target/identity-scratch-*.log`; pretrial engine files are retained under
`target/evidence/identity-key-scratch-20261112/baseline/` and rejected source
under its `rejected/` sibling. Repeat with the workload and size commands above;
the current source is the restored baseline, not the removed trial. The trial's
behavior and rejection are documented here; no prototype feature remains.

### Remaining design boundary

The same independent implementer reviewed paired node/key refills without
changing code. They could pack routing/key blocks across fresh records, which
differs from the earlier separate-class batches, but the existing free lists
cannot preserve pairing through general reuse. Inline keys consume nodes alone;
variable key lengths consume different classes; 62-byte keys share the node's
class; values can consume those classes; deletion and successor payload transfer
also separate partners. Each free entry stores its class and next address, not
its allocation role or partner. Reliable pairing would need new bookkeeping,
search/unlink work or restricted reuse. Another refill-size sweep is not justified.

Changing blob class geometry likewise needs an explicit format decision:
class `c` currently denotes exactly `2^c` bytes for extent admission, payload
capacity and reuse. Shorter physical blocks under the same discriminator would
violate that invariant. The current allocator already chooses the smallest
supported class for payload plus its 16-byte header. Merely increasing inline
key width also changes the current node fields and adjacent inline-value tail.

Retain the current engine. The next substantive step for
[#158 — Routing-key locality and blob density](https://github.com/dragginzgame/ic-storage/issues/158)
is a separately reviewed layout proposal that budgets full routing keys, values,
metadata and scalar-key regressions together. Before implementing an incompatible
image, select a new discriminator/version and an explicit migration or reset
disposition for all affected consumers. This investigation grants no reset
authority and adds no legacy reader. No production layout winner is established.

### Combined compact-layout experiment

**Rejected; production restored.** A separate unexposed implementer authored
fresh-only STORMAP2/version 1 using the owned AVL engine, full decoded-key ordering
and governed adapters. No ICSS source or expressive tests entered that context.
Experimental framing does not authorize production retirement or consumer resets.

ICSS's [16-byte threshold](https://github.com/dfinity/stable-structures/blob/v0.7.2/src/btreemap/node/v2.rs)
selects eager key decoding, unlike Storage's separate-allocation threshold.
ICSS groups multiple keys before values, with node overflow. This experiment
instead tests locality within our existing algorithm.

Declared key bounds select node/inline-key widths of 128/16, 160/48 or 192/80
bytes for maxima ≤16, 17–48 or larger/unbounded respectively. The 1 KiB header
persists and validates geometry; routing excludes the 24-byte inline-value tail.
The tagged 31-byte benchmark key declares a larger bound and selects 192/80.
Blob classes use 32-byte steps through 2,048 bytes, then powers of two: 256-byte
values occupy 288 rather than 512 bytes; 1,024-byte values occupy 1,056 rather
than 2,048. Exact-class reuse has no splitting/coalescing; fresh density is not
a guarantee about heterogeneous resize history.

Matched identity comparison: 1,296 observations/216 medians, 1k/10k/100k rows,
29/31/62-byte keys, 256/1,024-byte values, three rounds, 128-page ICSS buckets.
All ICSS counters match. Toko changes .17→.18; the reference harness changes
only its name custom section, while executable/data sections and application
reference modules remain identical. Selected 100k tagged-key/256-byte results:

| Operation | Production | Initial candidate | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Cold get | 1,003,869 | 132,567 | 694,340 |
| Second get | 46,241 | 852,484 | 208,033 |
| Third get | 43,869 | 52,567 | 54,340 |
| Cold miss above | 1,086,820 | 136,502 | 1,105,357 |
| Cold get after churn | 1,086,442 | 135,681 | 779,801 |
| Replace | 1,092,078 | 221,766 | 1,047,062 |
| Insert | 2,311,489 | 2,332,838 | 2,110,159 |
| Remove | 2,300,648 | 2,169,364 | 2,722,243 |

151/198 non-control identity medians improve; 47 regress. Phases are separate
executions with prerequisite reads outside the timer: their sum is not one
real call. Tagged-key reservation drops 68.625→47.125 MiB at 256-byte values
and 215.125→120.625 MiB at 1,024; ICSS uses 40.0625/120.0625 MiB.

**All 616 scalar ordered medians regress.** Depth has 4,704 observations/784
medians at 256/4,096 rows; Cell/Vector's 120 non-control medians and all ICSS
controls remain identical. Scalar Map median penalties are 30.8%/13.3%.
Caching the exact frontier-derived node ceiling reduces them only to 30.0%/12.8%;
all 616 still regress. The latter retains every count check and recomputes on
reopen. It receives no new identity run after failing the scalar gate.

| Application total/code bytes | Production | Initial | Cached ceiling | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Map | 103,359/87,879 | 105,016/89,239 | 105,042/89,265 | 90,687/49,664 |
| All | 117,228/99,939 | 119,098/101,504 | 118,885/101,275 | 119,816/77,566 |

Initial all-types harness: 10,137,182→9,857,150 total and 8,911,582→8,636,237
code bytes. This is not application size. ICSS harness code stays 10,237,716;
its name-section change reduces total 11,936,166→11,930,307.
Both candidates pass 50/51 engine tests, nine public contracts, native/Wasm lint
and sixteen size consumers (131 calls/two upgrades). These are fresh-candidate
checks, not production migration qualification. No actual Auth endpoint or new
stable-IO/heap-allocation attribution is established.

Initial source/identity: comparison attempt.uJ7RAp; size attempt.lFXcmI.
Cached-ceiling depth: attempt.JGU86r; size attempt.NmiMLW, under the existing
comparison/consumer-size evidence roots. Complete matrices, controls and logs:
`target/evidence/map-layout-20261112/`. Baselines: attempt.0QzmBQ/attempt.oxT55t.
Reproduction uses prepared tools/caches, BENCH_WORKLOAD=identity-lookups or depth,
BENCH_SAMPLES=1, BENCH_UPSTREAM_BUCKET_PAGES=128 and make benchmark-storage;
application sizes use make benchmark-storage-size. Frozen experimental source
must be restored first: ordinary recipes rebuild production. Hashes alone cannot
reconstruct an experimental archive removed by explicit cleanup.

### Compact routing representation — rejected

Independent compact nodes carry 104 bytes/16 key bytes while retaining the
experimental stored geometry/count ceiling. Comparisons admit wider routing
buffers; successor transfers preserve key tails. All three trials regress all
616 ordered medians (4,704 observations/784 medians each), with Cell/Vector and
ICSS unchanged. Final scalar-Map median penalties: 4.95%/2.24% at 256/4,096;
worst ordered penalties: 52.54%/45.17%. These are heterogeneous row statistics.
A fixed-size prefix parser removes six slice-failure sites and slightly improves
instructions without changing application size. An inline hint adds no measured
benefit: code is identical to fixed-prefix, only data differs. Checks remain.

Selected uniform scalar Map operations, one operation per measured batch:

| Rows / operation | Production baseline | Compact node | Fixed prefix / inline hint | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| 256 / get | 91,733 | 95,723 | 95,467 | 94,781 |
| 256 / third get | 12,992 | 17,485 | 17,197 | 15,261 |
| 256 / scan limit 1 | 92,741 | 94,930 | 94,642 | 97,122 |
| 4,096 / get | 175,750 | 181,302 | 180,950 | 184,814 |
| 4,096 / third get | 18,308 | 24,877 | 24,461 | 22,295 |
| 4,096 / scan limit 1 | 176,387 | 179,207 | 178,823 | 187,194 |

First compact trial only: 1k shuffled records, 31/62-byte keys, 256-byte values,
uniform 1/32 batches, three rounds. Selected tagged-key batches of 32:

| Operation | Production baseline | Compact node | ICSS |
| --- | ---: | ---: | ---: |
| Get | 1,714,095 | 1,454,561 | 1,579,389 |
| Third get | 936,431 | 914,120 | 1,130,535 |
| Key scan | 898,061 | 734,852 | 425,732 |
| Entry scan | 1,030,055 | 801,933 | 585,455 |

Scans remain slower than ICSS; these controls do not qualify later parser
refinements. All three have the following application total/code bytes:

| Application | Production baseline | Compact trials | ICSS |
| --- | ---: | ---: | ---: |
| Map | 103,359 / 87,879 | 104,620 / 88,925 | 90,687 / 49,664 |
| All collections | 117,228 / 99,939 | 119,239 / 101,709 | 119,816 / 77,566 |

Growth is within budget; scalar probes do not establish Auth size. All-types
harness total/code bytes: compact 9,869,274/8,602,107; refined 9,867,882/8,600,747.

All trials pass 56 engine tests and native/Wasm lint; the first passes nine public
contracts. Each size run qualifies sixteen modules, 131 calls/two upgrades each.
Compact reads, transfers, reuse and corruption tests do not qualify migration.

**All three trials are removed.** The six-file production engine is restored
exactly; locks are unchanged. Production already uses a fixed-size read buffer,
so the parser refinement supplies no standalone improvement to retain.

Frozen depth/source attempts: `attempt.HHwszb`, `attempt.0X739T`, `attempt.d3tO55`
under `target/evidence/storage-comparison/`; matching application attempts:
`attempt.srskGy`, `attempt.jEUlQm`, `attempt.mLZa9K` under
`target/evidence/storage-consumer-size/`. The complete 784-row scalar comparison,
long-key controls, logs and disassembly are in `target/evidence/compact-routing-20261112/`.

### Complete benchmark-handler diagnostic

Frozen baseline/compact/ICSS modules receive the boundary-only transform:
request decoding, preparation, prerequisite reads, work, reply/cleanup and probe
cost; population/install/host dispatch are excluded. This is not Auth.
No linear memory/data is added. Roundtrip controls match 60 counters/backend;
compact's selected probe control matches 60, baseline's full sweep all 432 backend
inner medians. Compact's full uninstrumented sweep was not repeated after the
scalar failure, so universal absence of probe effects is not established.
Each sweep passes 1,296 observations/216 medians and final-state checks; all
ICSS handler medians match. Non-control improvements/regressions: 192/6,
worst +0.26% (removal). These counts are not weighted throughput.

Complete-handler medians for tagged keys and 256-byte values:

| Records / handler | Production baseline | Compact node | ICSS |
| --- | ---: | ---: | ---: |
| 1k / cold get | 962,453 | 801,662 | 961,543 |
| 10k / cold get | 1,290,190 | 812,183 | 1,305,934 |
| 100k / cold get | 1,620,598 | 739,909 | 1,310,600 |
| 100k / second get, including one prerequisite get | 1,668,718 | 1,586,578 | 1,511,804 |
| 100k / third get, including two prerequisite gets | 1,714,464 | 1,631,196 | 1,559,295 |

At 100k, complete cold improves 54.3%, versus roughly 88% in its inner interval.
With prerequisite reads the gain is 4.9%, still slower than ICSS. This neither
solves scalar regressions nor qualifies adoption; tracker/allocator attribution
is unmeasured here. Full matrix: compact-routing-20261112/handler-comparison.csv.

Boundary modules, hashes, controls and summaries: `target/evidence/compact-routing-20261112/`.
Reproduce from frozen modules with Binaryen disassembly, the existing transform
and reassembly (no optimization passes), then the frozen driver through Testkit:
`--workload identity-lookups --samples 1 --upstream-bucket-pages 128 --trace-execution-cost`.
Ordinary recipes rebuild restored production, not rejected layouts.

### Allocation density and routing isolation

**Neither control is adopted.** `STORMAP3` changes blob classes only; 128-byte
nodes, 16-byte inline keys and original routing remain. `STORMAP4` keeps power-of-two
blobs with compact wider-key routing/count ceiling. Dedicated 160/192-byte node
classes preserve geometry, but cannot share freed blobs as the combined trial did.
Experimental frames authorize no production retirement.

Density leaves 336/616 scalar ordered medians unchanged; 280 increase, at most
0.287%. Routing regresses all 616, with scalar-Map median increases 5.00%/2.45%
at 256/4,096 records and worst ordered increase 52.92%. Cell/Vector controls
remain unchanged. This isolates the large scalar penalty to the wider-routing
implementation, not the dense blob classes.

Selected 100k tagged-key/256-byte-value results. Inner and complete-handler
instruction scopes remain separate; second/third handlers include prerequisites.

| Metric | Production | Density only | Routing only | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| Cold get, inner | 1,003,869 | 926,511 | 122,593 | 694,340 |
| Second get, inner | 46,241 | 44,124 | 922,464 | 208,033 |
| Third get, inner | 43,869 | 44,124 | 42,519 | 54,340 |
| Cold handler | 1,620,598 | 1,543,260 | 739,627 | 1,310,600 |
| Second handler | 1,668,718 | 1,589,258 | 1,665,868 | 1,511,804 |
| Third handler | 1,714,464 | 1,635,274 | 1,710,131 | 1,559,295 |
| Stable reservation, MiB | 68.625 | 47.125 | 68.625 | 40.0625 |
| Map app, total/code bytes | 103,359/87,879 | 103,772/88,186 | 104,470/88,787 | 90,687/49,664 |
| All app, total/code bytes | 117,228/99,939 | 117,633/100,246 | 119,089/101,571 | 119,816/77,566 |

Application growth is +413/+307 total/code bytes for density Map and +1,111/+908
for routing Map; all-collection growth is +405/+307 and +1,861/+1,632. These
scalar size probes are not Auth or full-identity application measurements.
Density reduces 100k tagged/1,024-byte reservation from 215.125 to 120.625 MiB.
At 1k tagged keys, entry scans of 32 cost 1,030,055 production, 713,519 density,
1,038,919 routing and 585,455 ICSS instructions. Routing alone does not supply
the dense candidate's scan/memory benefits. No actual Auth endpoint is qualified.

**Resize history rejects the fine-grained density policy as a general default.**
An identical native test uses 32 records, two full ascending/descending sweeps
through 63 value lengths (25,49,81,...,2001), then settles at 1,024 bytes: 253
resize steps. Every old return, replacement, population and final reopen agrees.

| Engine backing capacity | Production | Density |
| --- | ---: | ---: |
| Fresh final-size population | 2 pages | 1 page |
| After resizing | 3 pages | 33 pages |
| Distinct historical blob blocks | 192 | 2,016 |

Native engine pages exclude governance; ICSS was not run in this control.
Resizing reserves 196,608 versus 2,162,688 bytes; repetition adds nothing.
Stranded free-block capacity is 63,488 versus 2,095,104 bytes, not an ongoing
leak. This adversarial history demonstrates a limit of fresh-population evidence.

Density/routing pass 50/59 engine tests, nine public contracts each, native/Wasm
lint and sixteen size consumers each (131 calls/two upgrades). Scalar/long-key
matrices have 784/120 medians each. Inputs/settings are frozen per attempt.
Each identity/handler run passes 1,296 observations/216 medians; probe inner
counters and ICSS controls match. Whole-handler medians improve/regress in
180/18 density cases (worst +0.044%) and 167/31 routing cases (worst +6.34%).
Production's six-file engine is restored exactly; locks and consumers are unchanged.

Depth/source: `attempt.OGPUUz`/`attempt.Ih2UBK`; size: `attempt.N2r53i`/`attempt.u0gLiX`
in the existing evidence roots. Identity/handler results, controls, hashes and
resize sources/histories: `target/evidence/layout-isolation-20261112/`. The earlier
recipes apply to archived experimental source, not restored production.

### Coarse allocation classes — not selected

The restricted implementer tested one generic policy: retain power-of-two bins
and add a 3/2 midpoint between them, starting at 96 bytes to preserve 32-byte
alignment. No knobs, splitting/coalescing or routing changes: nodes remain
128 bytes with 16 inline key bytes. There are 11 external classes through 2 KiB
versus six production/63 fine-grid; the full header still has 116 classes.
Fresh-only STORMAP5 framing rejects production images. This is no format decision.

The same 253-step, 32-record resize history and fresh controls pass:

| Native engine capacity | Production | Coarse | Fine-grid control |
| --- | ---: | ---: | ---: |
| Fresh 1,024-byte population, pages | 2 | 1 | 1 |
| Fresh frontier, bytes | 70,656 | 54,272 | 38,912 |
| After resizing, pages | 3 | 4 | 33 |
| After resizing, frontier bytes | 134,144 | 229,376 | 2,134,016 |
| Historical blob blocks | 192 | 352 | 2,016 |

The coarse history retains 33.3% more reserved pages and 71.0% more frontier
bytes than production; repetition adds no capacity. Fresh frontier falls 23.2%.
Across 73 fresh-size controls through 98,289 bytes, 36 use 25% smaller blob
extents and 37 equal extents. These are uniform populations, not mixed-size
consumer traces. Native engine accounting excludes governance; no ICSS result
is asserted for that internal control.

Scalar depth leaves 336/616 ordered medians equal and raises 280 by at most
0.530%; all 120 Cell/Vector non-control medians and ICSS controls are unchanged.
The 120-row long-key control improves 55/116 non-control medians and regresses
61. Selected tagged-key batches of 32 improve get 1,714,095→1,562,924 and entry
scan 1,030,055→873,688 instructions; ICSS uses 1,579,389/585,455. Third get rises
936,431→946,340. Single tagged-key nth rises 94,804→174,621; composite nth-back
94,932→174,986. Their roughly 80k increments are observed, not an independently
attributed page-fault explanation.

Full identity settings remain matched: 1k/10k/100k rows, three key shapes,
256/1,024-byte values, three rounds, one sample and ICSS 128-page buckets.
Inner medians improve/regress 83/115; complete-handler medians 103/95, with
worst handler increase 6.32% (1k composite/1,024-byte get after churn,
1,258,185→1,337,747). Row counts are not weighted application throughput.
Selected 100k tagged-key results:

| Metric | Production | Coarse | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| 256-byte cold get, inner instructions | 1,003,869 | 1,004,328 | 694,340 |
| 256-byte cold handler, instructions | 1,620,598 | 1,621,057 | 1,310,600 |
| 256-byte second handler, including prerequisite | 1,668,718 | 1,664,844 | 1,511,804 |
| 256-byte third handler, including prerequisites | 1,714,464 | 1,708,677 | 1,559,295 |
| 1,024-byte cold handler, instructions | 1,900,848 | 1,823,754 | 2,166,425 |
| 256-byte stable reservation, MiB | 68.625 | 56.125 | 40.0625 |
| 1,024-byte stable reservation, MiB | 215.125 | 166.125 | 120.0625 |
| Map application total/code bytes | 103,359/87,879 | 103,553/88,036 | 90,687/49,664 |
| All application total/code bytes | 117,228/99,939 | 117,414/100,096 | 119,816/77,566 |

Map grows only 194 total/157 code bytes; All grows 186/157. These are scalar
application probes, not Auth. The all-types harness separately grows
10,137,182/8,911,582→10,141,256/8,915,640 total/code bytes; ICSS is
11,930,307/10,237,716. The selected 256-byte cold gap remains; the larger-value handler improves 4.06%. Reduced
fresh capacity does not establish a better default given resize and iterator
regressions. Production is restored byte-for-byte. No consumer, format adoption
or publication changes; actual Auth and new stable-IO/heap attribution remain
unmeasured. Next isolate the changed access footprint in the iterator/churn
regressions before selecting another geometry or adding allocator machinery.

Qualification passes 49 engine tests, nine public contracts, native/Wasm lint,
16 size consumers (131 calls/two upgrades), 784 depth/120 long-key medians and
1,296 observations/216 medians per identity/handler run. All 432 backend inner
medians match between ordinary/probed runs; 60 uninstrumented roundtrip controls
match. All reference counters and eleven unaffected/reference application
modules remain identical. Frozen module/driver hashes verify after completion.

Source/depth: comparison attempt.WLYg1e; size: consumer-size attempt.wEQPF3.
Raw controls/source/reports: `target/evidence/coarse-classes-20261112/`.
Root lock stayed unchanged. Concurrent qualification rustls-webpki .16→.17 was
preserved; those frozen runs used .16 and did not qualify .17.

### Access-path attribution — 2026-11-13

**The extra iterator event is a stable-read path change, not extra tree work.**
For tagged nth, round three, both layouts make 14 reads/1,359 bytes with identical
allocation costs. Production reads nodes in distinct 4 KiB pages; coarse reads
1,205,440 then 1,207,168 in page 294. The latter call costs 80,324 instead of
324 instructions, using the same already-used destination. Both later pay one
80k event on the selected key's allocation header.

[PocketIC 16.1.0](https://github.com/dfinity/pocketic/releases/tag/16.1.0) identifies
IC revision e9e42a6dd74acb9049017863e944cd5799be5b2b. Its
[read-path selection](https://github.com/dfinity/ic/blob/e9e42a6dd74acb9049017863e944cd5799be5b2b/rs/embedders/src/wasm_utils/system_api_replacements.rs#L600-L750)
uses the first-access helper only when every requested page is untouched;
otherwise it copies from native stable memory. The
[helper reads the backing buffer](https://github.com/dfinity/ic/blob/e9e42a6dd74acb9049017863e944cd5799be5b2b/rs/embedders/src/wasmtime_embedder/system_api/stable_memory.rs#L30-L48).
For all 36 complete read-only traces, this path model predicts every recorded
read cost exactly: 220 + bytes + 80k per newly encountered native 64 KiB region.
This is scoped diagnostic calibration, not a universal heap/stable fee formula.
Denser packing can trigger native mapping earlier even with fewer occupied pages.

The 6.32% post-churn median also needs context: costly events across three rounds
are production 6/6/7 versus coarse 7/5/7. Both total 19. Uninstrumented complete
handlers sum to 3,854,319 versus 3,853,134 instructions (−0.031%). Individual
regressions remain real; a median across different key paths is not repeated
measurement of one call.

Expanded 20-round means, 1k records/256-byte values, single-result rank cursors;
complete benchmark handlers, not Auth:

| Key / operation | Production | Coarse | ICSS |
| --- | ---: | ---: | ---: |
| Tagged / nth | 765,137 | 781,191 | 2,384,387 |
| Tagged / nth-back | 768,390 | 776,444 | 2,360,413 |
| Composite / nth | 773,212 | 777,266 | 2,953,840 |
| Composite / nth-back | 760,958 | 773,012 | 2,869,054 |

Coarse's handler overhead is 0.52–2.10% here. Keep the resize tradeoff and unchanged
100k cold gap in view; no default/format is adopted. Next assess whether checked
routing/key reads can be combined for contiguous allocations without widening
persistent nodes, reading value payloads or weakening corruption admission.

The 162 paired identity traces retain equal read counts/bytes and allocation
counts; 100k tagged/256-byte cold paths each still pay 12 costly read events.
28 driver tests, lint, formatting and probe controls pass. Diagnostic-only
changes add zero product Wasm; prior application-size evidence is reused.
Raw evidence: `target/evidence/coarse-access-trace-20261113/`.
Expanded runs contain 4,800 observations per variant; all ICSS samples
and the first three ordinary rounds match. No publication or engine change.
