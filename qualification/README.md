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

### Borrowed comparisons during B-tree search

`make storage-engine-matrix` includes the `compare_stored_keys` installed test. Each map contains 256 even keys with a common prefix and a big-endian numeric suffix. Two codecs reopen the same tree, first decoding each pivot into an owned byte vector and then comparing borrowed slices. Both arms verify returned values and byte-for-byte backing preservation after update calls. The point workloads contain 256 probes each. The seeks start at 73 different bounded ranges and take one entry from either end.

| Key width | Operation | Decoded instructions | Borrowed instructions |
|---|---|---:|---:|
| 16 bytes | Hits | 6,566,426 | 5,739,203 |
| 16 bytes | Misses | 7,033,920 | 6,081,669 |
| 16 bytes | Forward seeks | 2,416,467 | 2,147,989 |
| 16 bytes | Reverse seeks | 2,392,717 | 2,136,587 |
| 512 bytes | Hits | 51,719,481 | 49,982,622 |
| 512 bytes | Misses | 56,257,540 | 54,312,539 |
| 512 bytes | Forward seeks | 20,567,446 | 20,002,048 |
| 512 bytes | Reverse seeks | 20,069,163 | 19,533,733 |

The IC DB comparison used two installed v0.31.0 fixtures with the bounded count change described above in both builds. The baseline used Storage v0.80.1. The candidate used the same Storage source with the comparator change, plus a single `StoreKey::cmp_encoded` override returning `self.0.as_slice().cmp(stored)`. Reproduce using IC DB's `m2-bulk-selection` and `m2-listing-costs` workloads. Both Wasms passed the same four ignored `listing_costs` tests with Rust 1.100.0.

| IC DB workload | Released comparator | Borrowed comparator |
|---|---:|---:|
| Projected read, 32 distinct hits, 64-byte note | 2,122,557 | 1,978,109 |
| Projected read, 32 positions with duplicates and misses, 64-byte note | 1,242,458 | 1,159,853 |
| Three full-row ULID pages, sum of query instructions excluding open | 690,162 | 677,126 |
| 440 returned rows over 23 pages, listing instructions | 86,070,863 | 82,079,073 |
| Uncompressed fixture Wasm, bytes | 1,254,753 | 1,254,499 |

Baseline Wasm SHA-256 was `ef10d46b5625da9e08280339adf295382cd1eb7b5c6be6e20ea35dedc4b87518`. Candidate SHA-256 was `785764ff46e00184beccf3ebdc1f7130a77c7d3e504b0f9f3eeb35a59a3309f7`. Custom codecs can opt in when comparison honors their decoding checks and ordering. Signed and little-endian numeric keys retain typed ordering through the default implementation.

### Inclusive range seeks that match a branch key

The cursor can yield an inclusive endpoint immediately when it finds that key in a branch. The next iterator step visits the child on the requested side. The installed `compare_stored_keys` fixture probes a singleton range at each of the 256 stored keys in both directions. It verifies the key returned and compares the full stable backing after the measured calls. Both builds include borrowed key comparison. The table gives total instructions for the borrowed byte-vector arm.

| Key width | Direction | Before | After |
|---|---|---:|---:|
| 16 bytes | Forward | 7,576,524 | 7,309,691 |
| 16 bytes | Reverse | 7,600,638 | 7,311,238 |
| 512 bytes | Forward | 74,187,810 | 71,708,431 |
| 512 bytes | Reverse | 74,696,253 | 71,662,321 |

In the same installed IC DB v0.31.0 comparison, 32 separate projected `.eq(ID, id).first()` queries with 32 hits and a 64-byte note fell from 3,420,240 to 3,337,280 instructions after database open. Projected `get_many` on those inputs stayed at 1,978,109 instructions. The 440-row listing over 23 pages moved from 82,079,073 to 82,079,165 instructions. The measurement Wasm grew by 17 bytes. Reproduce the Storage comparison with `make storage-engine-matrix` and the IC DB comparison with the listing and bulk-selection workloads above.

