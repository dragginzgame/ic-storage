# Independent Vector behavioral specification

Status: restricted implementation handoff; qualification belongs to the parent
reviewer. The maintainer authorized fresh/reset backing and explicit rejection
of incompatible images. No automatic initialization or compatibility reader.

## Information boundary

This specification describes public behavior and resource requirements. Its
author has seen the previous source and does not implement the replacement.
The separate implementer receives this document, reviewed interface declarations
and new public acceptance tests only, alongside repository policy/manifests.
Do not inspect previous source, internal tests, layouts, diffs, source history,
registry ICSS implementation or sibling code. Record actual inputs read and
independently choose the new format and algorithm. ICSS remains external
comparison tooling, never production source or a production dependency.

## Public contract

`StableVec<T: Storable, M: Memory = DefaultMemoryImpl>` owns one exclusive,
already admitted `RuntimeMemory<M>` lease. The host owns declarations, grants,
allocation commitment and backing custody. Vector must not allocate identities
or create a separate manager.

| Operation | Required behavior |
| --- | --- |
| `create(memory) -> Result<Self>` | Empty allocation only; refuse nonempty memory without writes. Initialize an empty vector. |
| `reset(memory) -> Result<Self>` | Explicitly replace the image with an empty vector. No obligation to erase unreachable payload bytes. |
| `load(memory) -> Result<Self>` | Read-only admission of the new format. Refuse empty/foreign/unsupported framing without resetting it. Reject incompatible element bounds before interpreting records. |
| `get(&self, index: u64) -> Option<T>` | Decode the selected live record, or return None without codec invocation when out of range. |
| `set(&self, index: u64, item: &T)` | Replace an existing record without changing length; invalid index traps. |
| `push(&self, item: &T)` | Append one record. A successful call makes it visible at the previous length. |
| `pop(&self) -> Option<T>` | Remove and return the final live record; empty returns None without decoding. |
| `len(&self) -> u64`, `is_empty(&self) -> bool` | Observe the current committed length, including mutations through the same owner. |
| `iter(&self) -> impl DoubleEndedIterator<Item = T> + '_` | Ordered values, forward/backward mixed consumption, direct skipping and correct exhaustion without duplicates. |
| Drop | Release the lease without rewriting or erasing backing. |

Unbounded element codecs are unsupported and must trap on construction/admission.
Finite fixed-width and variable-width codecs are supported, including zero-width
values and records that cross a Wasm page boundary. Encode/decode failures and
declared-bound violations trap. Enforce bounds independently of overridable codec
convenience helpers. Constructors retain typed framing/reservation refusals.
No second successful-call Result interface is added.

Reads and mutation arithmetic must admit complete ranges before allocation or IO.
Stored lengths must not cause an unbounded allocation or traversal. A malformed
variable record must be bounded before allocating its decode buffer. Do not
validate or decode unrelated records on indexed access, iterator skipping,
`count`, `nth`, `nth_back` or `last`. Skipped malformed records need not trap;
an actually selected malformed record must trap.

Iteration must remain memory-safe if methods taking `&self` mutate the vector
while an iterator exists. Appends need not extend an existing iterator. Document
the selected behavior when records are removed during iteration; do not read
outside the current admitted extent. No mutation epoch, separate journal or
automatic scan is required. Stable-state iteration is the comparison baseline.

## Format, failure and cost requirements

Choose fresh framing with explicit version and type-bound admission. Reject old
images before interpreting their records. Keep a single authoritative format
implementation and delete the previous production engine at cutover. No aliases,
fallback engines or format migration layer.

IC execution rollback supplies transaction rollback. Native mutation unwinding
must invalidate the owner through the existing lease guard; subsequent backing
access and new leases refuse. Read-only admission failure releases its lease
without modifying bytes. All normal operations must observe poisoned ownership,
including operations which otherwise need no physical IO.

