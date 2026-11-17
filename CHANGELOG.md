# Changelog

## [0.76.1] - 2026-11-17

### ✨ Added

- Qualify ordered owners across separate messages, with explicit dual-allocation
  close/reopen, controlled transient startup, query isolation and failed
  source/target upgrade rollback. Reuse the scalar-session startup builder and
  requalify its installed lifecycle. Add constant-time ordered allocation usage
  observation, host relocation/poisoning tests and isolated lookup cost samples.
- Add optional `ordered` bounded byte-key/value storage for the R4 DB workload:
  allocator-owned AVL nodes, forward/reverse ranges, prefixes, key continuation,
  deletion/reuse, preflighted batches and sealed portable checkpoints. Qualify
  512-byte keys, 4-KiB rows and separate 16-KiB aggregate-job values on host,
  including provider-pool readmission and allocation-failure preservation.
  A closed installed 64-row/index profile now covers compatible upgrades,
  full-width cursor pagination, native/stable publication rollback and portable
  recovery in both allocation directions. DB cutover remains pending.
- Add optional `ic-storage::checkpoint` backing: byte-defined bounded records,
  fixed-width pages, exact layout admission and one-use region claims over
  caller-owned memory. Opening never initializes or repairs existing storage.

### 🔄 Changed

- Replace ordered nodes' maximum-sized inline arrays with 56-byte nodes and
  separately owned variable-size payload blocks. Preflight both allocations,
  scrub/reuse/split freed blocks and validate complete payload ownership on
  admission. Native format identity changes; logical `ORDMAP01` stays unchanged.
  The installed 64-row/index profile uses 54% fewer initialized native bytes.
  Coalescing and larger sustained-churn qualification remain open.
- Replace qualification's legacy Storage dependency with the product checkpoint
  backing and a Platform stable-memory binding. The new format does not import
  legacy bytes; IC message rollback owns multi-record/native publication atomicity.
  Remove the older transitive Platform runtime and `ic0` dependency.

### 🧪 Testing

- Add `ordered-native-probe` to CI and `ordered-native-test` for on-demand IC
  lifecycle qualification. Preserve source/artifact identities and entry-body
  instruction/transient-memory samples across two export/recovery epochs.
- Requalify scalar, record, vector, map and paged recovery through the owned
  backing, including durable jobs and failed-upgrade/publication rollback. Paged
  batches read one new page, write 616 progress bytes and reserve three stable
  pages; existing instruction and requested-allocation ceilings still pass.
- Retire the M0-versus-legacy-Storage benchmark, its runner, canister and startup
  profile. Keep ic-stable-structures as the only benchmark baseline and enforce
  that exclusion in the benchmark dependency graph. Preserve historical results.

## [0.76.0] - 2026-11-17

### 🔄 Changed

- Make `ic-storage` the native collection entry point and rename its optional
  procedural macros to `ic-storage-derive`. Move the unchanged M0 format into
  private `ic-storage-m0`. Only the main library and derive crate publish; the
  native API does not preserve the legacy 0.74 API or stored formats.
- Adopt Testkit 0.109.0 for qualification and its published server launcher,
  with explicit prepared-server and per-attempt evidence directories.

### 🧪 Testing

- Start reusable owner lifetime qualification with a private host service holding
  two admitted maps across separate calls. Verify 1,536 records per root, exclusive
  custody between calls, complete close before relocating the backing, and fresh
  admission afterward. Expected allocation refusal preserves prior writes; a
  caught unwind quarantines both owners and refuses publication.
- Add an installed generation-owned map service using the existing product Map
  and provider pool. Keep owners alive through separate messages; close both
  before upgrade and rebuild them after controlled startup clears transient
  headers. Verify 3,072 records, unchanged admission count across 32 lookups,
  explicit close/reopen, query-write isolation, complete retained-byte rollback,
  corrupt target refusal, failed source/target hooks and bidirectional upgrades.
  Add artifact and installed checks to CI. This trusted-source fixture is not
  yet the public runtime.
- Add an on-demand generation-owned map versus ic-stable-structures matrix:
  `make benchmark-matrix` and `make benchmark-matrix-smoke`. Measure nine operations
  across three populations, three batches, three key schedules and two sibling-root
  shapes using the same outer instruction meter. All 972 full-matrix samples pass
  complete two-root state checks; native admission stays at one. Save CSV, paired
  median Markdown tables, memory observations, artifacts and source/lock evidence.
  Native uses fewer instructions in 142 of 144 non-control cells; single inserts
  crossing the 1,536-record growth boundary remain about 45% more expensive.

## [0.75.6] - 2026-11-17

### 🧪 Testing

- Add an installed native-map versus ic-stable-structures 0.7.2 comparison with
  no legacy Storage backend. All 324 samples verify complete state across three
  populations, three batch sizes and three key schedules; another 54 samples
  isolate owner admission. Preserve the initial baseline and source/artifact
  evidence, including the remaining cost of owner reconstruction on each call.
- Reuse validated provider bounds for an exclusive native loan and keep both
  map owners alive through a bounded lookup/replace/delete/insert batch. Revoke
  loan authority at settlement and reject root/capacity/operation substitution.
  Installed late-batch failures roll back prior mutations and permit clean retry.
  At 1,536 records, 64-record replacement/deletion/reinsertion now costs less than
  stable structures; lookup and growing insertion cost 1.115× and 1.256×. Single
  lookup remains 4.05× because complete table admission still runs once per call.
  Upgrade, corruption, growth and paged recovery checks remain passing.
- Measure host native-pool occupancy at actual allocation points, including
  reserved/sealed roots and old/new rehash tables. Paged recovery of 4,608 records
  peaks at 172,032 bytes versus 147,456 bytes retained in a 196,608-byte pool;
  reset and regrowth reproduce the peak without further pool growth. Check
  resizing, failed allocation and fragmented-space refusal with unchanged
  owners. These host observations do not qualify installed native-pool peaks.
- Add a separate checkpoint-provider allocation meter that forwards requests to
  `System`, tracks live requested bytes and records a conservative realloc overlap
  bound. Calibrate allocation, zeroing, grow/shrink, failed requests, meter reset
  with live blocks, authority and trap rollback. Across 119 installed samples,
  the largest requested-byte peak is 197,841 bytes; large recovery batches peak
  at 100,567 bytes with a 116,617-byte realloc bound. Every sampled call returns
  to zero live checkpoint allocations and stays below the 256 KiB budget.
  Installed native-pool peaks and allocator-internal overhead remain unmeasured.
