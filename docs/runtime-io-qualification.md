# Runtime IO and maintainer improvements qualification

## Platform 0.15, Wire 0.12 and Observability 0.23 refresh — 2026-10-31

The working tree based on released `150bf6a`, including the uncommitted #144
release-group implementation, selects registry Platform 0.15.1 and Wire 0.12.1
in both maintained graphs, Logging/Metrics 0.23.1 and Testkit Native 0.73.0.
The separately prepared Testkit CLI is 0.73.0; Build/editor/Host stay 0.73.0.
The canonical installer admitted its cached generation and existing PocketIC
16.1.0 server without retiring older tools. Client/server remain 16.1.0 and the
stable-structures substrate remains the exact 0.7.2 identity.

Platform's canonical owned identities now come from Types 0.15.0. Wire core's
optional Types 0.1 schemas and the separate endpoint crate are unselected;
there is one owned Types provider in the selected graphs. The native meter
retains its non-runtime Platform 0.13 provider; production has one runtime owner.
Metrics preserves default native-wire metadata on the selected Wire 0.12 identity.
No application source or stored format changed. Local package versions remain
0.58.1 in their three release groups, with independent 0.59.0 pending notes.

Locked offline builds produced three original Wasm fixtures and the independent
runner. The installed run exited 0: malformed frames, 12 invalid IO traps,
upgrade failure/clean retry, Storage reopen/rollback, six manager faults,
composed custody and Logging query/update/upgrade/text-budget/watermark checks
pass. Exact source/tool/server/runner/Wasm hashes, runner binary, launchers and
server/runtime logs remain under `target/qualification/runtime/attempt.6EwNrA/`.
Source checks match after execution. Only original Wasm was run, without the
optimizer/performance matrix or a live canister deployment.

Evidence under `/tmp/ic-persistence-refresh19/` includes both full locked metadata
observations, graph/pins, strict affected library/test/helper and independent
runner Clippy, 187 Memory tests (eight explicit ignores), 12 Logging Stable tests
and 29 Metrics Stable tests. Rust 1.91 native checks, formatting, and real native
versus Shared exact group-file plans pass. Existing group recovery evidence from
`/tmp/ic-persistence-groups144/` applies to the unchanged Build/Host renderer and
native release adapter. Full release gates and hosted macOS were not rerun;
no commit, package-version bump, tag, publication or reset ran.


## Observability 0.22 and Wire 0.11 Logging qualification — 2026-10-31

The working tree based on released `150bf6a` (0.58.1), retaining the preceding
Wire 0.11 adoption, selects registry Logging/Metrics 0.22.0. Metrics retains its
default native metadata support via `native-wire` and now shares Wire 0.11,
removing the older Wire 0.10 provider/derive. Its Wasm normal/build graph contains
Serde without Wire. Platform 0.12, Tooling/Testkit 0.72.1, stable-structures 0.7.2
and PocketIC client/server 16.1.0 remain unchanged. The independent host lockfile
is byte-identical to the incoming selected lockfile.

`CARGO_NET_OFFLINE=true make test-runtime-logging` passed after rebuilding the
original Logging Wasm fixture and independent runner with locked offline inputs.
The focused installed run covers query non-persistence, update/upgrade rollback,
reopen, empty-history watermarks, multi-row text-budget eviction, individually
oversized append refusal, forged retained-ceiling rejection and upgrade reopen.
Exact tool/server/runner/Wasm hashes, original Logging artifact, retained runner
and server/runtime logs are under `target/qualification/runtime/attempt.ENwqsq/`.
Source checks match immediately after the run. Other runtime suites and optimizer
variants were not run or relabelled as this focused Logging proof.

Native evidence under `/tmp/ic-persistence-observability17/` includes both locked
metadata observations, graph/pins, strict adapter library/test Clippy, 12 Logging
Stable tests and 29 Metrics Stable tests. Rust 1.91 checks the adapter libraries
and Logging Wasm fixture. Selected feature inspection confirms `default` plus
`native-wire`; Wasm Metrics remains metadata-free. Formatting/local links pass.
No owned Rust source, durable format, text ceiling, custody or release policy
changed; Logging's construction cleanup preserves admitted message parts.
The pending 0.59.0 notes retain the exposed Wire/Metrics identity alignment.
No full release gate, commit, package-version bump, publication or reset ran.


## Platform 0.11 and Tooling 0.72 adoption — 2026-10-30

The working tree based on released `c2ee327`, including the preceding unpublished
Wire adoption, selects registry Platform 0.11.1 and Wire 0.10.0 in both maintained
catalogs. The library selects Metrics/Logging 0.21.0 and Tooling 0.72.0. Older
native Platform/Wire identities are removed without unrelated package changes.
The canonical installer prepared and admitted Build editor and Testkit CLI
0.72.0, retaining earlier generations and logs. The first offline server check
refused a concurrently held tool-store lock (75); its unchanged retry passed
after that owner finished. Build editor admission also passes. PocketIC remains
client/server 16.1.0 and stable-structures remains the exact 0.7.2 substrate.

The three original Wasm fixtures and independent runner were rebuilt locked and
offline. The Testkit 0.72 run exited 0: malformed-frame/IO refusals, upgrade
recovery, Storage reopen/rollback, six manager faults, composed custody and
Logging query/update/upgrade/text-budget/empty-watermark behavior pass. Source
checks match after execution. Exact source/tool/server/runner/Wasm hashes,
retained runner, launchers and server/runtime logs remain under
`target/qualification/runtime/attempt.atoCde/`.

Focused native evidence is under `/tmp/ic-persistence-deps13/`: full locked
metadata for both graphs, production graph/pins, affected library/test/helper
Clippy and independent-runner Clippy pass. There are 187 passing Memory tests
(eight explicit ignores), 12 Logging Stable tests, 29 Metrics Stable tests and
69 release-helper tests. Rust 1.91 checks cover the affected libraries/tests and
native release helper. The selected bootstrap recovery function passes with the
actual prepared 0.72 editor and substituted Cargo effects; no generic runner
matrix or live release is inferred. Formatting and local links pass.

No owned Rust source, stored format, runtime custody, common catalog or server
byte change was needed. Optional Wire endpoint/platform-types integration stays
disabled. Full release gates, optimizer variants and native macOS were not
rerun. No commit, package-version bump, release, publication or push ran.


## Platform 0.10 and Observability 0.19 adoption — 2026-10-30

The working tree based on `b281a60`, preserving preceding Wire/Tooling adoption,
selects registry Platform 0.10.0 in both maintained catalogs and Metrics/Logging
0.19.0 in the library graph. IC Tooling libraries and prepared CLI pins remain
at the latest published 0.71.0; Testkit admits the unchanged PocketIC 16.1.0
server, paired with Platform's 16.1.0 client. Stable-structures remains 0.7.2.
The production Wasm graph has one Platform runtime owner and one Wire provider.
The native test meter retains its required non-runtime Platform 0.9.1 copy.
Metrics now shares Wire 0.8 metadata, removing the previous 0.7.25 provider.