Target predictable indexed access and bounded allocation work: fetching the
first/middle/last record must not scan the population or decode neighbors. Keep
common scalar reads inexpensive, and avoid repeated allocations/copies where the
new design can do so simply. Geometry, buffering and caching choices are the
implementer's decisions, not requirements to reproduce previous internals.
No universal instruction or size improvement is assumed before measurement.

## Qualification owned by the integration reviewer

Use public model tests for mixed updates, reopen, record widths, fixed/variable
bounds, empty values, out-of-range operations and bidirectional iteration.
Count actual codec calls and backing IO independently of private layouts.
Inject read/write/growth failures and check lease/custody behavior. Exercise
arithmetic and corrupt framing/record boundaries against the new design.

Compare all supported catalogue types with external ICSS; preserve unsupported
type admission checks. Include populated single-record reads and cold opens,
mutation, iteration and large records. Retain baseline artifacts and distinguish
instruction, page/IO and application-Wasm outcomes. Installed qualification must
cover new-format upgrades, IC rollback and rejection of old images without
destroying the old application. No deployment is part of this implementation.

## Implementer input record and selected design

Only the original behavioral sections above were supplied as the design handoff.
This input record and subsequent qualification sections were added afterwards
by the source-exposed integrator; the implementer did not reread those additions.

The separate implementer read only AGENTS.md, TOKO-TOOLING.md, both product
manifests, this specification, rules/contributions.md, applicable sections of
rules/engineering-details.md and docs/principles/rust-code-hygiene.md, plus the
parent's restricted interface declarations. It did not inspect previous source,
internal tests, history, registry implementation or sibling repositories.
Parent integration only removed retired code, added public acceptance/refusal
checks and applied mechanical formatting/documentation fixes.

The new adapter owns STORVECT version 1: a 32-byte header with finite bound,
fixed/variable kind, reserved zeros and u64 committed length. Fixed records use
their declared width; variable records add a uniform four-byte length. Admission
checks the full occupied extent without decoding records. Length is cached under
exclusive custody. Reads use a 64-byte stack buffer for small selected values
and an owned buffer otherwise. Direct iterator skips/count avoid codec calls.
Iteration documents its behavior under same-owner removal/replacement.

The former engine/vec.rs, its old-layout tests and unused Vector-only codec
helpers are removed. B-tree code, allocation backing and their required notices
remain pending separate replacement. This record describes the process and
inputs; it is not a claim that the whole library is independently implemented.

A measured large-record regression prompted a second restricted handoff of only
Memory::read_unsafe's public initializing-read safety contract. The implementer
read only its two new files, then changed large records to allocate spare Vec
capacity, initialize it through that API and publish length after success.
Partial-read tests ensure decoding never observes incomplete bytes. Parent
integration added narrow unsafe-code lint expectations around those operations.
The 64-byte stack path is unchanged. This is not a new public API.

A later review supplied only the observed iterator regression and requested a
review of redundant checks in the new source. The implementer removed two
iterator calls through public get after already admitting the live index;
selected reads still check the full slot extent and owner health. Endpoint,
truncation, skipping and poisoning behavior are unchanged. It read only its own
two implementation/test files during both refinements.

The uniform variable-record length costs four bytes per slot, including very
small bounds; fixed records have no prefix. That is a capacity tradeoff for one
encoding, not a universal memory reduction. Small variable records can therefore
reserve more capacity than ICSS's narrower length representations.

## Qualification and matched results — 2026-11-10

Evidence root: `target/evidence/vector-replacement/`. The frozen pre-Vector
baseline is the accepted Cell harness under `target/evidence/cell-replacement/`;
all pending B-tree changes are already in that baseline. The final harness is
`accepted-owned.wasm.gz`; published ICSS 0.7.2 remains the same frozen external
reference. Initial candidate runs remain available, including the rejected
zero-initialized large-buffer candidate and the runner's diagnostic-text refusal.
The runner now recognizes the independent unbounded-type refusal explicitly.
No production compatibility path was introduced.

