# Storage / ICSS full comparison — 2026-11-10

All maintained named recipes, the ordinary matrix, three depth bucket controls,
size consumers and diagnostics completed. The comparison drivers validated
192,444 observations with no unexpected collection failure. Four original runs
needed complete retries after their server lifetime expired; two retry wrappers
also reported a separate root-lock edit after their comparison validation.
Those failures remain recorded below rather than being relabeled as clean exits.

## Measured source and scope

Storage 0.73.0 at `87cf55e4ca58db415da1639d72ed4cef04eeeb94`, with the
pending Toko Tooling 0.39.8 adoption. All inventoried product and qualification
Rust sources are unchanged by adoption. The reference is published
**ic-stable-structures 0.7.2**, not an earlier Storage implementation or the
optional ICSS Git control.

Rust/Cargo 1.99.1, Testkit 0.95.0 and PocketIC 16.1.0 were admitted before the
runs. The independent `qualification/Cargo.lock` selects Platform Runtime
**0.21.0** throughout. A separate edit updated the root lock to Runtime 0.21.1
while benchmarks were running. That edit is preserved. It did not change the
qualification lock, source hashes or measured Wasms; these results do **not**
qualify the root's new Runtime selection. The existing root Build 0.81.2 and
Host 0.76.3 updates were also preserved.

Primary comparisons use ICSS's stock 128-page buckets. Additional depth
controls use one- and eight-page ICSS buckets; native IO attribution and installed
admission also use eight-page ICSS buckets. Storage retains its own governed
backing. Reservation, instruction counts and compiled bytes are distinct
measurements. Neither an ICSS tuning control nor a smaller reservation establishes
a generally better production default.

Instruction figures use the existing operation timer, excluding setup, request
preparation and final verification. The Vector-opening preset measures opening,
operation preparation and closing as documented by that preset. Auth lookup
stages include outer Wire decoding, whose nested counter must not be added to
the total. These are synthetic collection workloads, not complete application
throughput or production Auth endpoints. See the
[maintained benchmark contract](../../qualification/ic-storage-benchmarks/README.md).

## Type coverage

All five collection shards of the combined ICSS/Storage catalogue completed:
73 concrete types, 565 selected type/role cases, 54,318 timed observations.
519 cases are supported by both backends. The remaining catalogue entries
include 53 expected backend refusals and 18 backend entries that are inapplicable
because the selected key lacks `Ord`; they are not successful paired operations.

| Collection | Selected cases | Storage supported | ICSS supported | Timed observations | Expected backend refusals |
| --- | ---: | ---: | ---: | ---: | ---: |
| Map | 173 | 170 | 163 | 30,224 | 7 |
| Set | 73 | 70 | 67 | 3,252 | 3 |
| Vector | 73 | 57 | 53 | 5,200 | 36 |
| Cell | 73 | 73 | 73 | 2,472 | 0 |
| Region | 173 | 170 | 163 | 13,170 | 7 |

Expected refusals cover unbounded Vector elements and the selected ICSS fixed
Option encoding cases. Trait-inapplicable key cases are recorded separately.
All completed operations check their results and final collection contents
against the independent model. No performance win is assigned to a refused case.

## Populated collections

Selected depth rows: 4,096 records, uniform access, batch/scan limit 32, u64
scalar cases (Set carries unit values), three seeds and three stateful rounds.
Values are median measured instructions; a ratio below one favors Storage.
Cell is a singleton fixed
32-byte value with batch 32, not a 4,096-value collection.

| Collection / operation | Storage | ICSS | Storage / ICSS |
| --- | ---: | ---: | ---: |
| Map get | 1,159,867 | 930,340 | 1.247 |
| Map replace | 1,937,599 | 1,650,755 | 1.174 |
| Map remove | 2,116,144 | 2,191,182 | 0.966 |
| Map scan | 773,363 | 312,054 | 2.478 |
| Set get | 1,135,265 | 838,132 | 1.354 |
| Set scan | 336,755 | 212,181 | 1.587 |
| Vector get | 103,413 | 102,932 | 1.005 |
| Vector set | 188,052 | 180,724 | 1.041 |
| Vector push | 201,716 | 186,644 | 1.081 |
| Cell get | 3,152 | 3,152 | 1.000 |
| Cell replace | 176,336 | 182,160 | 0.968 |
| Region get | 1,234,380 | 952,331 | 1.296 |
| Region replace | 2,013,904 | 1,674,410 | 1.203 |

The populated-record recipe changes the value and key shapes. Selected rows
below retain 4,096 records, uniform access, batch/scan limit 32 and three
seeds/rounds. It additionally covers hot-16 and Zipf access in the raw report.

