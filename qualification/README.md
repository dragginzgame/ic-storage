# Installed runtime qualification

This independent virtual workspace contains the unpublished host package
`ic-storage-runtime-qualification/` and owns its separate `Cargo.lock`.
It also contains the explicitly selected [Storage/DFINITY map comparison](ic-storage-benchmarks/README.md).
The default member remains the qualification runner; normal runtime qualification
does not build or run the comparison.
The package keeps PocketIC dependencies out of
the library's normal graph. It installs the supplied
[`wasm-io-qualification`](ic-storage-test-fixtures/tests/fixtures/wasm_io_qualification.rs) fixture in a
caller-owned server selected by IC Testkit. The Storage-owned
[`wasm-storage-qualification`](ic-storage-test-fixtures/tests/fixtures/wasm_storage_qualification.rs)
fixture additionally checks Cell, Map, Set, Vector and map regions across 1,031
named allocations through successful upgrades and rollback of trapped
updates/upgrades. Vector coverage includes fixed u64 slots and variable
4,096-byte-bound records, replacing 256-byte payloads with 2,048-byte payloads
and back. It never starts or downloads a server.
The Logging-owned
[`wasm-logging-qualification`](ic-storage-test-fixtures/tests/fixtures/logging_qualification/main.rs)
fixture checks query non-persistence, update/upgrade rollback, canonical reopen
and watermarks after pruning every row. Its bounded native Wire protocol is shared
with the host runner; it installs no library endpoint or lifecycle hook.
Allocation and collection fixtures also share fixture-only bounded Wire contracts with the
runner in [`io.rs`](ic-storage-test-fixtures/src/io.rs)
and [`storage.rs`](ic-storage-test-fixtures/src/storage.rs).
Requests and responses bind method identities, including unit requests and
installation/upgrade arguments. Their limits are 128 encoded bytes, 4,096
allocation units, 128 nodes and depth eight. Canisters reject oversized input
before copying it. Malformed, foreign and oversized requests are checked against
the installed fixtures without retained mutation. Rebuild fixtures and runner
together: the earlier raw packet protocol has been retired. These schemas add
no public library endpoint and change no durable ledger or collection format.

Testkit 0.102.0 re-exports the async PocketIC client and selects server 16.1.0.
Drivers own a current-thread Tokio runtime, five-minute polling budgets and
awaited best-effort instance deletion. The managed CLI bounds server lifetime;
the polling budget is not an absolute HTTP deadline. The composition fixture
independently selects Platform 0.31 / Runtime 0.21. Native client dependencies
do not enter product Wasm. Prepare the
server explicitly with `make install-runtime-server`,
check it offline with `make runtime-server-check`, and prepare the selected IC
tools with `make install-ic-tools` before running `make test-runtime`.
The latter builds the supplied fixture and runner offline, then qualifies the
original Wasm and Binaryen's `-O3`, `-Os` and `-Oz` outputs. Optimization admits
`memory.copy`/`memory.fill` through `--enable-bulk-memory-opt`, without selecting
a broader feature set. Each variant receives the same
startup, IO and upgrade checks with a separately managed Testkit server. Wasm,
input hashes and optimizer/server/runtime logs remain under
`target/qualification/runtime/` in the repository root, including after failure.
Normal library Wasm builds and budgets remain unoptimized. Prepare both locked
dependency caches separately.

For the focused Logging proof, run `make test-runtime-logging` after preparing
the server and both dependency caches. It builds and installs only the original
Logging Wasm; it requires no optimizer and skips the allocation/collection runtime suites.
The full `make test-runtime` includes Logging in each optimization variant.

The binary-only runtime qualifier inherits Toko through
`target.'cfg(any())'.dev-dependencies`: this keeps source-tooling selection in
Cargo metadata and the lockfile without compiling an unused marker in unit tests.
The exact workspace pin and cargo-machete exemption remain required.

Use the [current handoff](../docs/status/current.md) for the owned-engine
qualification and limitations. The [earlier record](../docs/runtime-io-qualification.md)
retains historical source/format-specific evidence. The `IC_MEMORY_QUALIFICATION_WASM`, `IC_STORAGE_QUALIFICATION_WASM`, `IC_LOGGING_QUALIFICATION_WASM`
paths and `IC_TESTKIT_POCKET_IC_URL` are required when invoking the runner
directly. `--measure-valid-io` skips the rejection
and upgrade checks and does not require the Storage fixture; its output alone is not qualification evidence.
`--logging-only` requires only the Logging Wasm and the server URL and conflicts
with measurement mode. The Logging proof reports assertions, not cost measurements.
`--storage-only` similarly requires only the Storage Wasm and server URL, and
qualifies collection admission, IO, upgrades and rollback without running the
allocation-IO or Logging suites. It conflicts with both other selection flags.
Add `--storage-install-wasm /absolute/path/to/earlier-compatible-fixture.wasm`
to populate with an earlier fixture before upgrading to
`IC_STORAGE_QUALIFICATION_WASM`. Both fixtures must implement the same test
protocol and admit the same persisted format. The runner additionally checks
successful mutation and trapped-write rollback after that upgrade; it performs
no migration or automatic reset. Omitting this option retains the usual
same-build install/upgrade proof. Record both input modules with the evidence.
`--help` lists the supported arguments without reading Wasm or contacting the
server. Unknown and trailing arguments are rejected before runtime setup.
Measurements retain the `valid_io` instruction rows and additionally report
`valid_io_cycles` from the actual cycle-balance change around each 1,000-operation
update. That cycle total includes update admission and lease opening outside
the inner instruction counter. Corruption qualification exercises both lease
opening and growth checkpoints and subsequent poisoned-handle refusal.

The shared Wire fixtures have a fresh source-bound measurement baseline in the
[installed qualification record](../docs/runtime-io-qualification.md#shared-wire-fixtures-installed-qualification--2026-10-30).
The inner instruction counter still covers only the 1,000 IO delegates; total
update cycles include Wire decoding/reply work and lease opening. Earlier raw
packet measurements remain historical evidence for their original builds.
