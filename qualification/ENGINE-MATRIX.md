# Cell and B-tree audit · 25 November 2026

## Expanded type and allocation-boundary matrix

The expanded run covers 431 paired instruction comparisons, including all 203
node-default cases. Storage used fewer instructions in 339 cases, upstream in
77, and 15 tied. Every original case retained its outcome. The 228 additions
contributed 173 Storage wins, 47 upstream wins and eight ties. Each engine ran
twice per fixture on fresh installations with identical measurement replies.
[Full measurements and build identity](expanded-engine-matrix.json)

The following ratios divide Storage instructions by upstream instructions.
Each type fixture inserts 256 permuted keys, looks up all 256, reads a 64-entry
range, deletes 128 alternating keys and verifies the ordered survivors. Both
engines include key/value construction and equality checks in these timings.

| Key → value | Insert | Hit | Range | Remove |
|---|---:|---:|---:|---:|
| UTF-8 String → Option<u32> (Some) | 0.47 | 0.39 | 0.66 | 0.46 |
| (u16, u32) → 64-byte array | 0.52 | 0.59 | 1.03 | 0.48 |
| u128 → UTF-8 String | 0.59 | 0.54 | 0.69 | 0.60 |

A separate correctness check stored and retrieved `None::<u32>` in Storage.
Upstream 0.7.2 rejected the insert because its optional codec advertised a fixed
five-byte width and encoded None in one byte. The paired optional-value timings
use Some throughout. The driver verifies the exact upstream rejection separately.

| Adjacent payload sizes | Storage physical pages grown | Upstream physical pages grown |
|---|---:|---:|
| 32 → 33 B | 4 → 6 | 4 → 4 |
| 236 → 237 B | 6 → 8 | 5 → 5 |
| 1,004 → 1,005 B | 12 → 20 | 12 → 12 |
| 8,172 → 8,173 B | 68 → 132 | 70 → 70 |

These are cumulative physical 64 KiB pages grown over the same 512-row map
and Cell sequence. The first boundary crosses the inline limit. The others
cross a power-of-two block limit after adding the 20-byte frame. At 8,172 B
Storage already needed 44.87M instructions for a full values scan versus
20.92M upstream, even though its stable footprint was slightly smaller.
At 4,096 rows of 33 bytes, Storage used 46% of upstream instructions for ordered
inserts, 33% for alternating deletion, and 104% for the full values scan.

## Node allocation baseline

The second run adopts upstream’s 1 KiB node target for unbounded codecs and
its three-quarter sizing rule for bounded codecs. Storage applies the rule to
its own encoded node and caps the initial reservation at 1 KiB. Both trees
have degree six. The Cell has no analogous node sizing parameter.

| Workload | Previous Storage | New Storage | Upstream |
|---|---:|---:|---:|
| 256 ordered inserts, 16-byte values, instructions | 13,418,914 | 12,687,900 | 26,082,297 |
| Full scan of 512 values, 8 KiB each, instructions | 44,966,349 | 44,933,075 | 20,976,083 |
| 512-row sequence, 16-byte values, physical pages grown | 3 | 4 | 4 |
| 4,096-row sequence, 16-byte values, physical pages grown | 7 | 12 | 13 |
| 512-row sequence, 8 KiB values, physical pages grown | 131 | 132 | 70 |

All 203 paired cases passed with two fresh installations per engine and fixture.
Storage won 166 instruction comparisons, tied seven and lost 30. Larger node
reservations bring small-value footprint closer to upstream. Separate large-value
records still round up independently and account for the main footprint gap.
[Node-default measurements and Wasm identity](node-defaults-matrix.json)

## Original audit before the node sizing change

The original Storage B-tree used fewer instructions for every tested point lookup, insert, overwrite and deletion. Upstream had a material advantage in large-value scans and retained stable footprint. The owned Cell used fewer instructions for same-length replacement at every width. Its cached reads tied upstream.

Of 203 paired workload cases, Storage used fewer instructions in 166, upstream in 30, and seven tied. These are equally counted cases, not a workload-weighted speedup. Both engines passed the checked return values, survivor comparisons, reopen, upgrade and rollback witnesses. [Exact counts, heap, page growth and build identities](engine-matrix.json)

## Findings in priority order

