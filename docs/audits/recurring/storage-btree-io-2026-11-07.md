# B-tree IO experiments against ICSS 0.7.2

Reference means published **ic-stable-structures 0.7.2**, not an older Storage
engine. Baseline is Storage 0.68.5 at `79d2c1d`. These are synthetic collection
handlers, not application endpoint speedups. Public APIs and stored geometry
remain fixed. No additional cache, journal or tuning parameter is introduced.

## Matched workload and evidence

The existing `depth` preset exercises all five supported forms: u64/u64 and
u64/32-byte Map, u64 Set, u64 Vector, fixed-32-byte Cell and u64/u64 map regions.
It uses 256/4,096 records, uniform/hot-16 access, batches 1/32 and three stateful
rounds. Cell has one value. Each ordinary or boundary run validates 4,272 calls
and 712 aggregate comparison cells. Full-handler costs include request/reply
codecs, input preparation and cleanup, excluding population setup.

Rust 1.99.1, Runtime 0.19.5, Wire 0.17.6 and PocketIC 16.1.0 remain selected by
the independent qualification graph. ICSS uses eight-page buckets. Both backends
use the comparison profile and eight-MiB synthetic stack. The incoming root-lock
Wire 0.17.9 edit is preserved; it does not supply the installed benchmark graph.
Native production tests use that root lock. No dependency adoption is claimed.

