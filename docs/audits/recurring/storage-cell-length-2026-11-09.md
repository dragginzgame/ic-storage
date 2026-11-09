# Cell encoded-length cache — 2026-11-09

Retain this first Cell-specific improvement. The private engine keeps the admitted
or successfully persisted encoded length in a `u32` field beside the cached value.
This consumes additional heap state (including any alignment padding), not stable
bytes or another encoded payload. DFINITY **ic-stable-structures 0.7.2** caches
only the value and rewrites framing on each replacement. Storage's existing
codec-independent writer remains shared across value types.

`engine/cell.rs::flush_bytes` still encodes and validates the length. When it equals
the cached length, the existing payload necessarily fits in the admitted exclusive
memory: write only that payload, or nothing for an empty payload. New/reset and
size-changing writes retain the capacity check, growth and header update. Update
the cached length/value only after a successful write. Loading initializes the
cache from the admitted stored header, not by re-encoding a potentially
canonicalized decoded value. The ownership contract prohibits independent writes
through aliases; this is not a cache for externally mutable backing.

SCL1 bytes, public APIs, trap behavior, native poisoning and IC rollback contracts
are unchanged. This is not a journal, checksum, second Cell implementation or
migration. The reused ICSS code retains its provenance and license notices.

## Matched instruction results

Baseline: `attempt.Ghlr0u`; candidate: `attempt.B7ATSm`, both under
`target/evidence/storage-comparison/`. Baseline is HEAD `07c1b1f` plus the pending
Build/Host adoption, overflow-initialization optimization and ordinary size-consumer
modules. These unrelated changes and both lockfiles remain fixed.

Rust 1.99.1; benchmark Wire 0.18.0, Runtime 0.20.0, PocketIC 16.1.0 via Platform
client 0.19.0; ICSS 0.7.2 with eight-page buckets. Root Runtime 0.20.3 is a separate
incoming lock selection. One sample per case; depth has three stateful rounds.
These are **inner-loop instruction counts**, not full-handler or application
throughput. ICSS Wasm and instruction counts are identical between trials.

| Workload | Improved | Equal | Regressed | Scope |
| --- | ---: | ---: | ---: | --- |
| All-collection depth | 8 | 712 | 0 | Only Cell replacement changes |
| Full Cell catalogue | 332 | 822 | 82 | 73 types, 1,236 cells |
| All-collection bulk | 2 | 110 | 4 | Only Cell replacement/grow/shrink changes |

All **7,024** raw logical results match after excluding instruction counters:
4,320 depth, 2,472 catalogue, 232 bulk. Stable-page counts match throughout.
The catalogue's 822 control/read cells are exactly unchanged. Of its 274
replacement cells, 262 improve and 12 regress; growth and shrinkage each split
35 improved/35 regressed. Size-changing cases add at most 21 instructions
(0.01304%). Bulk growth/shrinkage adds 19 instructions. These small regressions
are accepted for the common same-length benefit and bounded size cost.

| Selected case | Storage before | Storage after | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| [u8;32] replace, depth batch 32 | 179,547 | 172,859 | 183,037 |
| [u8;32] get, depth batch 32 | 3,643 | 3,643 | 4,349 |
| u64 replace, catalogue batch 8 | 165,810 | 164,154 | 166,802 |
| Unit replace, catalogue batch 1 | 160,769 | 464 | 160,907 |
| 64 KiB bytes replace, bulk batch 1 | 386,412 | 386,203 | 386,511 |
| 64 KiB bytes grow, bulk batch 1 | 1,490,377 | 1,490,396 | 1,227,657 |
| 1 MiB bytes replace, bulk batch 1 | 3,769,452 | 3,769,243 | 3,769,957 |
| 1 MiB bytes grow, bulk batch 1 | 6,001,097 | 6,001,116 | 5,739,124 |

[u8;32] replacement improves 3.725% against prior Storage and is 5.560% below
ICSS in this batch. Unit/empty replacements avoid all stable writes and their
associated charges: the near-100% saving is an empty-payload special case, not a
general Cell claim. Large-value growth still favors ICSS. Cached reads gain
nothing from this change, and full application calls still pay surrounding work.

## Wasm size

Broad harness, comparison profile (opt-level 3, fat LTO, one codegen unit,
existing eight-MiB stack): **11,495,087 → 11,497,579 (+2,492 bytes)**.
The size-consumer baseline `attempt.yEPseq` was admitted against current source
hashes before editing Cell; candidate is `attempt.JcCuvj`.

| Small consumer | Storage before | Storage after | Delta | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Base | 16,950 | 16,950 | 0 | 16,946 |
| Map | 133,297 | 133,297 | 0 | 90,212 |
| Set | 132,741 | 132,741 | 0 | 90,012 |
| Vector | 108,314 | 108,314 | 0 | 64,564 |
| Cell | 105,207 | 105,377 | +170 | 63,143 |
| Region | 134,436 | 134,436 | 0 | 89,937 |
| Map + region | 137,018 | 137,018 | 0 | 110,862 |
| All | 147,910 | 148,075 | +165 | 119,326 |

