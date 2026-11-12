# Independent allocation backing specification

This original section is the restricted implementation handoff. Parent has seen
previous source and owns acceptance/integration, not implementation. Implementer
may read only this specification, supplied declarations, repository policy and
manifests, and its newly written files. Do not inspect previous source/tests,
diffs/history, registry implementation, sibling repositories or source-exposed
inventory/design documents. Record actual inputs. The maintainer authorizes a
new format requiring fresh/reset backing: reject every incompatible image,
without implicit initialization, conversion, fallback or repair.

## Required responsibility

Own durable named-allocation metadata and translate admitted logical byte ranges
to backing memory. Existing runtime owns grants, current admission, exclusive
leases and physical-backing custody. Do not duplicate those policies or introduce
a public allocation API. Existing leases may outlive the parent runtime.

An allocation has an immutable validated name, durable opaque ID and logical
page length, initially zero. Binding a set adds only missing names, retains
omitted names/IDs/bytes, and never reuses IDs. Names are not numeric slots and can
exceed 256. Existing private callers use one-based entry indexes as IDs; preserve
that integration representation. Binding input order is deterministic. New binding
or positive successful growth advances generation once; no-op binding, zero grow,
reopen and diagnostics do not advance it.

Resource policy is `StorageConfig { allocations: usize, extents: usize, pages: u64 }`:
retained name count, total eight-page backing allocation units, and physical page
ceiling including metadata. Defaults are 4096, 16384 and 1048576. A Wasm page is
65,536 bytes. Each allocation reserves in eight-page increments; its unused
reservation is inaccessible until logical growth. Never reclaim or share an
allocation's reserved capacity with another allocation. Each newly admitted
logical byte is zero until written. Interleaved growth must preserve all existing
logical addresses and neighbors. Allocation geometry is a product constraint;
persisted metadata layout, indexing and algorithms are yours to design.

The created image records its selected capacities. Reopen may select tighter
live-usage limits but cannot increase the image capacities. Refuse additions or
growth before effects when budgets, arithmetic or capacity would be exceeded.
Physical growth refusal must perform no metadata write, retain usable owner
state and remain retryable. A zero grow returns the previous logical length.

## IO, integrity and failure contracts

Creation is allowed only for empty backing; opening is read-only and never
initializes or fixes nonempty bytes. Use a distinct new discriminator/version,
validate all metadata before trusting it, and reject unexplained physical tails.
Validate count/offset arithmetic before allocation or IO. Reject duplicate names,
invalid IDs, overlapping physical ownership, impossible logical coverage,
corrupt active controls and unsupported framing. Use the existing FNV helper
for accidental-corruption checks if needed; no new hash/dependency is authorized.
Control checks are not authenticity or a cryptographic security boundary.

Whole-directory integrity checks belong at open, explicit verification/
diagnostics and new binding commitment. Ordinary admitted IO, size, lease opens
and growth must not scan all metadata. Repeated lookup must not decode persisted
names. Do not initialize or scan unused capacity. Large live metadata must be
read with bounded scratch rather than one equally large temporary byte buffer.
Choose a compact authoritative representation; no second transactional journal.

Data IO validates the complete logical range before any backing operation,
including overflow and zero-length accesses. Empty access at logical end is
allowed; beyond end is not. A range spanning physical discontinuities must read
or write exactly the requested bytes, preserving neighbors. Support safe reads,
writes and initializing raw reads. Favor cheap repeated scalar IO and contiguous
bulk IO without scanning cold allocations. Any cache must be bounded, private,
correct after growth, and excluded from authority decisions.

A backing IO panic poisons the current shared owner before subsequent IO or
leases can proceed. Explicit verification failure also poisons it. Preflight
refusals need not poison. Native metadata publication interrupted after effects
must not reopen as a valid partially published directory; reject it unless the
complete new state was persisted. IC traps roll back the execution, including
growth, so do not add a Wasm journal. Native arbitrary payload writes are not
transactions; owner poisoning does not reconstruct partially written payloads.
Do not claim that every completed final write followed by a host panic must be
unrecoverable. Only valid fully persisted directory states may reopen.

Diagnostics are read-only, integrity-checked and report real capacity. Reserve
one page in accounting for the protected root. For every valid image:
`physical_pages = 1 + metadata_pages + data_pages + unassigned_pages`.
`data_pages` is summed logical page capacity; `unassigned_pages` includes reserved
but unadmitted payload capacity. `directory_bytes` reports actual active controls;
reserved metadata capacity is separately counted. Do not invent free metadata.

