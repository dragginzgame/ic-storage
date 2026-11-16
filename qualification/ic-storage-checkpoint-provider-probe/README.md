# Installed checkpoint provider experiment

This unpublished fixture tests independent checkpoint opening after an ordinary
IC upgrade discards the source Wasm heap. It does not retain native pointers,
admit native layouts or complete the design's incompatible-runtime gate.

The separately selected `native-map` feature connects two native maps to the
[bounded two-map envelope and recovery contract](../ic-storage-native-feasibility/native-map-recovery.md).
Its locator is `native.checkpoint.maps.v1`, with a 2256-byte load limit and at most
64 records per map. It uses the product map codecs and scalar staging, followed
by actual native construction after checkpoint Rust returns. Run
`make native-map-checkpoint-test`. Select fixture features independently;
`native-map-job` takes precedence in an all-features build. The additional
`native-map-job` feature uses `native.checkpoint.maps.job.v1` and native format 6
for [durable two-map jobs](../ic-storage-native-feasibility/native-map-jobs.md),
with a 2360-byte load limit, controller authority, epochs, cancellation, frozen
resumption and atomic consumption. Run `make native-map-job-test`. Bare map
checkpoints are not silently enrolled; staged map import/restart remains unqualified.

The separately selected `native-job` feature uses the same governed provider
under `native.checkpoint.job.v1` for [durable migration admission](../ic-storage-native-feasibility/migration-job.md).
Its controller/epoch/consumption contract does not change the ordinary fixture.