| Priority | Finding | Consequence and next measurement |
|---|---|---|
| 1 | Large external values cost stable space. With 512 rows of 8,192 bytes, the map-and-Cell sequence grew physical stable memory by 131 pages for Storage and 70 for upstream. | The owned allocator rounds an external value plus its 20-byte frame up to a power of two. An 8,192-byte payload enters a 16,384-byte block. Measure size classes around page and power-of-two boundaries before changing the allocator. |
| 2 | Reading all 512 values at 8 KiB took 44.97M instructions for Storage and 20.98M for upstream. The 64-entry forward range took 5.65M and 2.82M. | Storage used just 10,292 additional live heap bytes for the full values scan versus 201,308 upstream. Profile the separate frame read, payload checksum and materialization before changing that memory tradeoff. |
| 3 | At the same width, deleting 64 alternating keys took 8.56M instructions and 16,170 additional heap bytes for Storage versus 72.25M and 262,280 bytes upstream. | Preserve the exact-key and retry regression tests. Storage stages groups of 32 writes. Its delete path still reads the previous payload before discarding it. Any discard-only optimization needs an explicit choice about maintaining that integrity check. |
| 4 | Key shape matters. The 64-entry range over 256-byte shared-prefix keys cost Storage 1.86M instructions versus 1.62M upstream. Shorter shared-prefix ranges favored Storage. | Add the long-key range to the scan optimization baseline. Numeric-key results alone overstate scan performance. |
| 5 | The owned map reopening and clearing paths cost more instructions in every primary fixture. At 16-byte values they took 34,991 and 9,553 versus 24,891 and 2,728. | Loading checks root framing, persisted bounds, cursor and free-list heads. It defers validation of routing and value records until access. Reopen success establishes header admission, not a complete image audit. |
| 6 | The 8-KiB Cell growth case took 696,922 instructions for Storage versus 377,834 upstream. | Both added one physical stable page. Storage also added two Wasm heap pages while upstream added zero. Prior map work leaves different heap high-water marks. Repeat with isolated Cells before attributing the difference to the Cell write routine. |

