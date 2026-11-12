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