| Case / operation | Storage | ICSS | Ratio | Reserved pages, Storage / ICSS |
| --- | ---: | ---: | ---: | ---: |
| u64 → 32-byte Vec, get | 1,500,249 | 1,401,552 | 1.070 | 34 / 129 |
| u64 → 32-byte Vec, scan | 1,102,684 | 559,216 | 1.972 | 34 / 129 |
| u64 → 4-KiB Vec, get | 6,000,204 | 16,989,535 | 0.353 | 562 / 385 |
| u64 → 4-KiB Vec, replace | 8,842,556 | 29,834,519 | 0.296 | 562 / 385 |
| u64 → 4-KiB Vec, scan | 3,923,485 | 3,958,040 | 0.991 | 562 / 385 |
| 32-byte key/value arrays, get | 2,259,818 | 1,416,821 | 1.595 | 42 / 129 |
| 32-byte key/value arrays, scan | 1,397,853 | 502,716 | 2.781 | 42 / 129 |
| WireRecord Vector, get | 1,281,633 | 1,274,216 | 1.006 | 42 / 129 |

These results do not select one universal winner. Small-record traversal remains
an instruction disadvantage; larger Map values can improve substantially while
using more stable capacity. Stable pages here are 64-KiB capacity units, not
counts of charged heap faults.

## Bucket controls

The same depth recipe and binaries also complete with ICSS bucket sizes 1 and
8. At the selected 4,096-row, uniform, batch-32 `Get` observations:

| Collection | Storage pages | ICSS: 1-page buckets | ICSS: 8-page buckets | ICSS: 128-page buckets |
| --- | ---: | ---: | ---: | ---: |
| u64 Map | 34 | 4 | 9 | 129 |
| u64 Set | 34 | 3 | 9 | 129 |
| u64 Vector | 26 | 2 | 9 | 129 |
| u64 Region | 34 | 4 | 9 | 129 |

ICSS Map read instructions are 941,230 / 930,340 / 930,340 for buckets 1/8/128;
Storage is 1,159,867 in all three controls. One-page ICSS buckets save further
capacity with a small read-cost increase here. The eight-page setting matches
the stock read count with substantially less reservation. This is a matched
small-scalar workload result, not evidence that one setting wins every population,
value size, mutation history or allocation count.

## Large payloads

Eight populated records (one for Cell), single operations, uniform access,
three seeds and one stateful round. Growth changes payload size, rather than
rewriting an unchanged value. Selected results:

| Case / operation | Storage | ICSS | Ratio |
| --- | ---: | ---: | ---: |
| 64-KiB Map get | 456,771 | 1,058,286 | 0.432 |
| 64-KiB Map replace | 844,517 | 4,300,716 | 0.196 |
| 1-MiB Map get | 6,023,131 | 16,151,418 | 0.373 |
| 1-MiB Map replace | 8,593,815 | 64,546,506 | 0.133 |
| 1-MiB Region replace | 8,676,063 | 64,547,328 | 0.134 |
| 64-KiB Vector push | 1,034,475 | 716,595 | 1.444 |
| 1-MiB Vector get | 4,970,306 | 5,049,695 | 0.984 |
| 64-KiB Cell replace | 466,190 | 386,389 | 1.207 |
| 1-MiB Cell replace | 3,849,319 | 3,769,429 | 1.021 |
| Cell shrink to empty, either initial size | 240,689 | 160,853 | 1.496 |

These measured counters include applicable memory charges. A fixed instruction
difference alone does not establish a CPU, allocator or page-charge explanation.

## Iteration, ordering and session-shaped reads

The iterator and ordered presets validate 7,722 and 12,312 observations
respectively. The ordered preset's 256-row u64 Map, uniform batch 32, costs
453,846 / 4,394,836 instructions for `Nth`, and 414,246 / 7,980,104 for
`IterLast` (Storage / ICSS). Its ordinary scan costs 201,371 / 146,922.
Specialized terminal/rank operations and sequential iteration therefore need
separate comparisons; fast terminal access does not establish a faster scan.

The auth-shaped preset validates 480 timed calls / 240 paired rows across
1/512/1,024/2,048 records, 256/2,048-byte inner payloads, first/middle/last
positions, and query/update execution. It uses one seed and one round.

For the 2-KiB first-record `Get` in an update execution, Storage costs 86,461
and ICSS 86,148 instructions at both one and 2,048 populated records. At 2,048
records, middle/last reads cost 86,461 / 166,148 and 86,461 / 166,261 respectively.
This test does not reproduce a population-dependent increase for the selected
first-record read. It does not measure Canic configuration projection,
caller-to-index lookup or Auth's inner session decoder. Read-position and memory
charge effects must not be generalized into a uniform cache or codec speedup.

The retained-handle Vector read/update-history preset additionally validates
2,520 observations, including reads after set, push and pop. The reopening preset
validates 12,672 observations plus 528 initialization measurements, across six
types at 256/4,096 entries. Its timer includes lease/view lifecycle work.

