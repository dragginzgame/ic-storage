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
