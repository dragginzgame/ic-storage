# Current handoff

The benchmark runner now follows the selected IC Testkit 0.51 API. Both seed
calls pass an explicit anonymous principal; installation constructs InstallSpec
with the prior Wasm/init bytes, cycle top-up and unset sender/label. Measurement
calls and reply limits are unchanged. This resolves the five native compiler
errors in the 20261017T101544Z release validation attempt.
Focused benchmark all-target checking, native/Wasm Clippy and the host protocol
test pass with the incoming dependency selections (IC Memory 0.47, IC Host
Artifacts/FS 0.25 and IC Testkit/Candid 0.51). No manifest, lockfile, package
version or Git index/ref changes are made. The full release gate and simulator
benchmarks are not rerun for this API repair. Earlier evidence follows.

The latest 0.4.0 cleanup reads overflow keys once during ownership auditing,
shares unbounded cursor descent and hashes the benchmark's loaded Wasm bytes.
IC Host Artifacts 0.24 becomes an explicit native runner dependency; no selected
dependency version, public API or persisted format changes. All 38 regular
storage tests, three adapter tests, the protocol test and native/Wasm Clippy
pass. The three new audit/cursor regressions pass on Rust 1.91. The page-read
regression fails with the old audit and passes with the new one.
Three mixed-workload builds pass 768 measurements with exact repeats and
unchanged capacities/results. The combined change increases row instructions
0.6899% and index instructions 0.0084%; one row scan increases 33.7659%.
Isolation attributes almost all of the increase to the audit change; cursor-only
totals increase by 160 instructions per workload. The audit is retained for its
single-pass I/O and ownership checks, not as an overall instruction speedup.
See [the qualification](../design/canonical-workloads.md#single-pass-key-audit-and-shared-cursor-descent).
The fixture matrix, extended campaign and full release gate are not rerun.
Package versions, Git index and refs are unchanged. Earlier evidence follows.

The latest 0.4.0 cleanup removes five unused adapter error re-exports while
retaining LogWriteError, centralizes the benchmark protocol and moves inline
record initialization/limits into the page codec. Existing wire discriminants,
labels and seed schedule are frozen by a host protocol test. Independent leaf
and branch page expectations prove the 32/16-byte inline boundaries, replacement
zeroing and child offsets retain V1 bytes. The byte test passes on Rust 1.91.
All 35 regular storage tests, three public adapter tests, protocol test and
Rustdoc examples pass. Native and Wasm Clippy are clear after resolving shared
module target usage and enum-import/branch lints. The new runner completes the
previous canister's mixed workload with an identical CSV. Both suites pass
against the new canister, with exact repeats and unchanged capacities/results.
The matrix has 230 unchanged and 276 increased costs, at most 0.0586%; wins
against upstream remain 381/506. The 320,000-operation differential campaign
passes. See [the qualification](../design/canonical-workloads.md#shared-benchmark-protocol-and-inline-record-ownership).
No dependencies, package versions, Git index or refs are changed. Earlier evidence follows.

The latest 0.4.0 cleanup removes duplicate input validation from insertion,
shares committed/staged overflow reads and isolates structural audit state
under `btree/audit.rs`. Key-before-value errors and corruption checks remain
intact. All 34 regular native tests, Clippy and the Rust 1.91 validation-count
regression pass. The 320,000-operation differential campaign and Wasm Clippy
also pass. Fresh before/after builds pass both benchmark suites (4,560
measurements total), exact repeats, unchanged capacities/results and model
checks. Mixed totals increase 0.0133% for rows and 0.0103% for indexes; the
worst fixture increase is 0.5139%. We still beat upstream in 381/506 fixture
cases. See [the comparison](../design/canonical-workloads.md#single-input-validation-shared-overflow-reading-and-audit-isolation).
Source snapshots and benchmark attempts preserve the incoming dirty work.
No version, dependency selection or Git index/ref change is made here.


The incoming IC Memory 0.46 update is now qualified. Collection constructors
consume exclusive non-cloneable leases; tests cover concurrent-open refusal,
release on collection drop and release after custom-tree constructor errors.
Host backing imports use IC Memory root exports. The published API now consumes
0.46 handle types, so pending notes move to 0.4.0; package versions are unchanged.
Hosts must retain exclusive backing custody across runtime lifetimes.

All three public integration tests pass on Rust 1.91; the valid README example
and seven compile-fail examples pass. Native library/test and Wasm benchmark
Clippy pass. The leased-memory benchmark passes 256 model-checked measurements
with exact repeats; see [the qualification](../design/canonical-workloads.md#published-ic-memory-046-leases).
IC Memory PR #120 is refreshed against 0.46; its ten fixture tests, test-support
Clippy and Rustdoc pass. The regenerated scratch consumer patch passes all three
adapter tests and Wasm checking. That helper still awaits merge and publication;
its adoption is not included in the registry-backed source here. Earlier notes
below retain historical dependency and qualification identities.

Released source is 0.3.1 (`a7ed957`); pending work is 0.3.2. Committed and
transactional node search now share one private helper with different page
readers; transaction reads still prefer staged pages. The benchmark shares
upstream map operations and scan loops while retaining fixture codec bounds
and key-only laziness. Native storage tests, library/Wasm Clippy and the Rust
1.91 overflow-key corruption regression pass. The 320,000-operation differential
campaign passes against upstream and the heap model. Both benchmark builds pass 256
mixed measurements each, with exact repeats and model checks; details are in
[the workload report](../design/canonical-workloads.md#shared-search-and-upstream-benchmark-operations).

Fixture bootstrap is contributed in IC Memory PR #120 and qualified with
consumer changes in an isolated checkout. Adoption here remains gated on its
registry release; no Git/path dependency is introduced. See
[upstream ownership](../upstreams.md#fixture-runtime-contribution) for the exact
adoption steps and the prepared consumer patch. Package versions and lockfile
are unchanged. Earlier handoff evidence follows.

Released source is now 0.3.0 (`af1f181`); pending work is 0.3.1. Deletion now
uses one transactional search; misses discard all staged balancing and leave
bytes untouched. A successful single-leaf deletion uses one read instead of
two. A missing key beneath minimum-size children uses six reads rather than
the old two-read membership check. This is a known miss-path tradeoff.
Both benchmark modes share fresh-instance setup and IC Testkit installation.
The local dependency parser is replaced by the separately pinned upstream
helper described in [upstream ownership](../upstreams.md#dependency-boundary-helper-adoption).
The incoming Cargo.lock change is preserved; no package versions are changed.
Both benchmark modes compile, and the mixed mode passes 256 measurements with
exact repeats and model checks. Current instruction totals are 0.9194 times
upstream for mixed rows and 1.6444 for mixed indexes; details and retained
artifacts are in [the workload report](../design/canonical-workloads.md#single-search-deletion-and-shared-installation).
Native library tests, library/runner Clippy, formatting, pins, snapshot integrity
and local dependency-boundary checks pass. The 320,000-operation differential
campaign and the deletion regression on Rust 1.91 also pass; Wasm Clippy passes.
The upstream checker passes its
focused rejection fixtures and ShellCheck. No full release gate is run.

Earlier handoff evidence follows.

Released source is 0.2.0; the pending breaking API batch is 0.3.0. Public custom
and upstream collections now use explicit wrappers consuming IC Memory 0.45
runtime handles. No raw inner collection, Deref or memory extraction is exposed.
IC Storage privately depends on both IC Memory and ic-stable-structures; the
benchmark uses the storage adapter. The local dependency guard permits the
substrate only in the infrastructure library. Package versions are unchanged.

Allocation admission is enforced by the handle type; exclusivity is not yet
enforced by IC Memory. The exact upstream prerequisite and dirty sibling
boundary are recorded in [upstream ownership](../upstreams.md#collection-ownership-boundary).
IcyDB adoption and exclusive leases remain outstanding. The governed mixed
workload now passes 256 measurements with exact repeats and model checks; see
[the new baseline](../design/canonical-workloads.md#governed-memory-baseline-with-ic-memory-045).
Rows total 1.0017 times upstream instructions; index totals 1.8389 times upstream.
Prior benchmark numbers used different memory plumbing.
Focused qualification: both public adapter tests pass on Rust 1.91; six
compile-fail examples and the valid documentation example pass. Native library
and Wasm benchmark Clippy, all-target MSRV checking, warning-free Rustdoc,
formatting, links and dependency pins pass. The dependency guard rejects a
renamed consumer import. Full release validation was not run.
IC Memory 0.45 request construction and IC Testkit 0.50 finite Candid reply
limits are adopted in the test and benchmark callers. Incoming dependency
updates are preserved. Public adapter tests on Rust 1.91, documentation tests,
native library/runner and Wasm Clippy, pins and boundary checks pass.
The earlier handoff paragraphs below retain historical qualification.

The maintainer has selected Rust 1.91 as the library MSRV. The root manifest,
`make msrv`, README and Linux/macOS CI setup now agree on 1.91.0; development
and the benchmark retain Rust 1.99. Normal targeted resolution now selects
ICU 2.3.0/provider 2.3.1, idna_adapter 1.2.2 and time 0.3.55 with companions.
The incoming IC Metrics 0.11.7 update remains intact. This consumer compiler
requirement moves the pending changelog to 0.2.0; package versions are unchanged.
Earlier 1.85 and pending-0.1.7 references below describe historical qualification.
The final library passes its all-target MSRV check and 32 regular codec/storage/
upstream tests on Rust 1.91. New MSRV-aware Clippy suggestions use let-chains
and fixed-size slice chunks; native library/test/runner and Wasm benchmark
Clippy pass. The updated runner and final Wasm pass 256 mixed-workload
measurements with model checks and exact repeats. Formatting, pins and links
pass; full release validation and macOS execution remain outside this change.

The latest investigation preserves the incoming IC Metrics 0.11.7 lock update.
ICU/time lag comes from Cargo's Rust 1.85 workspace fallback, not an upstream
version cap: they are native benchmark dependencies, absent from the library
and benchmark Wasm graphs. Targeted dry runs and current registry metadata are
documented in [upstream ownership](../upstreams.md#benchmark-dependency-resolution);
no dependency upgrade or resolver-policy change is made.
Branch-child vector preallocation is rejected: small index gains came with
4.95–5.61% higher mixed row-value scan totals. The incoming production source is
restored and its rebuilt Wasm matches the fresh baseline. Details and both
experimental artifacts are retained in the canonical-workload investigation.

The latest 0.1.7 continuation borrows single-page overflow-key bytes during
binary search, avoiding a temporary allocation/copy without changing persisted
bytes or checksum/chain validation. All 16 mixed-workload operation totals improve;
index reads improve 2.05%, index mutations 1.4–1.9%, reverse row-key scans 10.97%.
In the older matrix, 56 cases improve and 450 increase by at most 0.1164%.
Both suites repeat exactly (2,280 new measurements); upstream counts are unchanged.
The new corruption test covers short/exact-page/chained keys and preserves bytes
on rejected operations. All 23 storage tests pass on Rust 1.99 and 1.85;
four upstream tests, the 320,000-operation campaign and native/Wasm Clippy pass.
Details and retained artifacts are in the investigation linked below. Prior
workload additions remain in the working tree; no release metadata is changed.

Released source is now 0.1.6 at `654a2d7`; older version references below describe
their historical evidence. The pending 0.1.7 batch adds canonical-store workloads
and [copy/layout/read-cost evidence](../design/canonical-workloads.md).
It inspects committed IcyDB `996498ad` without touching its dirty working tree.
Mixed row key scans win, but the new mixed-index workloads lose to upstream.
An isolated 48-byte inline-value prototype regresses 478/506 matrix cases and
is rejected. A deliberately weakened checksum diagnostic isolates 64.8% of
the 4 MiB lookup cost; production verification remains intact. All 6,584 successful
measurements repeat exactly. Native/Wasm benchmark Clippy, formatting and doc
links pass; full release and macOS qualification are not claimed. Production
B-tree source, dependency selections and package versions are unchanged.

The repository implements its own stable-memory B-tree in `crates/ic-storage`.
It supports custom encoded-key order, inline small payloads, overflow chains,
page reuse, fallible operations, lazy bidirectional ranges and structural audit.
The custom engine uses the upstream Memory interface; the adapter separately
uses upstream collection implementations. IC Memory remains the allocation owner.

Read [the IcyDB boundary](../design/icydb-btree.md) and
[the package contract](../../crates/ic-storage/README.md) before changing the
format, comparator, allocator or mutation publication behavior. No IcyDB
checkout changes or retained-data conversion have been performed. Native model
and failure tests and Wasm checks do not establish canister rollback/recovery.

Shared Tooling 0.15.4 is recorded in the root snapshot. Its revision
`879eb2b021522252a67e908dbc217a0f4efb2a6f` matches remote HEAD as checked on
2026-10-16; all 99 exported files pass snapshot verification. Uncommitted sibling
changes are not adopted. Development uses Rust 1.99.0; the library retains
Rust 1.91.0 support following the maintainer's explicit floor change.

The released 0.1.1 patch makes value replacement reuse existing overflow pages
without splitting tree nodes. It preserves the public API and stored format.
Regression coverage includes full-root replacement, overflow/inline transitions,
reopening, page reuse and rejected growth without partial writes.

Additional 0.1.1 qualification exercises 700 length-framed keys against a decoded
key model through splits, mixed inline/overflow replacements, deletions, bounded
interleaved scans and reopening. The focused lifecycle test passes on Rust 1.99
and 1.85; the integration test target passes Clippy. No production change was
needed for this coverage. This remains a synthetic comparator, not IcyDB's codec.
The accepted IcyDB transition is a hard cut without a legacy reader; retained-data
disposition and canister qualification remain required before adoption.

Cargo.toml and Cargo.lock select released 0.1.5, tagged at `caaadcc` (`v0.1.5`).
Current qualification uses IC Host FS 0.22.1 and IC Metrics 0.11.5 from that
release's lockfile. Release metadata is untouched during implementation.

Released 0.1.2 reuses the located node and reads the previous overflow value once
during replacement. Native interface read traffic is halved for the index,
journal-sized value and row fixtures; writes and reserved capacity are unchanged.
The reproducible command and before/after measurements are in the
[IcyDB design](../design/icydb-btree.md#native-memory-traffic-qualification).
The API, persisted format and staged-write failure contract remain unchanged.
Validation: 16 storage tests pass on Rust 1.99 and 1.85; the opt-in traffic
fixture, Clippy, Wasm compilation, rustdoc, formatting and doc links also pass.
This does not establish canister instruction costs, heap usage or IcyDB integration.
Release versions, commits, tags and publication remain maintainer-owned.

Direct upstream qualification now compares both persisted trees with a heap
model: 8,000 regular and 320,000 extended seeded operations passed, plus boundary
ranges, maximum payloads and rejection of actual upstream-format memory. The
four regular tests pass on Rust 1.99 and 1.85; the extended campaign passed in
release mode on 1.99. Traffic fixtures compare unbounded upstream storage and a
bounded index layout. Small index entries cost more bytes/capacity in our tree,
while large-value replacements use less traffic than the unbounded upstream
fixture. Fewer Memory calls do not prove fewer IC instructions; actual IcyDB
schemas and bounded large-value comparisons remain unqualified. See the design
document for reproducible commands, measurements and limitations.

Released 0.1.3 adds profiles using IcyDB's bound metadata from committed revision
`e104fe1c9f5921336f0e5b292acbda99f8311476`: variable index keys up to 16,668
bytes with variable one-byte values; fixed 12-byte journal keys with variable
64 KiB chunks; variable data keys up to 262 bytes with unbounded row values.
Synthetic bytes preserve sizes/order but do not implement the actual codecs.
The seven-profile comparison passes on native Linux; broader index bounds make
upstream capacity much larger than the earlier eight-byte-bound baseline.
Our small-entry read/write byte costs remain higher. See the design document's
bound-matched results before extrapolating from either baseline.

The 0.1.3 continuation extends those profiles through shrink/regrowth and
delete/refill phases, auditing and reopening between phases. All seven profiles
reuse our existing capacity. It also removes a redundant read during overflow
reclamation: `free` returns its validated prior page to chain traversal. The
new traffic regression fails before the fix and passes afterward. A 4 MiB row
removal now reads 8,454,144 bytes instead of 12,677,120. No API, persisted format
or release metadata changes are made.
Validation: all 18 regular storage tests pass on Rust 1.99 and 1.85; the four
regular upstream tests and 320,000-operation extended campaign pass on 1.99.
The seven churn profiles, Clippy, Wasm compilation, formatting and doc links pass.

`ic-storage-bench` now supplies a non-published canister and native runner using
IC Testkit 0.48.0, IC CDK 0.20.3 and IC Host FS 0.21.0. Its Rust 1.99 floor is
separate from the public library's Rust 1.85 floor. `make bench-run` accepts an
explicit pre-admitted server; regular CI compiles the native and Wasm targets,
while simulator execution remains opt-in. No server is downloaded implicitly.

Two fresh runs per engine/profile completed with PocketIC server 16.0.0 and
client 16.1.0 on Linux. Each operation's instruction count repeated exactly.
The measured results change the traffic-only conclusion: our key-only scans
use fewer instructions in these fixtures, but large-row reads and mutations
use more. A 4 MiB replacement costs 215,655,911 instructions here versus
124,318,147 upstream. See the design document for all results, artifact hashes,
measurement exclusions and retained failed/successful attempts. These synthetic
canister results do not qualify actual IcyDB codecs, production subnet costs,
rollback or upgrade recovery. No dependency was added to the library itself.

Instruction investigation retains two compatible optimizations: checksum
iteration in eight-byte chunks with the same FNV-1a byte order, and boxed page
buffers in the transaction staging index. The former reduces loop overhead;
the latter avoids moving 4 KiB buffers when the heap B-tree balances. Combined,
4 MiB replacement falls from 215,655,911 to 146,834,753 instructions and removal
from 265,860,686 to 195,445,266. Total Wasm heap capacity after replacement falls
from 17,367,040 to 13,893,632 bytes. Large-row operations still trail upstream.
Two fresh runs repeat all counters exactly; the design records intermediate
measurements and artifact identity. API, persisted bytes and staging semantics
are unchanged. Two new checksum tests compare the original persisted algorithm
and reject corruption at every page byte. These and all 18 regular storage
tests pass on Rust 1.99 and 1.85; four regular upstream comparisons, the extended
320,000-operation campaign, and native/Wasm Clippy pass on 1.99. Formatting and
documentation links pass. Full release validation and macOS remain outside this run.

The canister runner now covers 15 fixtures and ten ordered operations, with
shared host/canister descriptors. It exercises inline and overflow boundaries,
long keys, tree sizes, misses, key/value scans, shrink/regrow, removal/refill and
new insertions. Each measured build produces 600 rows across two fresh runs;
all counters and result lengths repeat exactly. Bulk value collection replaces
byte-by-byte flattening in the scan harness; earlier scan evidence is retained.

New free pages compute the identical FNV-1a checksum of their known-zero tail
using a compile-time multiplier. Existing pages still verify every byte.
Expanded 4 MiB removal falls from 195,445,228 to 155,867,505 instructions;
shrink falls from 196,375,461 to 156,797,740. Both still trail upstream. Mid-size
rows can win, while 17-byte values and long keys expose substantial losses;
value scans must not be conflated with key-only scans. See the design for the
full matrix, reproducible commands, hashes and measurement limitations.
Three checksum tests and 18 regular storage tests pass on Rust 1.99 and 1.85;
four regular upstream tests and the 320,000-operation campaign pass on 1.99.
Native and Wasm Clippy pass. No public API or persisted format change is made.

Released 0.1.4 folds verified trailing zero chunks into the checksum using the
same FNV-1a arithmetic. It still inspects all covered bytes; full nonzero payload
pages pay one failed suffix check. The format and integrity guarantees are
unchanged. A new test compares the original checksum at all 4,088 possible
last-nonzero-byte positions, alongside existing full-page corruption tests.

The runner now tests both zero-filled and nonzero-filled key suffixes, producing
1,200 measurements per build. Fresh baseline/optimized builds use the same
working dependency selections. All 300 operation/profile/pattern comparisons
improve by 0.67–53.13%, and each build repeats every counter/result length exactly.
With nonzero keys, 17-byte reads improve 37.6% and value scans 52.3%; maximum-key
lookups improve only 9.4%, versus 42% with zero-filled keys. Small-value mutation
and large-row costs still exceed upstream. Full measurements and artifact
identities are in the design document; these remain synthetic canister fixtures.
Four checksum tests and 18 storage tests pass on Rust 1.99 and 1.85. Four regular
upstream tests, the 320,000-operation campaign and native/Wasm Clippy pass on 1.99.

The 0.1.4 continuation collects the returned value while reclaiming its overflow
pages. The preliminary existence lookup reads keys only, removing a second
payload traversal. Nonzero-key 4 MiB removal falls from 154,691,671 to
106,900,172 instructions versus upstream's 120,839,997; 1 MiB removal also becomes
cheaper than upstream. The 16-byte inline fixture regresses about 0.12%, and
small overflow values remain expensive. All 1,200 matrix measurements repeat
exactly. The tightened read-budget test fails before the change and passes
afterward. A new late-payload corruption test proves no writes, unchanged bytes
and length, and successful removal after repairing only the injected corruption.
Four checksum and 19 storage tests pass on Rust 1.99 and 1.85; four upstream
tests, the 320,000-operation campaign and native/Wasm Clippy pass on 1.99.
API, stored format and release metadata are unchanged.

Pending 0.1.5 removes the second payload read during shrink. The old chain is
fully validated and collected first, then excess pages are staged free in the
same reverse order as before. Ordinary freeing still reads/checks the live page;
the new staging helper is private to callers that already performed validation.
The read-budget test fails before the fix and passes for inline and overflow
shrink with growth refused, including returned data and reopening checks.

The runner adds changed-byte replacement, asserting the old return and new
stored bytes, then restoring common inputs outside measurement. Both fresh
builds produce 1,320 measurements with exact repeat counts. Nonzero-key 4 MiB
shrink falls from 155,648,170 to 102,600,261 instructions versus 120,853,461
upstream. Before the encoder optimization below, changed-byte same-size
replacement remained about 146M versus 124M.
Four checksum and 20 storage tests pass on Rust 1.99 and 1.85; four upstream
tests, the 320,000-operation campaign and native/Wasm Clippy pass on 1.99.
No API, format, allocation ownership or package version changes are made.

The 0.1.5 continuation shares one heap-buffer encoder for new and replacement
overflow pages. It writes the header and payload directly, then zero-fills only
the remaining tail before sealing. Byte-for-byte tests cover empty, partial and
full payloads and pointer extremes. Nonzero-key 4 MiB changed-value replacement
falls from 145,687,077 to 132,826,728 instructions, still above upstream's
124,157,534. New insertion falls from 79,478,225 to 66,626,558. Both intermediate
and final builds repeat all 1,320 measurements exactly; no instruction increases
appear against the prior matrix. Small-value overhead remains significant.
Five codec/checksum tests and 20 storage tests pass on Rust 1.99 and 1.85;
four upstream tests, the 320,000-operation campaign and native/Wasm Clippy pass
on 1.99. No unsafe code, dependencies, format or API changes were introduced.

The next 0.1.5 optimization skips unchanged replacement-node and header writes.
It still validates the full old payload before staging replacement bytes;
changed metadata remains published under the existing failure contract.
Nonzero-key 17-byte changed replacement falls from 655,289 to 278,828 instructions
(still 1.46 times upstream), and 4,072-byte replacement falls from 713,329 to
336,868 (0.68 times upstream). The matrix now beats upstream in 232/330 cases;
126 cases increase against the prior build by at most 0.036%. Both fresh runs
repeat all 1,320 measurements exactly; artifacts and hashes are in the design.
The new write-budget regression fails before the fix and passes afterward.
Five codec and 21 storage tests pass on Rust 1.99 and 1.85; four upstream tests,
the 320,000-operation campaign and native/Wasm Clippy pass. Formatting and links
pass. API, persisted bytes, dependency selections and release metadata are
unchanged; large-row reads/value scans and small-value mutations still need work.

A further 0.1.5 checksum optimization inspects verified zero tails in 32-byte
blocks with an initial eight-byte guard for dense payloads. All covered bytes
are still checked and the persisted checksum is identical. The unguarded
experiment regressed large dense reads and was superseded; both attempts are
retained in the design evidence. The guarded version improves all 330 measured
cases against the prior build, up to 6.10%; nonzero-key 17-byte reads improve
3.08% and value scans 5.77%. Dense 4 MiB gains are negligible. All 1,320 rows
repeat exactly. Five codec and 21 storage tests pass on Rust 1.99 and 1.85;
four upstream tests, the 320,000-operation campaign, native/Wasm Clippy,
formatting and doc links pass. No API, format, dependency or version changes.

The expanded 0.1.5 matrix now has 23 fixtures: eight additions cover 31/32-entry
root transitions, 32/33-byte keys, and 32/256/1,024/8,192-byte rows. Each build
produces 2,024 rows. An inline-key allocation optimization was rejected after
the new 8 KiB value scan exposed a 14.90% regression and higher heap capacity.
The retained insertion change reuses the already-validated root and child nodes,
refreshing after splits. All 92 insertion/refill cases improve, up to 17.55%;
the other 414 counts and upstream counts are unchanged. All three measured
builds repeat exactly (6,072 rows). The new read-budget test fails before the
fix and passes afterward, including branch descent, reopening and validation.
Five codec and 22 storage tests pass on Rust 1.99 and 1.85; four upstream tests,
the 320,000-operation campaign, native/Wasm Clippy, formatting and links pass.
New cases confirm that small overflow rows and 33-byte keys remain expensive.
Evidence and artifacts are in the design; API, format and release metadata
remain unchanged. No publication or IcyDB integration was performed.

After committed source `8d124e2`, overflow encoding folds its freshly zeroed
padding into the checksum directly; reads still inspect the entire page.
The byte-for-byte encoding test covers all 4,073 payload lengths at three
successor-pointer values. The final 23-fixture matrix improves 472/506 cases;
34 increase by at most 0.042%. Nonzero-key small overflow replacements improve
1.7–1.9%; dense large-row gains remain absent. Both candidate builds repeat
all counters across 2,024 rows each; artifacts and hashes are in the design.
Native Clippy caught a narrowing cast in the first candidate; the retained
checked conversion passes. Five codec and 22 storage tests pass on Rust 1.99
and 1.85; four upstream tests, the 320,000-operation campaign, native/Wasm
Clippy, formatting and links pass. API, format, versions and dependencies
remain unchanged. That qualification preceded the 0.1.5 release.

Pending 0.1.6 constructs tree-node and reclaimed-page encodings directly in heap
buffers, removing intermediate 4 KiB copies while preserving all stored bytes
and validation/publication rules. A fresh 0.1.5 baseline uses IC Testkit 0.49.0
and the release lockfile. The combined change improves 214/506 instruction cases
and leaves 292 unchanged, with no regressions or upstream count changes.
Nonzero-key 4 MiB shrink falls from 102.6M to 98.3M and removal from 106.9M to
102.6M; root-splitting insertion improves 10.38%. All three builds repeat their
2,024 measurements exactly. Artifact identities are recorded in the design.
Five codec and 22 storage tests pass on Rust 1.99 and 1.85; four upstream tests,
the 320,000-operation campaign, workspace/Wasm Clippy, formatting and doc links
pass. No dependency, API, persisted-format or package-version changes are made.

The next 0.1.6 continuation reads overflow chains and nodes directly into
caller-owned stack buffers, avoiding intermediate page results while retaining
bounds, checksum, chain and staging validation. The fresh baseline preserves
the incoming IC Metrics 0.11.6 lockfile change and prior heap-encoding work.
All 506 instruction cases improve, up to 33.30%; the final build beats upstream
in 375 cases. Nonzero-key 17-byte value scans fall from 399k to 326k; 4 MiB reads
fall from 62.4M to 53.7M and changed replacement from 132.7M to 119.7M, now below
upstream's 124.2M. Large reads and value scans remain more expensive than upstream.
All three builds repeat 2,024 rows exactly; upstream counters differ by at most
three instructions between builds. Five codec and 22 storage tests pass on Rust
1.99 and 1.85; four upstream tests, the 320,000-operation campaign, workspace/Wasm
Clippy, formatting and links pass. Evidence and hashes are in the design.
No API, format or release metadata changes are made by this continuation.

A further 0.1.6 continuation reuses buffers when reclaiming chains and reading
free-list allocation metadata, eliminating the transaction page-return wrapper.
All validation and staged-publication guarantees remain. The rebuilt starting
Wasm matches the prior artifact exactly. The new matrix improves 130/506 cases,
leaves 376 unchanged and has no regressions or upstream count changes. Nonzero-key
4 MiB removal falls from 102.6M to 85.2M instructions (29.49% below upstream);
regrow/refill improve about 5%. Both runs repeat all 2,024 rows exactly.
Five codec and 22 storage tests pass on Rust 1.99 and 1.85; four upstream tests,
the 320,000-operation campaign, workspace/Wasm Clippy, formatting and links pass.
Artifact identities and remaining qualification limits are in the design.

Iterator boundary-key vectors now reuse capacity after the first excluded
boundary, retaining separate owned keys for returned entries and all bound checks.
All 92 scan cases improve; nonzero-key 32-byte-row key scans improve 3.42% and
value scans 2.00%. Zero-suffix maximum-key value scans improve 10.62%; large
value-scan gains remain negligible. Across the fixed operation sequence, 478
cases improve, 26 increase and two are unchanged; the largest increase is 0.241%
for 32-byte-key shrink. Upstream compiled-build shifts remain under 0.02%.
Both runs repeat all 2,024 counters/results exactly. Five codec and 22 storage
tests pass on Rust 1.99 and 1.85, including mixed-direction/bounded ranges;
four upstream tests, the 320,000-operation campaign, workspace/Wasm Clippy,
formatting and links pass. Artifacts and tradeoffs are recorded in the design.

Insertion now passes its already-validated full node to splitting instead of
reading it again. Root-split insertion uses two stable reads instead of three;
a full-child split uses four instead of five. The strengthened read-budget test
fails before the fix and passes afterward, with reopening/content checks.
Root-splitting insertion improves 8.85%; new insertion in the 2,048/4,096-entry
index fixtures improves 3.34/3.36%. Fourteen cases improve, 78 increase by at most
17 instructions (0.00573%), and 414 are unchanged; upstream counts are unchanged.
Both runs repeat all 2,024 rows exactly. Five codec and 22 storage tests pass on
Rust 1.99 and 1.85; four upstream tests, the 320,000-operation campaign,
workspace/Wasm Clippy, formatting and links pass. Artifact identities and
qualification limits are in the design. No API, format or version changes.
