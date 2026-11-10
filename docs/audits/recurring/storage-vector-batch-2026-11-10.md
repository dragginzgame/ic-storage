# Vector and grouped admission experiments — 2026-11-10

**Final retained design:** direct Vector length writes and one custody-based
`open_memory` path without lease-time rescanning. The unpublished batch API and
checksum cache were removed after the investigation below. Earlier "retained"
sections record intermediate decisions, not the final public contract; the final
section owns the delivery result. The iterator ownership trial remains rejected.
Base: `bd81c729`, Storage 0.71.2, published
ICSS 0.7.2, Rust 1.99.1 and PocketIC 16.1.0. Dependencies and compiler selections
are unchanged. These are installed synthetic operation measurements, not complete
Auth/Canic endpoint timings. No publication is included.

## 1. Vector length publication: retained

The Vector header is already allocated after construction/admission. Writing its
length directly avoids the growth-capable helper's redundant size query. Payload
writes still precede length publication; governed write bounds and poisoning
remain active. This removes no actual backing read: allocation size is in memory.

Matched `vector-reads`, 256 rows, batch 32, uniform keys, u64 values:

| Operation | Storage before | Retained length write | ICSS |
| --- | ---: | ---: | ---: |
| Get | 95,060 | 95,060 | 102,932 |
| Set | 182,740 | 182,740 | 180,724 |
| Push | 190,420 | 187,412 | 186,644 |
| Pop | 182,836 | 179,764 | 188,692 |

The preset also covers fixed arrays, Wire records, 31/4096-byte chunks, hot and
uniform access, batches 1/32 and three mutation/read rounds. One sample is used
per round; the table contains aggregate medians. Native Vector behavior tests
cover failure, malformed lengths, reopen, mutation and iterator truncation.

## 2. General iterator ownership: rejected

The trial removed the reusable buffer and called the existing direct-entry
decoder for forward/reverse/terminal reads. It retained live-length checks and
passed behavior tests. Scalars and 32-byte arrays improved approximately 2% at
batch 32; large byte payloads improved substantially. However, the complete
73-type Vector catalogue exposed regressions, including types whose decoders
cannot retain the supplied buffer. The exact cause of the large inline Blob
terminal-path regression was not isolated; it is not attributed solely to copies.

| Case / operation | Storage before | Rejected trial | ICSS |
| --- | ---: | ---: | ---: |
| Chunk 64 KiB, Scan, batch 1 | 612,859 | 386,778 | 452,452 |
| Chunk 1 MiB, Scan, batch 1 | 8,579,257 | 4,970,136 | 7,858,877 |
| WireRecord, Scan, batch 32 | 227,718 | 230,470 | 223,289 |
| Array256, Scan, batch 8 | 94,646 | 98,240 | 94,452 |
| Blob65536, zero-byte payload, Last, batch 1 | 889,567 | 1,115,111 | 7,145,057 |

Rows come from separate matched curated, bulk and type workloads; do not sum them.
The type sweep has 146 backend coverage dispositions and 5,200 raw samples per
run, including expected unsupported/refusal controls. Strings
are unbounded and rejected as Vector elements, not silently assigned a bound.
The trial saves 1,500 bytes in the Vector-only application and 177 bytes in the
all-collections application, relative to the length-write change. Its broad
catalogue Wasm shrinks another 136,094 bytes. These savings do not justify the
25.4% Blob terminal-read and 3.8% array-scan regressions. All iterator changes are
reverted; the existing consumed `last()` ownership optimization remains intact.

## 3. Checked batch admission: retained

`MemoryRuntime::open_memories(&keys)` checks current admission for every key,
rejects duplicate/existing leases, verifies the complete backing once, then
claims independent handles in input order. Errors claim none. Detached handles
retain custody and release independently. Empty batches check bootstrap/health
without IO. Conflicts use `RuntimeOpenError::Directory(DirectoryError::AlreadyOpen)`;
the existing single-name API and its error mapping are unchanged.