Raw evidence is under `target/evidence/storage-overflow-tail/` and
`target/evidence/storage-node-header/`. Frozen comparison attempts retain source,
input and module identities under `target/evidence/storage-comparison/`.
Reproduce with `BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage` and
the [benchmark guide's boundary instrumentation](../../../qualification/ic-storage-benchmarks/README.md#iterator-coverage).
Every ordinary/probe response is compared, including per-round inner counts;
the probes add no linear memory. Stable pages and ICSS costs are controls.

## 1. Overflow-tail writes: retained

ICSS writes a null terminator at every node-save completion. Storage now writes
it only after shortening the overflow chain. Loaded chains already terminate;
new pages initialize their next pointer to null; the normal node save writes the
initial link. Removing any pages still repairs the new tail before completion.

Baseline `0rbqyd` and candidate `CtmaHJ` improve/match/regress **168/544/0** cells
in both timing scopes. Maximum loop saving is 1.42%; maximum full-handler saving
is 1.15%. ICSS costs, stable pages and all 4,272 ordinary/probe responses match
at each step. Uncompressed full-catalogue Wasm grows **24 bytes**:
14,360,060 → 14,360,084; code-section payload 12,136,503 → 12,136,527.

Full-handler instructions below use 4,096 records, batch 32, uniform access;
Cell is the one-record control in that population context.

| Case | Storage 0.68.5 | Tail-write change | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| u64 Map Get | 1,417,434 | 1,417,434 | 1,568,886 |
| u64 Map Replace | 2,133,703 | 2,125,154 | 2,251,444 |
| 32-byte Map Replace | 3,209,558 | 3,201,014 | 3,342,919 |
| u64 Set Reinsert | 1,899,063 | 1,890,519 | 2,065,794 |
| u64 Region Replace | 2,274,684 | 2,263,959 | 2,274,181 |
| u64 Vector Set | 695,155 | 695,155 | 772,509 |
| 32-byte Cell Replace | 870,826 | 870,826 | 955,587 |

The change does not establish universal ICSS parity: for example, Region
RemoveInserted remains 2,734,250 versus ICSS's 2,675,605 in this same context.
ICSS has no region API; that reference reopens its map for each operation.

Native attribution covers 160 logical cases across Storage, ICSS over governed
backing and ICSS's manager (480 rows). For 32 replacements of 32-byte values in
a 4,096-record Map, Storage issues 548 writes / 7,712 bytes; governed ICSS issues
580 / 7,968. Both access 58 native 4-KiB ranges and write 30. These range counts
are not IC page-fault charges. The separately metered engine regression verifies
zero finish writes for unchanged/new tails and valid links through partial/full
shrinkage, exact page ends and subsequent freed-page reuse.

Qualification passes the chain regression, 23 adapter tests, all seven ICSS
image-interchange tests, strict core/benchmark lint and installed original
Storage IO/upgrade/rollback across 1,031 allocations. Cross-reading now checks
intermediate images after shrinking values to empty, growing to 64 KiB and
shrinking again. Other optimizer profiles, the complete installed 73-type
timing matrix and release gates were not rerun for this development experiment.

## 2. Adjacent node-prefix reads: rejected for size

ICSS reads the seven-byte node header and eight-byte first overflow link
separately. Trial `7TT6E0` reads both in one packed prefix without changing the
stored header or the existing page-size calculation. A one-node u64 lookup
then uses four reads instead of five, transferring the same 35 bytes.

Relative to the retained tail-write step, both loop and full-handler matrices
improve/match/regress **552/160/0** cells; maxima are 2.75% and 2.02% respectively.
Stable pages and ICSS costs are unchanged, and every ordinary/probe response
matches. Selected full-handler results use the same scope as the table above:

| Case | Tail-write baseline | Rejected prefix read | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| u64 Map Get | 1,417,434 | 1,402,077 | 1,568,886 |
| u64 Map Replace | 2,125,154 | 2,109,797 | 2,251,444 |
| u64 Region Get | 1,488,348 | 1,470,005 | 1,585,791 |
| u64 Region Replace | 2,263,959 | 2,245,616 | 2,274,181 |

The initial variant adds **19,220 bytes** to full-catalogue Wasm
(14,360,084 → 14,379,304); code payload grows 12,136,527 → 12,155,970.
A refinement moves admission into the existing load routine, avoiding extra
header arguments between routines, but still adds **19,690 bytes** total
(14,379,774; code payload 12,155,416). Both exceed the 5–10 KB limit without an
exceptional demonstrated benefit. The refined variant passed 24 adapter tests
and strict lint but was rejected on size before installed benchmarking.

Both implementations and the prefix-only read-count test were reverted.
Final source hashes match retained trial `CtmaHJ`; all 78 final core tests,
strict lint and formatting pass. The earlier candidate's 28
benchmark/model tests and image-interchange results remain historical trial
evidence; they are not a claim that its code is retained. Function-level reasons
for the code-size growth were not isolated. No additional generic abstraction
or API change was introduced to rescue it.

## 3. Empty node fields: retained

The private NodeWriter now skips backing calls for empty fields inside its
already allocated initial page. ICSS 0.7.2 forwards these calls. Logical end
accounting, overflow allocation, codec checks and public memory bounds checks
remain unchanged. Leaf child lists and Set's zero-byte values benefit.

Candidate `IWdjZT` against tail baseline `CtmaHJ` improves/matches/regresses
**162/544/6** cells in both timing scopes. Maximum loop saving/regression is
8.19%/0.0273%; full-handler saving/regression is 6.38%/0.0125%. The six tiny
regressions are single-operation hot-key Map/Region mutations; the extra branch
is not free. Stable pages and ICSS costs match. Every ordinary/probe response
matches across 4,272 calls, including inner instruction counts.

Full-handler instructions, 4,096 records, batch 32, uniform; Cell has one value:

| Case | Tail-write baseline | Empty-write change | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| u64 Map Replace | 2,125,154 | 2,122,638 | 2,251,444 |
| 32-byte Map Replace | 3,201,014 | 3,198,766 | 3,342,919 |
| u64 Set Reinsert | 1,890,519 | 1,830,919 | 2,065,794 |
| u64 Set RemoveInserted | 2,184,691 | 2,096,483 | 2,354,532 |
| u64 Region Replace | 2,263,959 | 2,260,093 | 2,274,181 |
| u64 Vector Set | 695,155 | 695,155 | 772,509 |
| 32-byte Cell Replace | 870,826 | 870,826 | 955,587 |

Full-catalogue Wasm grows **13 bytes**: 14,360,084 → 14,360,097; code payload
12,136,527 → 12,136,540. All 480 native IO rows exactly match the tail baseline:
governed backing already suppresses physical zero-byte writes. This saves the
wrapper/checking work, not payload IO or page charges. No broader parity claim:
Region RemoveInserted still costs 2,728,806 versus ICSS's 2,675,605 here.

All 79 core tests, 28 benchmark/model/codec/interchange tests, strict core lint,
formatting and installed original Storage IO/upgrade/rollback pass. The new
regression checks empty writes at/beyond page ends without losing logical extent
or overflow reservation. Raw results: `target/evidence/storage-empty-writes/`.
The same profile/type-timing/release limitations above apply. Against deployed
0.68.5, both retained changes together improve/match/regress **168/544/0**
full-handler cells, save up to 7.42%, and add **37 bytes** of benchmark Wasm.