For the 4,096-row u64 Vector under uniform access, single-request `OpenControl`
costs 4,082 / 83,233 and `Get` including opening costs 4,826 / 84,030 instructions
(Storage / ICSS). Batch-32 `Get` costs 114,958 / 113,351. The opening control must
not be subtracted as though its first-access charges were independent, and the
single-read ratio must not be extrapolated to batched reads or whole endpoints.

## Application Wasm size

Eight paired production-shaped consumers, `wasm-size` profile (`opt-level=z`,
fat LTO, one codegen unit, abort, stripped symbols). Every artifact validates
118 calls and two upgrades. These byte counts match the previously recorded
independent-engine sizes; the tooling adoption did not change them.

| Selection | Storage bytes | ICSS bytes | Storage minus ICSS |
| --- | ---: | ---: | ---: |
| Base | 16,942 | 16,938 | +4 |
| Map | 113,038 | 90,208 | +22,830 |
| Set | 112,860 | 90,008 | +22,852 |
| Vector | 99,954 | 64,560 | +35,394 |
| Cell | 97,685 | 63,135 | +34,550 |
| Region | 114,417 | 89,933 | +24,484 |
| Map + region | 115,957 | 110,858 | +5,099 |
| All collections | 126,406 | 119,318 | +7,088 |

The broad synthetic instruction harness is a different artifact/profile:
Storage **8,734,158 bytes**, ICSS **11,084,040 bytes**, using `comparison`
(`opt-level=3`, fat LTO, one codegen unit, abort) and an eight-MiB stack on both
sides. Gzip changes installation transport only. Its smaller Storage result does
not establish a smaller application. Storage's application integration also owns
allocation governance and custody; the raw ICSS consumer does not provide those
same services.

## Diagnostic scope

Native collection IO validates 3,840 rows across Storage, ICSS on governed
Storage backing, and ICSS on its own manager. For the 4,096-row fixed-value Map
with uniform batch-32 reads, Storage performs 370 physical reads / 35,408 bytes;
ICSS performs 1,997 reads / 17,117 bytes on either backing. Fewer calls do not
imply fewer instructions. This instrumented native workload is attribution,
not an IC instruction or heap-page-charge measurement.

Multi-allocation diagnostics cover 32/64 allocations against ICSS buckets
1/8/128. After the six size-churn phases, the 32-allocation run reserves 1,802
pages for Storage versus 1,147 / 1,345 / 4,097 for ICSS respectively; the
64-allocation run reserves 3,594 versus 2,291 / 2,689 / 8,193. Thus Storage's
advantage over stock buckets is not an advantage over every tuned ICSS layout.

Nine Storage-only small-record size cases each preserve a 50-page high-water
through six deletion/reuse cycles. Nine allocation histories also validate;
their changing payload sizes can raise high-water. These controls do not prove
arbitrary churn is bounded to the initial capacity.

Installed admission covers 864 Storage and 600 ICSS observations. At 16 names
and one populated bucket per allocation, opening one allocation costs
164,440 / 160,391 instructions, and 32 held-handle reads cost
252,909 / 244,900 (Storage / ICSS). At one name, reopening the backing costs
23,013 / 805,538; at 16 names, 167,372 / 805,538. Storage performs governance and
admission work absent from the raw ICSS manager. Higher name counts remain
stress controls, not application sizing recommendations; ICSS stops at its
supported 255-memory limit.

## Evidence and completion

The suite envelope is `target/evidence/storage-full-suite/attempt.BNiLXO`.
Raw outputs remain in the per-run directories; no bulk JSONL/CSV dumps are
added to documentation. They remain disposable `target/` artifacts, not a
permanent archive. Source archives, lockfiles, compiler identities, Wasms,
hash checks and failed attempts are retained locally. Reproduction uses the
maintained runners, not a promise that target artifacts survive `cargo clean`.

The table names directories under `target/evidence/storage-comparison/`; each
contains a `report/summary.csv`, coverage/configuration and raw observations.
Repeated population/shard exports are counted once, using the aggregate where
available. The totals below exclude incomplete attempts, initialization and
the separately reported admission diagnostics.

| Recipe / ICSS bucket pages | Seeds | Validated observations | Completed evidence directory |
| --- | ---: | ---: | --- |
| Depth / 128 | 3 | 13,536 | `attempt.zw4zWc` |
| Types / 128 | 1 | 54,318 | `attempt.45ivXK` |
| Control / 128 | 3 | 1,224 | `attempt.WY9Hqu` |
| Records / 128 | 3 | 23,652 | `records-retry.8lrYFv` |
| Bulk / 128 | 3 | 720 | `bulk-retry.K2JNEi` |
| Boundaries / 128 | 3 | 12,960 | `boundaries-retry.SSRFus` |
| Iterators / 128 | 1 | 7,722 | `attempt.sU65yO` |
| Ordered / 128 | 1 | 12,312 | `attempt.u4n8Os` |
| Auth lookups / 128 | 1 | 480 | `attempt.rqO0UP` |
| Vector reads / 128 | 3 | 2,520 | `attempt.hhjHvn` |
| Vector open / 128 | 3 | 12,672 | `attempt.hDlGPL` |
| Ordinary / 128 | 3 | 23,256 | `default-retry.sC77Kk` |
| Depth / 8 | 3 | 13,536 | `attempt.9qCiHA` |
| Depth / 1 | 3 | 13,536 | `depth-one.omHGGy` |

