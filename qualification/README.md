# Installed stable storage qualification

Two private packages exercise the current public library:

- `storage-canister`: 4,096 named allocations; owned B-tree maps for rows and index entries, plus a Cell over
  checked Storage leases. Ordinary init/post-upgrade rebuild transient owners.
- `storage-tests`: Testkit 0.110 installed tests for heap-replacing upgrades,
  failed post-upgrade rollback, query-write isolation, multi-root mutation and
  growth rollback, deletion and subsequent reuse.

Run `make storage-setup` explicitly when the selected server is not prepared,
then `make storage-installed-test`. Cargo uses the pinned compiler and lockfile and inherits the caller’s network settings. Server evidence remains under `target/evidence/testkit/`.
This does not migrate or qualify a deployed consumer's application schema.

Run `make storage-engine-matrix` to compare the owned Cell and B-tree with upstream on PocketIC. The [engine audit](ENGINE-MATRIX.md) covers behavior, instruction counts, heap use, stable-memory growth and Wasm size.

The [consumer benchmarks](CONSUMER-BENCHMARKS.md) compare IC DB, Auth, Wasm Store and Blob Storage against their locked Storage releases on PocketIC.

## Exact-key deletion baseline

`make storage-installed-test` compares individual `remove` calls and
`StableBTreeMap::remove_many` on fresh installations of the same release Wasm.
The fixture models the physical key count for an indexed parent and 31 or 255
indexed children. It seeds twice as many records as it deletes, with an 8 KiB
value every thirteenth key and 15-byte values elsewhere. Both paths construct
the same ordered set. The instruction interval covers set construction, removal
and the count check. IC DB admission, index validation and message reply are
outside this interval.

| Logical mutations | Physical keys | Selection | Point instructions | Bulk instructions | Peak additional heap bytes (both) |
| ---: | ---: | --- | ---: | ---: | ---: |
| 32 | 126 | Adjacent | 29,693,050 | 29,693,052 | 30,207 |
| 32 | 126 | Alternating | 44,322,289 | 44,322,291 | 30,207 |
| 256 | 1,022 | Adjacent | 270,550,511 | 270,550,513 | 48,587 |
| 256 | 1,022 | Alternating | 427,546,559 | 427,546,561 | 54,874 |