The three original Wasm fixtures and independent host runner were rebuilt locked
and offline. The original-Wasm run exited 0, covering malformed frames and IO
refusals, upgrade recovery, Storage reopen/rollback, six manager faults, composed
custody and Logging query/update/upgrade/text-budget/empty-watermark behavior.
Exact source/tool/server/runner/Wasm hashes, retained runner, launchers and server
logs are under `target/qualification/runtime/attempt.4dqXS3/`. Source checks
matched after execution. The initial sandbox run could not bind loopback;
its failure remains under `target/qualification/runtime/attempt.Rowi1N/`.
The retry used the same prepared tools with local-server permission.

Focused native evidence is retained under `/tmp/ic-persistence-adoption11/`:
187 Memory tests (eight explicit ignores), 12 Logging Stable tests and 29 Metrics
Stable tests pass, including corruption and checkpoint admission. Strict affected
Clippy, Rust 1.91 library checks, production graph/pins and formatting pass.
No owned Rust source, stored format, runtime custody, common catalog or server
byte change was needed. Previous release-helper evidence remains applicable to
the unchanged helper and native Tooling dependencies. No full release suite,
optimizer matrix or native macOS qualification ran; no release/publication ran.


## IC Tooling 0.71 installed adoption — 2026-10-30

The working tree based on `b281a60`, including the preceding Wire 0.8 adoption,
selects registry IC Tooling 0.71.0 for Build, Host Artifacts/FS/Process and the
native test meter. The separate Build editor and Testkit server CLI pins also
select 0.71.0. The canonical installer admitted the cached Build generation and
built/admitted the new Testkit generation, retaining older tools and attempt logs.
Both selected executables pass offline checks. No release or publication ran.

Testkit CLI 0.71.0 admits the existing authenticated PocketIC 16.1.0 server;
Platform 0.9.1 still supplies client 16.1.0. The original-Wasm run exited 0:
malformed-frame and IO refusals, failed/clean upgrade recovery, Storage reopen
and rollback, six manager faults, composed custody, Logging query/update/upgrade
semantics, empty watermarks and text-budget/forged-checkpoint probes all pass.
Exact source/tool/server/runner/Wasm hashes and server/runtime logs remain under
`target/qualification/runtime/attempt.CNI1ob/`, with launchers and source checks.
The host runner is retained there as well. Recorded source checks matched after
the run. No runtime source change followed this qualification.

Native evidence is under `/tmp/ic-persistence-tooling10/`: strict affected Clippy,
187 Memory tests (eight explicit ignores) and all 69 release-helper tests pass.
Consumer release-adapter groups passed until the bootstrap case correctly refused
an unprepared newly pinned editor; its failed fixture is retained. After explicit
setup, that unchanged bootstrap function passes in a selected scratch invocation
with the prepared native editor and substituted Cargo effects. This is neither
a live release nor a generic producer matrix. The dependency graph removes the
old native-only Platform 0.8.3 copy and retains one 0.9.1 owner; stored formats,
common catalogs and the server bytes remain unchanged. Formatting and local
links pass. Full release gates, optimizer variants and native macOS were not
rerun; no performance gain is inferred.


## Platform 0.9 and Logging 0.18 adoption — 2026-10-30

The working tree based on 0.56.4 `08e815d`, retaining the preceding uncommitted
module/recovery cleanup and incoming Wire 0.7.25 lock selection, is qualified in
two focused Linux x86-64 phases. Both use Testkit CLI 0.70.0 and PocketIC
client/server 16.1.0. Catalogs select registry Platform 0.9.0 in the library and
independent host graph; the qualified stable-structures substrate remains 0.7.2.

First, an original-Wasm run with Platform 0.9.0, Metrics 0.18.0 and the then-selected
Logging 0.17.7 passed Memory malformed-frame/IO refusal, upgrade rollback, Storage
reopen and update/upgrade rollback, manager faults, composed custody and Logging
watermarks. Exact hashes, Wasm and server/runtime logs are retained in
`target/qualification/runtime/attempt.co4GnJ/`. That phase is not relabelled as
Logging 0.18 evidence.

After the maintainer explicitly selected retirement of old Logging checkpoints,
Logging 0.18.0 and its discriminator-3 fixture were rebuilt and qualified with
the focused Logging-only runner. The run exited 0: query non-persistence,
update/upgrade rollback, empty-history watermarks and normal reopen pass. A
separate governed row proves eviction of two old rows, an individually over-budget
append preserving the prior checkpoint, refusal of a forged retained text ceiling,
explicit host repair, and upgrade reopen preserving the admitted row. The main
log is unchanged by that probe. Exact hashes, original Logging Wasm, source
observations and server/runtime logs are retained in
`target/qualification/runtime/attempt.V1TC6O/`. Source checks matched immediately
after each installed run; no runtime source fix followed the successful Logging run.
No deployed data was reset. Old Logging rows require explicit host administration;
there is no old decoder, inferred text ceiling or automatic reset.

Native evidence is under `/tmp/ic-persistence-adopt7/`. Memory 187 (eight explicit
ignores), Storage 17 and Metrics 29 library tests pass. Logging's 12 cases pass
across the main run and the selected record-budget correction: the former fixture
now evicts by text, so the record-size case fills the text ceiling exactly and
proves metadata still exceeds the encoded ceiling. The new cases cover forged
text usage, unchanged writes and explicit retirement of frozen discriminator-2
bytes. Strict native/Wasm/host Clippy, Rust 1.91 Wasm library checking, Logging
doctests, consumer release-adapter substitutes, graph/pin checks and source-bound
Shared snapshot verification pass. The actual validation runner also dispatches
the consumer graph gate with its captured checkpoint companion. Package file
lists include ordinary example support modules. These are focused proofs, not
release/package publication admission. Optimizer variants, macOS and full release
gates were not rerun; no performance gain is inferred.


## Runtime bookkeeping cleanup qualification — 2026-10-30

The working tree based on 0.56.3 `413cecc` passes Linux x86-64 original-Wasm
qualification after the six priority cleanup items. Diagnostics moved byte-for-byte
into its existing runtime namespace using normal module discovery. Named manager
layout offsets retain the selected MGR V1 values; opening and dropping leases
share bit-address calculation without moving admission or mutation boundaries.
Public APIs, durable formats, dependency catalogs and lockfiles are unchanged.

