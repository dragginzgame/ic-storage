# Empty B-tree value loading against ICSS 0.7.2

Keep a zero-length return in `Node::load_value_from_memory`; reject a general
zero-byte check in `NodeReader`. Both alternatives were compared with the same
pending tail/empty-write baseline, `IWdjZT`, from the
[preceding IO study](storage-btree-io-2026-11-07.md). Reference means published
**ic-stable-structures 0.7.2**, not an earlier Storage engine.

## Scope and controls

The matched `depth` preset covers Map, Set, Vector, Cell and map regions, with
256/4,096 records, uniform/hot-16 access, batches 1/32 and three stateful rounds.
Cell holds one record. Each run validates 4,272 calls and 712 aggregate cells.
Full-handler instructions include request/reply codecs and cleanup; setup is
excluded. These are synthetic collection results, not application speedups.

Keep Rust 1.99.1, Runtime 0.19.5, benchmark Wire 0.17.6, Testkit 0.79.0,
PocketIC 16.1.0, ICSS eight-page buckets and the comparison profile's eight-MiB
stack fixed. The independent root-lock Wire 0.17.9 edit remains untouched.
Neither dependency adoption nor production-canister size is measured here.

The ICSS module and driver are byte-identical to the baseline. Each candidate's
4,272 ordinary/boundary responses match, including per-round inner instructions;
the probes add no linear memory. Stable pages and ICSS costs remain unchanged.
All 480 native IO rows exactly match the baseline: governed backing already
suppresses physical zero-byte reads. Savings here are wrapper and reader setup
work, not reduced payload bytes or measured page charges.

## Alternatives and decision

The general trial (`z4Nsyp`) guards zero-byte reads inside NodeReader's initial
page path. Its loop and full-handler matrices improve/match/regress
**112/160/440** cells. Maximum loop saving/regression is 3.00%/0.722%; handler
saving/regression is 2.29%/0.584%. Total Wasm shrinks by 19 bytes, but the broad
regressions reject this implementation. Its code and implementation-only test
were removed before the narrower trial.

The retained trial (`sGL0uw`) returns an empty vector when the already-read
stored value length is zero, before constructing a NodeReader. Key/routing reads
keep their existing path; nonempty values still use the original reader and
initialization contract. Lazy loading and extraction both use this one helper.
Public APIs, persisted lengths, allocation geometry and codecs are unchanged.
No cache, new generic abstraction or format migration is introduced.

Both retained timing scopes improve/match/regress **368/344/0** cells. Maximum
loop saving is 5.55%; maximum full-handler saving is 4.24%, for the 256-record
Set RemoveInserted trace with batch 32 and uniform access. The small nonempty
Map savings are measured effects, not evidence of fewer backing reads.

Full-handler instructions, 4,096 records, batch 32, uniform; Cell holds one value:

| Case | Prior pending baseline | Rejected general read | Retained empty value | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| u64 Map Get | 1,417,434 | 1,421,818 | 1,417,434 | 1,568,886 |
| u64 Map Replace | 2,122,638 | 2,127,834 | 2,122,433 | 2,251,444 |
| u64 Set Reinsert | 1,830,919 | 1,812,105 | 1,791,373 | 2,065,794 |
| u64 Set RemoveInserted | 2,096,483 | 2,069,145 | 2,040,451 | 2,354,532 |
| u64 Region Replace | 2,260,093 | 2,258,794 | 2,259,888 | 2,274,181 |
| u64 Vector Set | 695,155 | 695,155 | 695,155 | 772,509 |
| 32-byte Cell Replace | 870,826 | 870,826 | 870,826 | 955,587 |

This does not establish universal ICSS parity. Region RemoveInserted remains
2,728,496 versus ICSS's 2,675,605 in this context; ICSS's region reference reopens
its map per operation because it has no matching region API.

| Full-catalogue Wasm | Total bytes | Code-section payload |
| --- | ---: | ---: |
| Prior pending baseline | 14,360,097 | 12,136,540 |
| Rejected general read | 14,360,078 | 12,136,521 |
| Retained empty value | 14,351,042 | 12,127,485 |

The retained step saves **9,055 bytes** (8.84 KiB), with no exceptional growth
allowance needed. Against deployed 0.68.5, the complete pending batch saves
**9,018 bytes**, improves/matches/regresses **368/344/0** handler cells and saves
up to **11.35%** in this matrix.

## Qualification and reproduction

Final candidate: all 80 core tests, 28 benchmark/model/codec/interchange tests,
strict core lint and formatting pass. The focused regression checks loading and
extracting lazy empty values without payload reads, nonempty neighbour reads,
replacement and reopening. Installed original Storage IO/upgrade/rollback passes
across 1,031 allocations, including malformed Wire admission and trapped updates.
The rejected trial separately passed the same test counts and installed checks;
correctness alone did not justify its performance tradeoff.

Reproduce with `BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage` and
the [boundary instrumentation guide](../../../qualification/ic-storage-benchmarks/README.md#iterator-coverage).
Raw evidence is under `target/evidence/storage-empty-reads/` (rejected) and
`target/evidence/storage-empty-values/` (retained). Frozen sources, inputs and
module identities are under `target/evidence/storage-comparison/attempt.*`.
Other optimizer profiles, the complete installed 73-type timing sweep and release
gates were not rerun. Package versions, Git history and publication are unchanged.
