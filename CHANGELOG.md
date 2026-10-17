# Changelog

## [0.4.1] - Unreleased

- Add a governed bounded stable-cell adapter. Encodings are checked before writes
  and on reads; rejected oversized replacements preserve stored bytes and cached values.

## [0.4.0] - 2026-10-17

- Adapt the benchmark runner to IC Testkit 0.51's explicit call principals and
  installation specification, retaining anonymous calls and bounded replies.
- Validate overflow keys in one pass while retaining page ownership and
  corruption checks; reuse endpoint descent for unbounded cursor seeks.
- Hash the benchmark's loaded Wasm bytes with IC Host Artifacts rather than
  reopening the file, so the reported identity describes the installed bytes.
- **Breaking adapter exports:** remove `CellInitError`, `CellValueError`,
  `LogInitError`, `NoSuchEntry` and `VecInitError`, which no supported wrapper
  method returns. Consumers must remove those imports; `LogWriteError` remains.
- Share benchmark operation IDs, labels, ordering and seed limits between the
  runner and canister, preserving existing `u8` wire values and CSV labels.
- Give inline-record capacities and payload initialization one codec owner;
  insertion, replacement and decoding retain the exact V1 persisted bytes.

- Validate each operation's input key once, preserving key-before-value error
  ordering. Share committed/staged overflow reads and isolate read-only tree
  auditing from mutation transactions without changing persisted bytes.

- **Breaking dependency contract:** consume IC Memory 0.46 non-cloneable
  allocation leases. A second open through the same runtime is rejected until
  the owning collection is dropped; failed fallible constructors release leases.
  Applications must update their IC Memory handles to the 0.46 API.
- Use IC Memory's narrow root exports for host backing setup; retain collection
  access through IC Storage. Hosts still own exclusive backing custody across
  runtimes. Persisted formats remain unchanged; no reset is required.

- Share node search between committed reads and transactions, retaining staged
  page visibility and borrowed comparisons for single-page overflow keys.
- Consolidate upstream benchmark operations and bounded scans while preserving
  each fixture codec and lazy value loading.

## [0.3.1] - 2026-10-17

- Remove the membership preflight from deletion. Missing-key transactions
  discard speculative balancing without publishing changes; misses beneath
  minimum-size children can perform more reads.
- Reuse IC Testkit installation and one fresh-instance setup path across
  both benchmark suites, retaining independent memory per comparison.
- Replace the local dependency-boundary parser with a revision-pinned Shared
  Tooling helper and a consumer-owned package allowlist.

## [0.3.0] - 2026-10-16

- Adopt IC Memory 0.45 allocation requests and IC Testkit 0.50 caller-selected
  Candid reply limits in the adapter tests and benchmark. Preserve incoming
  dependency updates, including IC Host FS 0.24.
- Qualify both wrapped engines with governed-memory mixed workloads: 256
  measurements repeat exactly with matching model results. Row instruction
  totals are near parity; index totals remain 1.84 times upstream. Retain this
  separately from earlier raw-memory performance evidence.

- Require IC Memory runtime handles for public collections. Replace generic
  public engine access with an explicit wrapper and remove memory extraction.
- Add governed wrappers for upstream maps, sets, cells, vectors, heaps and logs,
  with supported codecs and borrowed iterators through `ic_storage::adapter`.
  Persisted formats and upstream failure behavior remain unchanged.
- Allow private substrate dependencies in storage infrastructure only; check
  workspace consumers for direct dependencies, including renamed imports.
- Move raw-memory differential tests behind the private engine and qualify
  public reopening plus compile-time rejection of collection bypasses.
  Exclusive leases still require IC Memory support; no exclusivity is claimed.

## [0.2.0] - 2026-10-16

- Raise the library's minimum supported Rust version from 1.85 to 1.91 at the
  maintainer's request, updating the MSRV check and Linux/macOS CI setup.
  Consumers building with older compilers must upgrade to Rust 1.91 or later.
- Update the native benchmark's ICU dependencies to 2.3.x and time to 0.3.55
  under the new compiler floor, without disabling resolver fallback.
- Reject branch-vector preallocation experiments after the mixed workload
  exposes row-scan regressions despite gains in the older matrix.