Rust 1.99.1 built the maintained Memory, Storage and Logging fixtures and the
independent host runner with locked offline selections. Testkit CLI 0.70.0
admitted and managed PocketIC client/server 16.1.0. The run exited 0: refused
Wire frames, twelve invalid IO traps, failed/clean upgrade recovery, Storage
reopen and update/upgrade rollback, six manager faults, composed-handle custody
and Logging watermarks all pass. Recorded source hashes match after the run.

Exact inputs, tools, server, runner and Wasm hashes plus server/runtime logs are
retained under `target/qualification/runtime/attempt.aCDH9i/`. Its retained scratch
launcher selects only the original variant from the maintained runner script and
omits the unused optimizer input. The repository runner script is unchanged.
Native evidence is retained under `/tmp/ic-persistence-cleanup5/`: strict affected
Clippy, 187 Memory library tests (eight explicit ignores), all 69 release-helper
tests, formatting and documentation checks pass. No runtime source fix followed
the installed run. Optimizer variants, macOS and full release gates were not
rerun. The recorded IO measurements do not establish a performance improvement.


## Borrowed Logging encoder qualification — 2026-10-30

The working tree based on 0.56.2 `841d377` passes focused Linux x86-64 installed
Logging qualification after the ten cleanup changes. The encoder walks borrowed
rows instead of collecting an intermediate row vector. The same row schema,
magic, byte/resource bounds and bounded owned restoration remain; frozen-byte
and maximum-row native checks pass. Storage collection implementation moves keep
its public re-exports and lease/admission contracts unchanged.

Rust 1.99.1 rebuilt the Logging original Wasm and the independent host runner
with locked offline selections. Testkit CLI 0.70.0 admitted and managed PocketIC
client/server 16.1.0. The Logging-only run exited 0: query non-persistence,
update/upgrade rollback, checkpoint reopen and empty-history watermarks pass.
Exact input/tool/server/runner/Wasm hashes and runtime/server logs remain in
`target/qualification/runtime/attempt.5IgDJv/`, alongside a copy of the launcher
and the source hashes checked after execution.

Focused native and structural-preservation evidence is retained under
`/tmp/ic-persistence-cleanup4/`. The first launcher stopped during input hashing
because it named a nonexistent tooling-pin path; it did not start a server.
Its log and the corrected successful launcher are both retained. No fixture or
runtime source change followed the successful installed run. This proof covers
Logging only; the full Memory/Storage suite, optimizer variants, macOS and release
gates were not rerun. No performance improvement or fresh IO baseline is claimed.


## Shared Wire fixtures installed qualification — 2026-10-30

The working tree based on 0.56.1 `d4208ce` passes Linux x86-64 original-Wasm
qualification after the ten cleanup changes. Memory and Storage now share bounded
Wire protocols with the native runner; Logging retains its existing Wire protocol.
Library APIs and durable ledger/collection formats are unchanged. Storage gains
only dev dependency edges to already selected Wire/Serde packages; no package
version or independent-workspace dependency selection changed.

Rust 1.99.1 built the three fixtures and runner offline with locked selections.
Rust 1.91 also checked the changed Memory/Storage Wasm examples and libraries.
Testkit CLI 0.70.0 admitted and managed PocketIC client/server 16.1.0. The run
exited 0, covering five refused Memory frames, four refused Storage frames,
twelve invalid IO traps, two failed upgrades followed by a clean retry, Storage
reopen and trapped update/upgrade rollback, six manager faults, composed-handle
custody and Logging rollback/watermarks. Successful unit responses are decoded
by the runner as well. No fixture or runner source change followed the installed run.

Exact launch commands, source/tool/server/runner/Wasm hashes, original Wasm,
server/runtime logs and nine valid-IO instruction/cycle rows remain under
`target/qualification/runtime/wire-fixtures.0dwbpz/`. All recorded source hashes
matched after execution. CSV extracts retain the fresh Wire fixture baseline;
previous raw-packet results keep their original identities. The inner instruction
counter covers only the IO loop; update cycles also include protocol and lease
work. These measurements establish no codec or cleanup speedup.

Native focused logs are retained under `/tmp/ic-persistence-round3.9AXv4N/`.
The helper passes 69 tests, Memory 187 (eight explicit ignores), Storage 17 and
the host runner three (two explicit ignores). Strict affected Clippy, manifest
renderer byte parity, archive admission, pin/production graph and final formatting
checks pass. Optimizer variants, macOS and full release gates were not rerun.
This is working-tree qualification, not release/publication admission.


## Complexity cleanup installed qualification — 2026-10-30

The working tree based on 0.56.0 `956b2b9`, including merged Logging PR #140 and
the ten requested structural cleanups, passes Linux x86-64 original-Wasm
qualification. The incoming dependency lock was preserved byte-for-byte; this
is working-tree evidence with those selected inputs, not a clean release receipt.

The maintained Memory/Storage/Logging fixtures and independent runner built
offline. Testkit CLI 0.70.0 admitted and managed PocketIC client/server 16.1.0.
The run exited 0: twelve invalid-IO cases, failed post-persistence/geometry upgrade
rollback, clean generation advancement, Storage Cell/map/region reopen and trap
rollback, six manager faults, composed-handle custody and Logging watermarks all
pass. No source fix followed this installed run.

Retained source/tool/runner/server/Wasm hashes, original Wasm, build/server/runtime
logs, incoming lock diff and the exact launcher are under
`target/qualification/runtime/cleanup-ten.qDQNqi/`. All recorded source hashes
matched after execution. Native focused results and structural-review evidence
are under `/tmp/ic-persistence-ten.xUlwfE/`; the handoff binds their scope.
Optimizer variants, macOS and the full release suite were not rerun. Valid-IO
instruction/cycle rows do not establish codec or refactoring speed improvements.

## Wire ledger installed qualification — 2026-10-30

Linux x86-64 original-Wasm qualification passes for the 0.56.0 Wire source at
`956b2b92728f82f75dd51d4d10ed52f325529e91`. The incoming dirty root lock already
selects Build, Host Artifacts/FS/Process and Testkit Native 0.70.2 instead of the
release commit's 0.70.0; this selection was preserved and hashed before/after.
This qualifies those working-tree inputs, not the exact clean release graph.
The independent runner catalog/lock remain unchanged. Rust 1.99.1 builds the
supplied Memory, Storage and Logging fixtures offline in `wasm-size`, and builds
the native runner offline. The admitted Testkit CLI is 0.70.0, with PocketIC
client/server 16.1.0; Platform is 0.8.2 in the library graph and 0.8.1 in the host
runner. Server selection passed `make runtime-server-check` before the run.

The maintained runner passes:

- Twelve invalid-IO traps preserve neighboring markers and prior mutations.
- Post-persistence and geometry-conflict upgrade failures roll back; a clean
  retry advances the generation exactly once with payload/pages preserved.
- Storage Cell/map/region admission, retained reopen and trapped update/upgrade
  rollback pass without replacing rejected history.
