# Independent Cell replacement specification

Status: separately implemented Cell integrated; see the qualification results below.
The maintainer authorized independently designed formats on 2026-11-10 with
fresh/reset backing and explicit rejection of existing images. Reset is a host
action, never a recovery fallback. No deployed data conversion is required.

## Authorship boundary

This specification was prepared in a session exposed to the previous source.
It describes required observable behavior, not a source-independent authorship
certificate. An independent implementer receives this specification, reviewed
public interface declarations and the public acceptance harness only. Do not
supply previous engine source, internal tests, implementation diffs, old layout
diagrams or the replacement inventory as implementation instructions. Review the
handoff inputs before implementation. A fresh session alone is not proof of
independence. Do not paraphrase or translate the previous implementation.

ICSS may be executed as an external comparison library in unpublished benchmark
packages. It must not supply production source, codecs, traits or backing.

### Implementation record, 2026-11-10

The maintainer approved a separate restricted implementer. Agent
`independent_cell` started without the parent conversation and received the
behavioral contract plus Memory, Storable, lease/guard and error declarations.
It reported reading only AGENTS.md, TOKO-TOOLING.md, the two product manifests,
this specification, the public Cell acceptance suite, Rust code hygiene and
engineering policy. It did not read previous implementation files, internal
tests, source history, registry source or siblings. It overwrote the Cell adapter
and framing module from that handoff and authored new framing tests.

The implementer selected a 24-byte envelope: `STORCELL`, u32 version 1, four
zero reserved bytes, and a u64 encoded length, with integers little-endian.
The source-exposed integration reviewer removed the copied Cell engine and its
layout tests, updated consumers, and qualified incompatible ICSS-image refusal
using the external published comparison dependency. Mechanical rustdoc and
private-module visibility lint fixes were made during integration.

A first measured implementation unnecessarily wrote framing for empty-to-empty
replacement. The implementer received that cost observation, not the previous
engine's implementation, and independently added an admitted encoded-length
cache. Equal lengths omit header writes; empty-to-empty performs no backing IO.
The reviewer supplied the existing `RuntimeMemory::size` interface guarantee:
it checks health using admitted heap state without underlying backing IO. Every
replacement still validates health, encoding and declared bounds.

This records the actual process; it is not independent legal certification or
clearance of other Storage components. The parent reviewed the resulting code
but supplied no prior Cell implementation to the implementer. Existing Memory,
codec and allocation facilities remain in separate origin-review/replacement
scope, so the entire production dependency path is not yet source-independent.

## Observable contract

Cell owns one application value and an exclusive governed allocation lease.
The host admits declarations and commits allocation before construction. Cell
does not choose allocation IDs, construct independent managers or bypass custody.
Keep the current successful-call surface unless a separate API change is agreed:

| Operation | Required behavior |
| --- | --- |
| `create(memory, value)` | Succeeds only on empty allocation; refuses nonempty storage without modifying it. |
| `reset(memory, value)` | Explicit destructive replacement requested by the caller. No implicit reset in other operations. |
| `load(memory, maximum)` | Read-only admission; reject unsupported framing, impossible extents and values above the caller/type ceiling before allocating a payload buffer. |
| `get()` | Borrow the cached value, with no backing IO or codec invocation. |
| `set(value)` | Persist replacement and return the previous value. Successful reopen observes the replacement. |
| Drop | Release the exclusive lease; do not erase or rewrite the persisted value. |

The caller supplies the application codec and its declared bound. Empty,
same-size, shrinking and growing encodings are valid. Preserve existing
trap-style malformed-codec behavior and typed admission failures; do not add a
second successful-call API. Bound checks and arithmetic must precede allocation
and IO. A failed load must neither initialize storage nor leak a lease.

Persisted framing is the implementer's design decision. Use a new, independently
selected discriminator and explicit version admission; do not retain the old
layout merely for compatibility. The host-approved reset boundary is the whole
backing for the completed replacement. An incompatible image must be rejected
before payload decoding. No legacy reader, migration shim or fallback engine.