## Integration declarations

Provided types, available without reading implementations:

- `super::{AllocationId, Error, StableKey, StorageConfig}`. AllocationId is a
  tuple newtype over u64; this descendant module may construct/access `.0`.
- StableKey: Clone, Debug, Eq, Ord; `parse(impl AsRef<str>) -> Result<Self, _>`,
  `as_str(&self) -> &str`. Validated names are at most 128 UTF-8 bytes.
- Error variants: `ArithmeticOverflow`, `Budget`, `Invalid(&'static str)`,
  `AlreadyOpen`, `Bounds`, `AlreadyClaimed`, `Missing`, `Growth`, `Poisoned`.
- `crate::Memory`: size() -> u64 pages; grow(u64) -> i64 (previous or -1);
  read(u64, &mut [u8]); write(u64, &[u8]); unsafe read_unsafe(u64, *mut u8, usize).
  Successful growth appends zero bytes. Raw read fully initializes a caller-owned
  nonoverlapping destination on success, may initialize only a prefix on panic.
- `crate::hash::{FNV_OFFSET: u64, fnv64(seed: u64, bytes: &[u8]) -> u64}`.
- `crate::StorageDiagnostics`: generation:u64, allocations:usize,
  admitted_allocations:usize, live_leases:usize, extents:usize, data_pages:u64,
  metadata_pages:u64, directory_bytes:u64, physical_pages:u64,
  unassigned_pages:u64. Set admitted/live fields to zero; runtime fills them.

Replace the implementation behind this exact private surface:

```rust
pub(super) struct Entry { pub key: StableKey, pub pages: u64 /* private fields allowed */ }
pub struct Store<M: Memory> { pub(super) entries: Vec<Entry> /* private fields */ }
impl<M: Memory> Store<M> {
    pub(super) fn create(memory: M, budget: StorageConfig) -> Result<Self, Error>;
    pub(super) fn open(memory: M, budget: StorageConfig) -> Result<Self, Error>;
    pub(super) fn allocation_id(&self, key: &StableKey) -> Option<AllocationId>;
    pub(super) fn healthy(&self) -> Result<(), Error>;
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poison(&self);
    pub(super) fn generation(&self) -> u64;
    pub(super) fn verify(&self) -> Result<(), Error>;
    pub(super) fn bind(&mut self, keys: &std::collections::BTreeSet<StableKey>) -> Result<(), Error>;
    pub(super) fn grow(&mut self, index: usize, pages: u64) -> Result<u64, Error>;
    pub(super) fn pages(&self, index: usize) -> Result<u64, Error>;
    pub(super) fn diagnostics(&self) -> Result<crate::StorageDiagnostics, Error>;
    pub(super) fn read_data(&self, id: AllocationId, offset:u64, bytes:&mut [u8]) -> Result<(),Error>;
    pub(super) unsafe fn read_data_unsafe(&self,id:AllocationId,offset:u64,dst:*mut u8,count:usize) -> Result<(),Error>;
    pub(super) fn write_data(&self,id:AllocationId,offset:u64,bytes:&[u8]) -> Result<(),Error>;
}
```

## Work boundary and acceptance

Write only `crates/ic-storage/src/runtime/directory_model/persistence/mod.rs`
(overwrite without reading old contents) and new private submodules/tests under
its `persistence/` directory with fresh names. Do not read, overwrite or reference
the existing tests.rs; parent will retire old-layout tests and retain meaningful
public coverage. Use ordinary module discovery, no path overrides. Do not run
Cargo, format, Git or tool installation. Parent owns caller integration.

Before writing, propose your independently selected format and algorithms with
memory/cost/failure tradeoffs. Avoid large speculative frameworks or unrelated
public changes. Parent will qualify mixed named growth/read/write/reopen, limits,
missing/current admission, custody, lease release, faults at metadata publication,
malformed controls, initializing reads, installed rollback/upgrade and old-image
rejection. Measure application Wasm and representative ICSS comparisons across
all five supported collection forms. New format is not a deployment request.

## Implementation record — 2026-11-10

The fresh implementation agent read only this specification, AGENTS.md,
TOKO-TOOLING.md, root/package Cargo.toml, rules/contributions.md,
rules/engineering-details.md, docs/principles/rust-code-hygiene.md and
rules/hashing.md. It subsequently read its own new source and compiler diagnostics
for that source. The source-exposed parent supplied accepted public key grammar,
clarified that larger reopen budgets must clamp to saved capacities, and requested
bounded metadata reads. No previous implementation, tests, history, registry or
sibling source was supplied. The parent owns acceptance, integration and mechanical
Wasm-only lint corrections. This records a restricted implementation process, not
a whole-library independence or legal certification.