- Extend installed entrypoint cost measurement through cancellation, exact retries,
  replacement of completed/cancelled checkpoints, reverse-domain recovery, and
  shrinking/growing the governed page vector. Preserve per-shape TSV evidence and
  enforce the existing instruction/memory budgets and exact backing-write counts.
  All 115 additional samples pass: at 4,608 records, replacement costs about
  50 million instructions, cancellation about 23 million and reverse recovery
  peaks near 102 million per batch. Cancellation writes 104 bytes; exact retries
  write zero. Trimming and regrowth reuse physical stable pages without growth.
  Failed-call/whole management-upgrade costs remain open.

## [0.75.5] - 2026-11-17

### ➕ Added

- Connect the shared paged store to installed retained-map recovery. Verify
  native writes before committing continuation and progress, then publish both
  roots in the same IC message. Qualify compatible interruption, exact retries
  and explicit unfinished-target restart. The paged profile now admits 3,072
  first-root and 1,536 second-root records in native format 11, with sequential epochs
  and one page per call. Qualify same-root rehashing, partial final pages and
  72-page pair recovery. Reserve disjoint native windows and move
  checkpoint initialization/reset above them, enabling idle pool growth to
  192 KiB. Qualify growth rollback, retained occupied tails across upgrades,
  recovery into the separate target and ordinary growth refusal during
  frozen/private custody. Reserve each import page through the product map,
  ending both owners before expanding the private loan and retrying. Qualify
  combined growth/record/progress rollback, restart after corruption, empty-root
  variants and newer writes surviving consumed retries at 4,608 records.
  Live allocation peaks and application service budgets remain unfinished.
- Add controller-authorized paged source cancellation and repeated export/recovery
  epochs through the shared job state machine. Persist an explicit Ready envelope;
  reject unknown state, stale epochs and corrupt continuations before replacement.
  Reuse and trim the governed page vector, clearing prior continuation state in
  the export transaction. Qualify four installed epochs, three recoveries,
  reverse-domain reconstruction, cancellation/replacement rollback and completed
  retries preserving intervening application writes. No whole-checkpoint consume
  or production recovery authority is introduced.
- Observe continuation fingerprints directly from admitted scalar record scans,
  preserving native bucket order without reconstructing maps. Reuse the product
  portable encoder and keep existing checkpoint/continuation bytes unchanged.
  Validate job/cursor binding before scanning; completed retries need no scan.
- Share one governed paged-store owner between host qualification and an
  installed ordinary-upgrade probe. Verify page/manifest publication and
  continuation/metadata rollback through 4,608-record pairs, including traps
  after second-root writes and failed upgrades. This separate storage probe
  reconstructs transient maps; retained roots use the bounded native profile.

### 🔧 Changed

