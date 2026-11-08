# Combined B-tree prefix writes — 2026-11-08

Retain the experiment against delivered source
`56bead4292b65fda269d243370635f05ff8f749e`: encode the seven-byte node header and
adjacent eight-byte first overflow pointer together. `node/layout.rs::write_header`
is shared across key types, while `Node::save` retains all other write ordering.
Delete the unused `NodeWriter::write_struct` and `write_u64` helpers. DFINITY
ic-stable-structures 0.7.2 is the independent registry comparison, not an older
Storage release. Its node save uses separate prefix writes.

Public APIs, allocation governance, persisted BTN2 bytes and overflow links are
unchanged. A regression test checks leaf/internal prefixes, null/full-width
pointers and a physical 64-KiB crossing: exactly one 15-byte write. Existing
interchange and lifecycle tests cover more than this local encoding assertion.

## Instructions

Matched depth workload: populations 256/4,096; uniform/hot16 access; batches
1/32; three stateful rounds; one benchmark sample per case. Both runs have 720
aggregate cells and 4,320 raw samples. Every non-instruction raw result matches;
ICSS instructions, stable-page counts and ICSS Wasm are unchanged. These are
inner-loop instruction counters, not whole-handler or whole-application costs
(`execution_instructions` is null). The full type catalogue is compiled into the
harness; this experiment times the depth selection, not all catalogue types.

| Collection | Improved | Equal | Regressed | Largest reduction |
| --- | ---: | ---: | ---: | ---: |
| Map | 96 | 320 | 0 | 1.665% |
| Set | 32 | 56 | 0 | 2.129% |
| Vector | 0 | 104 | 0 | 0% |
| Cell | 0 | 32 | 0 | 0% |
| Map region | 40 | 40 | 0 | 1.802% |
| Total | 168 | 552 | 0 | 2.129% |

Representative 4,096-row, uniform, batch-32 cases (u64 keys/values except Set):

| Collection / operation | Storage before | Storage after | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Map get | 850,384 | 850,384 | 914,502 |
| Map replace | 1,580,100 | 1,568,616 | 1,596,676 |
| Set reinsert | 1,265,733 | 1,254,005 | 1,451,710 |
| Vector get | 94,903 | 94,903 | 102,932 |
| Vector set | 182,263 | 182,263 | 180,724 |
| Map region replace | 1,725,897 | 1,711,813 | 1,621,508 |

The change does not establish universal parity: region replace and Vector set
remain more expensive than ICSS in these selected cases.

## Wasm bytes

Broad full-catalogue harness: 11,501,988 → 11,494,527 (**−7,461**).
ICSS remains 11,091,029. This is harness size, not application size.

Separate small consumer fixtures, installed and behavior checked:

| Selection | Storage before | Storage after | Delta | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Base | 16,998 | 16,998 | 0 | 16,994 |
| Map | 132,668 | 132,704 | +36 | 90,264 |
| Set | 132,112 | 132,148 | +36 | 90,064 |
| Vector | 108,366 | 108,366 | 0 | 64,616 |
| Cell | 105,255 | 105,255 | 0 | 63,195 |
| Region | 133,807 | 133,843 | +36 | 89,989 |
| Map + region | 136,389 | 136,425 | +36 | 110,914 |
| All | 147,770 | 147,875 | +105 | 119,378 |

The small-consumer increase is within the per-change size budget. The broad
harness saving must not be presented as a saving for every application.

## Reproduction and qualification

Rust 1.99.1; PocketIC server 16.1.0 through Platform's 0.19 client; registry
ICSS 0.7.2, eight-page buckets. Broad harness uses opt-level 3, fat LTO, one
codegen unit, abort and its synthetic 8-MiB stack without a further optimizer
pass. Small consumers use their separate wasm-size profile. Run against frozen
source with prepared tools and both locked dependency caches:

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

Passed: 92 native tests (one manual test ignored), strict Clippy, seven image
interchange tests, eight installed size comparisons, and installed original,
O3, Os and Oz IO/upgrade/rollback qualification. No dependency selection changed.
This is focused qualification, not a release or a complete release-gate run.

Raw evidence remains disposable under `target/`:

- Depth baseline `target/evidence/storage-comparison/attempt.4lA6XZ`;
  candidate `attempt.otNL39` in the same parent.
- Size baseline `target/evidence/storage-consumer-size/attempt.MqU2FX`;
  candidate `attempt.xvP7ox` in the same parent.
- Lifecycle `target/qualification/runtime/attempt.2zFVNL`.
- Commands and focused-test logs `target/evidence/storage-prefix-write/`.

The next overflow-page initialization experiment is independent and has not
started. It must earn retention with its own matched results.
