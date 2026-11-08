# Shared B-tree prefix reader — 2026-11-08

## Decision and implementation

Retain one 15-byte read for the existing seven-byte BTN2 header and eight-byte
overflow pointer. `Node::load` reads the current layout; its
`read_header_and_overflows` helper validates magic/version before following links.
The helper is outlined and parameterized only by Memory, not by key type.
This shares validation and traversal across tree specializations without new
traits, directories or runtime registries. See
[loader](../../../crates/ic-storage/src/engine/btreemap/node/layout.rs) and
[regression tests](../../../crates/ic-storage/src/engine/btreemap/node/layout/tests.rs).

DFINITY ic-stable-structures 0.7.2 reads the header and overflow pointer separately.
Storage's prior adjacent-prefix trial was rejected for roughly 19 KB of broad
Wasm growth. This trial shares the validation as well as the read across key
types. This is a matched measurement of the combined change, not independent
attribution of IO reduction versus outlining.

Public APIs, node bytes, ownership, lifetime checks and corruption traps remain.
No migration, cache or persisted format is introduced. A malformed/truncated
prefix still traps; exact panic ordering is not an API guarantee.

## Matched qualification

Baseline product source: `4c1ae8be4111ea2edf25453614108cb2b63a0c04`.
Rust 1.99.1, PocketIC 16.1.0 through Platform's owned client 0.19.0; published
ICSS 0.7.2 with eight-page buckets. Both sides use identical logical requests.
The independent qualification lockfile is unchanged. The incoming root lockfile
updates are preserved, not adopted as part of this experiment.

Reproduce with prepared tools/caches:

```sh
BENCH_WORKLOAD=depth BENCH_SAMPLES=1 make benchmark-storage
make benchmark-storage-size
cargo test -p ic-storage --lib --locked --offline
cargo clippy -p ic-storage --lib --tests --locked --offline -- -D warnings
cargo test --manifest-path testing/runtime-qualification/Cargo.toml -p ic-storage-benchmarks --features owned,dfinity --locked --offline --lib icss_baseline --target-dir target/runtime-qualification
make test-runtime
```

Depth covers populations 256/4096, uniform/hot-16 access, batches 1/32,
three stateful rounds, all five collection forms, u64 and map byte values.
It is not a rerun of every type in the 73-type catalogue. The full-catalogue
harness is compiled, but runtime qualification here uses the depth preset.
Comparison profile: opt-level 3, fat LTO, one codegen unit, abort, no optimizer
pass, 8 MiB synthetic stack. These are measured inner-loop instructions, **not
whole-handler costs** or production throughput. One run per matched setup gives
no general statistical bound across compiler or workload changes.

720 cells: 552 improve, 168 unchanged, zero regress. All 4,320 raw records
match after excluding timed instructions, including requests/results/pages.
The ICSS Wasm is byte-identical and reference instructions are unchanged.

| Collection | Cells | Improved | Unchanged | Largest reduction |
| --- | ---: | ---: | ---: | ---: |
| Map | 416 | 400 | 16 | 2.53% |
| Set | 88 | 80 | 8 | 2.49% |
| Vector | 104 | 0 | 104 | 0% |
| Cell | 32 | 0 | 32 | 0% |
| Map regions | 80 | 72 | 8 | 2.54% |

Selected 4,096-row uniform batches of 32 (instructions per measured batch):

| Case / operation | Storage before | Storage after | ICSS |
| --- | ---: | ---: | ---: |
| Map-U64-U64 / Get | 866,002 | 851,362 | 914,502 |
| Map-U64-U64 / Replace | 1,595,718 | 1,581,078 | 1,596,676 |
| Map-U64-Bytes / Get | 1,324,693 | 1,310,053 | 1,387,383 |
| Map-U64-Bytes / Replace | 2,528,189 | 2,513,549 | 2,554,635 |
| Set-U64-Unit / Get | 719,848 | 705,088 | 826,477 |
| Vector-U64-U64 / Get | 94,903 | 94,903 | 102,932 |
| Vector-U64-U64 / Set | 182,263 | 182,263 | 180,724 |
| Region-U64-U64 / Get | 952,701 | 934,497 | 933,384 |
| Region-U64-U64 / Replace | 1,745,074 | 1,726,870 | 1,621,508 |

Region operations and Vector writes can still cost more than ICSS; this result
is not universal parity. Governance remains in Storage's measured path.

## Wasm bytes

