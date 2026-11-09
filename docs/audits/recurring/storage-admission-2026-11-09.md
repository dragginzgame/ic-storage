# Allocation admission — 2026-11-09

## Retained changes

Verification compares canonical root/allocation bytes with admitted state and
checks each bucket owner against the admitted sorted bucket vector. It no longer
reconstructs names, a name index or bucket vectors merely to discard them.
Checksums, canonical padding, physical extent, ownership and fail-closed poisoning
remain checked. Cold opening still constructs and validates the directory.

Opening and verification share bounded 2,560-byte control reads (16 allocation
rows or 160 owner rows). Partial tails read only active rows, and empty tables
perform no read. The callback is shared through a private dynamic visitor rather
than specializing the chunk loop for every consumer. No persistent cache,
reverse-owner mirror, new configuration, public API or format change is added.
This buffer size is a measured candidate, not a proven global optimum.

The starting production source is `2b333c5acedec85af51c8440c2dfce7730d2cc54`
(Storage 0.71.0). Changes are local and not published. The implementation lives in
`crates/ic-storage/src/runtime/directory_model/persistence.rs`.

## Matched installed measurements

The maintained `admission-wasm` example and native `admission` driver vary names
1/16/64/255/256/1024 independently of 0/1/32 live eight-page buckets. Buckets all
belong to the first allocation; a sentinel is checked after every measured call.
Native tests separately exercise interleaved owners. Each measurement runs in an
update execution, three samples per case. Values below are medians of inner
`performance_counter(0)` deltas, not complete endpoint costs or throughput.

`one` opens/drops the first handle and reads its logical size. `all` opens/drops
every handle and sums sizes. Keys already exist in heap; key construction and
policy bootstrap are outside the timers. `reopen` times constructing a new owner
from existing backing, with old-owner destruction before the timer and policy
bootstrap afterward. ICSS 0.7.2 uses eight-page buckets and has no named grants,
exclusive lease admission or whole-directory validation on `get`. Its 255-memory
limit excludes the larger cases; those are not simulated by multiple managers.

| Names / buckets / operation | Before | Verify without reconstruction | Final bounded reads | ICSS 0.7.2 |
| --- | ---: | ---: | ---: | ---: |
| 1 / 0 / one | 255,696 | 257,427 | 260,193 | 160,408 |
| 1 / 32 / one | 354,408 | 352,691 | 279,656 | 160,408 |
| 64 / 32 / all | 65,090,832 | 41,165,886 | 40,914,302 | 172,342 |
| 255 / 32 / all | 1,170,323,644 | 627,468,497 | 618,167,627 | 208,441 |
| 1,024 / 32 / all | 21,497,169,780 | 10,018,034,732 | 9,846,462,508 | Unsupported |
| 1 / 32 / reopen | 184,608 | 184,132 | 111,155 | 808,122 |
| 1,024 / 32 / reopen | 19,944,834 | 19,791,847 | 19,583,330 | Unsupported |

The intermediate change improves 50/54 cases and regresses four; the final change
improves 47/54 and regresses seven. The largest final regression is one name/one
bucket reopening: 93,272 → 98,821 (+5.95%, 5,549 instructions). One-name opening
adds 4,497–6,709 instructions for zero/one bucket. Sixteen names/one bucket
reopening adds 1,187 (+0.43%). Other tested cases improve. Retention is justified
by large-admission savings and bounded size cost, not universal superiority.
The final 1,024-name/32-bucket all-handle case improves 54.20%.

Verification still scans all active metadata for each lease. Opening N leases
therefore remains quadratic in names; this change does not remove that admission
contract. ICSS's much cheaper handle acquisition is not security-equivalent.
Cold opening remains dominated by constructing/indexing names at high counts.
Instruction shifts in the intermediate unchanged cold-open algorithm illustrate
that code/heap layout changes also affect accounting.