Implementation evidence includes [record framing](../crates/ic-storage/src/adapter/record.rs), [B-tree traversal, allocation and deletion](../crates/ic-storage/src/adapter/tree/mod.rs), [Cell framing](../crates/ic-storage/src/stable_cell.rs), [upstream B-tree](https://github.com/dfinity/stable-structures/blob/72b02ab65b6c6fbf53fbabf752732d2726539377/src/btreemap.rs) and [upstream Cell](https://github.com/dfinity/stable-structures/blob/72b02ab65b6c6fbf53fbabf752732d2726539377/src/cell.rs).

## Contract and correctness matrix

| Surface | Current Storage | Released upstream 0.7.2 | Evidence |
|---|---|---|---|
| Engine and backing | Degree-six B-tree, named allocations, single-owner lease. | Multi-entry B-tree on a supplied memory. | Both were given the same governed allocation implementation. No upstream MemoryManager was in the timed path. |
| Creation and reopen | Map and Cell creation require empty memory. Explicit reset replaces contents. Load refuses foreign framing. | Map offers new, load and init. Cell offers new and init. Init can write defaults on foreign magic. | Current foreign-map and foreign-Cell native tests, constructor source review, same-engine reopen measured on each side. |
| Data format | ICMAP003 map, STORCELL v1 Cell, 24-byte Cell header. | BTR map with versioned node layout, SCL v1 Cell, 8-byte Cell header. | Hard cut. The matrix creates separate fresh images and tests upgrades within each format. |
| Record codecs | Storage-owned trait, numeric types including signed integers, byte arrays, strings, byte vectors, options and pairs. | Separate trait with primitives, bytes, Principal, Reverse, bounded wrappers and tuple implementations. | Source review. Matching type names and operations do not establish encoding compatibility. |
| Bounds and resource admission | Map stores and checks key/value bounds on reopen. Frames have a 16 MiB payload ceiling. Cell verifies header version, extent and admitted length before allocation. | V2 map uses stored page size. Cell uses a u32 payload length and backing growth checks. | Source review. An unbounded Storage codec still encounters the map frame budget. Maximum-sized images were not run. |
| Record integrity | Map frames carry FNV-64 checksums. Node framing and pointer bounds are checked on access. Cell checks the envelope and application codec. | Header and node layout checks. Different allocation and overflow format. | Source review. Checksums address accidental corruption. Neither side received a comprehensive malformed-image campaign in this audit. |
| Reads and writes | Get, contains, insert, replacement, length, first/last, remove, pop, clear and reinsert. | Corresponding operations on the raw B-tree. | Both tested with deterministic records. The owned mixed-history test also compares against a standard ordered map. |
| Iteration | Keys, values, entries, bounded forward/reverse and mixed-direction traversal. External values can be omitted on key-only scans. | Corresponding traversals plus specialized range-key/range-value methods. | Timed forward/reverse and shared-prefix cases, fixed-width mixed-direction check, ordered full-pair verification. |
| Bulk deletion | Consumes a sorted distinct set, returns actual count, stages groups of up to 32. | The caller loops over the identical sorted set with point removal. | Adjacent and alternating keys, missing-key retry, full survivor values, empty-root/reinsert in the owned suite. |
| Capacity and reuse | Absolute reserve and allocated-pages APIs on the governed map. Exclusive lease is retained. | Raw upstream can return its backing via ownership transfer. | Source review for reserve and custody. Physical growth is measured per operation. Both clear and refill without reconstructing allocation authority. |
| Cell updates | Cached get; write payload then update header on length change. Set returns the previous cached value. | Cached get and set; upstream writes its Cell header and payload on replacement. | Empty through 8 KiB, same length, growth by 64 KiB, shrink and reopen. Full final payload checked. |
| Atomicity and lifecycle | IC message rollback covers staged data and allocation growth. Native backing is not transactional. | Same IC rollback boundary under the matched backing. | Both upgraded and verified all 64 final rows and the Cell, then trapped after deleting 32 rows and replacing the Cell. Whole physical stable memory matched byte-for-byte before and after the trap. |
| Collection coverage and dependency isolation | Product supplies Cell and B-tree. | Upstream also supplies Set, Vec, MinHeap, Log and MemoryManager. | Other upstream products have no owned counterpart in scope. Normal product and qualification-canister graphs exclude upstream; the comparison example uses the crate through dev-dependencies. |

## Instructions by operation and size

Each cell is **Storage instructions divided by upstream instructions**. Below 1.00 favors Storage. Ratios round to two decimals; exact ties and wins use unrounded counts. The six width columns use 512 live rows at peak. The final column uses 4,096 rows and 16-byte values. Batch totals include fixture assertions and payload construction where applicable.

| Operation | 0 B | 16 B | 32 B | 33 B | 1 KiB | 8 KiB | 4,096 rows |
|---|---:|---:|---:|---:|---:|---:|---:|
| map create | 1.02 | 1.02 | 1.02 | 1.02 | 1.02 | 1.02 | 1.02 |
| insert ordered | 0.45 | 0.51 | 0.58 | 0.47 | 0.34 | 0.25 | 0.51 |
| insert permuted | 0.46 | 0.52 | 0.60 | 0.49 | 0.35 | 0.26 | 0.55 |
| overwrite | 0.37 | 0.40 | 0.47 | 0.38 | 0.36 | 0.41 | 0.39 |
| hit | 0.44 | 0.52 | 0.60 | 0.55 | 0.53 | 0.60 | 0.59 |
| miss | 0.42 | 0.50 | 0.58 | 0.48 | 0.34 | 0.13 | 0.44 |
| contains | 0.39 | 0.46 | 0.56 | 0.47 | 0.33 | 0.12 | 0.50 |
| keys | 0.66 | 0.73 | 0.82 | 0.72 | 0.53 | 0.21 | 0.75 |
| values | 0.63 | 0.58 | 0.63 | 1.04 | 1.59 | 2.14 | 0.60 |
| range | 0.56 | 0.55 | 0.60 | 0.92 | 1.42 | 2.01 | 0.57 |
| reverse | 0.57 | 0.55 | 0.60 | 0.92 | 1.43 | 2.01 | 0.57 |
| endpoints | 0.46 | 0.54 | 0.62 | 0.58 | 0.52 | 0.55 | 0.52 |
| map reopen | 1.42 | 1.41 | 1.43 | 1.42 | 1.42 | 1.44 | 1.44 |
| delete point | 0.49 | 0.54 | 0.61 | 0.50 | 0.32 | 0.18 | 0.49 |
| delete adjacent | 0.35 | 0.43 | 0.48 | 0.38 | 0.26 | 0.17 | 0.43 |
| delete interleaved | 0.30 | 0.36 | 0.42 | 0.31 | 0.20 | 0.12 | 0.37 |
| retry | 0.30 | 0.33 | 0.37 | 0.32 | 0.20 | 0.06 | 0.41 |
| pop ends | 0.60 | 0.66 | 0.73 | 0.63 | 0.44 | 0.26 | 0.41 |
| reinsert | 0.46 | 0.52 | 0.61 | 0.49 | 0.32 | 0.19 | 0.53 |
| clear | 3.67 | 3.50 | 3.73 | 3.63 | 3.65 | 3.42 | 3.46 |
| refill | 0.46 | 0.52 | 0.60 | 0.48 | 0.32 | 0.21 | 0.52 |
| cell create | 0.99 | 0.99 | 1.00 | 0.99 | 0.99 | 1.00 | 1.00 |
| cell get | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| cell replace | 0.16 | 0.45 | 0.45 | 0.45 | 0.69 | 0.93 | 0.45 |
| cell grow | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.84 | 1.00 |
| cell shrink | 0.50 | 0.68 | 0.66 | 0.70 | 0.83 | 0.94 | 0.68 |
| cell reopen | 0.96 | 0.96 | 0.96 | 0.97 | 0.98 | 0.99 | 0.98 |

## Key and codec matrix

The fixed-width cases use u64 keys and u64 values. The byte-key cases use 256 distinct keys, all sharing a long prefix, and 32-byte values. Format construction, comparator cost and key decoding are included.

| Operation | Storage instructions | Upstream instructions | Storage / upstream | Peak heap bytes, Storage / upstream |
|---|---:|---:|---:|---:|
| fixed insert | 25,567,217 | 45,876,717 | 0.56 | 4,716 / 4,424 |
| fixed get | 10,533,398 | 21,722,476 | 0.48 | 949 / 1,808 |
| fixed overwrite | 15,786,650 | 37,735,616 | 0.42 | 3,446 / 1,904 |
| fixed range | 185,982 | 295,484 | 0.63 | 3,972 / 3,808 |
| fixed remove | 17,988,527 | 33,205,883 | 0.54 | 4,919 / 4,456 |
| key16 insert | 15,976,606 | 28,955,005 | 0.55 | 5,732 / 4,760 |
| key16 hit | 7,719,234 | 15,912,913 | 0.49 | 1,049 / 1,920 |
| key16 range | 334,879 | 489,615 | 0.68 | 2,317 / 2,236 |
| key64 insert | 21,481,212 | 34,294,580 | 0.63 | 7,748 / 5,880 |
| key64 hit | 12,277,830 | 17,921,069 | 0.69 | 1,657 / 2,352 |
| key64 range | 623,454 | 705,054 | 0.88 | 3,469 / 2,988 |
| key256 insert | 45,584,477 | 58,883,811 | 0.77 | 16,578 / 11,224 |
| key256 hit | 30,961,610 | 36,047,969 | 0.86 | 4,153 / 4,656 |
| key256 range | 1,864,636 | 1,616,954 | 1.15 | 8,077 / 6,508 |

## Stable footprint and Wasm size

Cumulative physical stable growth in 64-KiB pages during the primary timed map and Cell operations follows. This excludes common host bootstrap and the separate key-shape fixture. These counts include allocator and directory effects and measure allocated pages, not live record bytes.

| Value width | 0 B | 16 B | 32 B | 33 B | 1 KiB | 8 KiB |
|---|---:|---:|---:|---:|---:|---:|
| Storage | 3 | 3 | 4 | 5 | 19 | 131 |
| Upstream | 4 | 4 | 4 | 4 | 12 | 70 |

The two footprint builds compiled the same operations, assertions, entrypoints and allocation host, each with only one reachable engine. These are fixture sizes. A production application will retain a different set of functions.

| Release Wasm | Complete module bytes | Code-section payload bytes |
|---|---:|---:|
| Owned | 317,696 | 258,532 |
| Reference | 298,891 | 237,913 |
| Both | 442,204 | 365,025 |

## Reproduction and interpretation

Run `make storage-engine-matrix` with the local PocketIC server prepared. The target builds a release Wasm example and runs its ignored installed driver with the qualification lockfile. The driver runs both engines in the identical Wasm on fresh installations and requires the complete measurement replies to match on two runs per engine and fixture. The fixed-size build comparison uses the same locked Cargo build command with `STORAGE_MATRIX_SIZE_ENGINE=owned` or `reference`. Rust 1.100.0, release LTO, one codegen unit and a 1 MiB stack were used throughout. [Fixture](storage-canister/examples/engine_matrix.rs) · [Installed driver](storage-tests/tests/engine_matrix.rs)

Each primary run inserts half the keys in order and half in a deterministic permutation, then overwrites the second half. Get, miss and contains each probe half the peak cardinality. Keys and values scan the full map. Directional ranges consume 64 entries each. Deletion phases operate on disjoint selections of 64 keys. Set construction is included in both grouped deletion timings. Cell get repeats half the peak cardinality; same-length replacement repeats 32 times. Open timings include acquiring the named lease.

The IC instruction counter brackets the operation. Peak heap is the additional live allocated bytes over each phase's starting baseline. Stable and Wasm linear memory growth are measured separately. Independent checkpoints compare ordered pairs including complete payload bytes before and after deletions and after refill. The two runs per fixture are a determinism check. They do not give a production latency distribution.

The report represents the uncommitted working tree based on d4da0d1, including the owned-engine hard cut. The data file identifies the exact product source digest, dual-engine Wasm and both footprint builds. The published 0.79.0 crate has a different engine. The largest map measured here held 4,096 rows. Mainnet latency, concurrent clients, multi-million-row trees, application-level IC DB validation, arbitrary codecs and exhaustive corruption testing were outside this matrix.