The 2026-11-24 run used Rust 1.100.0, ic-stable-structures 0.7.2 and PocketIC
through ic-testkit 0.110.4. Wasm linear memory grew by zero pages in every
measured deletion. The qualification-only allocator measures the peak live
allocated bytes above the pre-set baseline. Both instruction columns include
that instrumentation. The released tree traverses once per key in both paths.
A future engine can run the same fixture against this baseline. [Storage issue #10](https://github.com/dragginzgame/ic-storage/issues/10)

## Historical 0.79 engine comparison

On 2026-11-25, the patched 0.7.2 tree reused the loaded leaf for successive
requested keys while node occupancy allowed. Qualification compared point and
bulk deletion on identical Wasm. Each case checked every surviving record after
upgrade, the retry count and byte-identical stable memory after a forced trap.

| Mutations | Selection | Point instructions | Leaf batching | Leaf batching with discarded payload reads skipped | Peak additional heap bytes (all paths) |
| ---: | --- | ---: | ---: | ---: | ---: |
| 32 | Adjacent | 29,694,060 | 11,363,930 | 11,358,189 | 30,207 |
| 32 | Alternating | 44,323,299 | 35,591,190 | 35,601,818 | 30,207 |
| 256 | Adjacent | 270,559,711 | 89,394,277 | 89,349,386 | 48,587 |
| 256 | Alternating | 427,555,759 | 346,714,471 | 346,781,818 | 54,874 |

The point column uses the retained leaf-batching build. The payload-skipping
variant was measured in a separate build. It changed bulk instructions by less
than 0.06% in either direction, so the engine retains ordinary entry removal.
All deletions grew Wasm linear memory by zero pages.

### Packaged 0.79.0 engine

The release uses slice-based I/O and explicit header encoding. Its standalone
engine produced the following PocketIC instruction counts on the same fixtures.

| Physical keys | Selection | Individual | Bulk |
| ---: | --- | ---: | ---: |
| 126 | Adjacent | 29,098,944 | 11,263,511 |
| 126 | Alternating | 43,576,000 | 35,075,938 |
| 1,022 | Adjacent | 263,486,963 | 88,222,596 |
| 1,022 | Alternating | 419,150,394 | 340,755,454 |

Peak extra heap matched the preceding table and Wasm linear memory growth was
zero pages in all eight runs.

## Owned B-tree hard cut

The same PocketIC workloads exercise the degree-six B-tree with up to 32
requested removals per group of staged writes. Every run reported zero Wasm
linear memory growth.

| Physical keys | Selection | Individual instructions | Bulk instructions | Individual peak extra heap | Bulk peak extra heap |
| ---: | --- | ---: | ---: | ---: | ---: |
| 126 | Adjacent | 11,067,339 | 8,869,491 | 12,758 B | 13,487 B |
| 126 | Alternating | 13,167,631 | 10,941,940 | 13,473 B | 15,486 B |
| 1,022 | Adjacent | 88,215,499 | 67,951,593 | 21,881 B | 23,153 B |
| 1,022 | Alternating | 106,485,827 | 83,656,481 | 22,511 B | 25,138 B |

### Bounded key ranges and shared reads

`make storage-read-paths` builds matching release Wasms with and without the shared-read experiment, then installs the combined comparison in PocketIC. Each map holds 512 even-numbered keys with 16-byte inline or 4,096-byte external values. Each read runs as an update, so the stable-memory comparison after each group can detect writes. Tests run before and after reopening. The measured region includes routing, payload decoding, instrumented backing reads and the same byte-for-byte result checks in both multi-key arms. It excludes preparing the sorted distinct input and sending the reply. The counters count 20-byte node and external-value record headers on this fixture. [Full Storage measurements](read-paths.tsv)

After reopen, 32 clustered inline probes took 825,072 instructions with repeated point reads and 176,985 with shared traversal (90 versus 9 routing nodes). Scattered inline hits took 911,759 versus 448,440 (90 versus 38 nodes). Scattered misses took 920,738 versus 452,892 (96 versus 43 nodes). With external values, clustered hits took 5,163,303 versus 4,527,735, with 32 external reads in each arm. Shared traversal retained ancestor routing buffers and yielded one value at a time. Peak additional heap on those external hits grew from 4,096 to 5,853 bytes. The single-key control uses the point path. The feature-enabled Wasm grew from 194,526 to 196,852 bytes. The callback remains available for experiments through Storage's `test-support` feature.

For a 32-key range over external rows after reopen, ordinary iteration took 2,044,970 instructions and read 32 external records. Key-only iteration took 639,848 instructions and read zero external records. Both read nine routing nodes.

#### IC DB count and threshold comparison

Two builds from a disposable IC DB v0.31.0 checkout used the same count-profile fixture and Rust 1.100.0. For reproduction, apply the [driver and threshold probe patch](ic-db-read-profile.patch) to a disposable checkout and run `make m2-count-profile`. For the second build, patch the qualification workspace's crates.io dependency on Storage 0.80.0 to this product crate and change the single `db.map.range` call in `count_keys` to `db.map.keys_range`. Use a separate evidence directory through `IC_DB_COUNT_EVIDENCE_DIR`. The driver correction reflects the current inclusive 512-candidate budget. The explicit 31-candidate count and 7/8-candidate threshold arms exercise refusal and exact-budget success. [All 180 paired observations](ic-db-read-paths.tsv)

With 512 rows after reopening, the automatic 511-row primary range count fell from 3,564,098 to 3,207,506 work instructions (10.0%). Database opening cost another 553,393 and 553,292 instructions respectively. The eight-hit threshold on that range fell from 170,854 to 165,210 work instructions (3.3%). A seven-candidate budget returned the same limit error in both builds. The validated count stayed at 6,431,875 work instructions. The driver checked every result and backing snapshot. The uncompressed count-profile Wasm shrank from 403,654 to 403,178 bytes. The baseline SHA-256 was `d5d3e084d987a874b4280737847072cf85141f320ba9ce9820fcf1434f0cb7eb` and the candidate was `f108c885e1965b718fcf1a4585beca78ff6e396b7b8e9ad4f2cf690606c847e3`.