The separate `native-record` feature reuses the scalar bridge for
[installed record conversion and versioned checkpoints](../ic-storage-native-feasibility/records.md#installed-record-profile).
It includes explicit V1→V2 field addition, checked V2→V3 narrowing and exact
V3→V2 widening; conversion policy is bound by the selected upgrade artifact.
A separate portable widening entry accepts only V3 checkpoints and constructs
fresh V2 state after heap replacement. The portable narrowing entry requires V2
and checks u32 range before constructing V3. Ordinary recovery remains schema-exact.
Run `make native-record-test`. Select it separately from `native-job`; the job
profile remains schema 1 and takes precedence when both features are enabled.

The separate `native-record-vector` feature stores the bounded
[record-vector envelope](../ic-storage-retention-probe/record-vector-recovery.md)
under `native.checkpoint.record_vector.v1`, with a 320-byte loaded-value limit
(256 bytes for V3, 320 for the explicit V2 conversion source).
It reuses the governed reader and scalar-only bridge composition, with six
record-specific callbacks. Run `make native-record-vector-test`. Select fixture
features independently. `native-record-vector-source` selects an ordinary Rust
`Vec<V2>` source for portable narrowing into V3; it takes precedence in an
all-features build without `native-map`. The target never implicitly converts V2 during V3 recovery.
The same Make target builds and qualifies both source and target profiles.
It also builds `native-record-vector-v2` for full-width native predecessor
export/recovery and `native-record-vector-convert` for explicit retained conversion
under keep mode. The latter adds five scalar conversion callbacks and a cold
source-admission gate; its new Rust runtime never opens an old V2 pointer or Vec.
Full-width V2 recovery remains available when narrowing refuses. These features
are separate closed fixtures, not an automatically selected migration mechanism.
The V2 bridge also exposes a private portable widening entry selected explicitly
by the linker: it accepts only a complete V3 checkpoint, reconstructs fresh V2
records, and leaves the checkpoint unchanged. Exact V2 recovery still rejects V3.

The `native-record-vector-job` feature selects the shared job adapter with V3
record fields, under `native.checkpoint.record_vector.job.v1` (360-byte maximum).
Run `make native-record-job-test`. Its envelope is `ICJRV001`; no V2 conversion
or adoption of the ordinary record checkpoint is implicit. Source, V2 and
conversion fixture features take precedence when combined; qualify this job
feature independently. See the [record job contract](../ic-storage-native-feasibility/migration-job.md#record-vector-job-profile).

## Frozen provider boundary

- Reader/writer: published `ic-storage` **0.74.1**, selected by the qualification
  lockfile, registry checksum
  `26d2520d368ccea2560cd496efce6408652ad681296111f9fe45561796c92ed0`.
- Provider directory: Storage `STORNAM1` version 1; cell: `STORCELL` version 1.
  Storage owns these formats and their checked reader, including the backing
  memory manager. This fixture never implements a parallel reader.
- Owner `native-checkpoint`, authority prefix `native.checkpoint.`, stable key
  `native.checkpoint.scalar.v1`. Allocation numbers are provider-owned.
- Value: `StableCell<Vec<u8>>`, maximum loaded payload 4216 bytes, containing the
  [scalar/vector checkpoint](../ic-storage-native-feasibility/checkpoint-format.md).
  Its schema, epoch, seal and digest checks precede root publication.

The reader accepts envelope versions 1 (at most 16 elements) and 2 (at most 512).
The named key is the original locator, not an envelope-version discriminator.
An empty `export_checkpoint` argument preserves the version-1 command; one
little-endian u64 argument equal to 2 explicitly selects version 2. Oversized
version-1 exports trap without updating the cell or freezing the source.

This repository owns this locator, adapter, envelope and conformance tests and
the maintenance obligation for the pinned reader while these checkpoints are
supported. The sibling Storage repository remains frozen. Changing provider,
locator or format requires an explicit conversion and matching reader evidence.

Initialization creates the owned cell. Import creates a new transient
`MemoryRuntime`, requires the existing allocation key before bootstrap, opens a
new governed lease, and calls `StableCell::load`; it never creates a missing cell.
The runtime constructor can initialize an empty provider directory, but a missing
locator then traps and the failed upgrade must roll back those writes. Neither
old Rust handles nor source module/allocator metadata locate the checkpoint.

## Protocol under test

Source state is an ordinary transient counter and `Vec<u64>`. Export writes one
complete bounded checkpoint and freezes mutation in the same atomic message.
An explicit ordinary `upgrade_canister` discards Wasm memory; no keep mode or EOP
marker is used. The new executable checks its freshly initialized static and empty
root, loads the checkpoint with an expected source epoch, constructs fresh native
values, and publishes the root. The provider checkpoint remains available.

Tests compare installed module hashes and logical values, mutate/re-export under
the target, and restore through a supported downgrade. Rejected imports compare
the complete stable bytes, old executable hash and old heap state, including an
injected trap after writing a replacement checkpoint. Fault injection covers
missing/unsealed exports, wrong epochs, truncation, payload corruption, unknown
schema, a missing provider locator and provider cell framing. The host-only framing mutation uses PocketIC's
test API; all canister-side stable access uses governed Storage handles.

This fixture has deliberately unauthenticated test endpoints, including corruption
and injected failure controls. It is not deployable application code. A production
migration authority must bind source identity/epoch, checkpoint digest and locator,
authorized target and recovery retention. This one-message test does not establish
a resumable migration, resource-exhaustion recovery, arbitrary graphs, retained
allocation validity or a compiler/allocator transition.

## Reproduce

With the existing compiler, cached dependencies and prepared test server:

```sh
make checkpoint-provider-lint
make checkpoint-provider-build
make checkpoint-provider-test
```

All checks are locked and offline. Setup is the existing explicit
`make retention-setup`. The ordinary Rust fixture uses a 1 MiB stack, 2 MiB
initial memory and 16 MiB maximum memory; these are test bounds, not a retained
page layout. The host validates both bounded Wasm inputs and records their hashes
under `target/evidence/checkpoint-provider/` before installation. No artifact
transformation or previous native-heap access is involved.

## Observed evidence

On 15 November 2026, the focused test passed locally on Linux with Rust 1.100.0
and the Testkit-pinned PocketIC server: two successful ordinary upgrades, eight
refused upgrades and one refused write while frozen. Both artifacts were 150104
bytes. Their installed identities were:

- A: `35e4965cbd1cb65b0d9a7f56f5e1e1dd4b7b0da12f119153c1ecb6fc65647922`
- B: `d159ac8ded7260aca3ce49909cb8cd8da59eb98d2ff8b42daafe88cec67aa307`

The explicit trap case checked the post-write trap message as well as rollback;
the missing-locator case checked that empty stable memory stayed empty after
refusal. Strict Clippy, dependency/graph policy, formatting and workflow checks
passed. Hosted macOS execution remains for CI; this local result does not claim it.

## Version-2 capacity evidence

On 15 November 2026, the original version-1 installed suite and the new version-2
suite passed with Rust 1.100.0 on x86_64 Linux and the prepared local server.
The new suite performs six successful heap-discarding upgrades over empty,
17-element and full 512-element state. It checks the complete checkpoint against
an independent byte/digest oracle, continues mutation after restoration, and
removes/shrinks then grows the full vector before re-exporting and restoring it.

Wrong epochs, truncation, corruption and unknown schema refuse. A target trap
after a governed stable write preserves the old module hash, stable memory,
checkpoint and heap state. Oversized population, partial population overflow,
unknown export versions, oversized legacy exports and frozen writes also preserve
state. The test-only `fill` and `pop` endpoints exercise these capacity cases.

Strict Clippy, eight codec tests and eight affected host checkpoint tests passed.
The installed phase passed both suites in 2.34 seconds, excluding preparation.
Source, compiler identity, build/check logs and installed output are retained at
`target/evidence/checkpoint-v2/attempt.MUq1l6/`; Wasm and identity reports are now
stored by digest under `target/evidence/checkpoint-provider/`, preserving older
named evidence. Current A and B identities are:

- A, 152220 bytes: `c475d040c669529ec96f7c190b23a2fb6f855fb81848584f577cd51a7f689c10`
- B, 152269 bytes: `5556caf43f6550fe422fc197fe7e655ee6c570878e82463ac0ed811e782e8023`

This cohort resolves the codec/provider capacity mismatch with the native fixture.
The later [connected native bridge](../ic-storage-retention-probe/native-checkpoint.md)
shares `provider.rs` and the codec while selecting scalar callbacks instead of
ordinary Global-owned application state. It exports inside the canister and
reconstructs under a relocated native domain; the host does not copy observed
values into the target. Ordinary-profile evidence above keeps its original scope.

## Platform Runtime 0.23.11 adoption (16 November 2026)

Runtime 0.23.11 supplies the synchronous surface requested in
[Platform #6](https://github.com/dragginzgame/ic-platform/issues/6).
Our three direct consumers select only `system-raw`, defaults disabled, and use
`ic_platform_runtime::raw`. Argument/caller copies, live controller checks,
raw append/reply and byte traps retain their signatures; benchmarks use
`instruction_counter()` for IC performance counter zero. Exports, buffers,
authorization, reply ownership and upgrade hooks remain consumer-owned. No
executor, callback registry, futures or initialization requirement is introduced.

The registry archive SHA-256 is
`4dc0059577167175de70e44128d415b772d9d20f3276cb3d8aeb09b01448ec09`,
matching the official crates.io index. The published source identifies commit
`cbbf26ade6644ed6fbcadfe3ccbafe809c362f25`; its
[supplier contract and qualification](https://github.com/dragginzgame/ic-platform/blob/cbbf26ade6644ed6fbcadfe3ccbafe809c362f25/docs/qualification/runtime-storage.md)
distinguishes raw synchronous calls from executor-managed replies.

The frozen published Storage 0.74.1 checkpoint oracle still selects Runtime
0.22.5 with only `ic0`. Its stable-memory binding calls share neither Runtime
Rust types nor executor state with our 0.23.11 raw calls. Dependency checks admit
two Runtime identities only in the checkpoint and Storage benchmark graphs;
the startup graphs still require one. All retain the prohibitions on the Platform
facade, Types, singleton guard and Toko Tooling. This does not modify the legacy
repository or assert that its transitive dependency has been upgraded.

The earlier 0.23.9 resolver refusal remains historical evidence under
`target/evidence/dependency-adoption/platform-0.23.9/`; that release omitted the
required synchronous API. Its archive SHA-256 was
`3b4f8c548f008420e4527abffc72663a29dc6712b00014d373b341ced0d74381`.
The new direct selection removes that blocker and the proposed compatibility
hold is no longer needed. Historical installed cohorts retain their original
compiler and dependency scope.

Focused consumer checks passed under Rust 1.100.0 with locked offline Cargo:
`fmt-check`, `deps-check`, startup/managed-graph/benchmark/checkpoint Clippy,
and freshly built installed startup (3 cases), managed graph (2 cases), and
ordinary checkpoint (2 cases) checks. Provider cohorts each passed 18 raw
artifact checks plus their selected cases:

| Scope | Evidence under `target/evidence/provider-link/` | Selected cases |
| --- | --- | --- |
| Native checkpoint | `attempt.w2TDQH` | 4 |
| Scalar records, retained and portable conversion | `attempt.zP4I9Z` | 7 |
| Record vectors | `attempt.FgnOOU` | 10 |
| Scalar governed jobs | `attempt.RpCepx` | 7 |
| Record-vector governed jobs | `attempt.DgXVFl` | 5 |

All cohorts have completion markers. The latter two include authorization,
restart/cancellation and seeded migration histories. Record-vector codec tests
and migration-job state-machine tests also pass. Adoption logs, supplier contract,
registry observation, selected graphs and locks are retained under
`target/evidence/dependency-adoption/platform-0.23.11/`.

The final benchmark cohort `target/evidence/simple-benchmarks/1794847479348/`
contains 216 validated samples and records both Runtime versions. The preliminary
cohort `1794847453508` passed but retained the old hardcoded Runtime report label;
use the final cohort for adoption provenance. Compared with historical cohort
`1794757137285`, complete uncompressed managed/Storage Wasm sizes are
36,878/136,314 bytes (previously 36,870/136,132); complete code-section sizes remain
34,265/105,199 bytes. This bounds observed artifact cost for these fixtures and
is not a general performance or production-persistence claim.

## Platform Runtime 0.23.12 adoption (16 November 2026)

The current direct requirement and lock select Runtime 0.23.12 with only
`system-raw`, defaults disabled. The official registry archive SHA-256 is
`6fa7c8ed9f3a84ba059ea06d2a9b21f6d77061ca33056c50bff2e58468337184`;
its source pin is `2881ded6c0206f191df386901101fc7f4d5fa949`.
The synchronous implementation is byte-identical to 0.23.11. The changed timer,
canister-version and executor code is behind unselected `call-ic0` features.
The selected Runtime normal graph has no dependency edges. No additional Platform
crate is used directly by this repository; the frozen Storage 0.74.1 oracle
continues to select its own 0.22.5 Runtime as described above.

The existing lockfile already selected 0.23.12 before this adoption; this work
aligns the declared requirement, current documentation and benchmark provenance,
and qualifies the selected release. Earlier 0.23.11 cohorts retain their original
scope. Focused validation covers dependency boundaries and host/Wasm Clippy,
then installed startup, native checkpoint recovery, record-vector governed jobs
and benchmark replies. These exercise raw replies, arguments, traps, live
controller checks and instruction observations under the current selected
features; they do not qualify the newly added executor/timer API.

These focused checks passed under Rust 1.100.0 with locked offline Cargo:
formatting, dependency boundaries, selected host/Wasm Clippy, installed startup
(3 cases), native checkpoint recovery (4 cases), record-vector governed jobs
(5 cases) and the job state machine (12 cases). Provider cohorts
`target/evidence/provider-link/attempt.KHjyod/` and `attempt.LfgXJc/` each also
passed 18 raw artifact checks and have completion markers. Benchmark cohort
`target/evidence/simple-benchmarks/1794854246588/` contains 216 validated samples
and records Runtime 0.23.12. Registry/source comparison, selected graph, locks
and logs are retained under
`target/evidence/dependency-adoption/platform-0.23.12/`.