- Update the [delivery roadmap](docs/DESIGN.md#delivery-roadmap) from inspected
  sibling requirements. Define completion gates for larger installed recovery,
  application root custody, a Canic-profile-shaped record qualification, ordered
  collections, service adapters and separately authorized consumer cutovers.
- Move the paged job's target domain from 163840 to 393216 and reject the old
  overlapping domain. Update independent job/continuation golden bytes; their
  framing and logical checkpoint formats are unchanged.
- Adopt Testkit 0.108.0 in the qualification workspace. Keep the PocketIC-only
  dependency graph and validate installed retention and map recovery with the
  existing prepared 0.106.0 server launcher.
- Verify first-party registry freshness for this release: Runtime 0.23.16,
  Host Artifacts 0.80.3, Testkit 0.108.0 and Toko Tooling 0.44.11 match current
  stable, non-yanked index entries. Keep the prepared 0.106.0 launcher until the
  next tooling-maintenance boundary. The frozen Storage oracle remains 0.74.1,
  also the latest published Storage version. This experimental 0.75.5 release is
  Git-only; all product crates remain unpublished.

### ⚡ Performance

- Measure complete successful paged entrypoints and actual backing I/O in
  instrumented qualification copies. Preserve TSV rows and executable hashes;
  calibrate read/write/grow counts and rollback. Apply each page in one admitted
  native scope through a checked scalar staging callback, preserving full
  validation and atomic publication. The 4,608-record workload's worst batch
  falls from 1,412,361,148 to 101,728,323 instructions (13.9× lower), and its total
  falls from 51.07 billion to 3.57 billion. Reads/writes remain 6,656/616 bytes.
  Tighten the sampled-call ceiling to 150 million instructions, retaining 2 MiB
  Wasm memory and 50 stable pages. These are fixture budgets, not production
  throughput or allocator-peak evidence. See the
  [measured scope and results](docs/design/paged-map-checkpoint.md#complete-entrypoint-cost-evidence).
- Verify installed map-import prefixes directly through the scalar reader,
  without rebuilding transient native hash tables. Sorted checkpoint lookup and
  an eight-byte seen bitmap reject wrong, missing and duplicate records before
  staging and after native writes. Full checkpoint decoding and prefix scans
  remain; the `NKPAIR01` job's 64-record-per-root limit is unchanged. The shared
  verifier now takes a compile-time bitmap bound, using 384 bytes for paged
  native prefixes without allocating another native map or bitmap.

### 🧪 Testing

- Qualify the new scalar staging import's exact signature/name, sequential
  key/value/acknowledgement protocol, custody and bounds. Refuse invalid native
  batch arguments, unbound staging, oversized counts and wrong read order.
  Preserve the 2688-byte native stack bound and test two-import leaf accounting,
  cycle refusal and complete memory-guard coverage. Installed mid-insert traps
  now roll back while both native owners remain live.
- Compare scalar fingerprints with independent installed native-slot observations
  under frozen-source and private-import custody. Active roots refuse this query.
  Revalidate governed storage rollback with the observation-based continuation
  boundary. Add installed combined native/progress fault coverage through final
  root publication, authority rejection, corruption refusal and explicit restart.
- Cover every supported 64-record job prefix length, record position and duplicate placement.
  Inject duplicate native records after real insertion and verify that the
  post-apply verifier traps, preserving the prior heap and stable bytes across
  partial, root-crossing and completing batches.
- Add `make paged-store-test` and hosted CI coverage for independent page-chain
  checks, reopening, exact retries, controller rejection, corrupt records and
  missing-store refusal without replacement writes.
- Extend installed paged recovery through 192+192 records. Verify rehash rollback,
  later-page corruption, continuation past damaged consumed pages, and restart
  after root 1 is complete but both roots remain private. The subsequent large
  cohort recovers 3,072+1,536 records and refuses a 1,537-record second root before
  checkpoint publication. Preserve the old bounded job profile and check large
  prefix bitmap boundaries, distant duplicates and oversized input refusal.

### ▶️ Try it

With the documented dependencies and PocketIC server already prepared, run
`make native-map-paged-test` for installed growth, recovery, cancellation,
repeated epochs and cost evidence. `make native-map-page-test` checks the shared
governed store on the host; `make paged-store-test` checks its installed rollback.

## [0.75.4] - 2026-11-17

### ➕ Added

- Bind two paged map roots to one epoch and a fixed 328-byte pair manifest.
  Export returns the manifest only after both page chains succeed; recovery
  keeps both candidates private until all pages validate. Current installed
  jobs retain their existing checkpoint format and limits.
- Reuse the shared migration job for paged-map progress, exact retries and
  explicit unfinished-target restart. Governed host batches keep the manifest
  unchanged and write nothing on identical retries. Installed integration remains
  pending.
- Persist byte-defined verifier continuations bound to the job and private native
  prefixes. Recovery reads only newly requested checkpoint pages; progress and
  continuation writes total 616 bytes per batch. Native fingerprint scans still
  grow with the imported maps, so total recovery CPU is not yet bounded.
- Verify installed map-import records after native writes and before publishing
  either root. Staged start/restart and new batches require a private verification
  acknowledgment; exact retries continue to preserve later application writes.

### 🔧 Changed

- Validate the qualification workspace with Runtime 0.23.15 and Host Artifacts
  0.80.3, retaining Testkit 0.107.4 and the prepared 0.106.0 test launcher.

### 🧪 Testing

- Extend the governed page fixture to empty, asymmetric and 4,608-record pairs.
  Opening reads 1,344 bytes and selected pages still read 1,104 bytes. Verify
  second-root export failure, cross-root substitution, late corruption, unknown
  locators and independent pair golden bytes without replacement writes.
- Exercise page-job reopening, root-boundary batches, damaged native prefixes,
  late-page corruption and completed retries after application writes. Add an
  independently encoded consumed-job golden and refuse oversized page offsets.
- Qualify continuation corruption, stale/mixed publication, native fingerprint
  mismatches and independent golden bytes. Measure fixed page reads after storage
  reopening, zero page reads on retries, and detection of damage to previously
  consumed pages when explicit restart rereads them.
- Exercise installed rollback after post-apply verification and injected native
  value corruption, with artifact checks for apply/verify/publish/reply ordering.

## [0.75.3] - 2026-11-17

### ➕ Added

- Add a portable map-page codec: a fixed manifest commits to sequential pages
  of at most 64 sorted records. Validate each page against the preceding hash,
  epoch, root, position and strict key order without rereading the whole payload.
  Governed host qualification reaches 4,608 records; current installed jobs keep
  their existing format and limits. The verifier is transient, not a durable cursor.

### ⚡ Performance

- Hash and validate separate job metadata/checkpoint slices directly. Metadata
  encoding uses fixed scratch space, and split decoding no longer assembles a
  temporary full envelope. Stored bytes, integrity checks and recovery limits
  are unchanged; full checkpoint reads and logical validation remain required.
- Reuse the logical maps validated during split job admission when advancing a
  recovery batch. Each request, including retries, decodes the checkpoint once
  instead of twice; complete integrity and retained-prefix checks remain.
- Decode portable map checkpoints directly into sorted logical records for
  atomic and staged recovery, avoiding transient hash-table reconstruction and
  the following record copy. The product decoder owns framing, integrity and
  duplicate-key rejection; stored formats and installed limits are unchanged.

### 🔧 Changed

- Consolidate modules with child directories under `mod.rs`, preserving their
  Rust module paths and contents.

### 🧪 Testing

- Check staged map envelope bytes against an independently generated golden,
  including both nested checkpoints, full-width values and durable progress.
- Count actual payload decodes and compare combined admission with existing
  transitions across empty, asymmetric and full pairs, invalid requests and
  completed retries. Meter the adapter's combined batch path and corruption refusals.
- Compare logical-record decoding with native reconstruction through 4,608
  records on the host, including full-width keys, deletion histories and
  nonadjacent duplicate keys with repaired digests.
- Qualify paged checkpoints through published governed Storage handles: opening
  reads 1,144 bytes and each selected page reads 1,104 bytes at all tested sizes.
  Cover independently encoded golden bytes, incomplete publication, missing or
  mixed pages, corruption and refusal without replacement writes. Add the focused
  `native-map-page-test` host target to CI.

### ▶️ Try it

After the [documented bootstrap](README.md), run the locked offline page checks:

```sh
cargo test -p ic-storage-native --features map --lib map::paged --locked --offline
make native-map-page-test
```

The [page contract](docs/design/paged-map-checkpoint.md) records bytes, custody
and limits. Installed recovery remains on the existing two-map job format.
Release checks retain the separately prepared Testkit 0.106.0 launcher alongside
the current 0.107.4 library; optional launcher refresh is deferred to the next
tooling-maintenance boundary.

## [0.75.2] - 2026-11-16

### ➕ Added

- Extend durable map jobs with batches of up to 64 records, exact retries,
  compatible interruption and explicit restart of unfinished imports. Validate
  both native prefixes against the checkpoint and keep application writes blocked
  until both maps are complete; completed retries preserve subsequent writes.
  Total recovery remains bounded to 64 records per map.

### 🔧 Changed

- Adopt Testkit 0.107.4 in the qualification lockfile. Its library sources are
  unchanged from 0.107.3; installed checks retain the separately prepared 0.106.0
  server launcher.

### ⚡ Performance

- Read native map checkpoints with one forward bucket scan per root, replacing
  repeated scans for every key and value. Enforce ordered scalar reads and
  preserve the existing checkpoint bytes and recovery limits.
- Store durable map-job progress separately from checkpoint bytes. Progress
  writes are fixed at 104 bytes and identical retries write nothing; full
  checkpoint reads and validation remain bounded to the existing map limits.

### 🧪 Testing

- Qualify staged map publication rollback, corruption refusal and recovery after
  native-header loss. The private job fixture now uses native format 8 and
  refuses older native formats; scalar callbacks remain non-reentrant.
- Check sequential reads on empty, full and deletion-sparse maps and unfinished
  import prefixes, including callback-order refusals and bucket-visit bounds.
- Meter the actual governed two-cell adapter and test missing/mixed/corrupt
  parts, plus IC rollback between checkpoint and metadata publication. Older
  single-cell jobs are rejected without automatic migration or reset.

### ▶️ Try it

From the repository root after the [documented bootstrap](README.md), prepare
tools once with network access and local installation:

```sh
make install-tools retention-setup
```

Run the locked offline atomic, staged and restart checks on the local server:

```sh
make native-map-job-test
```

Successful attempts contain `JOB_RECOVERED`, `STAGED_MAP_RECOVERED` and
`STAGED_MAP_RESTARTED` under `target/evidence/native-map-provider/`.

## [0.75.1] - 2026-11-16

### ▶️ Try it

From the repository root, with Rust 1.100.0 and the locked dependencies prepared
as described in [the bootstrap instructions](README.md), prepare tools once
(network access and local tool installation):

```sh
make install-tools retention-setup
```

Then run the locked, offline map-job qualification against the local PocketIC
server:

```sh
make native-map-job-test
```

Expected result: passing host, artifact and installed recovery checks, with a
`JOB_RECOVERED` marker under `target/evidence/native-map-provider/`. The crates
remain experimental and unpublished; map-job recovery supports 64 records per map.

### 🔧 Changed

- Adopt Toko Tooling 0.44.11 through the canonical exporter in both workspaces;
  admit prepared tools and retain bounded sanitized CI setup diagnostics,
  including hidden attempts, without replacing the original failure status.

- Rename the primary unpublished package and directory from `ic-storage-new`
  to `ic-storage`. Update consumers and keep the frozen registry oracle under
  the explicit `ic-storage-legacy` dependency alias. Preserve M0's byte format.

- Advance the direct Platform Runtime requirement to 0.23.12 with only
  `system-raw`; the lock selects 0.23.13. Existing synchronous operations are
  unchanged; new stable-memory/cycle APIs and executor/timer features remain
  unused. Preserve the frozen Storage oracle dependency.

- Adopt Toko Tooling 0.44.9 through the canonical exporter in both workspaces.
  Retain the flat qualification package layout rule and remove the superseded
  fleet-wide thiserror hold; PocketIC constrains only its own dependency graph.

- Adopt Testkit 0.107.1 and its original PocketIC client types; the lock selects
  0.107.2, whose source changes only unselected held-fixture tests. Qualification
  callers require the externally prepared server URL and explicitly delete their
  own instances; preserve request limits and manual simulation progression.
  Keep the 0.106.0 launcher as a separate pinned tool, outside the consumer graph.
  Remove Testkit's legacy Host/Platform edges from the qualification lockfile.

- Adopt Platform Runtime 0.23.11 with `system-raw` for caller-owned synchronous
  exports, resolving the API blocker tracked in Platform #6. Qualify startup,
  retained graphs, checkpoints, record conversions, governed jobs and benchmark
  replies without adding an executor. The frozen Storage 0.74.1 oracle retains
  its transitive Runtime 0.22.5; graph checks admit that pair only where needed.

- Adopt Toko Tooling 0.44.6 in both workspaces through its canonical snapshot
  exporter, including the clarified asynchronous hosted-CI policy. Preserve the
  current executable snapshot closure and unrelated dependency selections.

- Advance the unpublished product workspace to 0.75.0, above the latest published
  `ic-storage` 0.74.1. Preserve package identities, historical evidence and the
  independently pinned checkpoint reader; this does not establish compatibility
  with the published package or authorize publication.

- Adopt Toko Tooling 0.44.5 through the canonical exporter in both workspaces.
  Its dependency checker now admits the required exact tooling pin directly;
  remove the two redundant local pinning exceptions.
- Update the Store derive to `syn` 3.0.10 and reconcile both workspace locks.
  Upstream qualification macros retain their supported 2.x selections; no shared
  `syn` types cross that boundary. Derive, host and Wasm consumer checks pass.
- Migrate direct Host artifact use to published `ic-host-artifacts` 0.80.1.
  Use its bounded read/complete-identity API and explicit `wat`/`wasmparser`
  fixture admission, preserving limits, validation and evidence digest text.
  Testkit 0.106.0 retains its own legacy Host dependencies and prepared server CLI.
  The qualification lock now selects 0.80.2; artifact/benchmark Clippy and
  installed provider/retention checks pass with that selection.

### ✨ Added

- Add durable two-map recovery jobs through the shared migration state machine:
  controller-authorized export/cancellation, domain-bound sequential epochs,
  frozen-source resumption, exact retries and atomic single-use recovery of up
  to 64 records per map. Qualify heap replacement, corruption refusal, replay
  prevention and stable/native publication rollback on installed Wasm. Larger
  workloads and multi-message map import/restart remain separate qualification.

- Extend provider-loan qualification with a host allocation pool for simultaneous
  maps and growth candidates. Check fresh-controller reopening, fragmented-space
  refusal, failed-candidate reclamation and lost-owner settlement refusal. Add
  `make native-map-pool-firehose` for a three-root, million-record lifecycle in a
  96 MiB loan. Qualify the same pool on IC with two map roots, independent and
  joint growth, compatible upgrades and atomic publication rollback. Exercise
  768 records in the initial 24 KiB allocation, early growth refusal when candidates
  lack space, and reclamation after explicit root retirement.

- Add bounded provider-owned pool growth from 24 to 192 KiB between native loans.
  Qualify 4,608 retained records, compatible upgrades after adding Wasm pages,
  actual memory.grow refusal, dynamic access bounds and rollback of bytes and
  page count. Fill maps through a single scoped native batch.

- Connect two map checkpoints to the governed Storage adapter with explicit root
  identities, a shared epoch and complete-bundle integrity. Qualify full-width
  recovery after heap replacement, source freezing, corrupt/missing checkpoints,
  over-limit refusal and rollback through native reconstruction. Add explicit
  salvage after native metadata corruption. This profile supports 64 records per
  map; larger recovery, multi-message map imports and production root custody
  remain open. The separate durable job profile adds consumption and cancellation.

- Add `Map::from_reader` and `from_reader_in` for single-pass portable recovery
  using a 4 KiB record buffer. Share envelope/record validation with slice import,
  preserve checkpoint bytes and publish no candidate before integrity succeeds.
  Exercise one-million-record file export/recovery and destination reclamation
  after source failures, corruption and panic.

- Add `Map::rebuild_in` for native rehash into another allocator while preserving
  the source. The initial installed two-bank evidence is retained; the current
  shared-pool fixture uses ordinary product reserve/insert growth.

- Add the optional native `map` API for allocator-owned u64 keys and values:
  scoped access, geometric growth, tombstone-free deletion, bounded admission,
  panic poisoning and streamed portable checkpoint/recovery. Keep slot ownership
  and unsafe region custody separate; no private std header becomes persistent.

- Add explicit portable record-vector V3→V2 widening after heap replacement.
  Preserve field order and ceilings, reject incomplete or wrong-schema sources,
  and qualify publication rollback, full-width updates and exact V2 recovery.

- Add explicit portable scalar V2→V3 narrowing with range checks before native
  construction, publication rollback, ceiling preservation and full-width V2
  recovery after refusal. Share recovery mechanics with exact restore and widening.

- Add explicit portable scalar V3→V2 widening after heap replacement, sharing
  the existing recovery path and refusing other source schemas. Qualify corrupt
  source recovery, publication rollback and subsequent full-width updates.

- Qualify explicit installed scalar V3→V2 widening with destination-only source
  refusal, unchanged vector storage and ceiling, publication rollback, repeated
  narrowing/widening, and full-width updates plus portable recovery.

- Qualify controller-authorized V3 record-vector migration with durable epochs,
  cancellation, single-use recovery, frozen resumption and staged import/restart.
  Share the existing job state machine and authority adapter; use a distinct
  bounded envelope, governed locator and native fixture identity.
- Add independent installed record-job histories covering repeated cancellation,
  atomic/staged cutovers, retained mutations and allocation refusal before the
  fixed region is full. Check fields, capacity, custody and retired epochs across
  four deterministic 96-operation histories.

- Accept absolute `::core::primitive::u32`/`u64` field paths in Store declarations,
  preserving the existing representation and codec contracts.
- Add scoped access and checked updates for V2 scalar records, with host poison
  handling and installed IC mutation, rollback and recovery checks.
- Add an unpublished scalar-only `Store` derive for explicit schema/field IDs,
  public-layout metadata, bounded logical codecs and scoped getters. Replace V1/V2
  boilerplate with generated code while retaining explicit validation/conversion
  and the existing checkpoint bytes. Reject unsupported fields and custom Drop.
- Extend scalar declarations to u32/u64 with explicit field-kind metadata and
  padding-free logical encoding. Add checked V2→V3 narrowing with installed
  range refusal, publication rollback and recovery through checkpoint v4, keeping
  the existing vector unchanged. Test logical widening separately on the host.
- Qualify V3 padding rejection and independent checkpoint recovery after
  discarding deliberately corrupted native metadata.

### 🧪 Testing

- Exercise growth rollback during destination writes, partial owner release,
  before/between/after root publication and during failed upgrades. Verify both
  maps, source preservation, business-error/forgotten-view growth and pool exhaustion.
  Extend the million-key workload with native relocation and portable recovery.

- Add independent-model mixed histories, collision-chain and malformed-image
  checks, host region-controller reopening/refusal, and a public-API million-key
  firehose with 256 million mixed operations and complete portable recovery.
  These load measurements remain native host results.

- Qualify a closed installed native map profile with guarded slot access,
  compatible upgrades, deletion/reuse, allocation refusal, malformed-root/slot
  rejection and rollback before and after root publication. Preserve the existing
  vector admission rules and exercise map-only guard and stack counterexamples.

- Qualify retained V2→V3 record-vector conversion while preserving backing and
  capacity. Check whole-source admission, range refusal and publication rollback;
  retain a full-width V2 recovery path and exercise continued V3 allocation and
  portable recovery after conversion.
- Qualify explicit portable V2→V3 record-vector conversion from an ordinary
  Rust source. Validate the complete predecessor before narrowing and publish
  only a complete target; range failures preserve the source and checkpoint for
  explicit repair and retry. Keep V3 recovery and V2 conversion separate, and
  exercise continued native V3 operations and checkpoint recovery afterward.
- Qualify portable record-vector recovery through governed Storage checkpoints.
  Share the host V3 codec, discard the native heap, reconstruct in a relocated
  domain and continue allocating. Check frozen mutation, failed-export/import
  rollback, corrupt checkpoints and explicit recovery from a damaged old header.
  Run `make native-record-vector-test`; retained collection conversion and migration-job
  authority remain separate work.
- Qualify a closed native V3 record vector through installed mutation/reclamation,
  retained upgrades, ordinary errors, rollback and schema/custody refusal. Test
  record-specific guard bounds and reuse the shared provider/linker. Installed
  portable record-vector recovery remains a separate gate.
- Compile the actual derived V3 declaration in a closed no_std Wasm vector fixture.
  Check its representation, 64-byte peak stack and all 58 guarded memory sites,
  reusing the controller and access rewriter with a distinct record profile.
- Add installed WAT record-vector admission for exact V3 schema metadata and
  initialized field invariants. Exercise retained upgrades, custody and rollback,
  corrupt-source destination refusal, and old-format rejection in both directions;
  share its provider builder with the native record-vector profile.
- Exercise fresh V3 vector controllers over exclusively loaned live host backing,
  sharing the installed u64 allocator state machine. Check schema/initialization
  authority, growth/reuse, publication, lost-owner and unwind refusal, and logical
  payload reconstruction after teardown.
- Add V3-only host reopening through original live allocation authority. Reject
  schema/layout/length changes, substituted allocations, conflicting u64 claims
  and ledger corruption; forgotten/refused tokens keep their runs unavailable.
  Verify continued mutation/reclamation and portable recovery after refusal.
- Exercise fresh scalar-record vectors in the live host arena: growth/refusal,
  scoped mutation, destruction and run reuse, checked collection conversion and
  bounded portable recovery after source teardown. Failed candidates preserve
  existing roots.
- Fix generated Store names resolving through application imports. Add direct
  `no_std` consumer checks for shadowed names, absent implicit prelude, metadata,
  field ordering and malformed/invariant-invalid codecs to `make native-core-test`.
- Move Store declaration and scoped-view compile failures beside the product API;
  add refusal of unrestricted mutable record access and explicit-path equivalence.
- Reject escaping record views at compile time. Exercise ordinary errors,
  forgotten views, panic poisoning and checkpoint replacement on the host.
- Preserve record updates across compatible reopening and portable recovery;
  check ceiling/overflow refusals, frozen access and post-publication trap rollback
  while leaving vector storage intact. Run `make native-record-test`.
- Exercise generated codecs and views in those same host and installed paths;
  preserve checkpoint golden bytes and verify field-ID ordering, scalar layout,
  validation and compile-time rejection of unsupported declarations.

### 📝 Design

- Maintain one current pre-1.0 implementation. Qualification schema versions test
  upgrades and refusal; they do not promise support for historical releases.

## [0.1.12] - Development checkpoint

Pushed as `f2516ff`; no release tag or package publication.

### ✨ Added

- Add concrete scalar record versions and checked V1→V2 conversion in the native
  host fixture, preserving the existing vector allocation. Add portable recovery
  of both record fields through checkpoint v3; legacy readers reject the new
  schema and existing checkpoint bytes remain unchanged.
- Add an installed record profile with explicit schema admission, compatible
  reopening, V1→V2 conversion and checkpoint recovery in both allocation domains.
  Verify failed upgrades roll back record publication, vector bytes, module and
  stable state; run it with `make native-record-test`, also included in CI.

### 📝 Design

- Scope direct reopening to one supported runtime contract. Handle incompatible
  compiler, allocator and heap changes through planned portable migration with
  optional downtime; defer universal layout certification and automatic discovery.
- Prioritize controlled records, explicit application conversions and derive
  support while retaining per-type representation, ownership and recovery checks.
- Consolidate duplicated migration-test rollback assertions and job input loading.
  Remove stale implementation-status claims and replace repeated architecture
  progress narratives with links to their owning qualification records.

### 🧪 Testing

- Connect native-host poisoning and explicit checkpoint recovery to the same
  provider controller and scoped vector core used by the installed fixture.
  Test caught unwinds, forgotten views, ordinary errors and failed candidate
  reconstruction without treating host unwinding as rollback.
- Inject finalization failure after native owner release. Keep host access
  poisoned before and after root publication; verify installed IC traps restore
  the full provider header and payload before subsequent access and upgrades.
- Compare mixed installed native histories against an independent logical model,
  including repeated compatible upgrades, failed reconstruction and portable
  migrations in both directions.

### ▶️ Try it

Run `make native-feasibility-test` for the provider-host failure and recovery cases.
Run `make native-record-test` for installed conversion and portable recovery.
Run `make native-provider-test` for installed finalization rollback.
Run `make native-job-test` for mixed histories and repeated portable migrations.

## [0.1.11] - 2026-11-16

### 🧪 Testing

- Exercise normal callback errors and forgotten scoped views through the installed
  native vector. Preserve earlier writes and finalize custody before replying;
  distinguish ordinary refusal from traps that roll back the whole IC message.
- Add explicit checkpoint-based restart of unfinished staged imports after heap
  replacement. Recover from damaged native prefixes without reading the old
  heap, including when its upgrade hook refuses. Preserve checkpoint bindings;
  reject completed jobs and corrupt checkpoints, and test restart rollback.

### ▶️ Try it

Run `make native-provider-test` for the installed scoped error and forget cases.
Run `make native-job-test` with cached dependencies and the prepared local server
for staged restart, recovery, refusal and rollback cases.

## [0.1.10] - 2026-11-16

### 🧪 Testing

- Extract a reusable, unsafe-free scoped u64-vector core under `crates/` and use
  it in the installed native fixture. Check owner/reference/future escape,
  forgotten views and allocation refusal. Extend artifact admission to measure
  nested balanced stack frames and guard every access in the new compiler output.
- Add controller-authorized cancellation before heap replacement. Retire the
  sealed epoch, preserve the original vector allocation and resume native writes;
  identical retries leave later writes intact. Exercise cancellation rollback,
  stale-job refusal, compatible reopening and fresh export after cancellation.
- Add explicit frozen-source resumption across compatible upgrades. Rebuild
  transients, verify the sealed job against retained logical values and keep
  writes blocked; preserve subsequent cancellation and replacement recovery.
- Add staged checkpoint import with a version-2 durable cursor, batches of up
  to 64 values and explicit compatible resumption. Keep partial vectors
  inaccessible; verify exact retries, native/checkpoint disagreement and
  rollback before publishing the complete root.

### ▶️ Try it

Run `make native-core-test` for the scoped vector core and ownership checks.
Run `make native-job-test` with cached dependencies and the prepared local server
for cancellation, frozen resumption, staged import and governed admission cases.

## [0.1.9] - 2026-11-15

### 🧪 Testing

- Connect compatible native reopening to checkpoint export and relocated
  recovery by discarding and rebuilding the checkpoint runtime's transient
  domain. Admit its initialization/globals/table behavior before enabling the
  profile; exercise poisoned and grown memory, native refusal and reset rollback.
  The private Global allocator is rebuilt, not retained or declared compatible.
- Add a separate governed migration record with controller-authorized export,
  sequential durable epochs, source/destination domain checks and single-use
  consumption. Exercise controller revocation, stale/replayed jobs, frozen
  retries and rollback after stable/native publication at empty/full capacity.

### ▶️ Try it

Run `make native-checkpoint-test` with the prepared local server and cached
dependencies to exercise compatible reopening and explicit replacement recovery.
Run `make native-job-test` for the controller and durable-job admission cases.

## [0.1.8] - 2026-11-15

### 🧪 Testing

- Link checked vector accesses to the private provider, derive Rust inputs from
  admitted metadata and reject changed final artifacts. Document the bounded
  foreign-allocation and initialized-owner contract before installed execution.
- Exercise real Rust Vec operations across installed A→B→A upgrades, including
  growth, shrink, removal, clear/reuse and empty/full capacity. Verify native
  update/upgrade rollback, destination-only malformed-header refusal and lost
  custody.
- Add an explicit 512-element checkpoint envelope while preserving version-1
  bytes and its 16-element bound. Exercise governed provider recovery at empty,
  17-element and full capacity, failed-export/import rollback and subsequent
  shrink/growth.
- Connect the retained vector to the same governed checkpoint provider through
  scalar-only calls and disjoint native/Global memory regions. Freeze/export
  atomically, reject retained-heap reopening and reconstruct under a relocated
  allocation domain using explicit heap replacement. Exercise full capacity,
  return migration, continued native allocation and failed export/import rollback.

### ▶️ Try it

With cached dependencies and the prepared local server, run
`make native-checkpoint-test` for the connected native migration experiment.

## [0.1.7] - 2026-11-15

### 🧪 Testing

- Add checked vector access helpers and compiler instrumentation for all 49
  emitted memory operations. Exercise metadata/spare-capacity refusal, bounded
  operation-specific writes, offset overflow and overlapping copies in installed
  WAT tests. Guard imports remain unresolved in the rewritten Rust diagnostic;
  it is not executed or admitted as a native persistence implementation.

- Exercise installed vector-descriptor custody before native integration. Reject
  oversized length/capacity before wasm32 narrowing, require settlement before
  publication, and preserve the full header/payload after traps or refused
  upgrades. The WAT-only test includes missing source guards and full-region
  capacity boundaries; it does not execute a Rust vector.

- Check the compiled vector's transient stack discipline and direct-call graph.
  Reject overwritten frame locals, incorrect restoration, branch/return escapes,
  recursion and unreviewed control operators. Retain the operator listing for
  review; payload-address admission and installed vector execution remain open.

## [0.1.6] - 2026-11-15

### 🧪 Testing

- Exercise a transient single-vector allocator over exclusively loaned backing:
  fresh issuance preserves initialized payload, growth/shrink and destruction
  permit reuse, and invalid roots or forgotten owners refuse further access.
  Compile the same controller for wasm32 and inventory its stack/copy operations;
  scalar admission rejects this larger profile and no vector Wasm is executed.

- Add `make native-provider-probe` to compile, combine and inspect the bounded
  Rust/Wasm provider diagnostic. Reject overlapping compiler bounds, unexpected
  initialization/code/imports and altered final artifacts. Keep raw Rust functions
  private and admit trusted entrypoints that settle only after native access ends.

- Add `make native-provider-test` and its installed CI step. Exercise native scalar
  reads/writes across A→B→A upgrades, incompatible/corrupt headers, abandoned
  custody, and update/post-upgrade trap rollback on the local test server. This
  does not qualify native vectors or allocator issuance.

- Exercise a fixed-region Wasm custody protocol across installed upgrades:
  require explicit settlement, reject duplicate or abandoned claims, and retain
  source state after incompatible targets or traps. Destination refusal also
  holds when the source has no pre-upgrade guard. This does not yet qualify a
  Rust allocation provider.

### 📚 Documentation

- Narrow the IC backing-provider investigation to an explicit foreign-allocation
  boundary. Record the fixed scalar's unsafe-code argument, artifact admission
  and installed evidence separately from the remaining native allocator gate. See the [provider investigation](qualification/ic-storage-native-feasibility/provider-boundary.md).

### ▶️ Try it

With the already prepared pinned compiler, wasm target and locked dependencies:

```sh
make native-provider-probe
```

This emits compiler and linked-artifact diagnostics; no combined Wasm is executed.

With the previously prepared local test server, run the installed custody cases
alongside the original retention probe using `make retention-lint` followed by
`make retention-test`. Run `make native-provider-test` for the combined Rust/Wasm
scalar lifecycle.

## [0.1.5] - 2026-11-15

### 🧪 Testing

- Exercise allocator clones with independent live domains and cloning failures.
  Scoped detachment keeps each vector paired with its issuing allocator;
  failed cloning poisons access while preserving ownership for destruction.

## [0.1.4] - 2026-11-15

### 📚 Documentation

- Define the next allocation-domain experiment: independently owned backing,
  explicit custody transfer and fresh allocator issuance. Separate the host
  controller proof from the still-required IC provider and startup contract.

### 🧪 Testing

- Connect the existing safe scoped owner to arena-backed vectors and freshly
  reconstructed records without reallocating. Exercise real allocation refusal,
  retirement/reuse, panic and forgotten-ticket quarantine, and allocator lifetime
  confinement; the default Global-backed experiment remains supported.

- Enforce destination usable-region limits before native transfer and during
  allocation/reuse. Exercise unchanged-custody refusal and explicit portable
  reconstruction into a smaller arena after discarding the incompatible source.

- Transfer live backing through explicit consumed source authority, replace the
  allocator controller and issue the retained span afresh before vector creation.
  Exercise nonzero offsets, duplicate/lost authority, reservation refusal,
  allocation failure and reclamation; no cross-instance provider is implied.

- Add a private, fixed-width allocation-ledger image with version, geometry and
  integrity checks. Exercise independent golden bytes, truncation, corruption,
  resealed invalid metadata and active-owner refusal. Decoding does not grant
  allocation custody or create native pointers; IC reopening remains unresolved.

- Exercise single-root live suspension with complete allocation accounting,
  extra/forgotten-owner refusal, arena freezing and original-token single-use
  resume. Dropping or forgetting suspension keeps the arena unavailable for reuse.

- Connect portable checkpoints to source suspension in host handoff tests:
  recover after source/token loss, retry refused imports, cancel before target
  publication and export a new epoch after further writes. Keep durable migration
  coordination and installed native reopening explicitly outside this evidence.

## [0.1.3] - 2026-11-15

### ▶️ Try it

From the repository root, with the pinned compiler, cached dependencies and
previously prepared test server, these checks run offline:

```sh
make native-feasibility-test
make native-layout-probe
make checkpoint-provider-lint
make checkpoint-provider-build
make checkpoint-provider-test
```

Expect the ownership, layout diagnostic and installed checkpoint tests to pass.
First-time online setup is explicit: `make install-tools`,
`cargo fetch --manifest-path qualification/Cargo.toml --locked`, then
`make retention-setup`; it downloads dependencies and the pinned local test server.

### 🔧 Tooling

- Adopt Toko Tooling 0.44.4 through its canonical exporter and both workspace
  pins. Rename qualification package directories to their existing Cargo package
  names, updating workspace paths, fixture inputs, commands and documentation.
  Package identities, product versions and historical evidence remain unchanged.

### 🧪 Testing

- Validate the whole host allocation ledger before vector reopening, rejecting
  inconsistent extents, overlap, generation identities, occupancy and initialized
  lengths. Exercise fourteen corruptions and valid boundary cases; this remains
  live-domain qualification, not an IC pointer-validity proof.

- Add a host-only native ownership experiment with scoped scalar/vector access,
  tracked detach/attach/retire, owner-escape rejection and forgotten-ticket
  quarantine. Exercise ordinary error preservation and panic poisoning through
  `make native-feasibility-test`, included in CI. Native layout admission,
  persistence and native incompatible-runtime migration remain unimplemented.
- Add `make native-layout-probe` to inspect compiler-emitted host/wasm32 field
  layouts, distinguish equal-sized reordered records and exercise malformed
  metadata refusal. Keep raw diagnostics under target and explicitly separate
  those observations from persistence admission.
- Exercise real standard-library vectors with a bounded private host allocator,
  same-domain parking/reopening, run reuse and failure/forgetting cases. Document
  the pointer and ownership argument separately from unresolved IC reopening.
- Specify and test a bounded scalar/vector checkpoint with schema/epoch checks,
  completeness and integrity validation. Reconstruct after source-arena teardown
  and preserve an existing host root on corrupt input or target allocation failure;
  the native migration gate remains incomplete.
- Add an installed checkpoint-provider fixture using published Storage 0.74.1
  and fresh governed handles. Test ordinary A→B→A upgrades that discard Wasm
  memory, continued writes, rejected checkpoints and rollback after a target
  stable write. Record the provider/locator contract and add focused Make/CI
  checks; this does not qualify native heap reopening or compiler transitions.

### 📚 Documentation

- Define the intended derive-based stored-type surface, with recursive field
  eligibility, scoped ownership, portable codecs and explicit versioned
  conversions. Distinguish these declarations from runtime/layout admission.
- Describe a proposed IcyDB separation between stable index structure and
  evolving records, including when inline values or index changes require
  structural conversion or rebuilding.

## [0.1.2] - 2026-11-15

### ➕ Added

- Add allocation-free, `no_std` M0 image admission with typed refusals for
  incompatible metadata, integrity failures and invalid object/reference graphs.
  It validates unreachable objects and cycles without granting live-memory access.
- Add native M0 image initialization/readmission, explicit bounded growth,
  append-only allocation and session-bound graph access. Ordinary callback errors
  preserve successful writes; unexpected write failures poison the owner.

### 🧪 Testing

- Add opt-in installed benchmarks for M0 scalar and sequence operations versus
  published Storage 0.74.1, with complete state checks, instruction counts,
  artifact/source evidence and `make benchmark-simple`. Compile and lint both
  backends in CI without imposing a performance threshold.

- Add independent empty-state golden bytes and native corruption tests, including
  resealed malformed metadata, identity/schema mismatches and maximum index size.
  `make m0-test` and `make m0-lint-wasm` join the CI gate.
- Verify exact graph bytes, sharing/cycles after reopening, allocator continuation
  across a page boundary, unchanged resource refusals and native poison behavior.
  Add a runnable graph example and compile-fail checks for escaping handles,
  session-capturing futures and handles passed across images.

- Add an independent Rust qualification runner and small Wasm fixtures for
  retained scalar state, fresh transient state, incompatible metadata rejection,
  failed-upgrade rollback and retaining downgrades. Server preparation is explicit
  through `make retention-setup`; `make retention-test` runs offline.
- Add a controlled Rust canister startup probe with passive-data transformation,
  bounded transient clearing, artifact refusal tests and installed upgrade checks.
  `make rust-startup-build` prepares A/B; `make rust-startup-test` admits and tests
  them. The Platform binding path is std-linked and the fixture uses no heap.
- Extend installed qualification to retained memory growth, maximum-capacity
  refusal, growth rollback in updates/upgrades, and retention-mode/marker refusal.
- Add a fixture-only live-memory kernel and installed M0 graph tests for A→B→A,
  shared/cyclic references, page-crossing allocation, trapped update rollback and
  rejected/mutating post-upgrade rollback. Keep global allocation denied and
  admit the graph fixture through a distinct bounded artifact profile.

### 🔄 Changed

- Reuse admitted accounting for reads within an exclusive M0 session. Refresh it
  after fully validated writes; preserve entry/exit admission, handle checks,
  corruption refusals and poison behavior without changing the stored layout.

- Restrict the experimental image mutation API to retained-region slices so it
  cannot borrow the live Rust stack. Add retained-only admission with unchanged
  persisted layout and absolute error offsets.

### 📚 Documentation

- Select native retained collections with explicit versioned type conversion
  and a portable checkpoint path for incompatible runtimes. Define migration,
  interruption/recovery and toolchain-feasibility gates; archive the superseded
  M0 research plan while preserving its byte contract and evidence.

- Tighten native feasibility around complete layout/pointer admission, tracked
  scoped ownership and forgotten guards. Move portable migration before type
  conversion, select a transitional checkpoint-provider boundary, and restore
  explicit mutation, destruction and reclamation rules.

- Freeze the experimental M0 metadata, schema, root and allocation-index layout,
  including integrity coverage and canonical empty-state vectors, ahead of its
  implementation.

## [0.1.1] - 2026-11-15

### 🔧 Tooling

- Adopt Toko Tooling 0.44.3 with its vendor layout, canonical package inventory
  and dependency updater, formatting hook, and current workspace lint rules.
- Add Linux and macOS CI for formatting, dependency policy, Clippy and Rustdoc,
  with a separate repository secret scan.

## [0.1.0] - 2026-11-15

### 🔧 Tooling

- Bootstrap an unpublished Rust 1.100.0 workspace with authenticated Toko
  tooling, explicit tool setup, dependency inspection and policy checks, and a
  documented qualification location.
