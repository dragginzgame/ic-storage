# Consumer storage benchmarks · 25 November 2026

IC DB is the strongest fresh-install candidate in this run. Wasm Store is the
most important workload for allocator tuning. Its upload savings nearly
disappear after publishing the artifact and reading it once, while its stable
footprint grows substantially. Auth and Blob Storage saw sub-1% changes on the
measured lifecycle operations.

The comparison uses each consumer’s working-tree code with its locked Storage
dependency, then the same code with a path dependency on the owned-engine
candidate. IC DB, Auth and Wasm Store were using 0.78.9. Blob Storage used
0.78.8. Candidate builds used the source identified in the earlier engine
matrix. The [raw measurements](consumer-benchmarks.json) include the consumer
source identities, Wasm hashes, instructions, stable pages and native I/O counts.

## IC DB

The records canister ran its own query instrumentation on PocketIC. Its listing
test checked results against SQLite. Bulk lookups checked every result,
including duplicates and misses. The largest listing returned 440 rows over
23 pages. These are different access patterns over the same B-tree with
bounded 512-byte physical keys and variable-width row and index values.

| Measured operation | Locked instructions | Candidate instructions | Change |
|---|---:|---:|---:|
| Paginate the 440-row listing | 105,287,704 | 84,383,295 | −19.9% |
| Indexed top 20, 87 rows examined and fetched | 12,554,014 | 10,930,812 | −12.9% |
| Sorted top 20, 100 rows examined and fetched | 2,151,681 | 2,364,795 | +9.9% |
| Project 32 present keys, 64-byte note | 2,812,051 | 2,069,171 | −26.4% |
| Project one present key, empty note | 65,293 | 66,612 | +2.0% |
| First ULID ID-only page, open / query | 170,883 / 250,357 | 186,296 / 238,481 | +9.0% / −4.7% |

Opening the database can consume the savings on short pages. For the first ULID
page above, open plus query rose from 421,240 to 424,777 instructions. Benchmark
the sorted scan and open-plus-first-page alongside the longer indexed listing
when changing node traversal or root admission.

The DB native count and copy tests passed on fresh storage. One native text-index
qualification test failed at its deliberate corruption-injection step. It opens a
map created with bounded `StoreKey` using unbounded `Vec<u8>` keys and the
candidate rejects the codec-bound mismatch. Port that fixture to a test codec
with the same bound before treating the DB suite as green. The failure is
independent of the installed listing results.

## IC Wasm Store

The installed Store accepted eight **distinct** 1 MiB chunks, resumed across a
same-engine upgrade, published and verified the 8 MiB artifact, reused it under
a second release, read it back byte-for-byte, removed both aliases and reopened
the tombstones. Timers bracket the real state operations, including application
digest checks. Wire request decoding and response encoding are outside them.
The existing public read endpoint verified all bytes. A timed query called the
same state read method for each chunk.

| State operation | Locked instructions | Candidate instructions | Change |
|---|---:|---:|---:|
| Put all eight chunks | 975,191,205 | 830,483,228 | −14.8% |
| Publish complete artifact | 1,373,151,265 | 1,441,627,171 | +5.0% |
| Read each chunk once | 711,869,695 | 779,798,260 | +9.5% |
| Sum of those three operations | 3,060,212,165 | 3,051,908,659 | −0.3% |
| Remove last alias and reclaim payloads | 145,972,389 | 100,039,822 | −31.5% |

After the first chunk the candidate occupied 39 physical stable pages and the
locked build occupied 141. After all eight they occupied **263 versus 141**,
or **16.44 versus 8.81 MiB**. The candidate allocated another 32 pages for each
additional 1 MiB chunk. Its 20-byte value frame pushes each chunk into a 2 MiB
block. This is workload-sensitive: sharing chunk hashes or retaining only a
partial artifact changes the result. The native deduplication fixture, for
example, grew fewer logical pages on the candidate.

For this consumer, prioritize a large-value allocation class that can hold a
1 MiB payload plus framing without rounding to 2 MiB. Measure those eight puts,
one publish and repeated reads together. Profile frame verification and payload
copies on the read path while preserving the Store’s digest checks.

## IC Auth and IC Blob Storage

| Consumer and operation | Locked instructions | Candidate instructions | Change |
|---|---:|---:|---:|
| Auth, seal live session and issuer state | 1,414,625 | 1,411,881 | −0.2% |
| Auth, reopen after successful upgrade | 1,613,181 | 1,603,333 | −0.6% |
| Auth, seal / reopen after logout | 1,369,022 / 1,556,671 | 1,365,710 / 1,546,823 | −0.2% / −0.6% |
| Blob, authorize 63 more records | 36,365,246 | 36,314,720 | −0.1% |
| Blob, release 64 records | 69,503,451 | 69,410,651 | −0.1% |
| Blob, reopen full ledger | 1,572,133 | 1,570,158 | −0.1% |

Auth’s installed stable-host test exercised a live session, completed issuance,
a failed reopen with rollback, logout and recovery across two successful
upgrades. Both builds occupied six stable pages. The checkpoint is on the
upgrade path. These measurements do not establish an improvement to login or
signature verification.