This does not cache verification across executions or weaken the directory
checks. No stored format, checksum, grant or data-IO contract changes. Native
verification failures still poison the owner; IC rollback remains authoritative.

Median of three samples, 32 occupied eight-page buckets. Both Storage paths hold
all handles, read their sizes and drop them inside the measurement interval:

| Names | Storage sequential | Storage batch | ICSS held handles |
| ---: | ---: | ---: | ---: |
| 1 | 280,405 | 281,119 | 160,916 |
| 16 | 3,264,786 | 525,273 | 164,354 |
| 64 | 40,983,797 | 1,464,823 | 175,394 |
| 255 | 618,530,178 | 5,886,586 | 219,324 |
| 256 | 623,351,418 | 5,899,876 | unsupported |
| 1,024 | 9,848,744,244 | 26,315,284 | unsupported |

Batching avoids repeated population-wide verification: approximately 28x lower
cost at 64 names and 374x at 1,024. ICSS has no equivalent governance verification;
its column is a reference, not a security-equivalent implementation. Its 255-ID
limit remains enforced. Keep `open_memory` for single handles; batching has
overhead and does not improve the singleton case.

The maintained driver covers 1/16/64/255/256/1024 names and 0/1/32 occupied buckets,
including single opens, immediate drops, held sequential/batch opens, reopen and
append. It validates retained contents after every operation. There are 324
Storage and 207 ICSS samples, with no fabricated results beyond ICSS's limit.

## Wasm accounting

Original matched applications, `wasm-size` profile, no Binaryen pass:

| Consumer | Before | Length write only | Final, adopting batch | ICSS |
| --- | ---: | ---: | ---: | ---: |
| Vector | 96,447 | 96,314 | 97,574 | 64,560 |
| All collections | 136,268 | 136,159 | 137,275 | 119,318 |

Batch adoption adds 1,260/1,116 bytes relative to the retained length-write
control. The final all-collections delta is +1,007 bytes against the starting
application, within the 5–10 KB budget. The single-collection probes deliberately
exercise the batch API too; production singleton callers need not adopt it.
All eight application pairs pass installed calls and two upgrades. The dedicated
admission probe grows 93,418 to 95,898 bytes (+2,480), including the new comparison
handlers. This is distinct from the broad catalogue harness (over 11 MB).

## Qualification and reproduction

Use the maintained [benchmark commands](../../../qualification/ic-storage-benchmarks/README.md).
`BENCH_WORKLOAD=vector-reads BENCH_SAMPLES=1 make benchmark-storage` reproduces the
length-write comparison; `make benchmark-storage-size` qualifies application
sizes and upgrades. Frozen-module drivers with `--workload types --collection vector`
and `--workload bulk --collection vector` qualify the complete type and large-byte
controls. The README documents both admission backends and their native driver.

Native tests include one-verification IO counts, input order, independent drops,
detached custody, empty/duplicate/missing/held/unadmitted names, backing corruption
and injected verification panics. Strict product Clippy and focused native/Wasm
admission Clippy pass. Full product release gates are separate and have not been
run for this unpublished batch.

The final full Storage package test command passes (90 library tests, six runtime
tests, the compile-failure harness, 88 repository-helper tests and one doctest).
Final comparison `opdNNT` matches `CB1pDp` aggregate and per-round CSVs exactly.
Its broad harness is 11,450,111 bytes versus the baseline's 11,465,082; those
synthetic sizes are not application sizes. Rebuilding the admission probe after
restoring iterator reuse reproduces the measured owned Wasm byte-for-byte.
Formatting, diff checks and focused documentation-link checks pass.

Evidence under `target/evidence/`: comparison attempts `Z385g5` (baseline),
`CB1pDp` (length write), `sZv3O5` (rejected iterator); corresponding size attempts
`SkUj65`, `N32hHL`, `hPExLk`; final size attempt `tRR7Ih`. The
`vector-{iterator,types,bulk}-{before,trial}-2026-11-10` outputs retain frozen-run
results, including refusals. Admission modules, build logs and CSVs use the
`admission-batch-*-2026-11-10` prefix. Source snapshots and module hashes retained
by the standard wrappers distinguish the rejected experiment from final code.