- Six manager open/growth faults trap and roll back metadata/payload writes.
  Composed handles retain custody through runtime drop; final release and upgrade
  reopen preserve payloads and pages.
- Logging query non-persistence, update/upgrade rollback, canonical reopen and
  empty-history watermarks pass.

The explicitly managed Testkit run exited 0. Exact source, tool, server, runner
and Wasm hashes, the launch script, incoming-lock diff and build/server/runtime
logs remain under `target/qualification/runtime/wire-original.6qzY2Z/`.
Its launch script reproduces the single original-Wasm run through the reviewed
CLI lifecycle; it does not run Binaryen. All selected manifest/lock/pin input
hashes remained unchanged during execution. No fixture/source fix was needed.

This is current Wire-to-Wire installed recovery evidence. It provides no CBOR
migration reader or compatibility claim. Only disposable local canisters were
installed; no deployed data was reset. O3/Os/Oz, native macOS and the full release
suite were not rerun. The nine valid-IO instruction/cycle rows measure delegated
IO, not CBOR-versus-Wire codec performance.

## IC Platform 0.6.5 boundary qualification — 2026-10-21

The pending 0.50.9 working tree based on released 0.50.8 `5910f64` consumes
published Platform 0.6.5 in both libraries and the independent runner. Memory's
manager/backing and Storage's collection identities still resolve to the same
registry stable-structures 0.7.2. The new product guard refuses protocol identity
drift or failed observations before singleton graph admission.

Installed Linux original/O3/Os/Oz checks pass 12 invalid-IO traps per variant,
post-persistence/geometry rollback, clean retry and all nine valid-IO cases.
The 36 valid-IO rows match the earlier qualified instruction observations.
Native IO and allocation workloads separately retain all 40 and 86 non-timing
rows. Exact inputs, tools, server logs and Wasm variants are retained at
`target/qualification/runtime/attempt.hElm2V/`; focused logs are under
`/tmp/ic-persistence-platform065-*`. This qualifies local working-tree behavior,
not delivery, native macOS or release execution.

## IC Tooling 0.63.2 installed qualification — 2026-10-21

The pending 0.50.8 working tree based on released 0.50.7 `4dda4c1` passes
installed Linux qualification with Testkit 0.63.2 (`cli`), Platform 0.6.0 in the
independent runner and PocketIC client/server 16.1.0. Original, O3, Os and Oz
variants each pass 12 invalid-IO traps without retained mutation, both failed
upgrade rollback cases, clean retry and all nine valid-IO measurements.

Exact input/tool/Wasm hashes and server/runtime logs remain in
`target/qualification/runtime/attempt.gKHXbF/`. The earlier sandbox attempt
`attempt.9PPNhW/` retains its local-port bind failure; host execution passed.
Native observer adoption separately preserves all 40 non-timing IO rows and 86
allocation rows. This is Linux working-tree evidence, not hosted/macOS consumer
acceptance or release execution. The independent selected lock is unchanged.

## IC Tooling 0.63 installed qualification — 2026-10-21

Linux qualification passed with Testkit 0.63.0 (`cli`), Platform 0.6.0,
PocketIC client/server 16.1.0 and Rust 1.99.1. `make test-runtime` passed for
original, `-O3`, `-Os` and `-Oz` Wasm. Each variant passed 12 invalid-IO traps
without retained mutation, both failed-upgrade rollback cases, a successful
upgrade retry advancing one generation, and all nine valid-IO measurements.

Exact tool, source, binary and Wasm hashes plus optimizer, server and runtime
logs remain in `target/qualification/runtime/attempt.foqrZd/`. This is installed
Linux acceptance; it does not claim native macOS, hosted CI or release execution.
Earlier records below retain their original selections and evidence.

## IC Platform 0.2 client boundary — 2026-10-19

The pending 0.49.1 runner consumes registry IC Platform 0.2.0, archive source
`c7da516aef66d1f3623c85aa7cf304c17a5877d5`, with only its `pocket-ic` feature.
It exposes the upstream client; the independent lockfile selects PocketIC
16.1.0. Testkit CLI 0.25.4 still selects and manages server 16.1.0. The library
and supplied Wasm source, stable-memory substrate and stored formats are unchanged.

Locked metadata, all-target runner checking, strict Clippy and the actual Rust
1.88 check pass on Linux, along with formatting and dependency admission.
The prepared server's offline admission passes. These are not installed IO,
upgrade or rollback acceptance for the new client/server pair; that qualification
requires its separate authorization and remains pending, including native macOS.
Source/cache and compile logs are retained under
`/tmp/ic-memory-platform020.2hd8jkvq/`. Earlier receipts below retain their
original client/server identities.

## Binaryen 133 current-fixture qualification

On 2026-10-10, local Linux qualification of the uncommitted **0.35.3** candidate
based on released Memory 0.35.2 `0aec074fb3a15e913db25d07ca2bc2db9ef792fc`
passed for the original fixture and Binaryen 133 `-O3`, `-Os` and `-Oz` outputs.
The canonical 91-file snapshot selects Shared Tooling 0.3.4
`169d77b8440568c5200eede971625126181f7bb2`. Host 0.12.3 is already selected in
the incoming root lockfile; the independent graph remains unchanged.
Rust 1.99.0, PocketIC client 16.0.0 and Testkit CLI 0.25.4/server 16.1.0 remain
the selected compiler and runtime inputs.

All four variants passed startup, 12 invalid-IO traps preserving the neighboring
marker and prior mutation, rollback after a post-persistence trap and geometry
conflict, clean upgrade retry advancing exactly one generation, and nine valid
IO cases. The unchanged runner asserts state for each variant independently;
its instruction measurements are not an equivalence or performance guarantee.
The original fixture is 216,893 bytes; optimized outputs are 195,177 (`-O3`),
194,740 (`-Os`) and 194,331 (`-Oz`) bytes. These are fixture measurements, not
changes to the library's unoptimized budget probes.

`make test-runtime` retains Wasm, input hashes and per-variant logs in
`target/qualification/runtime/attempt.StvyQE/`. Setup, focused checks and the
runtime transcript are under `target/qualification/0.35.3-shared034/`.
Full `make validate` passes; its log binds the unchanged Rust graphs, strict
Clippy, tests, rustdoc, development packaging, MSRV and five raw Wasm budgets.
Subsequent evidence-output/collection changes are qualified by the affected
tooling, release-adapter and installed-runtime checks. Genuine Bash 3.2.57 and
GNU Make 3.81 on Linux also pass all four variants, retaining their inputs and
logs in `target/qualification/runtime/attempt.hdiKQ1/`. The earlier portable
attempt `attempt.FcOiUy/` remains retained too. This exercises portable
commands on Linux; it is not native macOS execution.
The first sandbox attempt could not bind localhost; the next native attempt
passed original-Wasm checks but Binaryen refused `memory.copy`/`memory.fill`
without feature admission. Both failures and their artifacts remain retained.
The final command admits those instructions with `--enable-bulk-memory-opt`;
it does not select `--all-features` or broaden the product's Wasm feature policy.
Previous IC executable and pin hashes remain unchanged after installation.

