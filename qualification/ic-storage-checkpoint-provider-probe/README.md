# Installed checkpoint provider experiment

This unpublished fixture tests independent checkpoint opening after an ordinary
IC upgrade discards the source Wasm heap. It does not retain native pointers,
admit native layouts or complete the design's incompatible-runtime gate.

The separately selected `native-job` feature uses the same governed provider
under `native.checkpoint.job.v1` for [durable migration admission](../ic-storage-native-feasibility/migration-job.md).
Its controller/epoch/consumption contract does not change the ordinary fixture.

The separate `native-record` feature reuses the scalar bridge for
[installed record conversion and versioned checkpoints](../ic-storage-native-feasibility/records.md#installed-record-profile).
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
all-features build. The target never implicitly converts V2 during V3 recovery.
The same Make target builds and qualifies both source and target profiles.

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