### Full regression comparison against 0.80.1

Run `make storage-compare-release BENCHMARK_BASE=v0.80.1` with the local Testkit server prepared. This builds the released library and the working tree with the same Rust benchmark sources, runs the installed engine matrix, read paths and lifecycle suites, and writes matched measurements and the six logs under `target/evidence/release-comparison.*`. A different base must support the qualification APIs and Cargo targets exercised by the current fixtures. Inspect the case deltas even when every correctness assertion passes.

The comparison below covers the combined borrowed-comparator and inclusive-seek changes against commit `497fe5c57eb3f8050caae221dfdb97a6666f5af4`. The primary engine driver runs each fixture twice on fresh installs and checks identical measurement replies. Positive deltas in the [complete paired data](regression-0.80.1.tsv) mean the candidate consumed more. Counts below give equal weight to individual observations. The IC DB profile counts include separately measured opening and execution. They are not a traffic-weighted application speedup.

| Instruction observations | Faster | Equal | Slower | Largest increase |
|---|---:|---:|---:|---:|
| Storage engine, 431 cases | 258 | 135 | 38 | 2.46% |
| Upstream reference control, 431 cases | 0 | 431 | 0 | 0 |
| Short/long comparator and seek probes | 24 | 0 | 0 | 0 |
| Point, shared, and range reads | 96 | 0 | 0 | 0 |
| Installed deletion workloads | 8 | 0 | 0 | 0 |
| IC DB listing fixture, existing StoreKey | 15 | 0 | 170 | 1.15% |
| IC DB listing fixture, borrowed StoreKey | 185 | 0 | 0 | 0 |
| IC DB count fixture, existing StoreKey | 0 | 72 | 522 | 0.63% |
| IC DB count fixture, borrowed StoreKey | 480 | 72 | 42 | 0.63% |
| Auth and Blob Storage | 0 | 10 | 0 | 0 |
| Wasm Store | 0 | 1 | 26 | 0.34% |

The largest Storage increase was 256 lookups of `(u16, u32)` keys with 64-byte array values, from 5,902,800 to 6,048,235 instructions. Explicitly inlining the default comparator in a separate test build produced the same result. The next increase was 0.61% for `u128` keys with string values. The remaining 36 increases were each below 0.13%. No measured Storage case increased its peak working heap or page growth. No read-path probe increased its routing-node or external-payload reads.

Three IC DB builds used the same disposable v0.31.0 sources, bounded count path, threshold probes and lockfiles. The release arm used Storage 0.80.1. The default and opt-in arms used the candidate with either the original StoreKey codec or its borrowed comparison. All four installed listing/bulk tests and the count-profile driver passed in each arm. The 32-hit projected query with a 64-byte note rose from 2,122,557 to 2,137,257 instructions with the existing codec and fell to 1,978,109 with opt-in. The 42 increases remaining in the opt-in count suite include raw-map controls with their own codec. The fixture Wasms shrank by 557 bytes for listing and 272 bytes for count with opt-in.

Auth, Blob Storage, and Wasm Store used the instrumented disposable fixtures from the [consumer benchmark](CONSUMER-BENCHMARKS.md) with the Storage dependency switched between arms. All installed drivers passed. Wasm Store's eight distinct 1 MiB puts together cost 0.0016% more, and reading the entire 8 MiB cost 0.0006% more. The initial build showed a 9,188-byte Wasm increase. A repeat using identical source paths and the same Cargo target produced 708,700 versus 718,261 bytes (+9,561, or 1.35%). Both repeats preserved the recorded stable-page counts. Raw data distinguishes the repeat from the first build. These consumer tests measure installed fixtures, not production traffic or mainnet latency.

The candidate Rust source fingerprint is `0e03cd6f47edc19a0f5291e000278eaf0d3be2a7b5a25d62ef997f37950dfe29` (SHA-256 of sorted product source paths, each followed by NUL and its binary SHA-256). The matched-path Wasm Store artifact SHA-256 values were `9db797f2b8b00c5d50c9fad091a4b9c7043cc3eb8e5b9f95e9ae394aad3c2020` for the baseline and `772dc7006f607e473fbb9861ff4f367e64b6d949b48224c9117102f2f3efc30e` for the candidate.