### Selected representation

STORNAM1 version 1 has a 96-byte active root, 168-byte name/size rows and 24-byte
physical-ownership rows. The root records saved capacities, active counts,
generation, geometry and active-row digest. Each ownership checksum binds its
physical index as well as owner and logical index. New growth rewrites the
complete affected allocation row. FNV checks detect accidental corruption, not
malicious rewriting with recomputed checksums.

Names are owned once, indexed by a sorted vector. Admitted per-allocation unit
vectors translate IO directly, coalescing adjacent requested physical units.
There is no translation cache or second authority registry. Opening and explicit
verification read active controls through a fixed 4,032-byte buffer; ordinary
IO/growth and handle opening perform no directory rescan. Verification compares
controls with the admitted ownership, not just their internal checksums.

Native publication invalidates the root before control writes and publishes the
complete root last. Panics poison the current owner; reopen accepts only complete
state or refuses without writes. IC relies on execution rollback and omits the
interruption marker. Growth refusal occurs before publication and is retryable.
Saved capacities never expand during reopen, even with a larger host budget.

Defaults reserve 17 metadata pages plus one root page: 18 pages / 1,179,648 bytes,
versus the former 15 pages / 983,040 bytes. The extra 192 KiB is real stable-memory
reservation, not compiled Wasm or payload. Eight-page data units remain the
product-selected geometry. The whole-backing format cut requires explicit fresh
or reset backing; there is no converter or old-image reader.

### Qualification and size evidence

All 134 selected native tests pass: 100 library, four public backing, 14 Cell,
five public runtime and 11 Vector cases. Strict native/Wasm Clippy passes, as do
28 comparison tests (three explicitly installed tests ignored in the native run).
The extended native runner and Wasm IO fixture also pass strict Clippy.
Installed original/O3/Os/Oz IO, storage, logging, custody, upgrade and rollback
pass at `target/qualification/runtime/attempt.QTT1Eo`. This includes 12 direct
physical-boundary traps and 12 governed-boundary traps, plus all-collection
upgrade across 1,031 names. The frozen old-Map consumer's upgrade to STORNAM1
traps, its old value/application survive rollback, and a fresh install works.
No compatible production migration is claimed.

Small matched consumer evidence is
`target/evidence/storage-consumer-size/attempt.lA2psO`; every pair passes calls and
two compatible upgrades. The previous Storage baseline is `attempt.bb266n`.
Values below are raw module bytes / code-section payload bytes (section ID 10,
excluding its section header), not gzip transport sizes. All builds use the
existing `wasm-size` profile; these are separate applications, not the broad
benchmark harness.

| Consumer | Previous Storage | New Storage | External ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Map | 120,743 / 103,738 | 125,524 / 107,983 | 90,208 / 49,244 |
| Set | 120,510 / 103,507 | 125,299 / 107,752 | 90,008 / 48,993 |
| Vector | 94,704 / 81,332 | 99,790 / 85,798 | 64,560 / 27,627 |
| Cell | 91,923 / 79,079 | 97,513 / 84,162 | 63,135 / 26,115 |
| Region | 121,976 / 104,903 | 126,607 / 109,010 | 89,933 / 49,072 |
| Map + region | 124,626 / 107,467 | 129,249 / 111,567 | 110,858 / 69,759 |
| All | 136,246 / 117,325 | 141,342 / 121,771 | 119,318 / 77,127 |

Individual application growth is 4,623–5,590 bytes, within the maintainer's
5–10 KB budget. This is independence work with an explicit size cost, not a Wasm
optimization. The broad benchmark harness grows 18,919 raw bytes (11,328,495 to
11,347,414) and 16,243 code bytes (9,642,487 to 9,658,730); its many instantiated
type cases are not the application-size benchmark. Default stable reservation
also grows by 192 KiB as recorded above.

The first candidate added a physical-size query to every IC backing operation.
Matched tests exposed substantial regressions; that candidate and its evidence
remain under `candidate-*`. The independent Memory implementer refined nonempty
IO to rely on the IC's checked syscall. The first refinement failed installed
empty-range admission and is retained only as refused evidence. The final version
checks empty ranges explicitly. No failed candidate is used as the benchmark
baseline or described as qualified.

### Final matched instruction results and limitations