All 162 samples per Storage variant and 108 ICSS samples passed logical-size and
sentinel checks. [The compact matrix](storage-admission-2026-11-09.csv) retains
all cases, medians and min/max. Native metering confirms reads per verify/open
are `1 + ceil(names/16) + ceil(buckets/160)` with unchanged active byte coverage.
At 1,024 names/32 buckets, one verification goes from 1,057 to 66 backing reads.
This read-count formula is not a conversion into instructions.

## Wasm budget

Rust/Cargo 1.99.1, `wasm32-unknown-unknown`, independent workspace `wasm-size`
profile: opt-level z, fat LTO, one codegen unit, abort, stripped symbols. No
post-link optimizer or custom RUSTFLAGS. PocketIC 16.1.0 via Testkit 0.79.0.
Code sizes below are section-10 payload bytes decoded from the same final Wasm
module's section headers. Total bytes include all sections; no compressed sizes.

| Module | Before total / code bytes | After total / code bytes | Delta total / code |
| --- | ---: | ---: | ---: |
| Admission harness, verify-only intermediate | 89,651 / 77,198 | 88,550 / 76,084 | −1,101 / −1,114 |
| Admission harness, final | 89,651 / 77,198 | 89,982 / 77,239 | +331 / +41 |
| Small Cell application | 90,537 / 77,854 | 90,973 / 77,975 | +436 / +121 |
| Small Vector application | 93,523 / 80,378 | 93,738 / 80,312 | +215 / −66 |
| All-collection application | 133,445 / 115,364 | 133,654 / 115,298 | +209 / −66 |
| Empty application control | 16,950 / 14,818 | 16,950 / 14,818 | 0 / 0 |

The ICSS admission harness is 60,587 total / 23,944 code bytes; these are harness
sizes, not minimal apps. The ICSS small all-collection application remains 119,326
total bytes. Storage is still larger. All eight consumer pairs passed matched
operations and two upgrades each in the existing size qualification. This proves
current-image reopening, not compatibility with retired whole-store formats.

## Follow-up: cold directory reconstruction

The follow-up starts from the qualified bounded-read implementation above.
It retains two small changes:

1. Reserve the validated live allocation count before decoding rows. The root
   bounds and physical-extent checks still run before this allocation. Reopening
   no longer repeatedly grows and copies the entries vector. Bucket vectors keep
   their existing incremental reconstruction, avoiding allocation from unverified
   logical page counts.
2. Classify non-ASCII, uppercase, whitespace and forbidden separator bytes in one
   pass in `key::validate`. The existing error precedence remains explicit after
   the scan; suffix and segment validation are unchanged. Parsing, deserialization,
   accepted keys and error contents remain unchanged. Tests cover combinations of
   invalid classes in both orders, in addition to the existing grammar/codec tests.

All figures below use 32 buckets; ICSS is the unchanged published 0.7.2 control.
The same 54-case admission matrix runs at each step with three samples per case.

| Cold reopen, names | Previous qualified | Exact capacity | Both changes | ICSS |
| --- | ---: | ---: | ---: | ---: |
| 1 | 111,155 | 111,367 | 111,156 | 808,122 |
| 16 | 288,467 | 286,872 | 283,328 | 808,122 |
| 64 | 983,218 | 978,980 | 963,964 | 808,122 |
| 255 | 4,243,796 | 4,233,170 | 4,171,110 | 808,122 |
| 1,024 | 19,583,330 | 19,551,333 | 19,298,345 | Unsupported |

Exact capacity improves 17/18 reopen cases, with one adding 212 instructions.
Both changes improve the same 17 cases by 0.33–1.99%; the remaining case adds
one instruction. Every one/all-lease measurement is unchanged from the previous
qualified source. All 324 additional samples pass contents/size checks. No
throughput claim or ICSS parity follows: high-population named reconstruction
still costs substantially more than ICSS's fixed-ID manager load.

The harness changes from 89,982 total / 77,239 code bytes to 89,998 / 77,255
for exact capacity alone, then 89,871 / 77,124 with both changes. The final
harness saves 111 total / 115 code bytes versus the previous qualified source.