The Blob probe called the actual `local::persistence::Service` against IC stable
memory. It used the full-capacity qualification workload with 64 ledger slots
and checked capacity refusal, reopen and drain. Both builds occupied four
stable pages. The native full-capacity test recorded 780 versus 260 logical
writes, but the same 781,125 bytes written. Encoding and persisting the bounded
ledger still occurs on each successful mutation. The measurement covers the
control service, excluding provider transfers and the public Wire endpoints.

## Release and verification scope

The owned format requires fresh allocations or a separately implemented and
verified migration. A direct upgrade of the 0.78.9-backed Wasm Store with an
upload in progress trapped in candidate post-upgrade. The test checked the
entire stable image byte-for-byte and resumed the predecessor with its original
upload status. A crate release also requires updating these consumers’ 0.78
dependency requirements. Production deployments and application data were
untouched.

Tests used Rust 1.100.0 release Wasm on a local PocketIC server. Each engine
used separate fresh installs. Store, Auth, Blob, and the DB bulk and ordered
listing tests repeated with identical counters. The additional DB membership
and ULID cases ran once per engine. Raw DB lines retain the individual fixture
fields and instruction paths. Store and Auth log samples contain the operation,
IC instructions and total physical 64 KiB stable pages. Store READ samples
contain the chunk index and instructions. Blob records give instructions and
physical pages. Native lease counters measure logical I/O separately from the
installed instruction measurements.

The DB fixtures were `projected_bulk_lookup_matches_full_and_fluent_reads`,
`ordered_listing_paths_match_sqlite_page_by_page`,
`measured_membership_and_empty_text_return_expected_counts` and
`ulid_time_listing_resumes_inside_a_millisecond`. Store used
`versioned_upload_resumes_across_two_executables_and_reuses_bytes` with
same-build reopen on both sides and a separate predecessor-upgrade refusal test.
Auth used `stable_auth_upgrade_and_failed_reopen_preserve_session_logout_and_certified_result`.
Blob used the service sequence from `full_capacity_reopens_and_cleanup_needs_no_new_slot`.
Tests used disposable copies of the local consumer sources. The results measure
these fixtures, with no weighting for production traffic or claims about mainnet latency.

### Per-map chunk allocation follow-up

On the candidate with configurable map sizing, two fresh runs per configuration
exercised the same Wasm Store lifecycle above. The only consumer change was
setting `allocation_unit_bytes: 257` when creating its chunks map. The releases
and artifacts maps used default sizing. The Store recovered the chunk policy
via its ordinary load path after an upgrade partway through the upload.

| Eight distinct 1 MiB chunks | Default (256 B) | Chunk map (257 B) |
|---|---:|---:|
| Physical stable pages after first put | 39 | 23 |
| Physical stable pages at complete publication | 263 | 135 |
| Instructions for all puts | 830,488,786 | 830,352,070 |
| Instructions for complete publish | 1,441,628,175 | 1,441,148,761 |
| Instructions for all reads | 779,799,372 | 779,802,396 |

Both installations returned the complete artifact byte-for-byte, recovered
after upgrade and passed deletion and tombstone checks. The 128-page saving is
8 MiB across the eight chunks. The instruction difference on each aggregate
above was less than 0.04%.

## V1 sibling compatibility check

The Toko Tooling 0.46.4 working tree supplies V1 Cell and B-tree storage for
the four direct consumers on fresh backing. Canic's sharding registry also
requires a three-component record codec. Storage now supports that row, and a
regression test inserts and reopens the actual
`(u32, [u8; 32], [u8; 30])` value under a 32-byte key.

| Consumer | Check | Scope |
|---|---|---|
| IC DB | Installed ordered listing against SQLite | Pagination and index traversal on the V1 map |
| IC Auth | Installed stable-host upgrade and failed reopen | Issuer, live session, logout, checkpoint recovery and rollback |
| IC Wasm Store | Installed versioned upload and reuse | Resume across a same-format upgrade, publish, read, delete and reopen |
| IC Blob Storage | Installed standalone production canister | Caller replay, cleanup and recovery across upgrade |
| Canic | Wasm type-check with DB store, both stable Auth features, Blob, Storage and sharding | Current Canic with its published Auth API, with Storage redirected to V1 |
| Toko application workspace | Wasm type-check of workspace libraries including all seven application canisters | Locked published dependency sources with their Storage dependency redirected to V1 |

All substitutions and builds used disposable copies. The four installed tests
exercise specific workflows. The Canic and Toko checks cover compilation.
These checks do not exercise an in-place migration from previous storage.

Coordinated adoption still matters. Canic's current session adapter needs the
canister version argument with the working-tree Auth checkpoint API. Against
the working-tree DB and Canic APIs, Toko still uses the earlier `get_many`
terminal and sharding assignment signature. The locked-API Toko build passed.
IC DB's native corruption-injection fixture still needs to reopen with a key
codec matching the persisted `StoreKey` bound. Keep the production bound
check when porting that fixture.