A final independent refinement admits the full range, then directly dispatches
the original buffer when one physical span covers it. Empty requests return only
after admission; fragmented requests reuse the first translation and process the
remaining spans. No cache or persisted format change is introduced. New tests
cover zero-page/empty endpoints, fragmented raw initialization and exact backing
call counts. The implementer used its own source and measured workload feedback.

The retained `direct-*` results contain 752 matched summary rows: 376 at 256
records and 376 at 4,096 records, spanning Map, Set, Vector, Cell and regions.
They use the existing depth workload, three rounds and one sample per round.
The external ICSS counters match the frozen baseline exactly for every row.
There are no instruction regressions from this final refinement against the
preceding bounds-qualified candidate; it costs another 344–385 application
bytes. Its gains do not eliminate the replacement's regression against previous
Storage. Do not describe the new backing as performance parity or a speedup.

Selected uniform Get batches of 32, with u64 keys/values (Set uses unit; Cell
uses a 32-byte value), are inner-loop metered instructions including relevant
page charges, not whole-handler/application throughput:

| Population | Collection | Previous Storage | New Storage | ICSS 0.7.2 |
| --- | --- | ---: | ---: | ---: |
| 256 | Map | 458,294 | 543,270 | 538,219 |
| 256 | Set | 442,831 | 520,786 | 526,280 |
| 256 | Vector | 100,178 | 103,285 | 102,932 |
| 256 | Cell | 3,152 | 3,152 | 3,152 |
| 256 | Region | 524,279 | 607,993 | 562,935 |
| 4,096 | Map | 802,789 | 934,522 | 914,982 |
| 4,096 | Set | 706,433 | 830,455 | 826,957 |
| 4,096 | Region | 886,088 | 1,020,606 | 933,864 |

Vector and Cell selected counters are unchanged between these populations.
Across each collection's complete summary rows (including controls), median
old-to-new changes at 256 / 4,096 records are Map +4.55% / +2.50%,
Set +3.74% / +3.34%, Vector +3.10% / +3.10%, Cell 0% / 0%, and regions
+5.10% / +3.27%. These medians do not conceal the larger selected batch
regressions above and are not a weighted application score. The complete joined
rows are `target/evidence/backing-replacement/comparison-final.csv`.

Remaining performance work: distinguish translation CPU work from changes in
charged heap/stable working sets. Some differences near 80,000 instructions
suggest a page-charge sensitivity, but no direct profile establishes that cause.
The replacement is safety-qualified pending work, not a performance-qualified
release. Do not ship it as an optimization or silently accept these regressions
as evidence that independence is free. The ordered engine and shared copied
helpers also still require independent replacement before whole-repository
clearance.

### Reproduction

Use the existing prepared tools, pinned Rust 1.99.1 and both locked offline
graphs. Build `ic-storage-benchmarks`' `storage-comparison-wasm` example in
`qualification/Cargo.toml`, feature `owned`, target `wasm32-unknown-unknown`,
profile `comparison`, with `RUSTFLAGS='-C link-arg=-zstack-size=8388608'`. Freeze
that module before changing source. The external ICSS module is built separately
with feature `dfinity`; neither graph includes the other production engine.
Run the native benchmark driver through the checked `ic-testkit-server run`,
passing explicit owned/reference artifact paths, a new output directory and
`--workload depth --samples 1`. The workload's retained configuration files own
the populations, seeds, execution mode and batches. This focused suite does not
claim to rerun the full 73-type catalogue at this stage.

Run `make benchmark-storage-size` for all small app pairs and `make test-runtime`
for original/O3/Os/Oz lifecycle checks. For old-image rejection, run the ignored
`incompatible_backing_upgrade_traps_and_preserves_previous_application` library
test through the same prepared server with `BACKING_PREVIOUS_WASM` and
`BACKING_REPLACEMENT_WASM` selecting the frozen old/new Map-only consumers.
The test verifies refusal, the previous app's continued operation and a fresh
new install. Preserve raw logs, source archives and hashes. The size run stores
complete reproducible source inputs independently of Cargo's build cache.

For code-section counts, parse each raw Wasm's sections after the eight-byte
header: read the section ID and unsigned LEB128 payload length, record the length
for ID 10, then skip that many bytes. Report full file length separately.
`wasm-sizes-direct.csv` retains all three compared artifacts' values. Current
formatting, Toko snapshot, production graph, license and link checks also pass;
no dependencies, lockfiles, siblings, Git commits or releases were changed.