### Issue follow-up on the 0.80.1 baseline

Each isolation build starts at tag `v0.80.1` and adds only the product change named below. The baseline and isolation builds use identical qualification sources. The earlier combined comparison above remains the record of the comparator and inclusive-seek experiment.

**Sorted visits (#12).** Both `visit_sorted_keys` and `try_visit_sorted_keys` are available with default features. The latter stops on the first visitor error and returns it to the caller. The caller owns the input bound, positional grouping, row decoding and projection. The native test rejects before reaching a deliberately corrupted later external payload and checks the backing contents. All 431 ordinary Storage engine instruction measurements matched the release exactly in the visitor-only build.

The IC DB v0.31.0 installed Document fixture was built with the [fallible consumer change](ic-db-fallible-visitor.patch) and visitor-only Storage. The default-feature build passed all 12 positional comparisons against the driver's oracle. Compare using `make m2-bulk-selection` in the disposable IC DB checkout used for the full regression comparison. The timed region includes grouping, validation, projection and result assembly after opening the database. With a 64-byte unselected note:

| Projected workload | Released point reads | Fallible shared visit |
|---|---:|---:|
| 1 hit | 70,626 | 70,404 |
| 8 distinct hits | 510,079 | 241,323 |
| 32 distinct hits | 2,122,557 | 773,539 |
| 32 positions with duplicates and misses | 1,242,458 | 584,031 |

The single-key arm uses ordinary point lookup in both builds. The complete IC DB measurement Wasm grew from 1,255,441 to 1,264,882 bytes (+9,441), including the consumer grouping change.

**Positioned lookahead (#14).** Run `make storage-lookahead`. Each installed update consumes eight full entries, then consumes one key by advancing the entry iterator, starting a bounded key-only seek, or advancing the positioned iterator with `next_key()` or `next_back_key()`. Each arm checks the key or exhaustion, external records read and unchanged backing. Native tests interleave front and back key-only and full-entry reads across node boundaries and through exhaustion. [All 24 installed measurements](positioned-lookahead.tsv)

On the forward 1 KiB successor, ordinary lookahead took 13,667 probe instructions and read one external payload. A fresh key seek took 12,986 and read two routing records. A positioned key step took 838 and read neither record type. Total page-plus-probe instructions fell from 533,676 to 520,711. The reverse probe fell from 14,316 to 1,514 instructions. Exhaustion added eight instructions versus ordinary lookahead in both directions in this fixture. The uncompressed matched measurement Wasm grew by 178 bytes when the positioned-key arm was included. In the lookahead-only Storage build, 91 of 431 engine cases improved, 337 tied and three increased; the largest increase was 0.0191%.

**Comparator attribution (#13).** Applying only the borrowed comparator to 0.80.1 reproduced the 2.46% increase in tuple-key point lookup (5,902,800 to 6,048,235 instructions) and the 0.61% increase in u128-key point lookup. The four-way installed Wasm Store test used identical source paths and the same locked upload/read/upgrade driver throughout.

| Wasm Store build | Uncompressed bytes | Difference from release |
|---|---:|---:|
| Release | 708,695 | 0 |
| Comparator only | 718,213 | +9,518 |
| Inclusive range seek only | 708,708 | +13 |
| Comparator and inclusive seek | 718,242 | +9,547 |

These measurements support delivering the sorted visitor independently while investigating the default comparator regressions.

Both isolated API builds passed the installed IC Auth, Blob Storage and Wasm Store drivers. The visitor-only build matched all 37 measured sibling instruction counts. The lookahead-only build matched the ten Auth and Blob observations. Both installed Storage lifecycle tests passed on each isolated build. In the full working tree, the release comparison found eight instruction increases among the 431 ordinary engine cases, with the tuple-key case still the largest; all 96 read-path comparisons and all eight deletion measurements had zero increases.

### Positioned lookahead against published 0.80.2

This comparison isolates issue #14 on tag `v0.80.2` (`e7c46dcd521b13ed7e4112926477a111a2734870`). The only product change is the positioned key advance in the entry iterator. Three installed IC DB builds used identical paths, release settings and fixtures. The first used 0.80.2. The second included #14 but still used `entries.next()` to check for a continuation. The third changed that one probe in `Db::page_typed` to `entries.next_key().is_some_and(|key| key.starts_with(&prefix))`. The default comparator and all other read paths came from 0.80.2.

The installed handler calls `Db::page::<BinaryObject>` and returns the actual rows and continuation. Each map holds 24 rows. Three payload distributions cover 8 bytes per row, 1 KiB per row, and 8 bytes except for 1 KiB on rows 9 and 17. Page limits are 1, 8, 16 and 32. The host follows each returned token, checks every ID and payload byte, verifies the exact terminal token and explicitly probes past the final row. It compares full stable backing around each set of update calls. All 204 page observations passed in every arm, before and after same-build upgrade. A fourth run seeded the baseline canister and upgraded it directly to the adopted candidate. It returned identical results and reproduced the corresponding fresh and reopened instruction pairs on all 204 observations. [Matched page measurements](ic-db-page-lookahead-0802.tsv)

Post-open page instructions after upgrade, with eight rows requested:

| Workload | Released | API present, old probe | Positioned probe | Change vs release |
|---|---:|---:|---:|---:|
| First page, 8-byte payloads | 118,403 | 117,818 | 117,157 | −1.05% |
| First page, 1 KiB payloads | 360,177 | 359,592 | 332,893 | −7.58% |
| Resumed page, 1 KiB payloads | 354,070 | 353,485 | 326,786 | −7.71% |
| First page of mixed rows, 1 KiB successor | 144,105 | 143,520 | 117,086 | −18.75% |
| Three pages, all 24 values of 1 KiB | 1,031,771 | 1,030,016 | 975,789 | −5.43% |

Including database open, the first full page of 1 KiB rows went from 547,188 to 519,904 instructions (−4.99%). For the partial 24-row page with limit 32, the two candidate builds matched exactly on post-open work in all three reopened fixtures. That path finishes before the full-page probe. In the 0.80.2 Storage microbenchmark the external successor cost 13,667 instructions with the ordinary forward probe, 12,986 with a fresh bounded seek, and 838 with the positioned key step. Only the first loaded the external payload. The standalone benchmark Wasm grew by 247 bytes when the positioned measurement arm was enabled. The IC DB Wasms were 1,269,474, 1,269,234, and 1,269,366 bytes respectively; adopting the consumer call added 132 bytes relative to the unused API and saved 108 bytes relative to the release build.

The complete installed Storage matrix had 91 improvements, 337 ties and three increases among 431 engine instruction comparisons. The largest increase was 335,094 to 335,158 instructions on the 16-byte-key range (+0.0191%). All 431 upstream controls matched. None of the 24 comparator/seek probes, 96 read-path probes or eight deletion measurements increased in instructions. Native mixed-direction and corrupted-value tests, Clippy and installed upgrade/rollback tests passed. Auth and Blob Storage had identical measured instructions and Wasm sizes. The Wasm Store upload/read/upgrade fixture passed, with its largest instruction increase at 0.0067% and its Wasm 522 bytes larger. [Paired Storage matrix](lookahead-0802-storage-matrix.tsv)

For reproduction, apply the [IC DB qualification patch](ic-db-primary-page-lookahead.patch) to the IC DB working source (HEAD `ba9accc4f0a73cfbd2ae97577e0af99c79522ff7`; SHA-256 of the pre-probe tracked diff `8b60a12294432d01ca4a6cbc609ea09532886c6184f646858fa887a0d3587315`). For each arm, build `make m2-sort-costs-artifact` and run the `listing_costs` driver test `primary_page_lookahead` with `--ignored --exact` under the prepared PocketIC server and `IC_DB_M1_RECORDS_WASM` set to that artifact. Set `IC_DB_PAGE_PREDECESSOR` to the baseline artifact for the cross-version run. The Storage suites were `storage-engine-matrix`, `storage-read-paths`, `storage-installed-test`, and the three-arm `storage-lookahead` probe.

### Borrowed key comparison against v0.80.3

Three builds used the same installed engine matrix and Wasm Store fixture. The baseline used v0.80.3. The second build added a default comparator method and the third used an optional associated constant, `Storable::CMP_ENCODED`. Both candidates specialized byte vectors. This comparison isolates the comparator from the inclusive seek change.

| Measurement | v0.80.3 | Default method | Optional constant |
| --- | ---: | ---: | ---: |
| 16-byte key hit, instructions | 7,736,074 | 7,172,897 | 7,027,974 |
| 256-byte key hit, instructions | 30,978,403 | 29,196,106 | 29,051,183 |
| Tuple key hit, instructions | 5,902,800 | 6,048,235 | 5,902,800 |
| u128 key hit, instructions | 5,844,356 | 5,879,899 | 5,844,356 |
| Wasm Store fixture, bytes | 709,224 | 718,741 | 709,234 |

Across 431 owned-engine rows, the optional constant reduced instruction counts in nine, matched 418 and increased four. The largest increase was 0.118% on the text-key/optional-value hit.

In an isolated IC DB v0.31.0 fixture, its bounded `StoreKey` opted into slice comparison. Both builds passed all five installed `listing_costs` tests. The 440-row listing took 85,733,651 instructions at baseline and 81,343,795 with the comparator. Both returned 23 pages. Indexed top-20 fell from 11,062,032 to 10,517,142 instructions with 87 rows examined and fetched in each run. The candidate Wasm was 132 bytes smaller.

#### UTF-8 string keys and combined working-tree results

All four instruction increases in the optional-constant matrix above occurred on `String` keys with `Option<u32>` values. The string comparator validates the borrowed pivot with `str::from_utf8` before comparing it. Invalid UTF-8 still traps. On the same v0.80.3 fixture, string inserts used 4.96% fewer instructions, hits 11.85% fewer, ranges 0.83% fewer and removals 3.75% fewer.

The following counts compare instructions against v0.80.3. The comparator-only build includes the byte-vector and string overrides. The combined build also includes the inclusive range-seek change.

| Workload | Cases | Comparator only, faster / equal / slower | Combined, faster / equal / slower | Largest combined increase |
| --- | ---: | ---: | ---: | ---: |
| Owned engine | 431 | 13 / 418 / 0 | 29 / 325 / 77 | 0.065% |
| Upstream controls | 431 | 0 / 431 / 0 | 0 / 431 / 0 | 0 |
| Encoded-key probes | 24 | 12 / 0 / 12 | 16 / 0 / 8 | 0.150% |
| Read paths | 96 | 0 / 96 / 0 | 0 / 72 / 24 | 0.028% |
| Deletions | 8 | 0 / 8 / 0 | 0 / 8 / 0 | 0 |

In the combined build, the largest encoded-key improvement was 12.31% on a reverse inclusive seek over present 16-byte keys. The largest increase was the decoded-key control on 16-byte misses. Reproduce the combined comparison with `make storage-compare-release BENCHMARK_BASE=v0.80.3`. Its paired TSV retains the values for each case.

Installed sibling fixtures compared the combined build with v0.80.3. The IC DB fixture opted in for its bounded byte key. It returned the same 440 rows over 23 pages in 81,343,887 instructions (5.12% fewer) and its Wasm shrank by 115 bytes. Auth and Blob Storage each matched all five recorded instruction samples. Their Wasms shrank by one byte and matched in size, respectively. Wasm Store passed the upload, read and upgrade scenario. Of its 27 instruction samples, 25 increased and two tied. The largest relative increase was 17 instructions on prepare (0.013%), and the Wasm grew by 27 bytes.