- Compare single-page overflow keys directly from verified page buffers during
  binary search, avoiding temporary key vectors. Mixed-index reads improve
  2.05% and mutations 1.4–1.9%; unrelated matrix cases regress at most 0.12%.
  Key integrity checks, custom ordering and stored bytes remain unchanged.
- Add canonical-store workload comparisons with variable-size rows and keys,
  nonsequential insertion, repeated folds/deletes/refills and bounded forward
  and reverse scans, checked against an independent heap model.
- Trace committed IcyDB codecs and retain isolated layout/checksum experiments.
  Wider fixed inline values regress most cases and are not adopted; large-read
  diagnostics identify checksum verification as the dominant measured cost.
  Production storage behavior and persisted format remain unchanged.

## [0.1.6] - 2026-10-16

- Reuse the validated full node during splits instead of rereading it.
  Root-splitting insertion improves 8.9%, and new insertion in the 2,048/4,096
  entry index fixtures improves about 3.3%, with unchanged split semantics.
- Reuse iterator boundary-key capacity across scan steps. Scan instructions
  improve by up to 10.6% in the synthetic matrix; some later mutation cases
  increase by at most 0.25%. Forward, reverse and interleaved range semantics
  remain unchanged.
- Reuse page buffers during reclamation and free-list allocation. The nonzero-key
  4 MiB fixture removes values in 85M rather than 103M instructions, about 29%
  below upstream; regrow and refill improve about 5%. Validation and stored
  bytes remain unchanged.
- Read overflow chains and tree nodes into caller-owned buffers, avoiding
  intermediate page copies while retaining full validation. In the nonzero-key
  fixtures, 17-byte value scans improve about 18%, 4 MiB reads about 14%, and
  4 MiB changed replacement falls from 133M to 120M instructions, below upstream.
- Encode tree nodes and reclaimed pages directly in heap buffers, avoiding
  intermediate 4 KiB copies. The nonzero-key 4 MiB fixture uses about 4% fewer
  instructions for shrink and removal; root-splitting insertion improves 10%.
  Persisted bytes and validation guarantees remain unchanged, with no
  instruction regressions in the expanded comparison matrix.

## [0.1.5] - 2026-10-16

- Fold freshly initialized overflow-page padding directly into its checksum.
  Small overflow replacements use about 1.7–1.9% fewer instructions; dense
  large-value cases increase by at most 0.042% in the expanded matrix. Reads
  still verify every byte, and exhaustive payload-length tests preserve the
  exact persisted encoding.
- Expand the canister comparison to 23 fixtures, covering root splits, the
  32/33-byte key boundary and intermediate row sizes. Reuse validated nodes
  during insertion instead of rereading them for descent: small inline-entry
  insertion improves about 17%, and small overflow-row insertion about 8–9%.
  Stored format, comparator behavior and failure guarantees are unchanged.
- Inspect zero-filled checksum tails in larger blocks while retaining the dense
  payload fast path and identical persisted checksums. The nonzero-key 17-byte
  fixture improves a further 3.1% for reads and 5.8% for value scans; all measured
  cases improve, with negligible gains for large dense rows.
- Skip unchanged node and header writes during value replacement, while still
  validating the complete old payload. Nonzero-key 17-byte changed replacements
  drop from 655k to 279k instructions; 4,072-byte replacements drop from 713k to
  337k, below upstream's 492k. Stored bytes and failure semantics are unchanged.
- Encode overflow pages directly in heap buffers, filling only the unused tail
  with zeros. In the nonzero-key 4 MiB fixture, changed-value replacement drops
  from 146M to 133M instructions and new insertion improves 16%, with identical
  stored page bytes. Same-size replacement remains about 7% above upstream.
- Reuse completed payload validation when freeing excess pages during value
  shrink, avoiding a second read while preserving the free-list order and
  staged-write failure contract. The nonzero-key 4 MiB shrink fixture drops
  from 156M to 103M instructions, below upstream's 121M.
- Add changed-byte replacement to the canister matrix, validating returned and
  stored values separately from identical-byte replacement. Same-size large-row
  replacement still costs more than upstream in these fixtures.

## [0.1.4] - 2026-10-16