Native qualification passes 95 library tests, 11 public Vector acceptance tests,
14 Cell acceptance tests and five runtime tests. The native comparison suite
passes 28 tests; its two installed-only tests also pass with supplied old/new
consumer artifacts. Those tests reject incompatible Cell/Vector upgrades and
prove rollback leaves the old application usable, then install the new format
fresh. Original/O3/Os/Oz installed IO, custody, collection upgrade/rollback and
logging qualification pass at `target/qualification/runtime/attempt.VYBrUR`.
Native and Wasm strict Clippy, formatting, production dependency graphs, license
and documentation checks pass.

### Application size

These are raw Wasm bytes from the reproducible `wasm-size` consumer profile,
including governance and ordinary application handlers, not the type catalogue.
The source archives, selected tools, hashes and installed two-upgrade checks are
retained in `target/evidence/storage-consumer-size/attempt.bb266n`. The matched
pre-Vector application baseline is `attempt.yFBZRG`.

| Consumer | Previous Storage | New Storage | ICSS | Storage change |
| --- | ---: | ---: | ---: | ---: |
| Vector only | 96,271 | 94,704 | 64,560 | -1,567 |
| All collections | 135,939 | 136,246 | 119,318 | +307 |

Map, Set, Cell and region-only sizes are unchanged. The large type-catalogue
harness separately changes from 11,345,883 to 11,328,495 bytes; its ICSS harness
is 11,084,040 bytes. Those are benchmark code sizes, not application sizes.
The earlier zero-filled candidate's Vector size was 93,742 bytes. The initializing
read fixed its large-read regression; the subsequent iterator refinement removes
a duplicate live-index check. The final Vector remains 962 bytes larger than the
zero-filled candidate and 1,567 bytes smaller than the pre-Vector baseline.

### Measured operations and scope

Rows below are configured matched IC instruction counters, including page
charges. They are different workloads and must not be summed. Retained-handle
loop timing excludes input preparation and response encoding. The Vector harness
calls `len()` before its timed loop; caching changes that surrounding IO too.
Therefore a large isolated-loop ratio is not an application speedup claim.

| Scope / operation | Previous Storage | New Storage | ICSS |
| --- | ---: | ---: | ---: |
| 256 rows, retained u64 Get ×1, inner loop | 80,819 | 977 | 81,143 |
| 8 rows, 1 MiB variable Get ×1, inner loop | 5,050,538 | 4,970,565 | 4,330,030 |
| 8 rows, 1 MiB variable Pop ×1, inner loop | 5,130,889 | 5,130,956 | 4,410,348 |
| 8 rows, 1 MiB variable forward Scan ×1, inner loop | 8,579,257 | 4,970,411 | 7,858,877 |
| 256 rows, u64 Get ×1, full benchmark handler | 571,520 | 491,374 | 651,835 |
| 256 rows, u64 Get ×32, full benchmark handler | 664,167 | 669,139 | 752,922 |
| 256 rows, u64 alternating iteration ×256, inner loop | 203,451 | 236,811 | 196,333 |

The full-handler single read improves about 14.0% against previous Storage and
24.6% against ICSS in this case. The 32-read batch regresses about 0.75% against
previous Storage while remaining below ICSS. The selected 256-element alternating iteration remains 16.4% above previous
Storage and 20.6% above ICSS, down from 30.8% above previous Storage before the
duplicate-check removal. Large Get and Pop also remain slower than ICSS. No universal parity or throughput claim follows from the new design.

The full-handler control includes request decoding, preparation, collection work,
response encoding/reply and cleanup, with boundary-probe overhead; it excludes
population and host execution setup. For Control ×1, previous/new/ICSS are
569,271 / 489,075 / 649,295. This demonstrates why subtracting or interpreting
inner-loop counters alone would misattribute page costs. `boundary/` retains
original, plain WAT round-trip and boundary-only modules. All 26 summary rows in
each plain round trip match its original exactly. The two diagnostic full-handler
runs use identical instrumentation and leave the ordinary modules authoritative.

Cold-open results are recorded separately in `accepted-vector-open/`: opening and
reading can still incur similar charges to ICSS depending on population and
selected pages. Cached length does not make open-free behavior available to a
caller that discards and reloads the collection each time. It remains an exclusive
owner optimization, not a separately synchronized state cache.