IC executions supply rollback. Do not add a transaction journal for that purpose.
Native interrupted mutation must not leave a usable owner that can silently read
partially updated bytes; retain Storage's owner-health integration. Cell must
not access neighboring allocations. Cached references follow ordinary Rust
borrowing and cannot outlive their Cell.

Owner health guards backing access and new leases. `get()` only borrows the
already cached Rust value; it need not perform a health check. A native caller
must discard the owner after a mutation panic. Qualification checks that another
mutation and new lease admission refuse before further backing IO, rather than
requiring cached borrows to trap. A failed read-only decode must release its lease
without rewriting the image; it does not itself publish a mutation.

## Qualification

The new public harness is `crates/ic-storage/tests/cell_contract.rs`. It checks
observable semantics with no engine imports, format constants or ICSS dependency.
The initial eleven tests passed against the previous engine to establish the
contract and now exercise the replacement. They include instrumented cached get/drop, read-only admission and ceiling
refusal before payload reads/decoding, codec refusal, initial/later growth refusal,
and partial writes at every observed same-length set write boundary. The injected
failures verify owner refusal before further IO and neighboring-cell preservation.
Write boundaries are discovered through `Memory`, not prescribed from old engine
internals. Further tests inject creation/reset writes and load reads. Eight new
framing tests cover the independently selected format and arithmetic boundaries.
External ICSS comparison tests reject empty and populated ICSS Cell images
without mutation, then verify explicit reset. This is not full qualification.

Qualification now includes 96 library tests, 14 public Cell tests, five public
runtime tests and 28 comparison-library tests. Strict Clippy passes for the
affected native targets. Native failures cover creation/reset writes, same-size
replacement writes, load reads, codec failures and initial/later growth refusal.
The zero-IO replacement test checks codec invocation and poisoned-owner refusal.
New writer tests cover shrink, empty, same-length and growing values.

Installed IO, custody, checkpoint, upgrade and rollback qualification passes
original/O3/Os/Oz modules at `target/qualification/runtime/attempt.12W8BY`.
The separate frozen-old-application test passes: an upgrade from the previous
Cell consumer traps, the old value and application remain usable, and a fresh
installation of the new consumer succeeds. No old-image decoder participates.

That last test intentionally requires external installed fixtures and is ignored
in ordinary native runs. Run it through the prepared Testkit server with
`CELL_PREVIOUS_WASM` and `CELL_REPLACEMENT_WASM` pointing to the respective
Cell-only size consumers, and `IC_TESTKIT_POCKET_IC_URL` supplied by the runner:

```sh
cargo test --locked --offline --manifest-path qualification/Cargo.toml \
  -p ic-storage-benchmarks --features owned,dfinity --lib \
  --target-dir target/runtime-qualification \
  incompatible_cell_upgrade_traps_and_preserves_previous_application -- --ignored
```

The retained old consumer is `target/evidence/storage-consumer-size/attempt.F0yQNz/cell-storage.wasm`;
the new consumer is in `attempt.yFBZRG`. Rebuild the old consumer from its
retained source archive when reproducing beyond the local artifact lifetime;
never substitute a current-format module as the old-image control.

The previous Cell engine and its layout-specific tests are deleted. Shared
facilities still used by other collections remain attributed until replaced.
No production release has been made. Before deployment hosts must arrange the
authorized fresh/reset backing boundary. A numerical cold-load instruction study
and an exhaustive native failure sweep of every growing-set directory mutation
are not established by the current cached-operation benchmarks; do not claim
those measurements or universal performance parity.

### Matched measurements

The accepted application-size run is `attempt.yFBZRG`; the pre-Cell working
baseline is `attempt.F0yQNz`. Both already include the pending B-tree changes,
so these deltas isolate this Cell step rather than the entire unreleased batch.
Each run qualifies all eight application pairs with calls and two upgrades.