## Follow-up: practical admission costs

The 1,024-handle total above is an extreme scaling test, not a normal application
operation. Names count allocations, not records: a million rows in one map do
not mean a million handles. The expanded maintained driver adds 2/4/8-name
groups and holds one handle across successive IC executions. Only allocation
zero grows; 32 buckets means 16 MiB reserved capacity with one initialized
eight-byte payload, not a populated 16-MiB business collection.

Fresh original modules, median of three samples, 32 buckets:

| Names in directory | Storage sequential all-held | Storage batch | ICSS all-held |
| ---: | ---: | ---: | ---: |
| 1 | 280,432 | 281,119 | 160,899 |
| 2 | 420,344 | 294,238 | 161,129 |
| 4 | 598,391 | 322,802 | 161,589 |
| 8 | 1,185,766 | 386,748 | 162,497 |
| 16 | 3,265,218 | 525,273 | 164,337 |

The changed harness is measured afresh; small differences from earlier rows are
not product regressions. ICSS still supplies no equivalent governance scan.

**Verified cause:** `Owner::lease` calls `Store::verify` for every open. Verification
reads the root, all live 160-byte allocation rows and all 16-byte bucket-owner
rows. Each allocation row is re-encoded, including its immutable name checksum,
before comparison. Thus one open is proportional to the whole active directory;
opening all N allocations separately repeats that work N times. It does not read
all application payload or unused reserved table capacity. At 1,024 names and
32 buckets, one verification reads 164,416 metadata bytes; repeating it 1,024
times reads 168,361,984 bytes and repeats the checksumming/comparisons.

For four names, one open costs 308,729 instructions. Separately, diagnostics,
which calls the same verification and then builds capacity totals, costs 306,130.
At 64 names those figures are 871,693 and 867,811. This supports verification as
the dominant cost; it is not an additive stage breakdown or an exact isolated
hashing measurement.

| Later execution through an already-held handle | Storage | ICSS |
| --- | ---: | ---: |
| Timer/control body | 212 | 214 |
| Size lookup | 160,305 | 160,311 |
| One eight-byte read | 160,585 | 160,467 |
| 32 eight-byte reads in one execution | 248,862 | 244,900 |

These counts are identical across the measured name populations with 32 buckets.
They include the measured operation's IC memory-access costs, not only scalar
instructions, and exclude setup/reply/checking outside the counters. The large
shared per-execution overhead cannot be multiplied by the number of accesses:
32 reads cost much less than 32 separate read executions. This is a raw backing
read control, not a whole Map/Vector/Cell or application endpoint benchmark.

Read-only sibling inspection confirms retained collections in Canic's
`crates/canic-core/src/storage/stable/env/mod.rs` and
`crates/canic-core/src/storage/stable/cycles/mod.rs`: their thread-local initializers
open an allocation and retain the resulting Cell/maps. Canic's runtime is also
retained in `crates/canic-core/src/memory/runtime/mod.rs`. This does not prove all
consumers follow that pattern or establish a deployed canister's allocation count.

**Next investigation:** reduce repeated metadata work at the opening boundary,
rather than optimize for ever-larger batches. Caching the checksum of immutable
admitted name bytes could preserve full comparison while avoiding rehashing;
benefit, heap overhead and Wasm cost remain unmeasured. Eliminating the whole
lease-time scan would change the existing corruption-detection contract and must
not be presented as a free optimization. Neither change is implemented here.

The follow-up changes benchmark code/docs only, preserving all pending product
work. Both installed runs pass (945 Storage and 663 ICSS samples). Native driver
and both Wasm-backend Clippy checks pass. Raw results/modules/build logs use
`target/evidence/admission-practical-final-*`; hashes and source snapshot identify
the exact inputs. The owned admission harness grows 95,898 to 97,270 bytes
(+1,372) for the new controls; this is benchmark code, not product Wasm growth.

## Retained follow-through: admitted-name checksum cache

