# Storage release notes

Current releases use the Storage naming. [Earlier published history](https://github.com/dragginzgame/ic-storage/blob/23deb81c48d58558ac3b0ce36bf266ddc25dca91/releases/persistence/CHANGELOG.md)
retains its original package and release identities.

## [0.73.2] - Unreleased

### 🔧 Changed

- Adopt IC Build 0.83 for native repository tooling and the release metadata
  editor. Contributor setup uses `make install-release-tools`; Storage's library
  API and stored formats are unchanged. See [the detailed notes](0.73.md#0732---unreleased).

## [0.73.1] - 2026-11-11

### 📦 Dependencies

- Adopt Toko Tooling 0.39.10 and development-only Wire 0.21.1, alongside Build
  0.82 and Runtime 0.21.1; extend native Map scan attribution with
  entry/key-only and governed-ICSS controls. Stored formats are unchanged.

### 🧪 Testing

- Give complete collection comparisons a bounded one-hour server lifetime so
  larger recipes can finish their validation. Per-request timeouts are unchanged.
  See [the detailed notes](0.73.md#0731---unreleased).

- Add matched Cell reopen comparisons through 1-MiB values, including repeated
  replacement, growth and shrinkage with reads after each mutation.

### ⚡ Performance

- Reduce Map replacement writes and small-value allocations; advance ordered
  cursors only when another result is requested. Owned entries, lazy decoding
  and checks on accessed nodes are preserved. See [the detailed notes](0.73.md#0731---unreleased).

- Initialize external Map and loaded Cell buffers directly from backing,
  preserving extent checks and partial-read failure handling. Matched 1-MiB
  Map reads and Cell load controls improve 17.4% and 17.9%; the buffer change
  adds 10 bytes to all-collection application Wasm.

- Replace the transient backing-custody tree with a compact sorted identity
  vector. Matched all-collection Wasm falls by 4,888 bytes (4,944 code bytes).
  The admission matrix records at most 420 extra instructions per measured call;
  custody, APIs and stored formats are unchanged. See [the detailed notes](0.73.md#0731---unreleased).

- Reuse checked range-bound paths for Map/region iteration instead of repeating
  the initial descent. Matched scalar scans improve 1.6% with shuffled insertion
  and 46.0% with sequential insertion; the shuffled region key scan improves
  33.6%. Range-aware application Wasm adds 197 bytes for Map and 58 for all
  collections. Public APIs, persisted formats and structural checks are unchanged.

## [0.73.0] - 2026-11-10

### ⚠️ Breaking

- Replace the copied ordered engine and private codec/address helpers with an
  independently implemented AVL engine shared by Map, Set and map regions.
  STORMAP1 version 1 rejects previous collection images without writes; hosts
  must explicitly reset/replace backing and update region framing admission.
  Public successful-call APIs and governed leases remain unchanged.

- Replace the adapted allocation backing with the independently implemented
  STORNAM1 version-1 named backing. Every previous whole-store image is rejected
  without writes: hosts must explicitly provide fresh/reset whole backing.
  Governance and successful collection APIs remain; saved capacities cannot
  expand on reopen. Default control reservation rises from 15 to 18 Wasm pages.
- Independently implement the public Memory interface, native backing and IC IO.
  Raw reads initialize caller-owned bytes; custom backings receive a bounded
  initialized-scratch fallback. Native partial-page tails remain inaccessible
  and positive growth of such a malformed backing is refused unchanged.

- Replace the copied Vector engine with an independently implemented bounded-slot
  vector using STORVECT version 1. Previous Vector images are rejected; hosts
  must explicitly provide fresh/reset backing. Length is cached under the
  exclusive lease, with health checks and no record scan during admission.

- Replace the copied Cell engine and envelope with a separately implemented Cell
  using the new STORCELL format. Previous Storage and ICSS Cell images are
  rejected without writes. Hosts must explicitly provide fresh/reset backing;
  no automatic reset or migration reader is included. Governed leases, cached
  reads, codec bounds and native failure poisoning remain enforced.


### ⚡ Performance and known limits

- The matched all-collection application is 14,936 raw Wasm bytes smaller, but
  large-population reads/scans and small-record capacity regress. At 4,096
  uniform u64 records, Map Get×32 rises from 934,522 to 1,145,264 instructions
  and Scan×32 from 326,047 to 773,764. These tradeoffs remain in this release;
  see [the detailed notes](0.73.md) for scope and the ICSS comparison.

### 🔧 Changed

- Adopt Toko Tooling 0.39.7 in both workspaces. Retire managed shared reference
  docs through the installer and use pinned upstream links. Group current Storage
  guides by topic, preserving historical evidence paths, CI caching, release
  commands, group selection and qualification gates.

### 🧪 Testing

- Add independent ordered-map model, range, alternating-iterator, codec, growth,
  reuse and corruption tests. Compare Map, Set and region behavior on separate
  Storage and ICSS images; verify incompatible upgrades preserve the previous
  application. Retain matched application size and depth performance evidence.
  Qualify all 565 selected catalogue cases across five collection forms, keeping
  expected backend refusals separate from paired performance measurements.

- Qualify the independent backing against byte models, interleaved growth,
  refusal, read-only reopen, checksummed corruption and every interrupted
  publication write. Add installed physical-memory range/overflow tests for
  safe and raw reads and writes, including empty boundaries and rollback.
  Verify incompatible whole-backing upgrade rejection preserves the old app.

- Add independent Vector format, public model, selective-decoding, corruption,
  extent, interrupted-IO and iterator-mutation coverage. Qualify old-image
  upgrade rejection, fresh installation and rollback. Compare the full type
  catalogue, populated/cold reads, bulk values and iterator boundaries with
  external ICSS, keeping application size and full-handler counters separate.

- Add public Cell acceptance coverage for growth/shrink, page boundaries, cold
  reopen, read-only admission refusal, lease release and explicit reset isolation.
  Count cached-read IO and codec calls; inject codec failures, growth refusal and
  partial writes to verify admission and poisoned-owner boundaries.
  Include frozen-old-image upgrade rejection and rollback, with fresh-install
  qualification for the new Cell format.
- Remove unused replacement-input iteration from matched Cell read/control
  benchmarks. This avoids timing copies of empty `Option` values with large
  inline payload capacity; both libraries use the corrected harness.
- Extend the matched Map/region IO matrix with shrink-to-empty, absent reads,
  deletion, repeated missing deletion and reinsertion, including edge-key and
  sequential access. Compare Storage with published ICSS on both governed and
  ICSS-managed backing, checking returned values and final contents.
- Installed Map, Set and region traces now distinguish repeated deletion of
  existing-tree holes from above-range misses, with strict phase admission and
  final-state checks. Later phases retain mutation history to expose deferred
  performance regressions.
- Add bounded batched Map read diagnostics that retain expensive read addresses,
  complete read call/byte totals and timed allocator counters without a heap
  trace buffer. Incomplete or overflowing diagnostics refuse comparison.

## [0.72.0] - 2026-11-10

### ⚠️ Breaking

- Handle opening now trusts the admitted directory throughout its continuous
  custody lifetime instead of rescanning backing metadata. Grants, exclusive
  leases, bounds and poisoned-owner refusal remain enforced. Hosts requiring
  explicit integrity observation must use `diagnostics()` or `memory_allocations()`;
  opening no longer detects out-of-contract raw metadata changes. Stored formats
  are unchanged; no reset is required. See [the contract](0.72.md).

### ⚡ Performance

- Reduce Vector Push/Pop overhead by writing its already allocated length header
  directly, while preserving payload-before-length publication and bounds checks.

### 🧪 Testing

- Extend admission comparisons with 2/4/8-name groups, retained-handle reads across
  executions, repeated reads within one execution and verification diagnostics.
  Keep large directory counts as stress controls rather than typical usage.

## [0.71.2] - 2026-11-09

### 📦 Dependencies

- Adopt IC Wire 0.19 for development and qualification; core Storage has no
  production Wire dependency. Stored collection formats are unchanged.
- Refresh the root lockfile to Build 0.81.1 and Host 0.76.2, removing the
  older SHA-2 dependency chain from the native tooling graph.

### 🛠️ Tooling

- Adopt Toko Tooling 0.38.2 and move the seven private benchmark/qualification
  packages into the independent `qualification/` workspace. Update build, CI,
  formatting and release callers without changing package identities or APIs.

- Fix native CI qualification on Bash 3.2 and prevent a nested retention test
  from replacing the hosted evidence archive output with a deleted fixture.
- Exclude only reviewed historical secret-scan fingerprints for public stable
  allocation names; retain full-history scanning and all other findings.

## [0.71.1] - 2026-11-09

### ⚡ Performance

- Reduce allocation-admission overhead by verifying admitted metadata without
  rebuilding the directory and batching active control reads. Preserve corruption
  checks and the current storage format. Preallocate decoded rows and classify
  name characters once to reduce cold-reopening work. Large admission cases improve substantially;
  some small cases regress. [Measurements and limits](../../docs/audits/recurring/storage-admission-2026-11-09.md).

- Keep each directory name solely in its entry and index entry positions. Cold reopening
  improves up to 48.3% versus the preceding optimization, with 2.7–2.8 KB
  application Wasm growth and small lease-opening regressions; allocation IDs
  and persisted bytes are unchanged.

### 🧪 Testing

- Add a reproducible installed admission/reopen matrix against ICSS, with names
  and populated buckets varied independently, plus metadata-tail and live-owner
  corruption coverage.

### 📦 Dependencies

- Adopt IC Platform Runtime 0.21 for Storage's `ic0`-only Wasm backing and
  Platform 0.28 for the independent composition fixture. Preserve the native
  production graph without Platform/Runtime and the Wasm graph without executor,
  guard, Types or Candid.
- Replace Platform's removed PocketIC facade in qualification drivers with
  Testkit 0.95's async client. Use caller-owned Tokio execution, explicit request
  budgets and awaited instance cleanup. The selected server remains 16.1.0;
  Testkit's older transitive Platform identity stays confined to native testing.

### 🛠️ Tooling

- Adopt Toko Tooling 0.37.5 across both workspaces. Contributor commands now use
  `toko-tooling-*`, with the renamed snapshot, baseline and LOC metadata; old
  command aliases are not retained. Preserve active offline guides/checkers as
  explicit roots and adopt the scoped latest-stable dependency policy. The new
  Wire benchmark oracle exception does not broaden Storage’s Candid permissions.
  Product APIs and release requirements are
  unchanged. [Adoption guide](../../docs/adoption/storage-tooling.md#current-cargo-owned-0375-selection).

## [0.71.0] - 2026-11-09

### ⚠️ Breaking

- Replace snapshot-directory allocation with Storage-owned ICSS-derived bucket
  backing. Hosts must explicitly retire/reset old whole-store backing or supply
  fresh backing; incompatible images are rejected without automatic migration.
- Remove `StorageConfig::wire`. Eight-page buckets and fixed creation-time tables
  replace snapshot extents; defaults reserve 960 KiB of controls and permit
  8 GiB of bucket data. Named IDs, grants, custody and exclusive leases remain.
  Review the [allocation contract](../../crates/ic-storage/ALLOCATION.md).

### ⚡ Performance

- Remove whole-directory snapshot rewriting and reduce matched small-consumer
  Wasm by about 14.6–14.8 KB. Results are workload-dependent; 64-KiB iterator
  allocation costs remain a documented tradeoff.
- Reduce redundant Cell framing work, combine B-tree overflow initialization,
  and transfer consumed Vector terminal buffers to decoders. Large-value Vector
  `last()` improves 32.6–41.7% in matched inner-loop measurements, with small
  scalar/array overheads. See [detailed results and limits](0.71.md).

### 🔧 Tooling

- Adopt Toko Tooling 0.36.1, IC Build 0.81.0 and IC Host 0.76.1 development tools.
- Extend bounded allocation/read diagnostics and use ordinary modules in both
  maintained Wasm size consumers, keeping benchmark code outside production.

## [0.70.0] - 2026-11-08

### ⚠️ Breaking

- Adopt IC Wire 0.18. Hosts exchanging Wire limits or errors must use the
  matching Wire type identity and align Storage and its adapters. Encoded
  directory and checkpoint bytes, budgets and admission are unchanged;
  no backing reset or stored-data migration is needed.

### ⚡ Performance

- Combine B-tree node-header and first overflow-pointer writes. The matched depth
  matrix improves 168/720 cells with no regressions (up to 2.13%); stored bytes
  and public APIs are unchanged. Broad benchmark Wasm shrinks by 7,461 bytes;
  the small all-collection consumer grows by 105 bytes. See the
  [comparison](../../docs/audits/recurring/storage-prefix-write-2026-11-08.md).

## [0.69.1] - 2026-11-08

### 🧹 Cleanup

- Replace inherited ICSS version-labelled engine names with role-based names:
  `node/layout`, direct `new`/`load`/`save`, `Header`, `HEADER_SIZE` and `clear`.
  Remove obsolete upstream migration comments and the retired-layout diagram.
  Public APIs, persisted discriminators and existing stored bytes are unchanged.

### ⚡ Performance

- Remove the small-node child-read threshold and use the existing direct
  address-vector reader throughout. Relative to the prefix-reader step, the
  depth matrix improves 552/720 cells with no regressions (up to 5.59%);
  broad benchmark Wasm shrinks another 14,563 bytes and the small all-collection
  consumer shrinks 191 bytes. Stored images remain unchanged.
- Read each B-tree node header and adjacent overflow pointer together, sharing
  validation across key types. The matched depth study improves 552/720 cells
  with no regressions (up to 2.54%); Vector and Cell remain unchanged.
  Broad benchmark Wasm shrinks by 16,278 bytes; the all-collection small consumer
  grows by 41 bytes. Public APIs and stored images are unchanged. See the
  [measurement](../../docs/audits/recurring/storage-shared-prefix-2026-11-08.md).

### 🔧 Tooling

- Adopt IC Host Artifacts/FS/Process 0.76.0 with compatible IC Build 0.80.0
  for native repository tooling; select the matching release-metadata CLI.
  Production dependencies and stored images are unchanged.
- Select Toko Tooling 0.36.0 through exact workspace dependencies inherited by
  every real package. Normal Cargo lockfiles replace the separate tooling lock;
  deployment checks member declarations, Cargo selection and installed receipt.
  Resolve intentional pin changes with Cargo, then run `make shared-tooling-update`.

## [0.69.0] - 2026-11-08

### ⚠️ Breaking

- Ordinary Map, Set, Vector and Cell operations and iterator items return their
  successful values directly. Remove redundant `?`, `unwrap` and `expect` calls;
  constructors, reservations and map-region admission retain typed errors.
  Remove the ineffective map cache setting and obsolete collection error variants.
  Codec failures still trap; current stored images need no reset or migration.

### 🔧 Tooling

- Adopt Shared Tooling 0.31.12, including clearer CLOC terminal output and the
  updated issue-maintenance policy. Existing release and publication flows remain unchanged.

### 🧹 Cleanup

- Use one memory and bounds contract, one Vector engine and simpler node
  representations. Carry canonical admitted keys through bootstrap without rebuilding
  equivalent sets. Preserve governed custody, native failure handling and disk formats.

### 🧪 Testing

- Use one structured type catalogue across both benchmark backends, preserving
  concrete type coverage and workload scopes. Retire historical Vector-layout
  probes and obsolete allocator predictions; retain independent ICSS comparisons.

## [0.68.11] - 2026-11-08

### ⚡ Performance

- Remove the redundant allocation-ID B-tree and search validated entries directly.
  Minimal consumers shrink by 2.9 KB; compact-loop timing is mixed (up to 7.60%
  slower for a Set case). Governance, APIs and stored images are unchanged.

- Share compiled B-tree code between ordinary maps and bounded regions. Full
  benchmark Wasm drops by 2,589,089 bytes and the consumer fixture by 9,183 bytes;
  memory dispatch adds instruction overhead. APIs and stored images are unchanged.

### 🔧 Tooling

- Adopt Shared Tooling 0.31.10 and IC Build 0.79.0 for owned-workspace formatting,
  nested LOC reports and the native release helper/metadata editor. Preserve
  runtime contracts and keep private maintenance separate from publication. CI
  adopts shared download caching and one repository-wide secret scan.

### 🧪 Testing

- Add two isolated, matched small-consumer Wasm crates and `benchmark-storage-size`;
  report stripped sizes only after installed operation and upgrade checks pass.
  Retain offline section/function inspection and attribution reproduction commands.

- Replace the old Platform reference facade with direct published ICSS in the
  unpublished benchmark workspace; preserve the modified production engines.

- Add an opt-in compact Wasm comparison using the existing depth cases for both
  Storage and ICSS. Keep full-catalogue coverage as the default and distinguish
  fixed governance costs from type-matrix expansion. [Details](0.68.md).

## [0.68.10] - 2026-11-08

### ⚡ Performance

- Share Cell memory-writing code across value codecs to reduce Wasm size, with
  unchanged APIs and stored bytes. Measured 32-byte writes trade 22 instructions
  each for the smaller binary. [Details](0.68.md).

## [0.68.9] - 2026-11-08

### ⚡ Performance

- Combine Vector header reads/writes and reuse engine framing admission. Preserve
  public APIs, stored bytes and failure behavior; reduce reopen instructions and
  benchmark Wasm size. Add a matched reopen/initialization workload against ICSS
  and buffer large population reports. [Details](0.68.md).

## [0.68.8] - 2026-11-07

### ⚡ Performance

- Borrow `u8` and `bool` encodings instead of allocating temporary byte buffers.
  Preserve decoding, stored bytes and public APIs. Extend whole-handler benchmark
  probes to custom type selections. [Details](0.68.md).

## [0.68.7] - 2026-11-07

### 🔧 Dependencies

- Adopt Platform Runtime 0.20 for Wasm stable-memory IO. Preserve the IC0-only
  production boundary, public APIs and stored layouts; qualify direct Runtime
  composition and the existing canonical CDK entrypoints. [Details](0.68.md).
- Qualify the pending root-lock update to IC Wire 0.17.11. Its new bool adapter
  is opt-in and unused here; Storage's existing codec behaviour is unchanged.

## [0.68.6] - 2026-11-07

### ⚡ Performance

- Avoid redundant B-tree overflow-tail writes when saving unchanged or growing
  chains, skip empty node-field writes and return empty values without payload
  reader setup. Preserve page reclamation, stored layouts and public APIs.
  Matched benchmark Wasm is smaller. [Details](0.68.md).
- Avoid searching a non-full B-tree root twice when inserting or updating a key
  held below it. Full-root replacement still avoids an unnecessary split.

### 🔧 Dependencies

- Refresh the root locked IC Wire selection to 0.17.10. Its published codec
  sources and dependency contract are unchanged from 0.17.7.

## [0.68.5] - 2026-11-07

### ⚡ Performance

- Combine stable Cell header writes, reducing write overhead without changing
  stored bytes, public APIs or cached-value behaviour. [Details](0.68.md).

### 🔧 Tooling

- Adopt Host 0.75.0 and matching IC Build 0.78.0 for native repository tooling.
  Prepare the updated metadata editor with `make install-release-tools`.
  Production dependencies and collection APIs are unchanged. [Details](0.68.md).

## [0.68.4] - 2026-11-07

### ⚡ Performance

- Reuse large-key read buffers when decoding owning B-tree keys, avoiding an
  extra String/byte-vector copy. Keep keys without destructors on their existing
  path. Public APIs, stored layouts and allocation governance are unchanged.
  [Details and measured tradeoffs](0.68.md).
- Find the last remaining Map/Set iterator entry through the reverse cursor,
  avoiding a full forward traversal. Bounded and partially consumed iterators
  retain their results; no stored-layout migration is required.

### 🔧 Tooling

- Adopt IC Build 0.77.0 for the native helper and release-metadata CLI.
  Its removed cache-timing getters have no Storage callers; public collection
  APIs and formats are unchanged. [Details](0.68.md).

## [0.68.3] - 2026-11-07

### ⚡ Performance

- Remove a temporary routing-address allocation and batch small fixed-width
  B-tree key reads across Map, Set and regions. Preserve public APIs, layouts
  and allocation governance. [Details](0.68.md).

### 🧪 Testing

- Extend depth comparisons with full-handler and bounded read/allocation probes,
  separating inner-loop costs from complete synthetic calls. [Details](0.68.md).

### 🔧 Tooling

- Adopt IC Build 0.76.0 for the native helper and release-metadata CLI. Align
  both workspaces on IC Wire 0.17.6 without changing stored codecs or public
  Wire requirements. [Details](0.68.md).

## [0.68.2] - 2026-11-06

### 🔧 Tooling

- Adopt Shared Tooling 0.31.0 with automatic grouped release capture, centralized
  candidate setup and shared batch recovery commands. Include snapshot checksum
  validation fixes and retire the old opt-in interface.
  [Details](0.68.md).

## [0.68.1] - 2026-11-06

### 🔧 Tooling

- Adopt Shared Tooling 0.30.7, enable lexical CBOR source checks and use its
  central CBOR dependency policy for the main workspace. [Details](0.68.md).
- Adopt IC Build 0.75.0 for the native repository helper and release-metadata
  recovery CLI. [Details](0.68.md).

### 🩹 Fixed

- Vector iterator `last()` reads only the final remaining live record, including
  after truncation or partial consumption. Collection layouts and successful
  results are unchanged; discarded records are not decoded. [Details](0.68.md).
- Map reopening and region operations admit framing from the header they load,
  removing a repeated read while preserving typed rejection. [Details](0.68.md).

### 🧪 Testing

- Add native Map/region IO and codec attribution against ICSS using both governed
  Storage backing and ICSS's own manager. [Details](0.68.md).
- Add a repeatable population-depth matrix across all five collection forms,
  with hot/uniform access, mutation histories and bucket-size controls. [Details](0.68.md).
- Add a Vector read/update history preset with scalar, fixed-byte, chunk and
  Wire values, reporting individual rounds alongside aggregate costs. [Details](0.68.md).
- Add frozen-Wasm diagnostics for stable reads, allocator accesses and separate
  full-handler comparison matrices, preserving existing heap layout. [Details](0.68.md).

### ⚡ Performance

- Avoid temporary heap allocations for Vector Get/Pop of small fixed-width
  values without destructors. Preserve owned decoding for other values; qualify
  cutoff boundaries and matched loop/full-handler comparisons. [Details](0.68.md).
- Resolve allocation indices at lease admission and avoid repeated region extent
  checks while preserving complete bounds admission and native failure handling.
- Restore ICSS's direct uninitialized-buffer read path through governed memory.
  Existing custom backings retain a default implementation. Wasm now requires
  published Platform Runtime >=0.19.4. [Details](0.68.md).

## [0.68.0] - 2026-11-06

### ⚠️ Breaking

- Hard-cut every collection to private copied ICSS 0.7.2 engines, retaining
  governed handles and successful-call APIs. Previous collection images require
  explicit rebuild/reset or fresh allocation; codec and engine failures trap.
  No collection checksums or legacy readers remain. [Details](0.68.md).

### 🔧 Changed

- Remove competing engines, custom block/record formats, node cache and Vector
  windows/epochs. Keep allocation governance and application codecs unchanged;
  qualify current-image interoperability with published ICSS. [Details](0.68.md).

### 🧪 Testing

- Generate a shared 73-type, 565-case Storage/ICSS matrix across all five supported
  collections, including ICSS's complete B-tree type grid and Storage's extra
  types. Verify results and runtime refusals, reporting unavailable comparisons
  explicitly. [Details](0.68.md).

### 🔧 Tooling

- Compact experimental evidence into current summaries and selected results;
  retire bulk exports and obsolete prototype patches while preserving current
  code and release/recovery records. [Details](0.68.md).

- Adopt Shared Tooling 0.30.5, enforce the Candid source boundary and CBOR
  dependency ownership in both workspaces, and keep new raw benchmark evidence
  under `target/evidence/`. [Details](0.68.md).

- Add per-read alignment diagnostics over original Vector images, explaining
  the 80k/160k instruction steps and untimed pre-read accounting. [Details](0.68.md).

- Expand Storage/ICSS Vector comparisons with a repeatable iterator preset,
  boundary lengths through 256 items, direct skipping, alternating ends and
  interleaved mutation. Keep wide-value controls separately bounded. [Details](0.68.md).

- Add a bounded ordered Map/Set comparison preset with iterator and range
  coverage around tree-node boundaries. Derive disjoint missing-key traces from
  batch size so larger runs cannot remove newly inserted records. [Details](0.68.md).

- Cover session-shaped Vector lookups at 1–2,048 records, with separate outer
  codec counters and query/update measurements; assert bounded native lookup IO
  and one decode after reopening. [Details](0.68.md).

## [0.67.1] - 2026-11-05

### 🔧 Tooling

- Adopt Shared Tooling 0.30.0; align fixture, benchmark and helper directories with
  LOC rules, replace broad lint allowances with fixes and scoped expectations,
  and escape captured diagnostic controls. [Details](0.67.md).

## [0.67.0] - 2026-11-04

### ⚠️ Breaking

- Adopt IC Wire 0.17 for StorageConfig::wire and Storage codec errors. Hosts exchanging Wire limits or
  errors must align their dependency to the new type identity. Codec bytes,
  budgets and stored layouts are unchanged; no backing reset is needed.
  Codec-only builds no longer select Wire's metadata derive. [Details](0.67.md).

## [0.66.3] - 2026-11-04

### ⚡ Performance

- Use a two-way Map/Set node cache so a protected ancestor can coexist with a
  colliding descendant, retaining the heap-byte budget and existing address hash.
  Selected baseline batch traces improve by up to 26.9%, with up to 1.2% warm
  and 1.5% cold whole-trace regressions. No allocation or persisted-format change.
  [Scope and tradeoffs](0.66.md).

- Replace inline Map values directly in encoded node frames, avoiding temporary
  buffers for unchanged entries. Preserve stored formats and integrity checks.
  Matched 256-row tests reduce inline replacement instructions by 3.7–7.5% for single
  updates and 19.5–26.6% for batches of 32; see [tradeoffs and scope](0.66.md).
- Decode Map/Set cursor keys directly from validated node buffers, avoiding a
  temporary encoded-key allocation. Key-only scans also skip unused inline-value
  buffers. Preserve range ordering and integrity checks. A matched 64-record run
  reduced scalar Set scan instructions by 7.9% and array-Map key scans by 15%.
  [Investigation](../../docs/audits/recurring/storage-icss-hotspots-2026-11-04.md).

### 🔧 Tooling

- Apply qualified group selection to private adapter PRs. Scope lint, feature,
  rustdoc and Wasm compilation to the admitted adapter in CI and release plans.
  Preserve full checks for core/shared/unknown changes and non-docs pushes, plus
  workspace archives, runtime qualification and native evidence obligations.
  [Selection and validation](../../docs/validation/storage-release-validation.md).

### 🧪 Testing

- Add a focused small-record capacity/reuse study covering external-value
  allocation boundaries and metadata IO. Experimental denser layouts remain
  research only; production allocation policy and retained formats are unchanged.
- Expand ICSS comparisons with a size-boundary workload, sequential access and
  bounded byte vectors in the default matrix. Preserve matched operations and
  independent result validation. Add per-round summaries so aggregate medians
  do not hide first-round versus mutated-state costs; production behavior is unchanged.
  [Details](0.66.md).

## [0.66.2] - 2026-11-04

### 🔧 Changed

- Decode borrowed map-entry values without an intermediate encoded-buffer clone;
  preserve consuming decode, codec errors, public APIs and stored formats.
- Extend B-tree histories and the shared-backing study across changing key/value
  sizes. Bulk benchmarks now exercise actual growth and check IC reply limits.
  See the [ICSS re-audit](../../docs/audits/recurring/storage-icss-hotspots-2026-11-04.md).

- Adopt Shared Tooling 0.29.4 (`628ccc7`), including the earlier UTF-8 evidence
  fixes and new test-layout, Metrics ownership and group-changelog rules.
- Guard Git-side release state against excessive retention and nested build caches,
  without deleting recovery evidence. Keep dependency-adoption evidence under `target/`.
- Move inline unit tests and private fixture helpers into `tests.rs` or `tests/`
  across Storage, its native helper and the independent qualification workspace.
  Preserve behavior and ordinary module discovery. CI-selection fixtures now
  remain under `target/ci-selection/`.

- Move remaining consumer fixture scratch under workspace `target/`, preserving
  failed evidence and explicit alias probes. Refresh the active audit guide for
  Storage's directory/lease model and Rust 1.99.1.

- Select eight-page buckets for the ICSS research comparison by default after
  matched 1/8/128-page measurements; retain explicit stock-128 and low-memory-1
  controls. Storage's production allocation policy is unchanged.

- Add a native shared-backing experiment for 32/64 allocations, mixed record
  sizes, interleaved growth and repeated churn, plus controlled external-value
  allocation-boundary probes. Keep production geometry and defaults unchanged.

## [0.66.1] - 2026-11-03

### 🔧 Changed

- Load the release stage catalog from admitted source at runtime, preserving
  standalone bootstrap and archive builds without embedding repository-root files.

- Qualify the native Rust group-proof collector and retained-proof recovery
  against real Shared runner/verifier evidence, including failures and tampering.
  [#156](https://github.com/dragginzgame/ic-storage/issues/156).

- Adopt Shared Tooling 0.28.2 (`5cf67f8`), centralize group evidence and
  baseline observation, and derive full dispatch from one stage catalog.
  Preserve release projection recovery, existing proof and pending work.

- Add consumer-informed `control`, `records` and `bulk` benchmark recipes,
  including byte vectors and opt-in 64 KiB/1 MiB records. Record sibling memory
  declarations and sizing boundaries; production defaults remain unchanged.

- Select release validation by proven change scope. Private adapter-only changes
  can omit unaffected checks; core/shared changes and repository CI retain the
  full roster. Group receipts preserve stage evidence for safe recovery.
  [Details](0.66.md).

## [0.66.0] - 2026-11-03

### ⚠️ Breaking

- Adopt Wire 0.16 for public codec errors and limits. Hosts exchanging those Rust
  values must align their Wire dependency; stored bytes and reopening stay unchanged.

### 🔧 Changed

- Adopt Shared Tooling 0.27.29: preserve implementation in authorized durable
  checkouts, require explicit direct-push and alternate-delivery authority, and
  reject incomplete tool-store inventories. Release planning supports private
  path-only development members; the tooling update preserves Storage's package APIs and formats.
  [Details](0.66.md).

## [0.65.5] - 2026-11-03

### 🧪 Testing

- Add an explicit ICSS allocation-bucket setting to the installed comparison
  across all supported structures; record it beside instruction and memory results.

### 🔧 Changed

- Adopt Platform Runtime 0.19 for IC0-only Wasm backing and Platform 0.19's
  fallible native qualification client. Preserve the optional execution boundary,
  public Storage APIs and stored formats; requalify installed lifecycle behavior.
- Encode B-tree nodes directly into their final checksummed buffer, removing a
  temporary payload allocation and copy. Membership checks skip value copies,
  and map reads decode inline values from the checked node buffer. Public APIs,
  stored formats and staged publication are unchanged. [Details](0.65.md).

## [0.65.4] - 2026-11-02

### 🔧 Changed

- Prepare and publish directory mutations under one exclusive store borrow,
  removing the copied base image and intermediate candidate indexes. Preserve
  backing verification, resource limits, poisoning and root-last publication.
- Admit source-projected release lockfiles with locked, offline Cargo fetching
  instead of a second dependency-update step. Keep exact source and archive checks.
  Public APIs and stored formats are unchanged. [Details](0.65.md).
- Reuse modified nodes and inspected siblings through B-tree deletion repair,
  and read external values into one checked payload buffer. Preserve staged
  publication, checksums and the current collection formats.

## [0.65.3] - 2026-11-02

### 🔧 Changed

- Scale optional map cache capacity with its byte budget and protect upper routing
  nodes from deeper-node collisions. Keep cache accounting and owner poison checks.
- Materialize mutable routing nodes only when they change; propagate child counts
  to avoid redundant rebalance probes. Public APIs and stored formats are unchanged.
  [Details](0.65.md).

### 🧪 Testing

- Correct comparison phase semantics: missing removals use separate keys, value
  growth precedes removal, and reinsertion follows removal. Check phase preconditions
  before execution and explicitly clean up inserted keys between rounds.
- Name above-population lookups `MissAbove`; retain historical mixed-trace results
  without reinterpreting their old labels. Add native cache and node-copy counters.

## [0.65.2] - 2026-11-02

### 🐛 Fixed

- Use Runtime >=0.18.1's safe IC0-only stable64 backing on Wasm. Default Storage
  no longer activates execution, CDK, guard, Types or Candid; native production
  remains Runtime-free. APIs and stored formats are unchanged. [Details](0.65.md).

## [0.65.1] - 2026-11-02

### 🔧 Changed

- Add checked, read-only allocation and physical-capacity diagnostics, including
  directory reservations and live lease counts. [Details](0.65.md).
- Simplify directory preparation by removing temporary indexed state and carrying
  the owned snapshot through reservation sizing.
- Split oversized free B-tree blocks when reusing them for smaller records,
  retaining the unused halves for later allocations. Stored formats are unchanged.
- Add opt-in byte-budgeted map node caching and bounded key-only map ranges.
  Read traversal retains one validated node buffer; mutations avoid unchanged
  ancestor writes and release excess capacity after substantial record shrinkage.
- Read fixed-size vector slots in one operation and reuse iteration buffers.
  Cache adjacent extent translations while preserving custody and bounds checks.

### 🧪 Testing

- Generate and minimize mutation/reopen histories with live/free range checks.
  Extend comparisons with a pinned upstream-head control, shuffled populations,
  Zipf access, repeated mutation rounds and explicit map cache controls.

### 🐛 Fixed

- Stop bounded scans before reading excluded external values, and defer reading
  following subtrees until another entry is requested. Unrelated corruption no
  longer fails an otherwise valid bounded scan.

### 🧹 Cleanup

- Remove the unused Testkit Native development dependency. Native backing IO
  metering already uses Storage's `test-support` helpers. [Details](0.65.md).

## [0.65.0] - 2026-11-02

### ⚠️ Breaking

- Adopt Wire 0.15 for public codec errors; hosts exchanging those types must
  align their Wire dependency. Codec bytes and budgets are unchanged.
- Replace the AVL engine with a multi-entry B-tree shared by Map, Set and regions.
  The new `ICMAP002` format requires explicit fresh/reset collection state and
  updated region framing admission. Public Rust APIs remain unchanged. [Details](0.65.md).

### 🔧 Changed

- Keep small values inline and load larger values separately; membership and
  key-only scans avoid unrelated payloads. Skip unchanged root metadata on replacement.
- Adopt Shared Tooling 0.27.27, Runtime/Platform 0.18, Build 0.74.0,
  Host libraries 0.74.1 and Testkit 0.79.0. Preserve the comparison-only
  Platform 0.17 stable-structures facade; production imports Runtime directly.
- Resolve the Platform 0.18 executor-guard conflict by using Runtime 0.18 directly
  ([#152](https://github.com/dragginzgame/ic-storage/issues/152)); qualify composition
  with the facade's runtime feature on native and Wasm.
- Restrict Storage's Runtime dependency to Wasm, removing it from native production.
  IC0-only Wasm backing remains blocked by [Platform #135](https://github.com/dragginzgame/ic-platform/issues/135).

### 🧪 Testing

- Add native `testing::MeteredMemory` and `MemoryIoCounts` under `test-support`,
  preserving backing custody, shared counters, reset and overflow detection.
- Expand installed comparisons to every supported structure and 28 concrete cases,
  including signed/wide keys, float vectors, inline-boundary and 4 KiB values,
  endpoint/bounded scans, growth/shrink and removal/reinsertion. Report physical
  stable pages alongside instructions.

## [0.64.0] - 2026-11-02

### ⚠️ Breaking

- Return typed codec/collection errors, including fallible map/set iteration and
  lazy value decoding. Require an explicit byte ceiling for cell loading.
- Protect cell/vector payloads with checksums (`ICCELL01` / `ICVECC01`). Reuse
  inactive directory arenas (`ICROOTR1` / `ICDIRR01`) and coalesce contiguous
  allocation growth. This extends the pending fresh/reset-backing hard cut.
- Extract the AVL deletion successor in one traversal. Native tests cover
  read-only error paths, repeated arena reuse and interrupted reuse/reopen.

- Rename the repository to `ic-storage` and release groups to `storage`,
  `metrics-storage` and `logging-storage`. Use the new adapter package names;
  published history and prior release receipts retain their original identities.

- Consolidate allocation and collections into `ic-storage`. Remove the `ic-memory`
  package; hosts and adapters must import runtime, key and lease APIs from
  `ic_storage`. No compatibility crate remains. [Details](0.64.md).

- Restrict collections to sibling-used Cell, Map, Set, Vector and map regions.
  Remove `StableLog`, `StableMinHeap`, `LogIter` and `LogWriteError`; no sibling
  application users were found in the local checkout scan.

- Replace the fixed-slot manager, upstream collections and codec/backing traits
  with owned named allocations and collection engines. Remove
  `ic-stable-structures`; adopt `StorageConfig`, namespace `AllocationGrants`
  and opaque u64 IDs. Fresh/reset whole backing is required: no legacy decoders
  or automatic reset. [Migration and limits](0.64.md).

### 🧹 Cleanup

- Avoid rewriting unchanged map ancestors on replacement, retain traversal
  cursors for scans, and index allocation extents for address translation.
  These changes preserve the current owned formats and public APIs.

- Share one owned AVL engine across Map, Set and regions; use bounded indexed
  slots for Vector. Retain exclusive custody, staged writes and corruption checks.

### 🧪 Testing

- Add `make benchmark-storage` for matched installed map operations against
  DFINITY, with validated results and retained instruction samples. The comparison
  stays outside production dependencies and release gates. [Methodology](../../qualification/ic-storage-benchmarks/README.md).