### Reproduction

Use the existing prepared tools and both locked offline graphs. Build the owned
comparison example with `RUSTFLAGS='-C link-arg=-zstack-size=8388608'`, manifest
`qualification/Cargo.toml`, package `ic-storage-benchmarks`, example
`storage-comparison-wasm`, target `wasm32-unknown-unknown`, profile `comparison`
and feature `owned`. Freeze that Wasm and its gzip transport before later edits.
Build the external reference with feature `dfinity` in a separate target directory.

Run the existing benchmark driver through the checked `ic-testkit-server run`
entrypoint, passing explicit `--owned-wasm`, `--dfinity-wasm` and a fresh
`--output` directory. Selected runs use `--samples 1` and workload `types`
(`--collection vector`), `bulk` (`--collection vector`), `vector-reads`,
`vector-open`, and `iterators` (the latter retains its whole-workload controls).
Workloads own seeds, populations, batching and execution mode; retain their
configuration JSON rather than reconstructing defaults from this table.

For the separate scalar full-handler probe, use the README's boundary-only
instrumentation and `--cases Vector-U64-U64 --rows 256 --patterns uniform
--samples 1 --trace-execution-cost`. Qualify plain WAT round-trip equivalence
first. The native driver receives the resulting boundary modules; no product
source instrumentation is required.

Run `make benchmark-storage-size` and `make test-runtime` with the prepared
local server. For installed old-image refusal, supply `VECTOR_PREVIOUS_WASM`
from `attempt.yFBZRG` and `VECTOR_REPLACEMENT_WASM` from the new Vector-only
consumer, then run the ignored `incompatible_vector_upgrade` native comparison
test through the same server runner. The old source archive is the reproduction
source for the baseline; the new source must not be used to rebuild the old format.

Remaining replacement work is the allocation backing, shared inherited helpers
and the common ordered engine used by Map/Set/regions. Their notices and import
records remain. This change does not deploy, publish or migrate sibling consumers.

### Complete matrix disposition

All five selected workloads finish and validate returned values/final state.
The type workload checks all 73 catalogue cases: Storage supports 57 and refuses
16; ICSS supports 53 and refuses 20. Expected unbounded/fixed-bound refusals are
recorded, not counted as successful performance comparisons. The summary contains
2,652 rows including refusals and Storage-only supported cases. Among 2,352
non-control rows supported by both libraries, new Storage uses fewer instructions
than ICSS in 1,471, equal in one, and more in 880. Against previous Storage it is
lower in 1,650, equal in two and higher in 700.

| Workload | Matched summary rows | Non-control paired rows | New lower / equal / higher than ICSS |
| --- | ---: | ---: | --- |
| Types | 2,652 | 2,352 | 1,471 / 1 / 880 |
| Populated reads and mutations | 140 | 140 | 78 / 0 / 62 |
| Bulk records | 26 | 24 | 18 / 0 / 6 |
| Iterator sizes and directions | 1,287 | 1,188 | 719 / 0 / 469 |
| Cold open, 256 rows | 352 | 308 | 55 / 0 / 253 |
| Cold open, 4,096 rows | 352 | 308 | 78 / 0 / 230 |

These counts describe coverage, not a weighted performance score. Cases overlap,
operation costs differ, and many differences are small. The unchanged ICSS
reference produces identical median instruction counters in every matched row
across the pre-Vector and accepted runs. `accepted-comparison.json` retains this
check and the separate previous-Storage comparison. Final data is under
`accepted-*`; earlier `candidate-*` and `final-*` artifacts remain experimental
evidence and must not be presented as the accepted implementation.

The accepted iterator refinement removes roughly half the selected long-scalar
iteration regression, not all of it. Some hot 4 KiB read batches and cold-open
256-byte records also regress against previous Storage. Those remaining costs,
large-record parity and tiny-variable-slot capacity are explicit follow-up work.
The format/correctness replacement is complete; performance parity is not.