Cache the immutable name checksum in each admitted `Entry`. New names compute
it from the canonical name bytes; cold decoding validates the stored checksum
before retaining it. Verification still compares every byte of each canonical
row and checks root/owner metadata. Mutable page-count checksums are recomputed.
This does not cache verification success or skip backing reads. No persisted
bytes, public API, grants, lifetime checks or publication order change.

The only additional persistent-in-heap field is one `u64` per entry; spare
capacity in the entry vector also consumes that space. No additional cache table
or invalidation mechanism is introduced because admitted names cannot change.
Binding retains its existing prepare/write/publish sequence and temporary key
list rather than introducing a second vector of prepared entries.

Fresh original Wasm, same driver/compiler/locks as the practical study, three
samples and 32 buckets. Open one handle with the indicated total directory size:

| Directory names | Before | Cached checksum | ICSS |
| ---: | ---: | ---: | ---: |
| 1 | 279,840 | 277,712 | 160,391 |
| 2 | 289,616 | 285,360 | 160,391 |
| 4 | 308,729 | 300,217 | 160,391 |
| 8 | 346,516 | 329,492 | 160,391 |
| 16 | 422,084 | 388,036 | 160,391 |
| 64 | 871,693 | 735,501 | 160,391 |

Verification saves 2,128 instructions per directory entry. Batch opening also
saves that amount per entry: four names fall from 322,802 to 314,290 instructions;
16 names from 525,273 to 491,225. Existing-handle read controls remain unchanged
(160,585 for one eight-byte read and 248,862 for 32 reads).

**Tradeoff:** cold reopen performs the same checksum validation and retains a
larger in-memory entry. It rises from 138,455 to 139,554 instructions at four
names (+0.8%), and 697,554 to 716,188 at 64 (+2.7%); the worst percentage in this
matrix is +2.98% at 1,024 names/zero buckets. Even one subsequent handle opening
saves more than that extra cold cost in the measured cases. These are separate
operations, not a measured end-to-end startup timer.

Every nonempty small-application selection grows 257 bytes; the empty control
and ICSS modules retain their previous sizes. Vector is 97,831 bytes (was 97,574;
ICSS 64,560), and all-collections 137,532 (was 137,275; ICSS 119,318). This remains
within the size budget. No blanket collection-operation speedup is claimed.

All 945 installed admission samples pass. The 91 library tests and six public
runtime tests include existing every-control-byte corruption, valid-but-changed
rows, owner swaps, interrupted publication, growth and custody coverage. The
renamed-row corruption fixture constructs a fresh canonical entry so it still
tests a changed name with a **valid** checksum. New coverage checks shortest and
128-byte names, changing page counts and exact encode/decode stability. An early
test fixture used an invalid name without `.vN`; its failure log is preserved,
and the corrected canonical fixtures pass. Strict core Clippy and all eight
installed small-application call/two-upgrade pairs pass.

Evidence: `target/evidence/name-checksum-*`, size attempt `ZtJVWP`; before controls
are `admission-practical-final-owned.csv` and size attempt `tRR7Ih`. ICSS references
are the unchanged frozen practical-study run and the newly qualified size pairs.
The maintained admission and size commands reproduce the measurements.

## Investigation: is lease-time verification necessary?

**Recommendation:** remove the full directory rescan from ordinary lease opening
for an already admitted, continuously claimed backing. Retain bootstrap/admission,
health, grant, identity, exclusivity and data bounds checks, and retain full cold
validation and explicit diagnostics. This is a proposed detection-contract change,
not a retained implementation in this investigation.

Evidence traced in the current implementation:

- [Custody](../../../crates/ic-storage/src/runtime/custody.rs) forbids unmanaged
  writes and competing managers over a claimed backing, and explicitly says it
  is not isolation against arbitrary linked code or a dishonest backing. Claims
  are shared across wrappers and retained until the final owner/lease drops.
- [Construction and bootstrap](../../../crates/ic-storage/src/runtime/mod.rs)
  validate the backing and current declarations before admitting names. A live
  runtime owns its admitted in-memory directory. Warm bootstrap already compares
  the established binding without backing IO.
