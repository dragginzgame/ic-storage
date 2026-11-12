# Histogram restoration and equality comparisons

This follows the [discard-result checkpoint study](storage-discard-mutations-2026-11-12.md#complete-metrics-checkpoint-calls).
It retains one local Metrics Storage optimization: compare full-width histogram
bounds/counts directly instead of using large-array equality. The same private
slice helper serves configuration admission and unchanged-state comparison.
No codec, stored format, public API, canonical numeric validator or policy changes.

## Baseline and attribution

Both lockfiles had already advanced to Metrics 0.29.12 and Logging 0.24.17 when
this work began. Those edits are preserved. A fresh baseline uses the existing
pending discard APIs and identical current locks/compiler/profile for candidates;
it is not compared against an older supplier graph. Missing selected archives
were fetched in an explicit locked preparation step after sandbox DNS failed.

Temporary counters in the actual adapter separate envelope admission, Wire
state decoding and restoration. A second instrumented build splits restoration.
Counters use Runtime's safe `ic0` interface; its temporary direct dependency and
all production instrumentation were removed after capturing those Wasms.
The native benchmark's optional `--canister-logs NEW.jsonl` captures debug records
from an explicitly instrumented canister. It does not instrument ordinary runs.

Selected repeated-call profile, one retained row, round 1:

| Bounds | Envelope | Wire decoding | Restoration |
| ---: | ---: | ---: | ---: |
| 1 | 1,388 | 2,205 | 1,118 |
| 16 | 1,388 | 10,713 | 10,353 |
| 256 | 1,412 | 133,000 | 159,714 |

The separate 256-bound restoration profile reports array preparation 13,398,
aggregate admission 381, canonical histogram restoration 87,873, and final
configuration comparison 53,559 instructions. These are instrumented-build
attributions, not additive components of the uninstrumented endpoint result.
Counters can change optimization and placement; printing happens after the timed
stages and affects subsequent work. The two profiling builds must not be summed.

Rust 1.99.1's local core source specializes integer slice equality through
`SlicePartialEq` / `compare_bytes` (`library/core/src/slice/cmp.rs`), with `u64`
marked `BytewiseEq` (`library/core/src/cmp/bytewise.rs`). The measured full-array
comparison was expensive on this Wasm target. This does not justify replacing
array equality throughout the repository without matched measurements.

## Retained change and uninstrumented results

`crates/ic-metrics-storage/src/metric/mod.rs` owns the numeric-word helper and
uses it after canonical restoration to compare the complete expected bounds.
Error ordering is unchanged. For larger histograms, `same_state` checks every
field explicitly: aggregate, overflow count/flag, bucket flags, bounds and counts.
Tiny histograms retain their existing equality path. No hash, truncated key,
cached validation or unchecked constructor is introduced.

The bounds-only candidate established the mechanism before extending it to
unchanged-state equality. Final results use **uninstrumented** modules and the
maintained checkpoint workload: 1/16/256 bounds, 1/64 rows, three rounds, seven
operations, with a second same-execution call for the first four operations.
Each build passes 126 operation rows and 198 adapter calls with state assertions.

Selected three-round medians, 256 bounds and one retained row:

| Operation | Before first | After first | Saving | Before second | After second | Saving |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Changed save | 667,217 | 620,223 | 7.04% | 507,217 | 460,223 | 9.27% |
| Unchanged save | 661,092 | 515,898 | 21.96% | 421,092 | 275,898 | 34.48% |
| Rejected regression | 547,847 | 500,853 | 8.58% | 307,847 | 260,853 | 15.26% |
| Load | 546,638 | 499,642 | 8.60% | 306,638 | 259,642 | 15.33% |

With 64 retained rows, changed saves improve 6.75% and unchanged saves 21.53%
on the first call; second-call savings are 8.77% and 33.44%. Sixteen-bound
histograms also improve. Across 42 first-call medians, 16 improve, 16 are
unchanged and ten one-bound controls increase by only 1–4 instructions.
Removal/missing-record controls are unchanged. Stable backing stays at 26 pages.
The [complete median matrix](metrics-histogram-comparisons-2026-11-12.csv) preserves
small-case regressions; no weighted fleet or whole-Auth speedup is claimed.

| O3 benchmark module | Total bytes | Code-section bytes |
| --- | ---: | ---: |
| Fresh baseline | 240,366 | 190,837 |
| Bounds-only experiment | 240,746 | 191,217 |
| Retained bounds + state equality | 241,537 | 192,017 |

The retained change costs **1,171 total / 1,180 code bytes**. These are matched
benchmark harnesses with all three histogram shapes and assertions, not minimal
application Wasms. The unrelated discarded prefix-buffer trial remains rejected.

## Safety, reproduction and evidence

Thirty Metrics unit tests pass, including existing corruption, numeric admission,
continuity, overflow and no-write rejection coverage. New coverage checks every
bound at 0/1/16/256 sizes with low-bit and high-32-bit differences, refusal before
writes/growth and successful cold reopening. Expanded equality coverage changes
every count lane above bit 31 while holding saturated aggregate fields constant,
ensuring the shortcut cannot hide distribution changes. Native product/test and
native/Wasm benchmark Clippy pass. No full release suite or new macOS result is
claimed. The change preserves the encoded representation; no image reset is used.

Use the [checkpoint benchmark commands](../../qualification/ic-storage-benchmarks/README.md#metrics-adapter-checkpoints)
with `--features metrics-checkpoints`. For the uninstrumented baseline, restore
only the prior two equality expressions in the private metric module: whole
`self == previous` after the existing scalar shortcut, and
`value.upper_bounds() != configuration` after canonical restoration. Freeze that
artifact and source before restoring the candidate. No comparison feature belongs
in the production adapter.

Raw evidence is under `target/evidence/metrics-profile-20261112/`: `baseline.csv`,
`bounds.csv`, `words.csv`, the three corresponding Wasms, `stages.jsonl` and
`restore-stages.jsonl`. Saved baseline/candidate/instrumented module sources and
manifest/lock inputs document the experiments. The earlier `profile.csv` and
`restore-profile.csv` include instrumentation overhead and are not speedup evidence.
The final source/lock/artifact hashes bind the measured inputs. Temporary Runtime
admission is gone; the ordinary native product graph has no Runtime dependency.

Wire decoding (~133k in the profile) and canonical histogram restoration (~88k)
remain larger costs. They belong to Wire and Metrics respectively. This change
does not duplicate or weaken their validators. Further work requires a separate
measured proposal; sibling code and issue handling remain with the maintainer.