| Small application | Previous total / code | Both changes total / code | Delta total / code |
| --- | ---: | ---: | ---: |
| Cell | 90,973 / 77,975 | 90,860 / 77,862 | −113 / −113 |
| Vector | 93,738 / 80,312 | 93,625 / 80,199 | −113 / −113 |
| All collections | 133,654 / 115,298 | 133,541 / 115,185 | −113 / −113 |

Five key tests, 21 directory/lease tests, strict core Clippy and formatting pass.
All eight matched consumer pairs again qualify operations and two upgrades each.
Raw follow-up evidence is `target/evidence/storage-reopen/` (`candidate` for exact
capacity, `key` for both), with final size qualification in
`storage-consumer-size/attempt.di1Yyb`. The CSV adds `exact-capacity` and
`single-pass` variants; earlier rows remain evidence for their own stages.
The previous release, dependency selection, benchmark harness, compiler and
profile remain unchanged. Both changes join the same pending 0.71.1 notes.

## Validation and reproduction

- 21 directory/lease tests and five public runtime integration tests passed.
- Corruption checks now cover live verification as well as cold admission, with
  no writes and poisoned-owner behavior. Additional cases cover valid-checksum
  name/size changes, swapped owners preserving counts, unexplained growth,
  interleaved ownership and the final partial owner chunk.
- Native read counts and exact byte totals cover empty and partial/full chunks.
- Strict core library/test Clippy, focused benchmark driver/Wasm Clippy, formatting
  and diff checks pass. Full release gates and broader collection instruction
  matrices were not run; collection algorithms and data IO are unchanged.

Build from the repository root with prepared locked caches and the selected
compiler (the example is compiled separately for each backend):

```sh
cargo build --locked --offline --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --example admission-wasm --features owned \
  --target wasm32-unknown-unknown --profile wasm-size --target-dir target/admission-build
# Preserve this Wasm before building the other backend at the same output path.
# Repeat with --features dfinity for the published ICSS control.
cargo build --locked --offline --manifest-path testing/runtime-qualification/Cargo.toml \
  -p ic-storage-benchmarks --example admission --target-dir target/runtime-qualification
```

The Wasm is `target/admission-build/wasm32-unknown-unknown/wasm-size/examples/admission_wasm.wasm`.
Using the explicitly prepared Testkit CLI, run each saved module:

```sh
"$cli" run --directory "$PWD/.tools/ic-testkit-server" --ttl 900 -- \
  "$PWD/target/runtime-qualification/debug/examples/admission" \
  --wasm "$PWD/path/to/saved.wasm" --output "$PWD/path/to/results.csv"
# Add --icss for its 255-memory limit.
```

`make benchmark-storage-size` reproduces the matched application measurements
and installed operations/upgrades. The new targets reuse the existing independent
benchmark package and dependency graph; no product dependencies were added.

Raw artifacts/logs live in `target/evidence/storage-admission/`: `matched-before`, `matched-verify`,
`qualified` and `matched-icss` Wasm with their `matrix-*` CSV files, isolated production source snapshots,
compiler/source hashes, native checks and failed attempts. All four final runs
use the same finalized harness sources; their counters also match the earlier
runs exactly. Early `batched` and
`final` artifacts precede the final checked conversions; only `qualified` records
qualify retained source. Size baselines are `storage-consumer-size/attempt.K69fx4`;
final qualification is `attempt.01meOW`. Regenerable raw files are not the only
copy of benchmark code or conclusions. No dependency selections changed.

## Follow-up: compact name index

Replace the cloned-key `BTreeMap<StableKey, AllocationId>` with a sorted vector
of entry indices. Entries remain the authoritative name owner and keep their
append-order allocation IDs. Cold admission decodes entries, sorts the indices,
and rejects adjacent duplicate names. Lookup uses binary search; binding inserts
indices without moving the entries or changing persisted bytes. The sort's
indexed-key comparator is outlined to bound its code growth. No new cache,
format, public API or migration is introduced.

