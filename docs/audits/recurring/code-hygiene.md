# IC Storage code hygiene overlay

Use the unchanged [shared code hygiene method](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/audits/code-hygiene.md)
and [audit contract](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/audits/README.md)
at those pinned upstream revisions. [The snapshot](../../../.toko-tooling.snapshot)
records the installed baseline and executables, not reference documents.
The shared method owns generic
questions, severity, verdicts, evidence and repair authority. This overlay owns
the crate-specific obligations below. An audit does not authorize fixes.

## Product authorities and traces

- Current [safety contract](../../../SAFETY.md), [advanced API](../../../ADVANCED.md),
  [current ledger](../../design/current-ledger.md) and [operations](../../operations/operations.md).
  Review current contracts; historical reports do not establish present behavior.
- Public API roots: [library exports](../../../crates/ic-storage/src/lib.rs),
  [runtime and allocation handles](../../../crates/ic-storage/src/runtime/mod.rs),
  [stable keys](../../../crates/ic-storage/src/key.rs),
  [backing IO](../../../crates/ic-storage/src/memory.rs),
  [codecs](../../../crates/ic-storage/src/storable.rs),
  [collections](../../../crates/ic-storage/src/adapter.rs) and the
  [host example](../../../crates/ic-storage/examples/composed_host.rs).
  Inventory constructors separately from inert/diagnostic DTOs and runtime owners.
- Trust transitions: declarations and namespace grants → host policy admission →
  validated directory candidate → durable snapshot/root publication → exclusive
  allocation lease. Trace [admission](../../../crates/ic-storage/src/runtime/admission.rs),
  [custody](../../../crates/ic-storage/src/runtime/custody.rs),
  [directory validation](../../../crates/ic-storage/src/runtime/directory_model.rs),
  [persistence](../../../crates/ic-storage/src/runtime/directory_model/persistence.rs)
  and [lease IO](../../../crates/ic-storage/src/runtime/directory_model/leases.rs).
  Recovery must reject malformed current-format bytes, overlapping extents and
  invalid identities before issuing authority. Keep constructors and decoding aligned;
  decoded DTOs and diagnostics never grant memory access.
- Admission and reopening remain distinct. Verify explicit grants and limits,
  backing custody and one live lease per allocation, including detached handles.
  The host owns truthful backing identity and excludes raw aliases. Runtime-owned
  persistence precedes capability publication; diagnostics grant no authority.
  Ordinary IO validates ranges and translates admitted extents without repeating
  complete directory validation. Foreign formats require explicit host replacement,
  never an implicit reset, fallback generation or legacy decoder.
- Collection boundaries include [record framing](https://github.com/dragginzgame/ic-storage/blob/4dea092f07e06749eb3f14aa24817c0962fbc0be/crates/ic-storage/src/adapter/record.rs),
  [B-tree routing and allocation](https://github.com/dragginzgame/ic-storage/blob/4dea092f07e06749eb3f14aa24817c0962fbc0be/crates/ic-storage/src/adapter/tree.rs),
  [Cell](../../../crates/ic-storage/src/adapter/cell.rs),
  [Vector](../../../crates/ic-storage/src/adapter/vector.rs), and
  [map regions](../../../crates/ic-storage/src/adapter/region.rs).
  Check bounded decoding, checksums, staged publication, cache invalidation and
  free-space reuse. Root loading and key-only scans do not validate every payload.
  Native interrupted writes poison ownership; IC traps rely on execution rollback.
- Classify panic sites against stable-memory input reachability and Storage's
  `Storable` contract. Preserve typed distinctions where operator action differs:
  corruption, invalid input, unsupported format, policy rejection and readiness.
- Trace bounds, checked arithmetic, growth failure, zero-length/page-edge IO,
  bounded decode/recovery, retained bytes and physical allocation accounting.
  Never describe allocated backing bytes as application payload occupancy.
- Consumer lifecycle and workload policy stay outside Storage. README/examples
  should show the shortest supported path; advanced authority/recovery caveats
  belong in ADVANCED/SAFETY.
  Archived API names and measurements retain their original source identities.

## Focused evidence

Inspect the selected owners under `crates/ic-storage/src`, `crates/ic-storage/tests`,
`crates/ic-storage/examples`, and `qualification`, plus relevant
Make/CI and maintained guides. Trace declared requests through admitted snapshots,
explicit host composition, examples and compile-fail consumers. Governed Metrics
and Logging checkpoints live in their respective `crates/ic-*-storage/` packages;
include them when the audited contract reaches those adapters. Runtime,
host release adapters and the independent PocketIC harness have different contracts;
name omitted families rather than implying they passed.

Use `rg` inventories for public/serde/panic/unsafe sites and their callers.
Prefer existing negative behavior and compile-fail capability tests over new
private-layout assertions. Preserve the repository's `--test-threads=1` selection
for focused runtime checks and full qualification.

- Formatting: `make fmt-check` checks both workspaces without builds or installs.
- Product changes: select the affected Rust test on the pinned toolchain using
  `cargo +1.99.1 test -p ic-storage --locked --offline <selection> -- --test-threads=1`.
  Keep test bodies and ordinary fixture helpers in `tests.rs` or `tests/` using
  ordinary module discovery; retain API doctests beside their documentation.
- Host tooling: select `make shared-tooling-check`, `make check-pins`, or named
  repo-tool tests/scripts for the affected contract. `test-tools`, `test-tooling`,
  `test-release-adapters`, `test-hooks` and `lint-tooling` are aggregates, not
  automatically focused checks. Inspect nested work before selecting them.
  Scripts substitute release effects; do not create commits/tags or activate the
  real hook merely to qualify a change.
- Ordinary fixture scratch belongs beneath the owning workspace's `target/`.
  Preserve failed evidence, exclusive directory creation and existing files.
  Explicit child-tool TMPDIR/alias probes and caller-selected evidence paths must
  retain their intended boundary tests; historical artifacts are not cleanup work.
- Check active builds before compilation or edits. Select focused checks for repairs
  under the [shared validation cadence](../../../TOKO-TOOLING.md#validation-and-evidence).
  Full `make validate` and release/group gates retain separate authorization;
  ordinary implementation ends after relevant focused checks pass. Rust 1.99.1
  is both the support floor and pinned development/release compiler. Admit actual
  compiler identities and preserve distinct feature/target/host evidence without
  repeating an identical compiler graph.
  Documentation-only work needs link, consistency and diff checks. Release,
  publication and installed PocketIC qualification retain separate authority.
  Audits do not install tools or upgrade dependencies. Preparing missing locked
  sources follows the shared Cargo network policy; offline gates remain offline.
- Performance evidence follows the owning qualification procedure, for example
  [runtime IO](../../runtime-io-qualification.md). Bind matched bytes/instructions/
  timings to compiler, lock, artifact and workload; counts are inspection aids.

## Reports and history

Keep reports in `docs/audits/recurring/`, using distinct dated filenames for new
runs. Record source/dirty scope, shared revision and overlay hash, chosen families,
checks and gaps under the common contract. This is not an automatic schedule.

The [frozen prior definition](frozen/code-hygiene-19efb34.md) reproduces historical
method evidence only and is ineligible for new runs. Old composite risk scores
are `N/A (method change)` against the shared finding-based method. Preserve
historical reports unchanged; any reused test evidence needs matching assertions
and identities. [Adoption review](shared-tooling-adoption-2026-10-06.md) maps the
prior obligations without claiming a fresh product audit.