- [Leases](../../../crates/ic-storage/src/runtime/directory_model/leases.rs)
  enforce live ownership separately from verification. Their indices remain valid
  because bindings append without removing or reordering existing entries. The
  runtime uses shared Rc/RefCell state; ordinary mutations update that same state.
- [Persistence and IO](../../../crates/ic-storage/src/runtime/directory_model/persistence.rs)
  already trust admitted metadata during reads, writes and growth. Logical bounds
  and bucket translation prevent governed data writes from reaching control rows.
  Native backing failures poison the owner; mutation publication and IC rollback
  retain their current contracts. Opening another lease is not required to notice
  a failure already recorded by the owner.

The extra scan detects metadata modified after admission through raw aliases,
unexplained backing growth, or silent backing faults/implementation defects. It
does not continuously protect existing handles: ordinary IO can continue without
opening another lease. Removing it therefore changes **when externally corrupted
metadata is detected**, rather than removing namespace authority, exclusivity or
normal IO bounds enforcement. Such corruption would still be checked by explicit
`diagnostics()`/`memory_allocations()`, the retained admission checks, and cold
reconstruction. Silent corruption is not made harmless by this proposal.

### Temporary two-line probe

Removed only the two `store.verify()?` calls in single/group lease opening,
compiled a separate Wasm and ran the unchanged admission driver, then restored
the source byte-for-byte. The checksum cache and all other pending work remain
in both controls. Median of three samples, 32 reserved buckets:

| Directory names | Current one-open | No-rescan probe | ICSS one-open |
| ---: | ---: | ---: | ---: |
| 1 | 277,712 | 162,304 | 160,391 |
| 4 | 300,217 | 163,182 | 160,391 |
| 8 | 329,492 | 163,621 | 160,391 |
| 16 | 388,036 | 164,487 | 160,391 |
| 64 | 735,501 | 165,365 | 160,391 |

All 945 installed admission samples pass. Retained-read costs are unchanged.
The dedicated admission probe shrinks 97,527 to 97,434 bytes (-93). This does
not establish production-shaped application size or installed upgrade/rollback
qualification for a retained change; those would be required during implementation.

The probe intentionally fails three of 91 library tests and one of six runtime
tests: an assertion that openings read metadata, two tests that corrupt backing
then expect opening to detect/poison it, and a public runtime test with the same
expectation. Their ordinary backing-panic checks pass before the corruption
assertion fails. These are evidence of the detection-contract change, not a clean
test pass. Other tested failure/custody paths pass. Original source is restored;
the unchanged full native suite is rerun separately in `restored-tests.log`.

### Smallest implementation line

1. Document that opening trusts an admitted owner for its continuous custody
   lifetime. Keep health/grants/name lookup/live-lease checks on every open.
2. Remove the rescan and replace scan-dependent tests with zero-IO opening and
   already-poisoned-owner refusal tests. Move explicit corruption detection checks
   to diagnostics/cold admission; keep the full failure-mode coverage.
3. Qualify native custody/partial failure and installed IO, upgrade and rollback,
   and repeat matched small-application size and admission measurements.
4. Reassess the **unpublished** batch API: its scan-amortization justification
   disappears. In this probe, sequentially opening/holding 16 handles costs
   263,666 instructions versus 267,681 batched. Retain batch admission only for a
   demonstrated atomic-group requirement, not for the previous performance claim.
   Likewise reassess the checksum cache now that verification leaves this hot path.

No persisted format reset is implied. The documented lease-time corruption
detection guarantee must nevertheless be changed explicitly and classified under
the repository's pre-1.0 release rules; it must not be silently shipped as an
unchanged guarantee. No default/optional scan mode or compatibility fallback is
proposed. This investigation has not changed the pending release selection.

Evidence: `target/evidence/lease-scan-investigation/` contains before/probe source,
the probe module and installed CSV, expected native-failure logs, and restored
qualification. Production sources and existing pending work are preserved.

## Final retained hard cut: one custody-based opening path