These are uncompressed module bytes. The small consumers use wasm-size, not the
broad harness profile. Both deltas fit the individual-change budget; Storage's
small consumers remain larger than ICSS because this does not remove governance.

## Correctness and reproduction

Passed 95 native tests (one manual test ignored), strict library/test Clippy,
seven ICSS interchange tests and installed original/O3/Os/Oz IO, upgrade and
rollback (`target/qualification/runtime/attempt.U5smiq`). The two new native tests
cover resizing across 64-KiB boundaries, reopen, empty values, payload-only IO,
and failed growth leaving cached/stored values unchanged. Interchange now also
covers Storage same-length and empty replacements after loading ICSS images.
All eight size selections pass installed calls and upgrades.

Build and run each source state with prepared tools/caches:

```bash
BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage
make benchmark-storage-size
cargo test -p ic-storage --lib --locked --offline
cargo clippy -p ic-storage --lib --tests --locked --offline -- -D warnings
cargo test --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --features owned,dfinity --locked --offline \
  --lib icss_baseline --target-dir target/runtime-qualification
make test-runtime
```

For each depth run, reuse its frozen `owned.wasm.gz` and `dfinity.wasm.gz` with
the absolute `target/runtime-qualification/debug/ic-storage-benchmarks` executable
through the installed `ic-testkit-server run --directory "$PWD/.tools/ic-testkit-server"`
launcher. Pass `--samples 1 --workload types --collection cell` for the full Cell
catalogue, then `--samples 1 --workload bulk` for safe large-value batching, plus
`--owned-wasm`, `--dfinity-wasm` and a distinct `--output` directory. The launcher
owns a bounded server lifetime (`--ttl 900`). Never use its version subdirectory
as the installation directory.

Additional raw evidence is under `target/evidence/storage-cell-length/`:
`baseline-types/cell` / `candidate-types/cell`, and `baseline-bulk-matched` /
`candidate-bulk-matched`. Initial launcher errors, an invalid native fixture key
and custom large-value runs exceeding reply limits are retained and excluded.
The successful runs above supersede those invalid experiment invocations without
raising codec limits. Evidence under target is disposable; source and commands
permit reproduction. No release or whole-application speedup is claimed.

## Follow-up: rejected resize specialization

Tested skipping the capacity query on shrink, writing only the four-byte length
for existing Cells, and omitting empty payload writes. New Cells still wrote the
complete header. This preserved SCL1 and required no migration. The first trial
passed 95 native tests, strict Clippy, seven interchange tests and installed
original/O3/Os/Oz lifecycle checks (`attempt.UUAy12`). Correctness was not the
reason for rejection.

Against the retained cache above, the first trial improved 118 catalogue cells,
left 822 equal and regressed 296. All 70 shrink cases improved, but 261 of 274
replacement cases regressed. Tiny Unit replacement at batch eight increased
1,245 → 1,277 instructions (2.57%). A second header-buffer arrangement reduced
the depth replacement penalty from four instructions per set to two, without
eliminating it. Both trials were rejected; engine and native tests were restored
byte-for-byte to the pre-trial length-cache source.

| Inner-loop case | Retained Storage | First trial | Second trial | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| [u8;32] replace, depth batch 32 | 172,859 | 172,987 | 172,923 | 183,037 |
| 64 KiB bytes replace | 386,203 | 386,207 | Not run | 386,511 |
| 64 KiB bytes grow | 1,490,396 | 1,490,393 | Not run | 1,227,657 |
| 64 KiB bytes shrink | 160,834 | 160,660 | Not run | 161,001 |
| 1 MiB bytes grow | 6,001,116 | 6,001,113 | Not run | 5,739,124 |

The first trial's Cell-only Wasm increased 105,377 → 105,388 bytes (+11);
all-collection consumer increased 148,075 → 148,080 (+5). ICSS remained 63,143
and 119,326 respectively. Broad harness grew 11,497,579 → 11,497,641 (+62).
The second trial's broad harness was 11,497,704 (+125); no second small-consumer
run was warranted after the common-path penalty remained. These tiny size costs
were acceptable, but the narrow shrink saving did not justify penalizing ordinary
replacement. The large growth gap is still unresolved: this specialization saved
only three instructions there.

First depth: `target/evidence/storage-comparison/attempt.tlDtSh`; second depth:
`attempt.Rc5Fz0`. Other evidence, source backups and logs are under
`target/evidence/storage-cell-resize/` (`types/cell`, `bulk`, `size.log`,
`lifecycle.log`). First catalogue/bulk and second depth have all 7,024 logical
samples and non-instruction summary fields identical to the retained baseline.
The second depth has eight regressed Cell rows and 712 equal rows. All comparisons
use the existing one-sample protocol; these remain inner-loop observations, not
whole-application speedups. No resize specialization remains in product code.