The expanded maintained harness adds `append`: cold reopen, policy bootstrap
including one new name, and opening that new allocation. Names are prepared
before timing and sort before the original names; each of the three samples
adds another name. Thus this includes index insertion but is not an isolated
insertion-cost measurement. ICSS only adds a numeric handle: it has no equivalent
name/grant registration, and cases that would exceed 255 IDs are omitted.

Matched previous-source and candidate builds use the same expanded harness.
All 216 samples for each Storage variant and 135 ICSS samples preserve logical
sizes and the populated allocation's sentinel. The compact CSV records the new
`index-control`, `compact-index` and `index-icss` variants, including sample ranges.

| Names / buckets / operation, first sample | Previous | Compact index | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| 1 / 32 / reopen | 111,187 | 111,094 | 808,122 |
| 255 / 32 / reopen | 4,171,110 | 2,507,958 | 808,122 |
| 1,024 / 32 / reopen | 19,298,345 | 9,985,058 | Unsupported |
| 255 / 32 / append | 15,740,383 | 14,105,142 | Exceeds ID limit |
| 1,024 / 32 / append | 67,554,618 | 58,231,146 | Unsupported |
| 1,024 / 32 / one | 10,246,151 | 10,329,314 | Unsupported |
| 1,024 / 32 / all | 9,846,462,508 | 9,846,460,987 | Unsupported |

Cold reopen improves in every measured case, by up to 48.30%. Append improves
45/54 samples; its worst regression is 0.66%. One-lease opening regresses
0.02–0.81%; all-lease costs change by −0.011% to +0.128%. The full integrity
scan per lease remains; this does not solve quadratic all-lease admission.
Index insertion shifts O(N) indices, replacing logarithmic tree insertion.
The measured append path supports this tradeoff for tested populations, not an
unbounded claim about arbitrary registration histories.

| Small application, total bytes | Previous | Compact index | ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Map | 118,621 | 121,315 | 90,212 |
| Set | 118,065 | 120,759 | 90,012 |
| Vector | 93,625 | 96,463 | 64,564 |
| Cell | 90,860 | 93,694 | 63,143 |
| Region | 119,842 | 122,545 | 89,937 |
| Map + region | 122,465 | 125,181 | 110,862 |
| All | 133,541 | 136,284 | 119,326 |

The all-collection code-section payload grows from 115,185 to 117,787 bytes
(+2,602); ICSS remains 77,127. Total module growth also includes data/framing.
The empty controls remain unchanged. This is explicitly a speed–size tradeoff:
2,694–2,838 additional application bytes, within the per-change budget. It does
not achieve the separate shared-Wasm-size reduction objective. All eight pairs
pass 118 calls and two upgrades per backend. Native coverage includes reverse
incremental binding, ID stability across reopen and a checksummed duplicate-name
image rejected without writes; 22 directory/lease tests pass.

Raw evidence: `target/evidence/storage-next/` and small-consumer qualification
`target/evidence/storage-consumer-size/attempt.IgJAUJ`. Named-module attribution
files are diagnostic builds; their totals include names and must not be quoted
as application sizes. Attribution identifies directory opening and sorting as
shared costs, but does not justify removing governance or integrity checks.

### Shared Wasm attribution boundary

The pre-index named all-collection builds have code-section payload sizes of
115,185 bytes (Storage) and 77,127 (ICSS), matching the respective stripped
build lengths, but the sections are not byte-identical. Treat function-level
attribution as diagnostic rather than a byte-exact partition of the released
module. `State::open`, including inlined admission/setup, accounts for 12,804
shallow bytes in Storage versus 3,478 in ICSS. Sorting/indexing and governance
contribute shared costs, whereas ordinary collections are not the only source
of the gap. These sizes are not additive savings estimates. Removing custody,
name validation or grant admission to match an ungoverned numeric-ID manager
would remove required functionality and is rejected. No separate Wasm-size
reduction has been qualified in this pass.
