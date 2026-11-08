# Why Storage is larger than ICSS

Storage 0.68.10 has not achieved the maintainer's requirement to be smaller than
**published ic-stable-structures 0.7.2**. Earlier reductions were mostly against
older Storage builds. They did not establish an advantage over ICSS. For example,
the [November 6 follow-through](storage-068-followup-2026-11-06.md#harness-footprint-and-evidence-scope)
already recorded 13,685,168 bytes for Storage versus 10,888,984 for ICSS.

The initial investigation changed only benchmark selection and documentation.
The shared-backing follow-through below changes private production dispatch while
preserving governance and public APIs/images; the size gap is reduced, not closed.

## Matched measurements

The existing full catalogue builds both suppliers with identical type/workload
coverage, Rust 1.99.1, fat LTO, one codegen unit, panic abort and the comparison
profile (`opt-level=3`). It retains function names and uses an eight-MiB synthetic
stack. Native driver dependencies are target-gated out of Wasm. ICSS uses eight
page buckets; Storage uses its governed allocations. These are deliberately
complete stacks, not equal-feature allocator implementations.

| Full catalogue | Storage bytes | ICSS bytes | Difference |
| --- | ---: | ---: | ---: |
| Original uncompressed module | 14,308,124 | 11,146,755 | +3,161,369 |
| Original code section | 12,088,524 | 9,534,340 | +2,554,184 |
| Matched Binaryen `-Oz --strip-debug` modules | 10,245,775 | 8,114,332 | +2,131,443 |
| Matched Binaryen code sections | 10,184,514 | 8,025,763 | +2,158,751 |

The gap survives symbol removal and optimization. Binaryen rewrites code even
when requested only to strip debug sections; do not interpret its entire module
reduction as symbol bytes. The optimized modules above are size observations,
not newly installed runtime qualifications.

A new opt-in `footprint` feature restricts the same canister harness to the six
existing depth cases: u64/u64 Map, u64/bytes Map, u64 Set, u64 Vector, fixed-32 Cell
and u64/u64 Region. Operations and validation stay intact. Unsupported cases and
vector-open explicitly refuse. Default builds retain the full catalogue.

| Compact depth build | Storage | Bare ICSS | ICSS on Storage backing |
| --- | ---: | ---: | ---: |
| Comparison module bytes | 381,684 | 255,058 | 335,872 |
| Comparison code bytes | 302,050 | 173,374 | 267,736 |
| `wasm-size` module bytes | 231,303 | 164,451 | 203,411 |
| `wasm-size` code bytes | 205,105 | 115,566 | 178,269 |

`wasm-size` selects Rust `opt-level=z` and strips symbols, with the other settings
held equal. These remain benchmark handlers, not minimal production canisters.
The compact gap is real: +126,626 bytes with the comparison recipe, +66,852 with
the size recipe. The full catalogue amplifies it but is not its only cause.

## Verified causes and their owners

### 1. Different backing types duplicate tree specializations

[StableBTreeMap](../../../crates/ic-storage/src/adapter/map.rs) owns a
`bridge::Map<K,V,RuntimeMemory<M>>`. [BTreeMapRegion](../../../crates/ic-storage/src/adapter/region.rs)
loads `bridge::Map<K,V,&RegionMemory<M>>` for each operation. The
[bridge alias](../../../crates/ic-storage/src/adapter/bridge.rs) passes that
memory type into the complete copied engine. The two paths therefore generate
separate search, mutation and traversal functions even for identical K/V pairs.

The [actual benchmark consumers](../../../testing/runtime-qualification/crates/ic-storage-benchmarks/benches/canister/collections.rs)
exercise both forms. On ICSS, the Region control reloads the same
`BTreeMap<K,V,BenchMemory>` type used by ordinary maps, so it can reuse compiled
engine code. Storage's bounded region semantics are additional behavior; the
comparison must not silently drop them to obtain parity.

After normalizing crate hashes and only the two backing type names, the full
Storage binary contains **1,059 paired function identities**: 1,014,462 encoded
bytes on the ordinary-map side and 1,188,359 on the region side. These numbers
establish repeated specialization; they are not a guaranteed 2.20-MB saving.
Inlined bodies and changed optimization decisions affect the achievable result.

The first production experiment should share a private governed tree backing
representation or outline memory-dependent work below the typed tree. Preserve
exclusive leases, bounded offsets/growth, native poisoning, region reloads and
public iterators. A common wrapper can add dispatch/checks to hot IO; measure
whole handlers and small consumers before retaining it. No prototype had qualified this change at the initial audit; the follow-through
below now records the implemented size/instruction tradeoff.

### 2. Governance has a measurable fixed footprint

The existing `dfinity-governed` control uses ICSS engines through Storage's
checked backing, rather than ICSS's MemoryManager. In the compact comparison
recipe it adds 80,814 module bytes over bare ICSS; Storage's collection stack
then adds another 45,812. Under `wasm-size` those differences are 38,960 and
27,892 bytes. These are controlled whole-module differences, not independently
additive costs of individual source modules.

[new_runtime](../../../testing/runtime-qualification/crates/ic-storage-benchmarks/benches/wasm.rs)
really invokes declaration/grant admission, directory initialization and lease
opening. [Directory](../../../crates/ic-storage/src/runtime/directory_model.rs)
contains key and allocation indexes, validation and Wire encoding/decoding;
[persistence](../../../crates/ic-storage/src/runtime/directory_model/persistence.rs)
adds snapshot preparation/publication. Heap B-tree/set and sorting instantiations
also contribute. Bare ICSS does not provide this complete contract.

For illustration, named compact functions include 17,924 encoded bytes in
`new_memory` (with inlined callees), 6,901 in directory validation and 5,062 in
commit preparation. Summing only functions named `ic_storage::runtime` would
understate the cost because much lives in callers and standard-library helpers.
Removing mandatory validation or moving initialization outside the measured
binary would not satisfy the goal. A separate fixed-footprint audit should
justify any index/control simplification against actual directory lookup,
mutation, corruption and lifecycle requirements.

### 3. Adapters and inlining amplify the broad matrix

By named function owner, full Storage engine bodies occupy 5,222,453 encoded
bytes versus ICSS's 4,218,880, plus 486,279 in Storage adapters. Map/region harness
bodies are also larger by 296,685/342,601 bytes. These are attribution clues,
not clean component boundaries: the harness contains inlined library work and
changes inlining when return types and memory types differ.

[Codec](../../../crates/ic-storage/src/adapter/bridge.rs) bridges fallible,
bound-checked Storage records to the engine's infallible trap contract; typed
collection adapters also retain successful-call Result APIs. Their code is
replicated across the large catalogue. Bound and failure checks are part of the
contract, so first try sharing type-independent work, as the retained Cell
writer did. Do not assume every wrapper survives optimization or remove checks
on the strength of a symbol count alone.

The Storage benchmark includes ICSS-native reference codecs for common test
values, but only 23 named ICSS-owned functions / 11,039 encoded bytes survive
there. It is not accidentally linking a second complete ICSS collection engine.

## Qualification and next decision

The compact owned/ICSS depth run validates all 4,272 calls and 712 comparison
cells across two populations, two access patterns and three mutation rounds.
Strict Wasm lint passes for owned, dfinity and dfinity-governed compact builds;
formatting passes. Governed ICSS's deliberate `last()` measurement has a scoped
lint expectation rather than substituting a different operation. Its compact
artifacts are size/build evidence, not an additional installed runtime run.

Production source is unchanged. The full catalogue remains the default. No
release, dependency adoption or weaker safety boundary is included. The pending
root Runtime/Wire lock changes are unrelated and preserved.

Next: qualify tree-backing code reuse first, then investigate the directory's
fixed code cost. Report both full and compact modules, both compiler recipes,
and whole-handler regressions. Getting below bare ICSS remains unachieved;
the evidence does not justify promising that either isolated change is enough.

## Evidence and reproduction

Full artifacts: `target/evidence/storage-comparison/attempt.LRun1o`, from the
Cell writer experiment subsequently shipped in 0.68.10. Compact source starts
from `9568c93` plus the benchmark-only footprint selection. The independent
lock retains Runtime 0.20.0, Wire 0.17.6 and ICSS 0.7.2. Raw artifacts, function
inventories, grouping/pair analysis, size controls, lint and installed output:
`target/evidence/storage-wasm-gap/`. Function groups may overlap conceptually
through inlining, but each encoded function is counted once in each inventory.

Follow the [benchmark footprint recipe](../../../testing/runtime-qualification/crates/ic-storage-benchmarks/README.md#compact-wasm-footprint-control).
Compare uncompressed bytes and the decoded code-section payload; also retain
compressed transport bytes and module digests. Do not compare an optimized or
restricted Storage build with the unoptimized/full ICSS catalogue.

## Direct published reference follow-through

The November 8 dependency cleanup removes the old Platform 0.17 facade from the
unpublished benchmark workspace and selects ICSS 0.7.2 directly. The production
engine sources are unchanged: one local modified implementation per algorithm,
with Map, Set and regions sharing the B-tree. Published Storage's normal Wasm
graph still excludes ICSS. The benchmark library needs the external reference's
traits/codecs, so its development-only workspace uses a regular Cargo dependency.

Matched full-catalogue comparison-profile modules are 14,308,122 bytes for Storage
and 11,146,755 for ICSS. Their code sections are exactly unchanged at 12,088,524
and 9,534,340 bytes. Storage's two-byte module reduction is not an executable-code
saving; the 3,161,367-byte module gap remains. Gzip transport sizes are 1,937,259
and 1,665,864 bytes. These are harness sizes, not minimal application sizes.

Builds and captured source live in `target/evidence/storage-comparison/attempt.ILEBwR/`.
The first installed run was blocked by sandbox port binding; two subsequent
launcher attempts used the version directory instead of the setup root and failed
before execution. Logs are retained. `report-retry3` uses the admitted setup root
and the same hashed Wasm artifacts. After capture, only the benchmark library's
import ordering and native-only test/example reference imports changed; no Wasm
implementation changed. Focused evidence under
`target/evidence/direct-icss-reference/` includes 28 passing native tests,
cross-image qualification, native example compilation and formatting.

The installed 64-row smoke completes successfully: 6,928 measured calls, 3464
comparison rows, 30 cases across all five collections, uniform/hot16 access and
batches 1/32. Returned values and final states pass the independent model. One
sample per cell is correctness evidence, not proof of small performance changes.

## Shared tree backing follow-through

Ordinary maps and region views now instantiate the engine with the same private
`Backing<TreeMemory<M>>`. Its two variants retain either the owned runtime lease
or the existing region Rc and range. Region operations clone that Rc without a
new allocation, reload the current root as before and release the temporary
owner when the operation ends. The private `RegionMemory` still owns translation,
empty-IO checks and bounded growth. Ordinary map reservation and panic guards
still use their original runtime allocation. Public APIs and image bytes do not
change; Cell and Vector retain their existing backing types.

| Matched artifact | Before | Shared backing | Published ICSS |
| --- | ---: | ---: | ---: |
| Full comparison module | 14,308,122 | 11,719,033 | 11,146,755 |
| Full comparison code section | 12,088,524 | 9,888,917 | 9,534,340 |
| Compact comparison module | 381,684 | 343,944 | 255,058 |
| Storage consumer qualification fixture (`wasm-size`) | 191,499 | 182,316 | Not equivalent |

The full module saves 2,589,089 bytes (18.1%); executable code saves 2,199,607.
The smaller consumer fixture saves 9,183 bytes. Both fixture builds use the same
tracked independent graph, compiler and profile; only the four backing/adapter
sources change. Source selection is restored after the baseline measurement.
The full candidate remains 572,278 bytes larger than ICSS; it does not achieve
absolute size parity.

The direct before/after depth run checks 4,272 calls across six cases, 256/4,096
rows, uniform/hot16 access, batches 1/32 and three mutation rounds. Its reference
artifact is **previous Storage**, despite inherited `dfinity` CSV column names.
These are timed collection-loop instructions, not complete endpoint counts.
Excluding Control, the ranges across matched summary cells are:

| Structure | Candidate instruction change versus previous Storage |
| --- | ---: |
| Map | −1.07% to +6.90% |
| Set | +0.15% to +3.37% |
| Region | +0.17% to +4.67% |
| Vector | unchanged |
| Cell | unchanged |

The worst observed case is 4,096-row byte-map RemoveMiss, batch 32:
805,917 → 861,560 instructions. The shared dispatch is a deliberate size tradeoff,
not an instruction optimization. One sample per cell establishes model-checked
behavior and observed instruction costs, not statistical throughput confidence.

Focused proof passes: 23 adapter tests, 28 benchmark/native cross-image tests,
strict library/test Clippy and formatting. The region snapshot test additionally
checks that operation-local owners do not escape. Installed IO, upgrade, rollback
and Logging qualification pass original/O3/Os/Oz in
`target/qualification/runtime/attempt.hsHqSf/`. No release/publication is performed.

Artifacts, original source snapshots, candidate diff, source hashes and logs are
under `target/evidence/shared-tree-backing/`. `against-storage` records the
before/after run; `against-icss` is the separate published-reference comparison.

The separate published-ICSS depth comparison also passes all 4,272 calls / 712
summary cells. The range of candidate/ICSS timed instruction ratios, excluding
Control, is below (less than 1 means Storage uses fewer instructions). Endpoint,
operation and population differences make these ranges unsuitable as a single
speedup score. The Region control uses an ordinary ICSS map reload; Storage also
enforces its region bounds.

| Structure | Storage / published ICSS instruction range |
| --- | ---: |
| Map | 0.008×–1.063× |
| Set | 0.006×–1.010× |
| Vector | 0.002×–1.020× |
| Cell | 0.838×–0.999× |
| Region | 0.956×–1.155× |

Native and Wasm strict Clippy both pass. Full raw matrices are retained under
`against-storage/summary.csv` and `against-icss/summary.csv` in the evidence root.

## Primary size benchmark: matched small consumer crates

`make benchmark-storage-size` now owns the application-footprint comparison.
Two unpublished consumer crates have separate production dependency graphs and
compile the same raw-byte endpoints. The full test catalogue and verification
model are absent from their Wasm. The benchmark README documents the exact
[protocol, build and qualification](../../../testing/runtime-qualification/crates/ic-storage-benchmarks/README.md#matched-consumer-wasm-size).

Both sides use Rust 1.99.1 and the stripped `wasm-size` profile without Binaryen.
Selected library source includes the pending shared-tree-backing change; the
independent lock selects Runtime 0.20.0, Wire 0.17.6 and published ICSS 0.7.2.
These are application examples with u64 records and six common endpoints, not a
universal footprint claim for all application codecs or workloads.

| Consumer | Storage bytes | ICSS bytes | Storage minus ICSS |
| --- | ---: | ---: | ---: |
| Empty endpoints control | 16,998 | 16,994 | +4 |
| Map | 137,262 | 90,264 | +46,998 |
| Set | 136,976 | 90,064 | +46,912 |
| Vector | 112,464 | 64,616 | +47,848 |
| Cell | 109,393 | 63,195 | +46,198 |
| Region | 138,314 | 89,989 | +48,325 |
| Map + Region | 140,816 | 110,914 | +29,902 |
| All five | 153,001 | 119,378 | +33,623 |

The empty controls have identical 14,818-byte code payloads. Their four-byte data
difference comes from the different package path in a shared-source panic string
and its layout. It is not silently subtracted. All 16 artifacts have zero custom
section bytes and identical canister export sets. An independent `ic-wasm info`
read confirms the combined artifacts' methods and absence of custom sections.

All artifacts pass 118 model-checked calls and two upgrades each: 1,888 calls and
32 upgrades total. Native driver lint, full/base Wasm lint, ShellCheck, formatting,
pin and dependency boundaries pass. New lock entries are only the two private
consumers; existing selected dependency versions remain unchanged. No production
source change is included in this benchmark addition, and nothing is published.

These figures replace the full catalogue's 572-KB gap as the primary small-app
size comparison. Storage's 46–48 KB single-collection premium is consistent
across these cases, but this does not prove that every byte is unavoidable
allocation governance. Map+Region and the combined app demonstrate shared code
costs; individual row sizes cannot be added. The smaller combined gap also
reflects differences in how the two real integrations reuse compiled code.

[Selected sizes](../../measurements/storage-consumer-wasm-size.csv) and
[identities/section metrics](../../measurements/storage-consumer-wasm-size-identities.json)
retain the compact results. Raw source archive, graphs, build/runtime logs and
all 16 artifacts are under `target/evidence/storage-consumer-size/attempt.k0WdIH/`.
The larger catalogue, compact depth and previous attribution controls remain
historical/diagnostic evidence, not normal production application sizes.

## Attribution of the small-consumer premium

The stripped artifacts above, not the broad catalogue, establish these exact
section differences (positive means Storage is larger):

| Consumer | Module difference | Code payload difference | Data payload difference |
| --- | ---: | ---: | ---: |
| Map | +46,998 | +71,037 | −24,397 |
| Set | +46,912 | +71,075 | −24,529 |
| Vector | +47,848 | +71,231 | −23,769 |
| Cell | +46,198 | +70,246 | −24,433 |
| Region | +48,325 | +72,208 | −24,245 |
| Map + Region | +29,902 | +53,944 | −24,349 |
| All five | +33,623 | +57,093 | −23,780 |

Small remaining differences are other sections and encoded section headers;
code/data payloads are not full module totals. ICSS's Cell artifact contains a
32,768-byte all-0xff run in its data payload, matching the unallocated-bucket
array in published 0.7.2 `memory_manager.rs` (`MAX_NUM_BUCKETS = 32768`, `new`).
Storage's Cell artifact contains no such run. Thus the 46-KB net difference
actually combines about 70 KB more code with about 24 KB less data. It is not a
hidden function-name section; these artifacts have zero custom-section bytes.

Symbol-retaining builds of Map, Cell and all-five locate the common code. Their
Storage code payloads differ by +44 bytes from the stripped artifacts; ICSS code
sizes are unchanged. Use the inventories as attribution evidence, not exact
causal costs of independently removable modules. In the diagnostic Cell build:

| Named owner/group | Storage bytes | ICSS bytes |
| --- | ---: | ---: |
| General sorting helpers | 14,110 | 0 |
| Rust heap B-tree helpers | 13,656 | 0 |
| Storage runtime / codecs | 20,972 | 0 |
| Consumer functions, including inlined initialization | 10,939 | 1,541 |
| Collection engine functions | 518 | 1,996 |
| ICSS memory-manager functions | 0 | 1,638 |

The Cell comparison deliberately avoids the collection B-tree. Its sorting and
heap B-tree code belongs to allocation/admission/directory machinery. The same
roughly 70–72 KB code premium across single-collection consumers supports a
shared-runtime explanation. It does not establish that all that runtime code is
necessary or that every byte can be removed.

Concrete source owners and candidates for a subsequent experiment:

1. [Directory indexes](../../../crates/ic-storage/src/runtime/directory_model.rs)
   retain both `by_key` and `by_id`. Validation already requires `image.entries`
   to have strictly ascending IDs, yet `by_id` separately maps each ID to its
   position. Replacing that derived index with binary search is a bounded local
   candidate; its actual size/speed effect is unmeasured. Lease admission, growth
   lookup and uncached IO use it, so these paths require performance coverage.
2. [Admission](../../../crates/ic-storage/src/runtime/admission.rs),
   [bootstrap](../../../crates/ic-storage/src/runtime/mod.rs) and directory
   candidate construction materialize key sets at several stages. The binary
   includes multiple sorting specializations, including stable sort for
   `StableKey` used by collection construction. Review which canonical ordering
   can be carried forward rather than reconstructed. Historical inclusion and
   arbitrary private callers must still preserve duplicate/authority checks.
3. [Snapshot persistence](../../../crates/ic-storage/src/runtime/directory_model/persistence.rs)
   and directory validation remain material: named Cell functions include
   4,707 bytes in validation, 3,619 in persisted read and 3,220 in commit preparation.
   Wire/Serde work also lives in callers and Rust helpers. These counts do not
   justify deleting integrity checks or replacing the persistence format.

No production implementation changes are made in this attribution pass.
`inspect-wasm-size.pl` preserves the inspection method in maintained source;
[reproduction instructions](../../../testing/runtime-qualification/crates/ic-storage-benchmarks/README.md#reproducing-size-attribution)
include symbol builds and explicitly separate them from reported stripped sizes.
The inspector's six inventories match an independent byte/function parser;
stripped input is also qualified for section accounting. Raw inventories and
logs are in `target/evidence/consumer-size-attribution/`, with compact attribution
retained in the existing measurement identities file.

## Removing the redundant ID index

The follow-through removes `Directory::by_id`. Validated entries already have
strictly increasing opaque IDs, so binary search supplies their positions without
another allocated B-tree. Lease opening, growth and uncached IO use that search;
cached lease indices and the span-hit path remain unchanged. Key lookup, authority,
duplicate/order validation, custody and persistence remain intact. No public API,
stored format or migration changes.

Matched stripped small-consumer module sizes, in bytes:

| Consumer | Storage before | Storage after | ICSS 0.7.2 | Storage reduction |
| --- | ---: | ---: | ---: | ---: |
| Map | 137,262 | 134,314 | 90,264 | 2,948 |
| Set | 136,976 | 134,028 | 90,064 | 2,948 |
| Vector | 112,464 | 109,531 | 64,616 | 2,933 |
| Cell | 109,393 | 106,460 | 63,195 | 2,933 |
| Region | 138,314 | 135,361 | 89,989 | 2,953 |
| Map + Region | 140,816 | 137,877 | 110,914 | 2,939 |
| All five | 153,001 | 150,062 | 119,378 | 2,939 |

Base remains 16,998/16,994 bytes. All 16 artifacts pass 1,888 checked calls and
32 upgrades. The remaining all-five gap is 30,684 bytes; Storage is still larger.

The compact depth workload compares 256/4,096 rows, uniform/hot keys and batches
of 1/32. Each reference run passes 4,272 calls / 712 summary cells. Ranges below
exclude empty `Control` loops and describe timed collection loops, not complete
endpoint costs; negative means fewer Storage instructions:

| Collection | Change versus previous Storage | Difference versus ICSS 0.7.2 |
| --- | ---: | ---: |
| Map | −4.94% to +5.89% | −99.25% to +6.45% |
| Set | −1.91% to +7.60% | −99.35% to +1.10% |
| Vector | 0% to +0.11% | −99.80% to +2.11% |
| Cell | unchanged | −16.23% to −0.08% |
| Region | −3.87% to +2.51% | −5.45% to +14.73% |

These ranges span very different operations; near-99% savings are not general
throughput improvements. The worst new Set regression is a 32-key above-range
miss at 4,096 rows: 707,579 to 761,339 instructions. A baseline rebuild from the
saved pre-change sources matches the previous compact Wasm SHA-256 exactly.
Control loops also change, so code generation/layout contributes to the measured
effect; the experiment does not isolate its share from lookup or heap placement.
Keep this as a small size/simplicity tradeoff, not a runtime-speed claim.

An explicit optimized native diagnostic compares 200,000 deterministic hit/miss
lookups at 1, 32, 256, 1,024 and 4,096 allocations. Binary search takes 0.63/2.22/
3.14/4.05/5.31 ms versus the heap B-tree's 1.13/3.30/4.91/5.67/7.50 ms in this run.
The tree control is bulk-built; this is a representation microbenchmark, not
historical IC instruction or page-charge evidence. Reproduce with
`cargo test -p ic-storage --lib compare_id_lookup_representations --release --locked --offline -- --ignored --nocapture`.
Large-directory IC timing remains unmeasured; installed lifecycle coverage is
not a performance proof.

Thirty native runtime tests and strict native/Wasm lint pass. New coverage checks
sparse IDs near the u64 limit, reopen/growth, missing IDs and malformed ordering.
Installed original/O3/Os/Oz IO, corruption, custody, rollback and upgrades across
1,031 allocations pass in `target/qualification/runtime/attempt.dZEIjv`.
Raw comparisons are in `target/evidence/directory-id-index/`; small-consumer
evidence is `target/evidence/storage-consumer-size/attempt.tvXumh`. The existing
measurement CSV retains before/after columns, and its identities JSON retains
both artifact sets. Maintained benchmark recipes recreate generated artifacts.
No release or publication was performed.