| Small-consumer selection | Storage before | Storage after | ICSS | Storage delta |
| --- | ---: | ---: | ---: | ---: |
| Base | 16,998 | 16,998 | 16,994 | 0 |
| Map | 133,400 | 132,843 | 90,264 | -557 |
| Set | 132,844 | 132,287 | 90,064 | -557 |
| Vector | 108,366 | 108,366 | 64,616 | 0 |
| Cell | 105,255 | 105,255 | 63,195 | 0 |
| Region | 134,461 | 133,990 | 89,989 | -471 |
| Map + Region | 136,955 | 136,564 | 110,914 | -391 |
| All five | 147,911 | 147,952 | 119,378 | +41 |

Broad benchmark harness: Storage 11,532,762 → 11,516,484 bytes (-16,278);
ICSS 11,091,029 unchanged. Harness size is not application size. The separate
small-consumer crates use the wasm-size profile and are behavior-checked after
installation. Both comparisons meet the per-change size budget, but Storage's
small-consumer Wasm remains larger than ICSS.

Native tests: 91 passed, one manual test ignored. Strict library/test Clippy
passed; seven ICSS image-interchange tests passed. New tests verify unaligned
prefixes crossing a Wasm page, exact IO counts, multi-page overflow links and
invalid magic/version rejection before following an invalid link. Installed
original/O3/Os/Oz IO, upgrade and rollback passed. Full release validation was
not run; publication still requires it.

## Rejected alternatives

Both lease-index trials reuse an admitted lease's index instead of searching
opaque allocation IDs on a translation-cache miss. Existing bounds, custody
and lifetime checks were retained. Neither is in the retained implementation.

| Trial | Improved / equal / worse cells | Best saving | Worst regression | All-consumer size delta |
| --- | --- | ---: | ---: | ---: |
| Index with usize cache key | 476 / 124 / 120 | 0.036% | 0.402% | -369 bytes |
| Index with existing u64 cache key | 0 / 72 / 648 | 0% | 0.593% | -339 bytes |

The first regressed Region and Vector; the second lost across active cases.
Small source-level lookup savings did not justify the measured result.

## Evidence locations

Raw artifacts are reproducible target outputs, not permanent archives:

- Comparison baseline `target/evidence/storage-comparison/attempt.tbXkHB`;
  retained `attempt.Y7uUgq`; rejected `attempt.IXENbU`, `attempt.Gqpk9h`.
- Small-consumer baseline `target/evidence/storage-consumer-size/attempt.yf77aG`;
  retained `attempt.RpFarD`; rejected `attempt.QNVE0W`, `attempt.q8qLnc`.
- Native and comparison logs `target/evidence/storage-node-prefix-shared/`;
  rejected experiment logs/patch `target/evidence/storage-lease-index/`.
- Installed lifecycle `target/qualification/runtime/attempt.2pJRTh`.

No commit, release, publication or sibling changes were made for this experiment.

## Follow-through: one child-array read path

Retain removal of `CHILDREN_BATCH_READ_THRESHOLD` in `Node::load`.
Every internal node now uses the existing `read_address_vec`; leaf nodes still
have no children to read. This deletes the separate per-child read/push loop
and its threshold, a net reduction of 14 source lines plus unused imports.
NodeReader continues to split physical overflow crossings, so one logical array
read does not promise one backing read when it crosses a node page.

DFINITY ICSS 0.7.2 retains the ≤4-child threshold in `btreemap/node/v2.rs`.
Its `src/lib.rs::read_address_vec` uses an intermediate byte buffer and converts
chunks into addresses. Storage already reads directly into its address vector
and initializes its length only after successful IO. Thus the old allocation
tradeoff does not describe Storage's current implementation. This result does
not establish that removing the threshold would also benefit unmodified ICSS.

Use the same reproduction commands and depth scope above. This step's baseline
is the retained prefix reader (`attempt.Y7uUgq`), not released Storage or an
older copy of Storage labelled as upstream. New comparison `attempt.K2QfeO`:
552/720 cells improve, 168 unchanged, none regress. All 4,320 logical sample
records match after excluding timed instructions; ICSS's module is byte-identical
and its instruction results unchanged. Stable pages are unchanged.

| Collection | Improved / unchanged / regressed | Largest reduction |
| --- | --- | ---: |
| Map | 400 / 16 / 0 | 5.31% |
| Set | 80 / 8 / 0 | 5.59% |
| Vector | 0 / 104 / 0 | 0% |
| Cell | 0 / 32 / 0 | 0% |
| Region | 72 / 8 / 0 | 5.30% |