| Raw application Wasm bytes | Before Cell replacement | Replacement | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Cell only | 93,639 | 91,923 | 63,135 |
| All collections | 135,547 | 135,939 | 119,318 |

Cell saves 1,716 bytes; the all-collection application adds 392 bytes. The broad
benchmark module is not an application-size proxy.

The first unchanged-harness comparison retained all 2,472 samples and identical
ICSS controls. It exposed large-inline-value regressions in both Control and
Get; the getter inline experiment did not help and was removed. For Blob300,
batch-eight Get increased from 10,675 to 13,953 instructions while Control rose
from 7,459 to 10,737. Do not silently discard those raw results or infer a
universal application speedup from control subtraction.

The accepted harness removes unused `Option<V>` replacement inputs from read
phases for both libraries. This is a separate measurement correction, not a
library speedup. With that matched harness, all 73 Cell types pass: 2,472 checked
samples, 1,236 paired rows. Among 962 non-Control rows, Storage is lower on 484,
equal on 327 and higher on 151. The bulk suite adds 24 checked samples and 12
paired rows; among ten non-Control rows Storage is lower on two, equal on two
and higher on six. These are one-sample deterministic instruction observations,
not a statistical throughput claim.

| Inner-operation instructions, corrected harness | Storage | ICSS 0.7.2 |
| --- | ---: | ---: |
| Empty unit replacement, single | 352 | 160,762 |
| u64 replacement, single | 160,705 | 161,050 |
| Blob300 cached Get/clone, batch eight, empty logical value | 10,432 | 10,432 |
| Blob65536 cached Get/clone, batch eight, empty logical value | 2,856,096 | 2,856,096 |
| Byte value replacement, 1 MiB | 3,769,130 | 3,769,835 |
| Byte value growth, 1 MiB to 1.5 MiB | 5,975,648 | 5,738,976 |
| Shrink after that growth | 240,589 | 160,955 |

Blob rows identify inline capacity, not logical payload bytes. Get includes
cloning and result construction. Both references use eight-page buckets. Growth
and shrink still expose backing/integration costs; the replacement does not
establish parity for every operation or erase those gaps.

Evidence is under `target/evidence/cell-replacement/`: `baseline-*` uses the
pre-replacement engine, `candidate-*` the initial independent engine, `final-*`
the refined engine with the old harness, and `harness-*` the accepted matched
measurement. Source/archive and module hashes are retained there. The accepted
production source matches the application-size and lifecycle-qualified source.
Standard reproduction is `make benchmark-storage-size`, and
`BENCH_WORKLOAD=types make benchmark-storage` / `BENCH_WORKLOAD=bulk make benchmark-storage`;
the retained runs selected the Cell shard through the documented driver.

### Direct payload reads — 2026-11-11

A separate restricted implementer refined the current independent framing
reader using this contract, current Cell source/tests and the owned Memory
interface. No predecessor or ICSS implementation was supplied. The reader now
initializes fresh vector capacity directly and publishes its length only after
successful IO. A partial-read panic drops a zero-length vector; empty reads still
perform no IO. Framing, ceiling admission, codecs and the public API are unchanged.

Patterned boundary tests exercise the default scratch reader and a direct
override, including prefix failures and read-only retries. Public load-failure
and lease-release tests, native/Wasm lint and installed upgrade/rollback pass.
Custom backends using Memory's default raw-read fallback incur chunked copying;
the production governed backing forwards directly. No universal custom-backend
speedup is claimed.

The new maintained `cell-open` workload measures reopening rather than cached
`get()`. It restores each declared payload before the load control, then checks
growth and shrinkage across three rounds. See the
[matched instructions, application sizes and reproduction](../measurements/storage-scalar-scans-2026-11-11.md#direct-buffer-initialization-step-5)
for the retained change. It does not change STORCELL version 1 or deployment state.
