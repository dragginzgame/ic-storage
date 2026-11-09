# Vector terminal buffer ownership — 2026-11-09

## Retained change

`crates/ic-storage/src/engine/vec.rs::Iter::last` transfers its byte buffer to
`T::from_bytes` as `Cow::Owned`, rather than borrowing it immediately before
consuming the iterator. Byte-valued decoders can retain that buffer without
allocating/copying another payload. `next`/`next_back` retain reusable buffers.
No public API, persistent format, range/truncation rule, allocation geometry or
release version changes. This joins the pending Storage 0.71 batch; not deployed.

The starting point is the pending bucket backing, not the old snapshot backing.
See [its attribution](storage-bucket-backing-2026-11-09.md). This fixes terminal
buffer copying; it does not fix forward/reverse scan allocation costs.

## Matched comparison against published ICSS 0.7.2

Bulk uses eight populated slots, uniform access, batch one, 64-KiB and 1-MiB
bounded byte values. ICSS uses eight-page buckets. Inner counters measure the
collection loop. Whole-handler counters additionally include request decoding,
preparation, result conversion, response encoding/reply and cleanup, with probe
overhead. Neither is a measured application endpoint throughput figure.

| Last operation | Before | Owned buffer | ICSS |
| --- | ---: | ---: | ---: |
| 64 KiB, inner instructions | 693,227 | 467,303 | 2,818,430 |
| 1 MiB, inner instructions | 8,659,625 | 5,050,661 | 29,589,069 |
| 64 KiB, whole handler | 11,566,766 | 11,340,794 | 13,771,794 |
| 1 MiB, whole handler | 175,287,482 | 171,678,470 | 193,737,200 |

Inner savings are 225,924 (32.59%) and 3,608,964 (41.68%). Whole-handler savings
are 225,972 and 3,609,012. The other 24/26 bulk cases have identical inner AND
whole-handler counts. All 52 samples retain identical results and stable-page
usage. ICSS counters are unchanged. Much of Storage's advantage over ICSS
already existed in its terminal-path optimization; do not credit it all to this
buffer transfer.

Scalar/fixed-array controls use 256 rows, uniform access and batches 1/32:

| Last operation | Before | Owned buffer | ICSS |
| --- | ---: | ---: | ---: |
| u64, batch 1 | 81,144 | 81,149 | 181,575 |
| u64, batch 32 | 105,460 | 105,620 | 3,316,756 |
| 32-byte array, batch 1 | 81,233 | 81,245 | 184,188 |
| 32-byte array, batch 32 | 106,982 | 107,366 | 3,400,416 |

These add five instructions per u64 result and 12 per array result; maximum
relative regression is 0.359% in this matrix. The remaining 48/52 control cases
are identical. All 104 control samples retain logical results and stable pages.
Keep this small tradeoff explicit rather than adding type-specific branching.
This is not a rerun of the complete 73-type catalogue.

## Wasm and safety qualification

The broad harness shrinks **11,462,663 → 11,460,016 bytes (-2,647)**. All eight
small-consumer Storage and ICSS selections retain their prior sizes; Storage
modules are byte-identical. Vector-only remains 93,523 bytes versus ICSS 64,564;
all-collection remains 133,445 versus 119,326. These fixtures use ordinary
iteration, not `last()`, so their equality is a control. They do not establish
terminal-path application-size savings; the broad harness exercises that path.
No optimizer or compiler change was made for the experiment.

Passed 14 focused native Vector tests and strict library/test Clippy. Extend the
existing byte-decoder fixture to assert owned input for direct `last()` and after
`next()` has filled the reusable buffer. Existing tests cover empty/exhausted and
partially consumed ranges, truncation, growth, one-slot reads, reopen and codec
behavior; no duplicate fixture is retained. All eight small-consumer pairs pass
installed calls/upgrades. This local change does not alter lifecycle persistence;
the backing's earlier installed rollback qualification remains separate evidence.

The plain WAT roundtrip reproduces all 52 ordinary bulk samples exactly. The
execution-only probe retains their inner counts and logical results, and its
paired ICSS outer counts are unchanged. Probe cost is not product instruction
cost; use the uninstrumented columns for the inner loop.

## Evidence and reproduction

Evidence: `target/evidence/vector-last-owned/` holds the saved pre-change engine,
module/source identities, build logs, `bulk/vector/`, `roundtrip/vector/`,
`traced/vector/`, `small-before/`, `small-current/` and native/lint logs. Size
qualification: `target/evidence/storage-consumer-size/attempt.K69fx4`, compared
with `attempt.73dNho`. Before bulk and whole-handler references are
`target/evidence/storage-bucket-cut/execution/vector-current-{roundtrip,traced}/vector/`.

Use the [benchmark procedure](../../../qualification/ic-storage-benchmarks/README.md)
with its unchanged comparison compiler/profile and 8-MiB stack. Run the driver
with `--workload bulk --collection vector --samples 1`. For small controls use
`--rows 256 --samples 1 --patterns uniform --cases Vector-U64-U64,Vector-U64-Array32`.
The README's execution-only WAT injector and `--trace-execution-cost` provide the
outer measurement, after qualifying the plain roundtrip. Small consumer sizes
use `make benchmark-storage-size`. Focused checks:
`cargo test -p ic-storage --locked --offline --lib vector` and
`cargo clippy -p ic-storage --lib --tests --locked --offline -- -D warnings`.

No dependency changes, sibling edits, commits or publication are part of this
experiment. Existing pending work and raw evidence are preserved.