The maxima are workload-specific; selected 4,096-row uniform batches of 32
show smaller savings (inner-loop instructions, not whole-call costs):

| Case / operation | Prefix baseline | Child batching | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Map-U64-U64 / Get | 851,362 | 850,384 | 914,502 |
| Map-U64-U64 / Replace | 1,581,078 | 1,580,100 | 1,596,676 |
| Map-U64-Bytes / Get | 1,310,053 | 1,309,075 | 1,387,383 |
| Map-U64-Bytes / Replace | 2,513,549 | 2,512,571 | 2,554,635 |
| Set-U64-Unit / Get | 705,088 | 704,115 | 826,477 |
| Vector-U64-U64 / Get | 94,903 | 94,903 | 102,932 |
| Vector-U64-U64 / Set | 182,263 | 182,263 | 180,724 |
| Region-U64-U64 / Get | 934,497 | 933,524 | 933,384 |
| Region-U64-U64 / Replace | 1,726,870 | 1,725,897 | 1,621,508 |

| Small-consumer selection | Prefix baseline | Child batching | ICSS |
| --- | ---: | ---: | ---: |
| Base | 16,998 | 16,998 | 16,994 |
| Map | 132,843 | 132,652 | 90,264 |
| Set | 132,287 | 132,096 | 90,064 |
| Vector | 108,366 | 108,366 | 64,616 |
| Cell | 105,255 | 105,255 | 63,195 |
| Region | 133,990 | 133,799 | 89,989 |
| Map + Region | 136,564 | 136,373 | 110,914 |
| All five | 147,952 | 147,761 | 119,378 |

Broad harness: 11,516,484 → 11,501,921 bytes (-14,563), with ICSS unchanged
at 11,091,029. The all-collection small consumer shrinks by 191 bytes. Combined
with the prefix reader, broad Wasm shrinks 30,841 bytes and the all-collection
small consumer shrinks 150 bytes relative to the initial baseline. Do not
interpret the broad harness saving as an application saving.

Qualification: 91 native tests pass (one manual test ignored), strict Clippy,
seven image-interchange tests, all eight installed size selections and installed
original/O3/Os/Oz IO/upgrade/rollback pass. Existing packed-address tests include
small counts, unaligned full-width addresses, incomplete initialization and size
overflow rejection. No format, public API, memory limits or governance changes.
Full release validation remains pending; this is not deployed.

Evidence: comparison `target/evidence/storage-comparison/attempt.K2QfeO`;
size `target/evidence/storage-consumer-size/attempt.f0Il3W`;
lifecycle `target/qualification/runtime/attempt.CG7Urw`;
logs `target/evidence/storage-child-batch/`. Both incoming lockfiles and the
previous prefix-reader changes are preserved.

## Naming follow-through

The maintainer requested Storage-owned naming, independent of ICSS's historical
version split. Move `node/v2` to `node/layout` and expose direct `Node::new`,
`load` and `save` instead of versioned helpers and forwarding methods. Rename
Vector's `HeaderV1` to `Header`, Cell's `HEADER_V1_SIZE` to `HEADER_SIZE` and
the private map engine's `clear_new` to `clear`; migrate the governed caller.
Remove inherited 1.0 deprecation TODOs and the retired map-layout diagram.
Persisted version bytes stay unchanged; source naming is not a storage migration.
Provenance continues to name the original ICSS source files accurately.

Matched small-consumer qualification against child batching (`attempt.f0Il3W`)
uses `attempt.WhUwyM` with unchanged independent-workspace product dependencies;
the added Toko Tooling dev-dependency has no application Rust imports. All eight
installed behavior checks pass. No timing matrix was rerun for this naming pass.

| Selection | Before | After | ICSS |
| --- | ---: | ---: | ---: |
| Base | 16,998 | 16,998 | 16,994 |
| Map | 132,652 | 132,668 | 90,264 |
| Set | 132,096 | 132,112 | 90,064 |
| Vector | 108,366 | 108,366 | 64,616 |
| Cell | 105,255 | 105,255 | 63,195 |
| Region | 133,799 | 133,807 | 89,989 |
| Map + Region | 136,373 | 136,389 | 110,914 |
| All five | 147,761 | 147,770 | 119,378 |

All-consumer change is +9 bytes; the largest selection change is +16 bytes,
within the size budget. This is a clarity change, not a performance claim.
91 native tests, seven ICSS image-interchange tests, strict Clippy, formatting
and documentation-link checks pass. No new installed lifecycle run or full
release qualification is claimed. Logs: `target/evidence/storage-engine-naming/`.
