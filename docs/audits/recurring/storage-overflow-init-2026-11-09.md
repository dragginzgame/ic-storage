# Combined overflow-page initialization — 2026-11-09

Keep one 11-byte write for the overflow marker and null next-pointer in
`NodeWriter::allocate_new_page` (`src/engine/btreemap/node/io.rs`). The predecessor
link is still published afterwards. This replaces the separate three-byte marker
and eight-byte pointer writes inherited from DFINITY ic-stable-structures 0.7.2.
No format, API, allocation geometry or failure/recovery policy changes.

The reused-page regression initializes a freed slot with nonzero bytes, then
checks its marker, cleared next-link, predecessor link, untouched payload and
four backing writes (two allocator-control writes, one prefix, one predecessor).
Existing growth/shrink/reuse and ICSS image tests remain authoritative for wider
histories; this optimization does not introduce a second overflow engine.

## Matched results

Baseline source is HEAD `07c1b1ffde28b6294efda307550f0d6f2f5ae2a1` plus the pending
Build 0.81.0/Host 0.76.1 adoption and incoming lock updates. All those inputs are
held fixed. No dependency selection is changed by this experiment.

Reference is published **DFINITY ic-stable-structures 0.7.2**, not earlier Storage.
Rust 1.99.1, qualification Wire 0.18.0, Runtime 0.20.0, PocketIC 16.1.0 through
Platform client 0.19.0 and ICSS eight-page buckets remain fixed. Root Runtime
0.20.3 is a separate existing lock selection. One sample per case; depth has
three stateful rounds. The broad harness uses opt-level 3, fat LTO, one codegen
unit, abort and the existing eight-MiB stack. Small fixtures use wasm-size.

Bulk uses eight records (Cell one), 64-KiB/1-MiB payloads, uniform access and
batch one, including growth/shrinkage. Depth covers all five collection forms,
populations 256/4,096, uniform/hot16 and batches 1/32. These are **inner-loop
instruction counts**, not complete handler or application speedups. The complete
type catalogue is compiled into the harness; only these selected workloads are
timed, not a fresh full-catalogue timing sweep.

| Collection | Bulk improved/equal/regressed | Depth improved/equal/regressed |
| --- | ---: | ---: |
| Map | 6 / 48 / 0 | 11 / 405 / 0 |
| Set | Not in bulk | 10 / 78 / 0 |
| Vector | 0 / 26 / 0 | 0 / 104 / 0 |
| Cell | 0 / 12 / 0 | 0 / 32 / 0 |
| Map region | 6 / 18 / 0 | 9 / 71 / 0 |
| Total | 12 / 104 / 0 | 30 / 690 / 0 |

Maximum bulk saving is 1.0235%; maximum depth saving is 0.0657%. All 232 bulk and
4,320 depth raw logical results match after excluding the instruction counter.
ICSS instructions, stable-page counts, and ICSS Wasm bytes remain unchanged.

Selected bulk instruction comparisons:

| Operation | Value bytes | Storage before | Storage after | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Map grow | 65,536 | 6,430,954 | 6,380,726 | 6,025,308 |
| Map insert | 1,048,576 | 72,957,246 | 72,689,187 | 70,243,125 |
| Region grow | 65,536 | 6,461,620 | 6,395,484 | 6,026,040 |
| Region insert | 1,048,576 | 74,645,658 | 74,292,729 | 70,243,902 |
| Cell replace | 65,536 | 386,412 | 386,412 | 386,511 |

This is a modest reduction in new-page overhead. Storage remains more expensive
than ICSS in the selected tree insertion/growth rows; the change does not prove
universal parity or a reduction in charged memory regions.

## Wasm bytes

Full benchmark harness: **11,495,071 → 11,495,087 (+16 bytes)**.
Separate installed small-consumer fixtures:

| Selection | Storage before | Storage after | Delta | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Base | 16,998 | 16,998 | 0 | 16,994 |
| Map | 132,704 | 133,345 | +641 | 90,264 |
| Set | 132,148 | 132,789 | +641 | 90,064 |
| Vector | 108,366 | 108,366 | 0 | 64,616 |
| Cell | 105,255 | 105,255 | 0 | 63,195 |
| Region | 133,843 | 134,484 | +641 | 89,989 |
| Map + region | 136,425 | 137,066 | +641 | 110,914 |
| All | 147,875 | 147,958 | +83 | 119,378 |

Both size measurements fit the per-change budget. Harness and application-fixture
size effects differ; do not substitute the +16-byte harness result for consumers.

## Qualification and reproduction

Passed 93 native tests (one manual test ignored), strict library/test Clippy,
seven ICSS image-interchange tests, all eight installed small-consumer comparisons
and installed original/O3/Os/Oz IO, upgrade and rollback qualification. Focused
qualification is not a full release gate or publication authorization.

With prepared caches/tools, run on each frozen source state:

```bash
BENCH_WORKLOAD=bulk BENCH_SAMPLES=1 make benchmark-storage
BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage
make benchmark-storage-size
cargo test -p ic-storage --lib --locked --offline
cargo clippy -p ic-storage --lib --tests --locked --offline -- -D warnings
cargo test --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --features owned,dfinity --locked --offline \
  --lib icss_baseline --target-dir target/runtime-qualification
make test-runtime
```

Raw evidence remains disposable under `target/`:

- `evidence/storage-comparison/attempt.roU6El` → `attempt.LsAkDs`: bulk.
- `evidence/storage-comparison/attempt.C0VA74` → `attempt.8tJpuL`: depth.
- `evidence/storage-consumer-size/attempt.suWi9d` → `attempt.mlmOvH`: size.
- `qualification/runtime/attempt.HvL6rX`: installed lifecycle.
- `evidence/storage-overflow-init/`: command and focused-check logs.

No commits, release, sibling changes or dependency upgrades belong to this trial.