The size run is `target/evidence/storage-consumer-size/attempt.cX7AWq`.
Native diagnostics, both admission CSVs, their artifact hashes, the complete
`result-index.json` and `root-lock-reconciliation.json` live in the suite envelope.

All comparison runs use the same two Wasm hashes:

- Storage: `1e9f3a3de7bbd1e1f41eac8d536be8df472399b5235a6df386b56b9db4ac40c3`.
- ICSS: `736915e84ed2c55b3d6fb6d6dde21d0410063fa87317cf9a97a96b9c03a2dff1`.
- Qualification lock: `785a3c97388a4705839d65a66014bc6252ced4916cb94de4de1a8820ed370a3a`.

Records (`attempt.o3f7ru`), bulk (`attempt.lIpofo`) and boundaries
(`attempt.Va5Ns9`) explicitly report server exit 124 after approximately
901 seconds. The ordinary run (`attempt.KQRWSj`) reports a connection reset
when its server reached the same deadline. Every retry starts fresh, uses the
original saved Wasms and an identical copied native driver/configuration, and
changes only the server lifetime to 3,600 seconds. No partial-prefix results are
mixed into the completed tables. The one-page depth control also reuses the
saved depth binaries.

The records and bulk retry drivers both validated their full results and passed
their source/artifact/qualification-lock checks. Their enclosing scripts then
exited 1 because the independent root lock had changed from Runtime 0.21.0 to
0.21.1, adding `ic0` 1.2.0. This is a recorded guard failure, not a successful
wrapper exit. The measured graph remains Runtime 0.21.0, confirmed by locked
Cargo resolution, unchanged qualification lock and identical binaries. The
root update is preserved and is not performance-qualified here.

After the frozen runs finished, the consumer comparison runner was changed to
one bounded **3,600-second server lifetime** for ordinary and type-sharded runs.
The five-minute request budget, operation timer, workload selection, model checks
and all Rust sources remain unchanged. Successful long retries exercise this
lifetime; focused shell validation qualifies the small orchestration edit.
The original runner and its failed attempts remain in the captured evidence.
A subsequent live `Cell-U64-Array32` smoke run through the updated Make
entrypoint passes its full guards at `attempt.pIAGug`, records the 3,600-second
lifetime and retains the same two Wasm hashes. It is separate from the 192,444
observations above.

## Reproduction and limits

Use the admitted compiler/tools and separately prepare the locked cache and
server as described in the [benchmark README](../../qualification/ic-storage-benchmarks/README.md).
Then run the maintained commands; each creates fresh evidence and checks its
own inputs. The explicit seed counts reproduce this pass's selection:

```sh
make benchmark-storage-size
for workload in depth control records bulk boundaries vector-reads vector-open; do
    make benchmark-storage BENCH_WORKLOAD="$workload" BENCH_SAMPLES=3 BENCH_UPSTREAM_BUCKET_PAGES=128
done
for workload in types iterators ordered auth-lookups; do
    make benchmark-storage BENCH_WORKLOAD="$workload" BENCH_SAMPLES=1 BENCH_UPSTREAM_BUCKET_PAGES=128
done
make benchmark-storage BENCH_ROWS=256 BENCH_SAMPLES=3 BENCH_UPSTREAM_BUCKET_PAGES=128
for pages in 1 8; do
    make benchmark-storage BENCH_WORKLOAD=depth BENCH_SAMPLES=3 BENCH_UPSTREAM_BUCKET_PAGES="$pages"
done
```

The README also owns exact build/run commands for `collection-io`,
`multi-allocations`, its `--small-records` / `--allocation-histories` modes and
the installed `admission` driver. Use fresh output paths; do not overwrite the
retained records. This pass used separate diagnostic build directories while
installed comparisons ran, without changing their inputs.

Optional ICSS Git/cache controls and Wasm instrumentation experiments are not
included. These results do not replace `make validate`, release qualification,
corruption/rollback qualification, a populated multi-GiB ledger, or complete
application measurements. Source reduction, instruction savings, reservation
and application Wasm savings are independent outcomes. The strongest next
targets exposed here are small-record traversal/packing and the shared
application-size overhead; the large-value wins do not erase those gaps.