Native CI runs these checks on Linux, macOS Intel and Apple Silicon. This local
Linux record does not establish macOS acceptance, delivery, publication or
qualification of a composed Canic/IcyDB application. The adoption owner is
[#53](https://github.com/dragginzgame/ic-memory/issues/53), with common pins owned
by [Shared #102](https://github.com/dragginzgame/shared-tooling/issues/102).

## Historical maintainer-improvements qualification

This record describes local Linux execution on 2026-10-05 of the uncommitted
maintainer-improvements candidate based on 0.25.13, HEAD
`11b983517a0bce1e2c2ab3fe86da76f6c6aa62a1`. It is focused crate qualification,
not a release receipt or qualification of a Canic/IcyDB application graph.
At execution the candidate had no assigned release version. The maintainer
subsequently selected 0.25.14. Recorded inputs and results precede the version
edit and subsequent governance/dependency-catalog changes; they do not qualify
the final 0.25.14 package. Recorded manifest and lockfile hashes identify the
actual inputs used at execution, rather than those later edits.

## Changes and compatibility

`RuntimeMemory` (historical source: `../crates/ic-storage/src/runtime/backing.rs`) checks safe reads, raw reads and
writes against the current virtual byte extent before upstream translation.
Its short-circuit subtraction avoids offset/length addition overflow. Manager
layout admission and growth already bound the virtual page count by the bucket
capacity, so conversion to byte extent fits `u64`. Empty spans at the extent
are accepted; empty spans beyond it are refused. Clones observe the manager's
current size rather than keeping a second extent cache.

The pinned `ic-stable-structures` 0.7.2 manager checks its bucket cache before
logical bounds, and its cached-span/translation arithmetic is unchecked in
release builds. The native baseline regression warmed the following allocation,
wrote at `u64::MAX`, and changed the preceding allocation's last valid marker
from `0x6D` to `0xFF`. This is executed corruption evidence. The crate guard
protects handles opened through `ic-memory`, including its ledger; direct
upstream `VirtualMemory` users remain outside this protection.

[`serialize_record`](../crates/ic-storage/src/stable_cell.rs) reserves the existing exact encoded
size before invoking the canonical encoder. The size counter counts opaque
payload lengths without rereading every payload byte. Neither codec nor
persisted bytes changes. `validate_integrity` (historical source: `../crates/ic-storage/src/ledger/integrity.rs`)
recognizes strictly increasing generations as unique and avoids building the
generation set. Unordered DTOs retain the original set and refusal order; record
key uniqueness and committed chain/recovery validation remain in place.

The durable ledger decoder (historical source: `../crates/ic-storage/src/ledger/record.rs`) uses the bounded
visitor to refuse more than 255 records before decoding an extra entry. Resolved
declaration snapshots are internal and have no serialized input contract.
`PendingBootstrapCommit` (then in `src/runtime/bootstrap.rs`)
now warns when discarded even after consuming its enclosing `Result`. Callers
with warning denial must complete persistence/confirmation or explicitly discard
the value in an intentional model/test path. It does not add automatic commit,
rollback or drop behavior.

No production dependency, MSRV, bucket geometry, allocation policy or stable
format changes. No compatibility readers or aliases were added.

## Proof of rejection and recovery

- The native release-mode matrix exercises all three IO methods with cold/warm
  handles, zero/one-page extents, zero-length spans, page/bucket boundaries and
  maximum offsets. Invalid spans leave backing read/write counters unchanged.
  The neighboring-marker regression checks all backing bytes and confirms that
  subsequently grown bytes remain zero. A sparse byte-addressed backing tests
  actual IO crossing 4 GiB through discontiguous buckets without a 4 GiB heap.
- The snapshot test supplies 255 valid entries followed by a malformed extra
  entry. Candidate JSON decoding refuses by count (`Data`); original source
  instead tries to parse that entry (`Syntax`). A truncated CBOR array advertising
  256 entries fails semantically before attempting to read its first entry.
- Generation validation is compared with an independent simple reference over
  22,621 sequences, including unordered generations and overlapping duplicate,
  future-generation and parent errors. Existing strict-chain tests still pass.
- Current stable-cell fixtures equal both actual `Storable::to_bytes` and
  `into_bytes` output. The pending-commit UI case denies `unused_must_use` after
  consuming the enclosing result and observes the pending type's diagnostic.

The installed [Wasm fixture](../qualification/ic-storage-test-fixtures/tests/fixtures/wasm_io_qualification.rs) and
host runner (historical source: `../qualification/ic-memory-runtime-qualification/src/main.rs`) use the current root
dependency graph. PocketIC 16.0.0 passed these executed checks:

- All three IO methods trap for end-plus-one-byte, empty span beyond end,
  `u64::MAX` plus one byte and `u64::MAX - 1` plus two bytes. All 12 responses
  are typed execution traps. A preceding valid write in the same update rolls
  back, and the neighboring allocation's marker, expected IDs and extents remain.
- A post-upgrade trap after durable bootstrap persistence preserves the original
  generation, IDs and data. A conflicting bucket geometry also traps and preserves
  them. A subsequent same-contract upgrade succeeds, advances exactly one
  generation and retains both stores and physical/virtual extents. Virtual time
  advances before each upgrade to avoid install-code rate limiting.

These installed results establish IC message rollback for the tested paths.
They do not make arbitrary native backing writes transactional or test migration
between incompatible formats. Raw-read pointer obligations still belong to the
caller, and a backing read failure can partially initialize its destination.

## Measured costs and benefits

Matched measurements use Rust 1.99.0, the same root lockfile, profile and fixture
source on each side. Baseline production source is the original HEAD archive;
current qualification examples/tests and their manifest entry are copied into
that archive solely to supply the same probes. Candidate production source is
the working tree identified below. No dependency upgrade is included.

| Measurement | Baseline | Candidate | Interpretation |
| --- | ---: | ---: | --- |
| Cached safe read, instructions per 1,000 one-byte calls | 206,273 | 311,273 | +105 per call for bounds enforcement |
| Cached raw read | 162,273 | 265,273 | +103 per call |
| Cached write | 181,273 | 284,273 | +103 per call |
| Core raw Wasm bytes | 241,274 | 241,603 | +329; ceiling stays 260,000 |
| Installed fixture raw Wasm bytes | 263,032 | 263,422 | +390; no fixture ceiling |
| Record encoding peak measured heap, 65,536 generations | 23,591,104 | 11,795,632 | About half, with two protected slots |
| Record encoding reallocations per call at that size | 7 | 0 | Same durable bytes |
| Generation validation allocations per call at that size | 10,922 | 0 | Empty allocation-record fixture isolates the generation set |
| Generation validation peak measured heap | 1,285,648 | 0 | Allocation-key validation can still allocate in real ledgers |

The instruction window brackets 1,000 operations on one opened/warmed handle,
including loop/counter overhead. It excludes bootstrap and handle opening. Empty
end-of-memory IO and a last-byte span have the same measured deltas. This is a
fixed extra per-operation cost, significant for these tiny cached operations;
it adds no backing stable IO, growth or buffers. Forcing just the small guard to
inline saved 9–11 instructions per call versus the uninlined guard at 78 additional
fixture bytes. The narrow Clippy expectation documents that measured choice.

The ignored native measurement counts this thread's requested heap bytes and
allocation/reallocation calls through `System`; it does not measure allocator
RSS, stable-memory usage, whole bootstrap or IC instructions. Fixtures contain
zero allocation records and histories of 1, 64, 1,024, 16,384 and 65,536 generations.
Inputs and two protected slots are constructed before measurement. Each phase
runs ten times and releases all measured allocations. Allocation counts are
totals; peak bytes are maximum concurrently requested bytes, not ten times peak.
Native encoding time varied between runs and was slower in the final candidate
large case; no native encoding speedup is claimed. The extra size-counting pass
trades work for bounded allocation. Whole-bootstrap IC impact remains unmeasured.

Retained matched CSVs:
[native allocations](measurements/maintainer-native-allocations.csv),
[IC instruction counts](measurements/maintainer-io-instructions.csv), and
[raw Wasm sizes](measurements/maintainer-wasm-size.csv).
No stable-memory footprint saving or net instruction reduction is claimed.

## Inputs and retained evidence

Compiler: Rust 1.99.0 (`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`,
LLVM 23.1.1), Linux x86-64. Wasm uses `wasm32-unknown-unknown`, `wasm-size`
(`opt-level=z`, fat LTO, one codegen unit, abort panic, stripped symbols).
The upstream stable-structures cache identifies commit
`72b02ab65b6c6fbf53fbabf752732d2726539377` for pinned 0.7.2.

SHA-256 inputs/artifacts at execution:

| Input | SHA-256 |
| --- | --- |
| Root `Cargo.toml` | `b185349e95ca3c4e72c99490b9df5523beba6b9468b2d9fb1c18684570db6ac8` |
| Root `Cargo.lock` | `19626029b5c4e9a7002d6c41844c159fcac093321f552be3cf62e09a08fd7f45` |
| Host runner `Cargo.lock` | `7bc872926fbeddd23706750c6205582ba9eae6c873612794e06e862503bb6be7` |
| `src/runtime/backing.rs` | `16c217dbcd10cad3b211e6c59e59566f2896b8d73ed3e0093c20064c70b9f665` |
| `src/stable_cell.rs` | `b8f4829c1f4f2da6cda8189e9d38c0274f74572e811a89b2c4e193c0c9f4b02c` |
| `src/ledger/integrity.rs` | `9984fc8ba37dd226b1f51b0b11c691e463845401811cdc6549ae5da78d57c3f7` |
| `src/declaration.rs` | `bc72bbe8ebb6002187cc9fdc0c636a616fbcb26410ca848527a884f8d32efd20` |
| `src/bootstrap.rs` | `6c11924267421c7f71fbf51f98b8a731d8fefbdb39232e7a98844c02de987a01` |
| Wasm fixture source | `7316418a592aec53f6a99baa740ea747c4662ff8d2172565c42dbd4e99065d5b` |
| Host runner source | `9ae614936854571256887abc3ba7db3892ec87b03bf1f24422cc229daf510bff` |
| Host manifest | `67a810278e127b2d178b1ab5a6dd9154c8541ca48a4aa270b3b6a0dcbfff5e5a` |
| Native measurement source | `ecbbe4bfb9de30e9363cf33e2d6581c48c0268ec29d6dedb7654fb78942a69ec` |
| Candidate installed Wasm | `9f3a18d0b6642ef82522730c88cc03f40a4d30e30a3cae5d6081260fc814318e` |
| Baseline installed Wasm | `d235870888c00dc529006a7fba878282b6b680d3eef40ce479d3bb509f112af6` |
| Cached PocketIC 16 Linux server | `69e324bdb68d32d878b7a9504b1379f08f8d1921272bacb065b0fabb3d0f3792` |

Local logs/artifacts are retained under
`target/qualification/maintainer-improvements/`: `installed-final.log`,
`allocation-baseline.log`, `allocation-candidate.log`, `bounds-baseline.log`,
and `decoder-baseline.log`. `installed.log` retains the initial sandbox refusal
to bind loopback; explicitly approved loopback execution subsequently passed.
The archived baseline build remains in `/tmp/ic-memory-io-baseline.89UhlB`.

Actual server lifecycle was supplied by the inspected, read-only IcyDB helper
`scripts/ci/run-with-pocketic-server.sh`, with `TMPDIR=/tmp` and the cached
server path. That sibling's observed HEAD was
`8e74b6bac5d054eab9dd9de3435f258d85926040`; helper SHA-256 was
`c9b093c091ae507fc7b43fa632bb40fe5f088fb47640211db26e503f00930177`.
The helper owned server teardown and scratch cleanup. This describes the actual
run, not a maintained dependency on a moving sibling checkout. The maintained
runner accepts an explicitly owned server URL instead.

## Reproduction

Wait for active Cargo/rustc/rustdoc processes before editing or compiling.
Preserve each selected lockfile. Dependency/cache preparation is a separate step;
no implicit download or online fallback belongs to qualification. Current
checkouts track the host runner's selected lockfile; preserve that selection.
A deliberate selection change needs a new qualification record and does not
relabel the historical measurements above.
For the selected lock, prepare cached dependencies with `cargo fetch --locked
--offline --manifest-path qualification/Cargo.toml`; a cache
miss requires separate authorized preparation. The library's own cache/lock
prerequisites remain those in [host support](operations/host-support.md).

From the repository root, run these focused builds sequentially:

```sh
cargo build --manifest-path qualification/Cargo.toml \
  --target-dir target -p ic-storage-test-fixtures --locked --offline --profile wasm-size \
  --target wasm32-unknown-unknown --example wasm-io-qualification
cargo build --locked --offline \
  --manifest-path qualification/Cargo.toml \
  --target-dir target/runtime-qualification
cargo test --locked --offline --release --test allocation_measurements \
  -- --ignored --nocapture --test-threads=1
```

The preceding historical results used PocketIC server 16.0.0. Current setup
uses the published Testkit CLI's reviewed server selection; prepare it separately
and repeat installed qualification for that pairing:

```sh
make install-runtime-server
make runtime-server-check
make test-runtime
```

Only explicit setup installs the CLI and server. The offline runtime target
builds before launch and Testkit owns bounded startup and teardown, retaining
server output under `target/qualification/runtime/attempt.*`. The runner still
borrows a caller-owned server and supplied artifact. For a direct invocation,
provide the same URL contract exported by Testkit's `run` command:

```sh
IC_TESTKIT_POCKET_IC_URL=http://127.0.0.1:4943/ \
IC_MEMORY_QUALIFICATION_WASM=target/wasm32-unknown-unknown/wasm-size/examples/wasm_io_qualification.wasm \
  target/runtime-qualification/debug/ic-memory-runtime-qualification
```

Use `--measure-valid-io` only for matched valid-IO measurement on supplied
baseline/candidate artifacts; it deliberately omits rejection and upgrade checks.
Retain command output and source/lock/artifact hashes for every rerun.

Other focused checks executed sequentially in this pass:

```sh
cargo test --locked --offline --lib --test diagnostic_metadata \
  --test memory_runtime --test compile_fail -- --test-threads=1
cargo test --locked --offline --release --lib runtime::read_tests \
  -- --test-threads=1
RUSTDOCFLAGS='-D warnings' cargo test --locked --offline --doc
cargo clippy --locked --offline --lib --test allocation_measurements \
  --test diagnostic_metadata --test compile_fail --test memory_runtime -- -D warnings
cargo clippy --locked --offline --target wasm32-unknown-unknown \
  --example wasm-io-qualification -- -D warnings
cargo +1.88.0 check --locked --offline --lib --test allocation_measurements \
  --test diagnostic_metadata --test compile_fail --test memory_runtime \
  --example wasm-io-qualification
cargo clippy --locked --offline \
  --manifest-path qualification/Cargo.toml \
  --target-dir target/runtime-qualification -- -D warnings
cargo +1.88.0 check --locked --offline \
  --manifest-path qualification/Cargo.toml \
  --target-dir target/runtime-qualification
cargo fmt --all -- --check
cargo fmt --manifest-path qualification/Cargo.toml -- --check
make verify-shared-tooling
git diff --check
```

## Qualification limits

Focused execution passed 279 library tests, nine compile-fail cases, 11 diagnostic
metadata tests, two public runtime tests, seven release-mode IO tests and six
warning-denied doctests. Strict selected root/fixture/runner Clippy and selected
root plus runner Rust 1.88.0 compilation passed. The installed runner was executed
with Rust 1.99.0. Formatting, shared snapshot verification and whitespace checks
passed. Full release gates, packaging, native macOS
execution, consumer application graphs, large-history installed bootstrap cost
and additional Wasm probes have not been qualified in this pass.

Current consumer manifests using `ic-memory = "0.25"` and historical qualification
receipts do not prove they compiled or ran this candidate. Maintainer-owned
commits and explicit release selection precede fresh release/consumer receipts;
no commit, tag, push, publication or deployment was performed here.

## Subsequent local policy alignment, 2026-10-05

After the version selection, local governance and dependency declarations were
aligned with reviewed dirty Shared Tooling policy. This is separate evidence
from the installed runtime measurements above. At that point the snapshot remained at
`41e1fd0ba41460bd2127cbf98ac8a4b2b2020d3e`; the dirty review was based on
upstream HEAD `b8537873ac124ad17b30e32aa23e9006a3e6ec21`. Reviewed dirty policy
SHA-256 values were
`9c95f254c59a1ef312d37981dcd8af2dadf4eda34cf9026cfeda4092a2716858`
for `rules/changelogs.md` and
`6a3d8a42bd3a1b1b0ae6014620f8e402a959d23214a55be7cc0989595f0bea02`
for `rules/cargo-dependencies.md`. The isolated qualification-workspace exception
is recorded in [AGENTS.md](../AGENTS.md). No new runtime measurements or full
workflow adoption are claimed.

The current numbered pending notes and local release preparation agree on
`[0.25.14]`. Published changelog history from 0.25.13 downward retains SHA-256
`e752ccb1d81580d0f55161a33ae7fa507c382befc9ea83ee38543d49fdcd69a4`,
identical to the original HEAD. Root and host lockfiles are unchanged by
dependency centralization, and the effective direct dependency declarations in
Cargo metadata equal those before centralization, including default features.

SHA-256 inputs for those focused checks:

| Input | SHA-256 |
| --- | --- |
| Root manifest | `043a6f23c339d241933c2d0f6a1702e8a689d62444b9a1d5932801042a877a91` |
| Root lockfile | `a330429dc9941eb36fb1fb8d8ebc4fffe2c73e9b98fd1d6247b9c6b8e37b87af` |
| Host manifest | `60deb16d65a681ced5f5eddb5369de3f0146046f8e3727c0adb75827bbd5d330` |
| Host lockfile | `7bc872926fbeddd23706750c6205582ba9eae6c873612794e06e862503bb6be7` |
| Local tooling implementation | `5d09e8701e4b42266ef91d6f955374ce899c469ba88fc5c89b259e3fe94f820d` |
| Local tooling tests | `6c9e189658c36cbad06dea9a97e06c0733c9fc30b5687548b7e421c640d420c4` |

Executed checks: `cargo test --locked --offline --example repo-tool` (13 tests,
all command effects substituted), `cargo clippy --locked --offline --example
repo-tool --lib -- -D warnings`, and `cargo +1.88.0 check --locked --offline
--lib --example repo-tool` passed. The isolated runner's locked/offline Rust
1.88.0 check with `--target-dir target/runtime-qualification` also passed.
Formatting, whitespace, locked/offline metadata for both roots, and existing
snapshot integrity passed. No release command, broad release gate, native macOS
qualification or live Git effect was executed.

The maintainer subsequently clarified that the current local Shared Tooling
rules are authoritative while being edited, including uncommitted changes.
[AGENTS.md](../AGENTS.md) now records that explicit policy-source exception.
Following those rules and implementing local alignment do not await an upstream
commit; the retained snapshot continues to identify its actual vendored inputs.

## Formatting hook adoption, 2026-10-05

The subsequent clean Shared Tooling revision
`c0206f1943238e21bd00fbe01658e6a0864c24fa` supplied the standard formatting
hook and installer. The reviewed snapshot was refreshed through its distribution
helper to 18 files, including all linked governance rules and both executable
hook files. Its SHA-256 is
`e1a40b26feace978c61a7052feb76360f07621466088890a3321cbf86dab39cf`.
The live-local authority exception remains in effect for future uncommitted
shared rule changes. This is tooling evidence; earlier runtime measurements are
not relabelled as builds of these inputs.

`make fmt` and `make fmt-check` now use cargo-sort 2.1.4 and pinned rustfmt for
both maintained workspaces. Locked/offline Cargo metadata before and after
sorting is equal after dependency-order normalization for each root. The two
selected lockfile hashes remain `a330429dc9941eb36fb1fb8d8ebc4fffe2c73e9b98fd1d6247b9c6b8e37b87af`
and `7bc872926fbeddd23706750c6205582ba9eae6c873612794e06e862503bb6be7`.
The published changelog-history hash is also unchanged. No dependency, toolchain,
runtime contract or release version was changed in this batch; 0.25.14 remains
the compatible pending release.

On Linux x86-64, the complete upstream `scripts/ci/test-git-hooks.sh` suite
passed directly from that reviewed checkout with prepared cargo-sort 2.1.4,
jq 1.8.1 and Rust 1.99.0, offline and with automatic Rust installation disabled.
It exercised selected refresh, odd filenames, unrelated edits, partial staging,
failure isolation, concurrent index/worktree changes, alternate indexes,
symlinks, installer refusal and both-workspace manifest sorting/metadata and
lockfile preservation. That upstream driver SHA-256 is
`20a4ef7586709833933b2cc24e91ac84fc882031ce1c8705a5139789c86edd2b`.

The consumer-owned `make test-hooks` additionally passed with the actual Make
targets and both workspace layouts. It checks selected refresh, partial source
and configuration staging, failure isolation, unrelated edits, installer
idempotence/conflicts, preserved comments and absence of builds or lock creation.
Fixtures reuse an existing commit read-only; they create no commits or tags.
This driver is not a patched vendored upstream script. CI now exercises it
natively on Ubuntu, macOS ARM64 and macOS x86-64; those configured macOS runs
are not yet qualification evidence.

| Focused-check input | SHA-256 |
| --- | --- |
| Root manifest after sorting | `83c84f82283f71ed5a6d3a8c80ce2337771ee9744ba7ae20b8f14c368e570959` |
| Consumer Make targets | `5247127149a5dbb38ffa04248442bae35827d5b2da7833a277c74e0bdf7e09d0` |
| Consumer hook driver | `b251105f24790ad86af86c4eed48770e16452eb360ddef20e2397fd7005beaff` |
| Local tooling implementation | `1e6ab7eb0a64a14ca130689d6a4fd6528b6dd7b8c9a6bd372d15e2e8ee6b6d68` |
| Local tooling tests | `fb9fe4e6812eac22e21411c23d4943bac91329c3b39402b2b1c2fc526b4ee63f` |

Focused release-helper tests passed on Rust 1.99.0 and 1.88.0 (13 each), including
rollback and preserved evidence/artifacts when the new prepared-metadata
formatting check fails before packaging. Git and release command effects in
those tests remain substituted. Selected helper/library Clippy with denied
warnings, Actionlint 1.7.12, ShellCheck 0.11.0, shell syntax, both-workspace
formatting, snapshot integrity and whitespace checks passed. Logs are retained
under `target/qualification/maintainer-improvements/shared-hooks/`.

Local activation was checked separately: the reviewed installer set only this
clone's `core.hooksPath=.githooks`; the hook remains executable. The real index
was not staged or passed through the hook. Source HEAD remains
`11b983517a0bce1e2c2ab3fe86da76f6c6aa62a1`. No commit, tag, push, publication,
full release gate or new runtime measurement was performed. The separate shared
release-runner integration gaps remain documented in [RELEASING.md](../RELEASING.md).

## Subsequent live-local tooling review, 2026-10-05

Shared Tooling next changed its local recovery policy to resume saved unfinished
intent automatically through the same normal release target, before selecting
another increment. It also corrected the installer for logical checkout aliases.
Those reviewed files were uncommitted over HEAD
`c0206f1943238e21bd00fbe01658e6a0864c24fa`; that commit does not identify their
changed bytes. The following inputs remained identical before and after the
focused upstream checks:

| Live local input | SHA-256 |
| --- | --- |
| Shared baseline | `1c52e09906f5b99411dbef850b51600663185d23fa7f159bc0e89bab28f6bbb6` |
| Shared release contract | `5f0497c3f1b6ac9b256014848468b065a6b34ef5369f62d7b91e5f3653ec4998` |
| Shared release runner | `980fb6babdf20d0df6e6295d724152343a5c458ab1451b54706c7c54e50f1810` |
| Shared release stub driver | `8f06048c269f1e481389790a55d5df89a864c84151777e43fa5d382e6c24562e` |
| Shared installer | `c48e767cb16fce756bea55492e33c62fd1e57560310d63bfb8a1adc118be2212` |
| Shared hook driver | `0d71950125e09b89c2ffb7800170e1cfbb2d9ab643aa9f8780e02897098af6a7` |
| Consumer Make targets | `c125c57b45e69bf744dbf95c1227fed71ebbd96b70ae904980d9153e2d709a6c` |
| Consumer hook driver | `34fa2dcf4f3ed933b95cc60d2b2b5d8f4d9f624f1836e9b8a24d2666e78f51c7` |

The consumer path-alias regression failed with the prior setup target's false
repository-root refusal (`alias-before.log`). It passed after `install-hooks`
invoked the recorded installer by physical absolute path (`alias-after.log` and
`alias-final.log`). This is a consumer setup-boundary adjustment; no vendored file
was patched or attributed to dirty upstream source. The 18-file snapshot and its
digest above are unchanged. Native CI's existing hook test target now includes
this alias case.

On Linux x86-64, the complete live upstream hook suite passed with prepared
cargo-sort 2.1.4, jq 1.8.1 and Rust 1.99.0. The complete live upstream release
command-stub suite also passed: normal-target retry for all three increments,
interrupted phases and lost replies, exact identity checks, conflict refusal and
artifact retention. Its Git effects were substituted; the only real Git command
in that driver hashes stdin without writing an object. No commits, tags or pushes
were created. Version and changelog helpers retained their committed SHA-256
values `37e028e2d9fee271ffdc06d06fc15d7a148ec524def3d9ed2afcd9910c0ec642`
and `a0f5d6f4639e5a62b8e4d34aef573f982441b85214e4e58e60149f10d72d0137`.
These upstream checks do not qualify ic-memory's release adapters or replace its
documented implementation work.

Consumer hook tests, Actionlint/ShellCheck, shell syntax, both-workspace formatting,
snapshot integrity and whitespace checks passed. Logs and reviewed upstream
input hashes are retained under
`target/qualification/maintainer-improvements/shared-local-update/`. The real
index remains unstaged, local `core.hooksPath` remains `.githooks`, published
changelog history and both selected lockfiles remain unchanged, and 0.25.14 stays
the compatible pending release. No runtime source changed, so runtime tests,
Wasm measurements and full gates were not repeated. Native macOS and a live
release were not qualified.