The maintainer authorized removal of the lease-time rescan and reconsideration
of the unpublished API. `open_memory` now checks bootstrap/current admission,
owner health and exclusive lease ownership without backing IO. Cold construction,
initial bootstrap, new binding commitments and explicit `diagnostics()` and
`memory_allocations()` retain complete verification. Native poisoning, logical
bounds, detached-handle custody and IC rollback are unchanged.

Remove the unpublished batch API and its duplicate admission path. Its only
consumers were the new qualification fixtures; no production consumer required
atomic group opening. The size consumer now uses individual opens. Remove the
checksum cache as well: it optimized scans that are no longer on the opening
path and added state/cold-load cost. No compatibility alias, flag, alternate
unchecked API or persisted-image migration is introduced. The ordinary Vector
length-write improvement remains.

Fresh final instruction comparisons, medians of three samples, 32 reserved
buckets; "before" is the previously retained checksum-cache implementation:

| Directory names | Before: one open | Final: one open | ICSS |
| ---: | ---: | ---: | ---: |
| 1 | 277,712 | 162,203 | 160,391 |
| 4 | 300,217 | 163,081 | 160,391 |
| 8 | 329,492 | 163,520 | 160,391 |
| 16 | 388,036 | 164,386 | 160,391 |
| 64 | 735,501 | 165,264 | 160,391 |

Opening/holding all 16 handles now costs 262,052 instructions, versus 2,720,450
for separate verified opens before the cut (ICSS 164,337). There remains overhead
for named admission and exclusive live-lease tracking. Retained-read costs are
unchanged: 160,585 for one eight-byte read and 248,862 for 32; ICSS 160,467 and
244,900 respectively. Cold reopen returns to the uncached control (138,455 for
four names, 697,554 for 64). No general collection-operation speedup is claimed.

| Application Wasm | Previous pending cache/batch | Final | ICSS |
| --- | ---: | ---: | ---: |
| Vector | 97,831 | 96,271 | 64,560 |
| All collections | 137,532 | 136,106 | 119,318 |

The final all-collections application saves 1,426 bytes relative to the previous
pending implementation, and 162 relative to the released-source baseline.
These are application probes, not the broad benchmark harness. Both size changes
and instruction comparisons use the same compiler/dependency selections.

### Safety and qualification

Native checks pass: 89 library tests and five runtime integration tests. New
coverage proves zero backing IO even with a backing configured to fail all IO,
retained custody/exclusivity, and refusal of an already-poisoned owner without
further IO. Raw-corruption detection is explicitly tested through diagnostics,
allocation inspection and cold reopen. Existing full-row corruption and partial
publication coverage is retained; tests do not pretend opening still verifies.

All 864 final Storage and 600 ICSS installed admission samples pass. All eight
small application pairs pass installed calls and two upgrades. The installed
IO, collection/checkpoint, custody, upgrade and rollback suites pass under
original/O3/Os/Oz Wasm in runtime attempt `GlemZe`.

This change exposed an older qualification weakness: the metadata-fault driver
accepted **any** trap, so an assertion about a retired checkpoint could pass as
rollback evidence. The fixture now verifies successful opening after raw fixture
corruption, explicitly checks diagnostics/allocation-inspection rejection,
checks poisoned old/new handles and a poisoned reopen, then traps with a unique
marker. The driver requires that marker before checking state rollback. All six
fault modes pass this stronger proof in all four optimization modes. The earlier
run `cCacwO` is retained but does not qualify this updated poisoning claim.

The changed detection guarantee selects pending **Storage 0.72.0**, carrying the
earlier compatible Vector improvement. No package version, lockfile, dependency,
sibling source or publication changes are included. Stored bytes are unchanged;
hosts needing a deliberate integrity observation use explicit diagnostics.

Evidence: `target/evidence/custody-cut-*`; size attempt `24uMv1`; final runtime
`target/qualification/runtime/attempt.GlemZe/`. The current admission driver drops
the retired batch operation and uses the same one-open/all-held operations for
both libraries. Historical frozen CSVs retain that retired experiment.
