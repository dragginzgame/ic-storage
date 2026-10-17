# Canonical-store workload investigation

The initial investigation starts from IC Storage
`654a2d706982fe7433b7ce432fe1e86592766a4e` (released 0.1.6), with the benchmark
additions in this working tree. It initially left production storage code
unchanged; the compatible optimization below follows that baseline. Format,
dependencies and release metadata remain unchanged throughout.

## IcyDB ownership and copy trace

Read-only source inspection uses committed IcyDB
`996498ad4edb5569964b68e0c0e3f5bb9df7d1c6`. Its sibling checkout contains unrelated
uncommitted codec work; that work is neither included in this evidence nor edited.
These are source observations, not a deployed IcyDB profile:

- [RawRow](https://github.com/dragginzgame/icydb/blob/996498ad4edb5569964b68e0c0e3f5bb9df7d1c6/crates/icydb-core/src/db/data/row.rs#L86)
  borrows in `to_bytes`, moves in `into_bytes`, and retains owned `from_bytes`
  input through `Cow::into_owned`. `RawJournalChunk` does the same. Removing
  `Storable` does not automatically remove a payload clone from these paths.
- [DataStore::read](https://github.com/dragginzgame/icydb/blob/996498ad4edb5569964b68e0c0e3f5bb9df7d1c6/crates/icydb-core/src/db/data/store.rs#L537)
  borrows heap/live-overlay rows and owns canonical stable reads. Tombstones
  avoid canonical reads. Canonical journal folds call stable insert/remove;
  application writes do not all immediately become canonical tree writes.
- The same store's key visitors use lazy range keys without requesting values;
  row visitors request the owned canonical value. Both support reverse order
  and early exit. IcyDB owns merging canonical/live entries and tombstones.
- [RawDataStoreKey](https://github.com/dragginzgame/icydb/blob/996498ad4edb5569964b68e0c0e3f5bb9df7d1c6/crates/icydb-core/src/db/data/key.rs#L338)
  borrows `to_bytes`, retains owned decoding input, but copies in its `Storable`
  `into_bytes`. That is not evidence of a copy on every tree request: upstream
  0.7.2 `btreemap::node::v2` serialization uses `key.to_bytes_checked()`.
  Its insertion consumes **values** with `into_bytes_checked()`. The key helper
  alone therefore does not explain the measured read gap.
- Raw index keys and index witnesses borrow serialization and move their owned
  buffers. Index order is codec-aware, not lexicographic envelope order.
  Journal keys encode their sequence/chunk integers into a fresh 12-byte buffer.
  These codec/comparator costs are not represented by our synthetic byte wrappers.

The existing synthetic `Bytes` wrapper already borrows serialization and moves
owned payload buffers. The new scans copy the requested projection directly into
one output buffer, without the old harness's intermediate per-key `Vec` clone.
This deliberately retains the older matrix unchanged as a separate experiment.

## Reproducible workload

```sh
make bench-build
cargo build -p ic-storage-bench --bin ic-storage-bench --locked
target/debug/ic-storage-bench "$POCKET_IC_BIN" \
  target/wasm32-unknown-unknown/release/ic_storage_bench.wasm workloads
```

Use an explicitly admitted PocketIC 16.x server. Omitting `workloads` still runs
the existing 23-fixture matrix. The new suite uses two profiles, two engines,
four ordered rounds and two fresh repetitions: 256 CSV measurements.

Each profile seeds 512 entries in the permutation `(i * 149 + 37) % 512`.
Keys cycle through 24/32/33/64/128/262 bytes with nonzero suffixes. Row values
cycle through 1/16/17/32/256/1,024/4,072/4,073/8,192/65,536 bytes; payload bytes
vary deterministically. Index values are one-byte witnesses. Upstream bounds
remain 262/unbounded for rows and 16,668/1 for index keys/values. These are
source-informed size and call shapes, **not production frequency estimates or
actual IcyDB encoded records**. In particular, long keys are deliberately common.

Each round performs 32 reads (24 hits/eight misses), 32 changed puts with varying
row sizes, 32 deletes, 32 refills, then four bounded scans: keys/values in each
direction, stopping after 16 entries. Mutation targets follow a deterministic
permutation and shift between rounds. Scans include the lower endpoint and
exclude the upper. Later phases inherit allocation history; results describe
this sequence, not isolated-operation averages or a workload-weighted score.

Inputs and independent heap-model expectations are prepared before timing.
Returned bytes are checked afterward. Every mutation batch also checks all 512
keys and audits our entire tree outside timing. Both engines use the same model
and inputs; repeat/result-length agreement is checked across CSVs. The measured
interval includes tree calls, result allocation and projection copying, but not
fixture generation, model checks, reply encoding or seeding. Heap/stable columns
are total capacities after the call, not allocated-byte deltas or live heap size.
No overlay, actual comparator, canister upgrade or rollback claim follows.

## Results

Instruction totals below sum four rounds from one repetition, in millions.
Both repetitions agree exactly in instructions, capacities and returned lengths.
A ratio below one favors IC Storage. Do not combine these totals into a general
IcyDB speedup: the operation mix and frequencies have not been measured there.

| Profile | Operation | Ours M | Upstream M | Ours/upstream |
| --- | --- | ---: | ---: | ---: |
| Rows | Read mix | 38.327 | 28.922 | 1.33 |
| Rows | Fold put | 75.917 | 74.831 | 1.01 |
| Rows | Fold delete | 106.847 | 86.686 | 1.23 |
| Rows | Refill | 98.608 | 72.443 | 1.36 |
| Rows | Forward keys | 2.381 | 8.432 | 0.28 |
| Rows | Reverse keys | 2.394 | 8.713 | 0.27 |
| Rows | Forward values | 12.297 | 15.043 | 0.82 |
| Rows | Reverse values | 10.004 | 14.003 | 0.71 |
| Index | Read mix | 26.095 | 12.423 | 2.10 |
| Index | Fold put | 35.953 | 25.599 | 1.40 |
| Index | Fold delete | 76.581 | 32.671 | 2.34 |
| Index | Refill | 66.178 | 29.177 | 2.27 |
| Index | Forward keys | 2.293 | 1.781 | 1.29 |
| Index | Reverse keys | 2.145 | 1.869 | 1.15 |
| Index | Forward values | 2.319 | 1.825 | 1.27 |
| Index | Reverse values | 2.173 | 1.905 | 1.14 |

After the final phase, row stable capacity is 7,274,496 bytes versus 4,587,520
upstream; index capacity is 1,507,328 versus 9,830,400. Our broad index bounds
save capacity here despite costing more instructions. Final heap capacity is
6,684,672/6,750,208 bytes for rows and 1,245,184/1,245,184 for index (ours/upstream).

## Isolated small-value prototype

An artifact-only copy under `target/layout-experiment` changes inline values
from 16 to 48 bytes and record stride from 80 to 112, retaining 31-entry nodes
and 32-byte inline keys. Both encode/decode and mutation thresholds change.
Its distinct `ICEXP481` magic prevents confusing experimental and current bytes.
All installations use fresh memory. The exact delta is retained as
`target/inline48.nJNbSC/prototype.patch`; no alternative reader or production
feature flag is introduced.

The 128-entry, 32-byte-row fixture reserves 65,536 bytes instead of 589,824;
its dense-key value scan improves 62.07%. But lookup increases 1.29% and changed
replacement 7.36%. Dense one-byte index lookup increases 21.51%. In the complete
matrix only 28/506 cases improve and 478 regress; upstream counters are unchanged.
Mixed-index operation totals regress 4.62–11.62%. Mixed-row read/put/delete/refill
regress 4.19–7.37%; reverse scans improve, while forward scans regress.

**Rejected for adoption.** More inline space does not by itself make the tree
better: larger fixed records impose costs on unrelated entries. These findings
favor investigating compact variable-sized records or shared small-payload
storage, with explicit allocation/reclamation and corruption proofs, before
committing to another persisted format. The experiment is not a production-
qualified format and does not run the old layout-specific corruption fixtures.

## Large-read cost isolation

A second artifact-only build under `target/read-diagnostic` uses distinct
`ICDIAG01` magic and bypasses only `page::read_into` checksum verification.
Bounds, node decoding and chain checks remain, as does header verification.
The exact delta is in `target/read-diagnostic-run.eTBfty/diagnostic.patch`.
This deliberately weakened build is a cost diagnostic, **not an optimization
candidate**; it must never read application data or replace integrity checking.

With dense keys, a 4 MiB lookup falls from 53,742,872 to 18,919,035 instructions
(-64.80%), versus 37,954,113 upstream. The four-row value scan falls from
270,971,689 to 131,697,434 (-51.40%), versus 200,787,292 upstream. Thus verification
and its associated control flow dominate the measured large-read gap. Eliminating
another `Storable` payload clone cannot explain this result. A faster checksum
would require a deliberate format decision plus corruption qualification; a
cache cannot simply skip validation of bytes read from stable memory.

## Evidence identity and qualification

All runs use Linux, Rust 1.99.0, the unchanged 0.1.6 lockfile, IC Testkit 0.49.0,
IC Host FS 0.23.1, IC Metrics 0.11.6, upstream 0.7.2 and the already admitted
PocketIC server 16.0.0/client 16.1.0. Each attempt retains Wasm, runner, CSVs and
logs. SHA-256 identities:

| Attempt | Wasm SHA-256 | Measurements |
| --- | --- | ---: |
| `target/canonical-baseline.B6kFdI` | `5765709c84beb0bcc1bed0e158c51dabdf34bd982e7f7ac9810e3a16b14f1d57` | 2,024 matrix |
| `target/canonical-workloads.sCRUjV` | `5765709c84beb0bcc1bed0e158c51dabdf34bd982e7f7ac9810e3a16b14f1d57` | 256 workloads |
| `target/inline48.nJNbSC` | `b5563e4dfd101bfce5367972da8c0c9d58995845ff89ecafa1274462b52e1cc0` | 2,024 matrix + 256 workloads |
| `target/read-diagnostic-run.eTBfty` | `1457a52f4baedf8b428b42f9555331c67e0c799e91fb7ea88240471a5fa542bf` | 2,024 matrix |

All 6,584 successful measurements repeat exactly within each build, and both
engines return matching lengths. Workload assertions compare actual bytes against
the model. The first baseline workload attempt failed in host tuple decoding
before printing any measurements; its error log is retained. The corrected host
uses the same multi-result Candid decoder as the original matrix runner. A
Rustdoc lint was corrected before final Clippy qualification.

Native runner and Wasm benchmark Clippy, formatting and doc links pass. Full
release validation and native macOS execution are outside this investigation.
The initial retained change was workload coverage and evidence, with production
B-tree source identical to released 0.1.6. Experimental trees are neither
published nor adopted by IcyDB.

## Borrowed single-page key comparison

The follow-up removes a real allocation/copy inside our tree: binary search now
compares overflow keys of at most 4,072 bytes directly from the verified page
buffer. It checks the page kind and terminal pointer before passing the complete
key to the caller's comparator. Transactional searches read staged pages first,
as before. Inline and multi-page keys retain their prior paths. The public API,
format, full-page checksums and mutation publication contract are unchanged.

The starting build reproduces the baseline SHA-256 above exactly, so the retained
baseline measurements are reused. The new artifact is
`target/borrow-overflow-key.Uv5PJA/benchmark.wasm`, SHA-256
`14afb81dc15a33aced885f0060a1b60e8b259ba94dad4ea471da9648b31cb027`.
That directory retains its runner, both CSV suites and logs. All 2,280 measurements
repeat exactly; upstream counters and result lengths are unchanged against the
baseline. Mixed-workload results still match the independent heap model.

Four-round instruction totals, millions:

| Workload | Before | After | Change |
| --- | ---: | ---: | ---: |
| Index reads | 26.095 | 25.559 | -2.05% |
| Index fold put | 35.953 | 35.448 | -1.41% |
| Index fold delete | 76.581 | 75.476 | -1.44% |
| Index refill | 66.178 | 64.919 | -1.90% |
| Row reads | 38.327 | 37.702 | -1.63% |
| Row reverse key scans | 2.394 | 2.131 | -10.97% |

All 16 mixed profile/operation totals improve, by 0.14–10.97%. In the older matrix,
56/506 cases improve and 450 regress; the largest regression is 0.1164%, for a
one-MiB fixture's missing lookup. Dense 33-byte index-key lookup improves 2.29%;
the deep 262-byte-key row lookup improves 2.33%. This is retained for the mixed
workload gains with a small measured cost elsewhere, not claimed as a universal
speedup. Index operations still trail upstream, and large-read checksum costs
remain unresolved. Lowering allocation also changes later allocator history, so
scan improvements cannot all be attributed solely to copying fewer key bytes.

A consumer regression test injects payload corruption, authenticated wrong page
kinds and authenticated invalid chain endings into 33/4,072/4,073-byte keys.
Reads, membership checks, scans and mutations reject the damaged tree; failed
operations preserve all backing bytes. Restoring the injected page permits a
successful reopen/read/audit. Existing semantic-comparator, staging and differential
tests exercise mutation paths and ordering.

All 23 regular storage tests pass on Rust 1.99 and 1.85; four upstream tests and
the 320,000-operation extended campaign pass on 1.99. Native library/test and
Wasm benchmark Clippy pass. Full release and macOS qualification remain outside
this continuation. No IcyDB checkout, dependency selection or version is changed.

## Rejected branch-child allocation experiments

A further continuation tested allocating branch child-pointer vectors once during
node decoding, using the validated entry count. The initial implementation grows
an empty vector while decoding. Two candidates used either exact `count + 1`
capacity or power-of-two capacity (minimum four) matching the old final vector
capacity on the measured toolchains. Leaf nodes still allocate no child vector.
No persisted layout, integrity check or public API changed in either candidate.

The incoming IC Metrics 0.11.7 lockfile change is preserved. All other dependency
selections remain at the incoming state. A new baseline avoids mixing those
inputs with earlier 0.11.6 artifacts. Artifacts retain both CSV suites, logs and
runner; each candidate also retains its experimental `page.rs`:

| Attempt | Wasm SHA-256 |
| --- | --- |
| `target/node-capacity-baseline.GitnlI` | `d04602f08a325b10df51d5a78ffb3ffea415c37c3fb921661ac9bc84a4a0d256` |
| `target/node-capacity.mIqCpR` | `6d9baf6d5eb97fc04bd13b12efecefe33c2b59d2720847e7ec50ea83f69d57a4` |
| `target/node-capacity-rounded.5sNUN1` | `0f6bb15cece8f26a8aa1ae420c9fe4d1ceb7ef7f446c0d6733f6926f6ce3aac4` |

The exact-capacity candidate improves 500/506 older matrix cases and leaves six
unchanged, with unchanged upstream instruction counts. But the mixed workload
exposes a tradeoff: index reads/puts/deletes/refills improve 0.52–0.99%, while
forward row-value scans increase from 12,209,370 to 12,813,870 instructions across
four rounds (+4.95%). One scan round accounts for most of the regression despite
unchanged heap capacity. This is consistent with allocator history affecting
result-buffer growth; capacity counters alone do not establish the mechanism.

Matching the prior final vector capacity does not fix the regression: forward
row-value scans total 12,893,916 (+5.61%), and forward/reverse row-key scans regress
3.28/3.52%. Index mutation gains narrow to 0.44–0.82%. **Both candidates are
rejected.** The broadened workload prevents treating the older matrix's nearly
universal improvement as sufficient evidence for adoption. The rounded candidate
improves all 506 older-matrix cases, with unchanged upstream instruction counts.
All 6,840 measurements across the three builds repeat exactly, and the engines
return matching result lengths; the mixed workloads also verify actual bytes.

The exact-capacity candidate passes all 23 storage tests on Rust 1.99/1.85,
four upstream comparisons, native Clippy and benchmark Wasm Clippy. Both candidates
pass the mixed-workload model assertions. After rejection, `page.rs` matches its
incoming contents byte-for-byte, the rebuilt Wasm matches the baseline hash,
and the retained library passes its locked Rust 1.85 check. No new production
change, dependency update, release or IcyDB modification is retained by this
allocation investigation. See [dependency ownership](../upstreams.md#benchmark-dependency-resolution)
for the separate ICU/time inspection.


## Governed-memory baseline with IC Memory 0.45

Both engines now use allocation-governed runtime handles, and upstream collections
are accessed through the IC Storage wrapper. This changes memory plumbing from
historical raw-memory runs; the numbers below are a new baseline, not an isolated
algorithm speedup/regression attribution. Runtime bootstrap occurs outside measured
operations. IC Memory 0.45, IC Testkit 0.50, IC Host FS 0.24, Rust 1.99 and PocketIC
server 16.0.0/client 16.1.0 are retained with the attempt lockfile.

| Mixed workload | Our instruction total | Upstream total | Ours/upstream |
| --- | ---: | ---: | ---: |
| Rows | 341,181,083 | 340,590,375 | 1.0017 |
| Index | 210,158,188 | 114,285,849 | 1.8389 |

Totals sum all eight operations across four rounds in one fresh run, excluding
seeding and bootstrap. Both fresh runs repeat every instruction, heap/stable
capacity and result-length counter exactly across 256 measurements. Paired engine
result lengths agree; the canister's heap-model checks also pass. Mixed index
performance remains an optimization target; these are synthetic codecs and do
not establish deployed IcyDB performance or exclusive allocation ownership.

Attempt `target/governed-memory045.by7rDA/` retains `runner`, `benchmark.wasm`,
`Cargo.lock`, `workloads.csv` and `workloads.log`. Wasm SHA-256:
`45d3b6319f04ef8e5d6fdbdd9195f8924a77f64da2bd0b55a1c2c3e1d8bd90d3`.
The native runner's finite Candid limits admit every boolean and counter reply.
The older full fixture matrix was not rerun for this focused adoption.


## Single-search deletion and shared installation

Pending 0.3.1 removes the preliminary membership search from deletion. The
transaction returns absence explicitly and is discarded without committing any
speculative merge/free-list updates. The regression fixture confirms one backing
read for successful single-leaf deletion (previously two), and byte-for-byte
preservation after a miss that stages a merge. The same minimum-child miss uses
six reads versus the previous two-read membership preflight: this optimization
favours successful deletion and is not a universal reduction in read traffic.

Both benchmark modes now share fresh-instance creation and use IC Testkit's
`InstallSpec`/`try_create_and_install`. No snapshot reuse, seed-budget change or
measurement-region change is introduced. The mixed-workload attempt retains
256 measurements with exact instruction/capacity/result-length repeats and
matching engine result sizes; all existing in-canister model checks pass.

| Workload | Our instructions | Upstream instructions | Ours/upstream |
| --- | ---: | ---: | ---: |
| Mixed rows | 313,148,947 | 340,590,375 | 0.9194 |
| Mixed index | 187,936,678 | 114,285,849 | 1.6444 |

Totals cover one fresh run, all four rounds and eight operations. The incoming
lockfile changes are preserved; this is current-build qualification rather than
an isolated same-lockfile before/after performance attribution. Index performance
still trails upstream. The full older fixture matrix was not rerun.

Artifacts live in `target/simplified-delete.lPIlSb/`: `runner`, `benchmark.wasm`,
`Cargo.lock`, `workloads.csv`, `workloads.log`. Wasm SHA-256:
`fa9569f630bfe826f5174cc39bc6e474cc313a699f09f0136ccfba837b45b8f3`.


## Shared search and upstream benchmark operations

The pending 0.3.2 cleanup puts binary search and the single-page borrowed-key
comparison in one private helper. Its reader callback selects committed pages
for ordinary lookup and staged-first pages for mutations. Chained-key readers
retain their existing validation. A benchmark-only generic map wrapper shares
upstream CRUD and scan loops while retaining index, journal and row codec bounds.
Bounded key scans still borrow keys and never load values; the older unbounded
fixture scan keeps its existing copy/collection behavior. Measurement windows,
operation order and independent model checks are unchanged.

Both builds use the same locked dependencies and runner. Each attempt produces
256 mixed-workload measurements with exact instruction, heap/stable capacity
and result-length repeats; paired engine result sizes and model checks pass.
Totals below cover all four rounds in one fresh run.

| Engine / workload | Before instructions | After instructions | Change |
| --- | ---: | ---: | ---: |
| Ours / mixed rows | 313,148,947 | 312,100,436 | -0.335% |
| Ours / mixed index | 187,936,678 | 186,908,830 | -0.547% |
| Upstream / mixed rows | 340,590,375 | 340,862,276 | +0.080% |
| Upstream / mixed index | 114,285,849 | 114,289,280 | +0.003% |

These are compiled-build effects of both extractions, not isolated algorithm
speedups. The complete older fixture matrix was not rerun. Fixture bootstrap
still uses the published IC Memory API; the unmerged helper is qualified in a
separate scratch consumer and is not part of either measured build.

Artifacts retain `runner`, `benchmark.wasm`, `Cargo.lock`, `workloads.csv` and
`workloads.log` in `target/search-cleanup-baseline.QUKtIN/` and
`target/search-cleanup-final.mX2Kkm/`. Wasm SHA-256 values respectively:
`d4a7c73dab02f16b7bfe4b6c49eb597d5b0e4e085abe26543fe863e256fb182a` and
`a495ef64e988f1286c1eac31e7ac4e9d19ea87644817a219bb22db7651e5baff`.


## Published IC Memory 0.46 leases

The current build consumes published IC Memory 0.46 with exclusive allocation
leases and narrowed host-memory exports. The fixture helper remains unmerged
and is not included in this measured build. Bootstrap and lease acquisition
occur outside measurement windows. Both engine wrappers retain their allocations
throughout each installation, so every operation uses the same live lease.

Attempt `target/lease046.Ff4fA5/` passes 256 measurements with exact repeated
instruction, heap/stable-capacity and result-length counters. Paired engine
result lengths agree and all in-canister model checks pass. Per-run totals:

| Mixed workload | Our instructions | Upstream instructions | Ours/upstream |
| --- | ---: | ---: | ---: |
| Rows | 311,848,476 | 341,486,251 | 0.9132 |
| Index | 186,930,855 | 114,279,668 | 1.6357 |

The attempt retains runner, Wasm, lockfile, CSV and log. Wasm SHA-256:
`feb8e3fe5d6f04f16e28434c54d95958a3c126b61075e5be83fc75360aaa3bb0`.
These counts qualify the selected dependency and ownership contract; they do
not isolate lease overhead from compiled-build changes. The full older fixture
matrix and deployed IcyDB remain outside this focused qualification.


## Single input validation, shared overflow reading and audit isolation

The next 0.4.0 cleanup validates keys at operation boundaries, so insertion no
longer repeats comparator self-validation in its search traversal. Key validation
still precedes value-limit refusal. A counting-comparator regression proves one
validation for insert/replace/get/contains and unchanged bytes after invalid
input. It passes on Rust 1.91. Lexicographic self-comparison can already be
optimized away; this benchmark does not quantify a real IcyDB codec's saving.

Committed payload reads and transactional key reads share one private chain
reader, with the transaction callback retaining staged-page visibility. Bounds,
checksums, page type, payload length and terminal-pointer checks are preserved.
Read-only auditing moves to `btree/audit.rs`; one audit object owns visitation,
previous-key, count and leaf-depth state. Recursive arguments and the
`too_many_arguments` suppression are removed. Formats and mutation publication
remain unchanged.

A freshly rebuilt baseline includes the incoming 0.46 lease adoption and all
prior dirty work before these three edits. Both attempts use identical lockfiles
and runner binaries, Rust 1.99 and PocketIC server 16.0.0/client 16.1.0. Each
build passes 2,024 fixture-matrix and 256 mixed-workload measurements, producing
4,560 measurements across both builds. Every instruction/capacity/result counter
repeats exactly across two fresh runs. Paired engine result lengths agree;
heap/stable capacities and results are unchanged between builds.

| Engine / mixed workload | Before instructions | After instructions | Change |
| --- | ---: | ---: | ---: |
| Ours / rows | 311,848,476 | 311,889,842 | +0.0133% |
| Ours / index | 186,930,855 | 186,950,150 | +0.0103% |
| Upstream / rows | 341,486,251 | 341,480,271 | -0.0018% |
| Upstream / index | 114,279,668 | 114,273,688 | -0.0052% |

In our 506 fixture comparisons, 88 improve, 12 remain unchanged and 406 increase.
The largest improvement is 0.0624% for `index_128/refill`; the largest increase
is 0.5139% for `index_full_root_dense/scan_values`. Upstream improves in 368
cases and is unchanged in 138, by at most 0.0198%. Our count beating upstream
remains 381/506. These maintainability changes carry small measured instruction
regressions; they are not presented as an overall performance improvement.

`target/three-cleanups-baseline.a2WJ30/` and
`target/three-cleanups-final.buUycf/` retain runner, Wasm, lockfile,
`workloads.csv`/`.log` and `matrix.csv`/`.log`. The final directory additionally
retains `comparison.csv` and `summary.json`. Wasm SHA-256, baseline then final:
`feb8e3fe5d6f04f16e28434c54d95958a3c126b61075e5be83fc75360aaa3bb0` and
`1da0559698c5c4ba41691dcaa2f7ccee0538b4232db8bd4b87f63e3d32c76e0e`.


## Shared benchmark protocol and inline-record ownership

The following 0.4.0 cleanup gives the runner and canister one operation/seed
protocol in `protocol.rs`. Explicit u8 discriminants, CSV labels, operation
order, seed limits and workload constants retain their existing values. Enums
are converted to/from u8 at the boundary; Candid argument and reply shapes do
not change. The host protocol test freezes the old IDs/order/schedule and
rejects invalid IDs. Constants used on only one target are scoped accordingly.

The page codec now owns 32/16-byte inline capacities, record stride and the
zero-fill/copy helpers used by insertion and replacement. Independent expected
leaf and branch pages verify constructor/replacement bytes, child offsets and
decoding at inline boundaries, including overflow-to-inline transitions. The
existing decoder still independently rejects inconsistent pointers/lengths.
The adapter also removes five unsupported error aliases; LogWriteError remains.

Native and Wasm Clippy pass after correcting enum imports and branch style.
All 35 regular library tests, three integration
tests, seven compile-fail examples and the valid Rustdoc example pass. The byte
compatibility regression passes on Rust 1.91, and the 320,000-operation extended
differential campaign passes against upstream and the independent heap model.

Attempt `target/protocol-layout.jBkgAK/` retains runner, Wasm, lockfile, CSV/logs,
comparison CSV and summary JSON. The new runner's 256-row mixed-workload CSV
against the prior canister is byte-identical to that canister's retained CSV.
Against the new canister, both the 2,024-row matrix and 256-row mixed suite pass,
with exact repeats, matching model results and unchanged heap/stable capacities.
The comparison reuses the previous final artifact
`target/three-cleanups-final.buUycf/` and its unchanged lockfile. This is combined
protocol/layout qualification, not an isolated storage-algorithm speed claim.

| Our mixed workload | Before instructions | After instructions | Change |
| --- | ---: | ---: | ---: |
| Rows | 311,889,842 | 311,915,326 | +0.0082% |
| Index | 186,950,150 | 186,974,934 | +0.0133% |

The fixture matrix has 230 unchanged and 276 increased instruction cases, with
a maximum increase of 0.0586%; none improve. The upstream instruction counts
are identical in both suites. Our count beating upstream remains 381/506.
Final Wasm SHA-256:
`735ab8df864e3f1219bd94741b2ea9d6d4f1c78f9e47721eb176be9c3bb48e79`.

## Single-pass key audit and shared cursor descent

The next 0.4.0 cleanup materializes overflow keys through the shared chain reader
while recording page ownership, replacing the prior audit-then-reread sequence.
Values remain audited without materialization. A traffic regression covers
0/32/33/4072/4073/16668-byte keys, live values and freed chains: every allocated
non-header page is read once, with no writes. It fails on the old audit at a
33-byte key (11 reads rather than 10). Authenticated shared key/value pages,
wrong page kinds, bad checksums and unterminated chains remain rejected.

Unbounded seeks delegate to existing cursor descent. Bounded seeks retain their
comparator and inclusion semantics. A cyclic authenticated branch reaches the
64-frame limit in either direction and then fuses without further reads. All
38 regular storage tests, three public adapter tests and the host protocol test
pass; the three new audit/cursor regressions pass on Rust 1.91. Native and Wasm
Clippy pass. No public API or persisted bytes change.

The native runner hashes its loaded Wasm buffer using IC Host Artifacts 0.24,
retaining the previous identity log shape. The direct dependency is native-only
and reuses an already selected version. Cargo.lock changes only the benchmark's
direct dependency list. No library or Wasm dependency is added.

Attempt `target/audit-cursor.u35jl0oh/` retains runner, lockfile, combined and
isolated Wasm builds, CSV/logs and comparison summaries. A fresh build with the
old audit and cursor reproduces the previous Wasm SHA-256 exactly, so the
baseline reuses `target/protocol-layout.jBkgAK/workloads.csv`. All three new
builds complete 256 measurements each, with exact repeats, matching model
results and unchanged heap/stable capacities and result sizes against baseline.
They use Rust 1.99 and PocketIC server 16.0.0/client 16.1.0 on Linux.

| Engine / mixed workload | Baseline | Cursor only | Audit only | Combined |
| --- | ---: | ---: | ---: | ---: |
| Ours / rows | 311,915,326 | 311,915,486 | 314,067,175 | 314,067,335 |
| Ours / index | 186,974,934 | 186,975,094 | 186,990,451 | 186,990,611 |
| Upstream / rows | 341,480,271 | 341,480,271 | 341,486,267 | 341,486,267 |
| Upstream / index | 114,273,688 | 114,273,688 | 114,279,684 | 114,279,684 |

Combined totals increase 0.6899% for our rows and 0.0084% for our indexes.
The largest individual increase is 33.7659% for row `keys_reverse_2`.
Cursor-only totals add 160 instructions to each of our workloads. The audit-only
build reproduces almost all of the combined increase, despite auditing running
outside measurement windows. These runs isolate the source change, not the
underlying allocator/compiler cause. The single-pass I/O improvement is retained
with this measured tradeoff; it is not an overall instruction speedup.
The fixture matrix, extended differential campaign and full release gate are
not rerun for this batch. An initial sandbox attempt failed to bind the local
server; its failure CSV/log are retained separately from the successful run.
Combined Wasm SHA-256:
`634e2a1156d5bc787d9ac9188ee4220cab5a68f769bf9f0a10d24a6139038af1`.