- Return removed values while reclaiming their overflow pages in one validated
  pass. In the nonzero-key 4 MiB fixture, deletion drops from 155M to 107M
  instructions, below upstream's 121M, while preserving staged failure handling.
- Reduce checksum work on verified zero-filled page suffixes while preserving
  the exact checksum and persisted format. In the nonzero-key 17-byte row
  fixture, reads use 38% fewer instructions and value scans 52% fewer than 0.1.3;
  large-row read improvements are smaller; replacement and small-value mutation
  costs still exceed upstream.
- Compare both zero-filled and nonzero-filled synthetic keys across the canister
  matrix, exposing how data patterns affect the measured gains.

## [0.1.3] - 2026-10-16

- Expand canister comparisons to 15 fixtures and ten operations, covering
  inline/page boundaries, long keys, tree sizes, misses, value scans and
  shrink/regrow/refill. Assert returned values and post-mutation contents.
- Compute newly constructed free-page checksums using their known-zero padding,
  preserving exact stored bytes and full read verification. The expanded 4 MiB
  fixture reduces removal from 195M to 156M instructions and shrink from 196M
  to 157M; small overflow values and long keys remain significant weaknesses.
- Reduce checksum loop overhead and avoid copying whole page buffers when
  balancing mutation staging. In the canister fixture, 4 MiB replacement drops
  from 216M to 147M instructions and removal from 266M to 195M, preserving the
  persisted format and corruption checks; both remain more costly than upstream.
- Add a non-published canister benchmark and IC Testkit/PocketIC runner for
  instruction, Wasm heap-capacity and stable-capacity comparisons. Keep its host
  dependencies separate from the Rust 1.85 storage library.
- Record repeatable instruction results: key scans favor our tree in the tested
  profiles, while large-row mutations cost more instructions despite less traffic.
- Reclaim overflow pages during removal without rereading each page to free it.
  A 4 MiB row removal drops from 12,677,120 to 8,454,144 read bytes in the native
  fixture; API, persisted format and staged-write failure handling are unchanged.
- Extend bound-matched comparisons through bulk shrink/regrowth, deletion and
  refill, with audits and reopening between phases to exercise persisted reuse.
- Compare upstream storage with IcyDB's declared key/value bounds across seven
  synthetic index, journal and row profiles, including read/write bytes, calls
  and reserved capacity. Add regular bounded-profile mutation/reopen coverage.
- Record how upstream bound metadata changes the earlier capacity comparison;
  actual IcyDB codecs and production subnet costs remain unqualified.

## [0.1.2] - 2026-10-16

- Add direct differential tests against ic-stable-structures 0.7.2 and a heap
  model, including seeded mutation campaigns, semantic key ordering, range
  bounds, large payloads, reopen cycles and rejection of upstream-format bytes.
- Compare native read/write traffic and capacity with upstream trees, including
  bounded index types; record both small-entry costs and large-payload benefits.
- Reuse the located node and read the previous overflow value once during
  replacement, halving read traffic in the native index/journal/row fixtures.
  The public API, persisted format and staged-write failure contract are unchanged.
- Add an opt-in stable-memory traffic qualification for gets, replacements,
  forward/reverse key scans and removals. Native byte counts do not establish
  canister instruction costs or production IcyDB performance.

## [0.1.1] - 2026-10-16

- Synchronize the local Cargo lockfile identity with the current 0.1.0 manifest
  so locked release preflight can run without changing dependency selections.
- Replacing a stored value reuses its overflow pages and leaves tree topology
  unchanged. Same-size or shrinking updates no longer need a second payload's
  worth of capacity; failed growth still preserves the existing data.
- Qualify semantic encoded-key ordering through multi-level tree mutations,
  mixed inline/overflow payloads, bounded bidirectional scans and reopening.

## [0.1.0]

- Establish the repository with the Dragginzgame engineering baseline, pinned
  developer tooling, Linux/macOS CI and standard release commands.
- Add the Rust workspace and library scaffold, upstream ownership guidance,
  Cargo release metadata, formatting hooks and guarded crates.io publication.
- Add an independent stable-memory B-tree with custom key ordering, lazy
  bidirectional scans, bounded large values, reusable pages and fallible
  corruption/capacity handling. Its new format requires fresh storage; IcyDB
  adoption and canister recovery qualification remain separate.