## Growth attribution: engine versus governed backing

The new `cell_resize_io_separates_engine_from_allocation_costs` test in
`testing/runtime-qualification/crates/ic-storage-benchmarks/src/tests/icss_baseline.rs`
compares Storage Cell, published ICSS Cell over Storage allocations, and published
ICSS Cell over its eight-page manager. Each starts with a populated value, grows,
shrinks to empty, and regrows within the retained capacity. It checks returned
values, counter overflow, allocation-free regrowth and matched engine IO.

Native observations (one allocation, default Storage budget):

| Initial → grown payload | Backing | Reads / bytes | Writes / bytes | Physical growth |
| --- | --- | ---: | ---: | ---: |
| 64 → 256 KiB | Storage | 2 / 102 | 4 / 262,254 | 3 pages |
| 64 → 256 KiB | ICSS eight-page manager | 0 / 0 | 5 / 264,232 | 0 pages |
| 1 → 1.5 MiB | Storage | 2 / 102 | 4 / 1,572,974 | 8 pages |
| 1 → 1.5 MiB | ICSS eight-page manager | 0 / 0 | 9 / 1,574,953 | 8 pages |

ICSS Cell over governed memory has identical read bytes, write bytes and growth
to Storage Cell, but two additional writes: ICSS writes the three header fields
separately, whereas Storage combines them. Native regrowth after shrinking needs
no metadata reads or physical growth in either implementation. The initial test
incorrectly expected equal write counts; that failed assertion is retained in
`target/evidence/storage-cell-growth/io.log`. The corrected comparison explicitly
accounts for the independently verified framing difference.

The source paths explain the additional responsibilities:

- `engine/cell.rs::flush_bytes` calls memory growth only when the encoded value
  plus its eight-byte header exceeds logical capacity.
- `runtime/directory_model/leases.rs::Lease::try_grow` enters
  `persistence.rs::Store::mutate`. This verifies the stored directory, prepares a
  candidate image, validates and encodes it, and installs a new snapshot/root.
  `Store::install` grows the backing and publishes metadata before Cell writes.
- Published ICSS 0.7.2 `memory_manager.rs::grow` updates bucket ownership and its
  size header without rereading the whole manager. Eight-page reservations mean
  the 64-KiB case already has sufficient physical space, unlike Storage's exact
  allocation. The larger case requires physical growth in both implementations.

This rules out extra payload bytes or extra Cell framing writes as the explanation
for Storage's growth deficit. It does not convert native IO into IC instructions,
attribute individual page charges, or prove that verification can safely be
removed. Directory publication preserves ownership and upgrade integrity; those
contracts remain unchanged. Exact-size versus bucket reservation also trades
memory for growth frequency, so increasing default reservations is not a free win.

Reproduce the native attribution with:

```bash
cargo test --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --features owned,dfinity --locked --offline \
  --lib icss_baseline --target-dir target/runtime-qualification -- --nocapture
BENCH_REFERENCE=dfinity-governed BENCH_WORKLOAD=bulk BENCH_SAMPLES=1 make benchmark-storage
```

### Installed growth attribution result

`target/evidence/storage-comparison/attempt.YySQyA` passes the 116-cell bulk
comparison (232 logical samples). Against the retained ordinary-manager bulk
reference, all logical results match after excluding instruction counters and
physical stable-page totals, which legitimately differ by allocator. All 116
Storage instruction rows are unchanged. One sample per case; the existing
`dfinity-governed` control adds an Rc wrapper and retains its primitive codecs,
so it is a backing attribution control, not perfectly identical generated code.

| Inner-loop operation | Storage | ICSS over Storage backing | ICSS own eight-page manager |
| --- | ---: | ---: | ---: |
| Grow 64 → 256 KiB | 1,490,396 | 1,490,349 | 1,227,657 |
| Grow 1 → 1.5 MiB | 6,001,116 | 6,001,069 | 5,739,124 |
| Replace 64 KiB | 386,203 | 386,651 | 386,511 |
| Replace 1 MiB | 3,769,243 | 3,769,691 | 3,769,957 |

The growth gap drops from 262,739 / 261,992 instructions to just 47 with shared
governed backing. This strongly locates the deficit in backing management rather
than Cell encoding or payload persistence. It does not yet isolate directory
verification, candidate construction, publication and memory charges from each
other. The next useful experiment is stage attribution of those operations,
including a larger allocation directory and reused versus newly reserved metadata
arenas. Do not remove integrity checks or choose larger default reservations from
this result alone.

Eight native interchange/attribution tests, strict qualification-library/tests
Clippy, formatting and diff checks pass. Logs live under
`target/evidence/storage-cell-growth/`. Only test and documentation source changes
were retained in this investigation. The Storage benchmark module is byte-identical
to `attempt.B7ATSm` at 11,497,579 bytes: total module and code-section deltas are
both zero. No new application-size run or product speedup is claimed. No commit,
publication, sibling edit or persisted-format change was performed.
