# Closed installed native map contract

This profile executes the actual `ic-storage-native::map::Map` implementation
against the private shared [pool allocator](provider-region.md#multiple-root-host-pool)
and a foreign WAT provider. Two independent maps occupy disjoint runs in one
exclusive loan. It extends the [product map evidence](../../docs/design/native-map.md)
to compatible installed IC upgrades, custody and joint publication rollback.
It is a bounded runtime qualification fixture, not an application Store API.

## Scope and memory authority

The contract uses Rust 1.100.0, wasm32-unknown-unknown, panic=abort, one codegen
unit, optimization level 3 and fat LTO. The collector builds the product `map`
feature with the locked offline graph and records source, compiler identity and
artifacts. No upstream implementation is copied.

The provider owns one exclusive live aligned allocation at 131072, initially
covering 1024 Slots: 24576 bytes. Between loans it may double that extent through
2048, 4096 and 8192 Slots (192 KiB). Each Slot is three initialized native u64 words in
`repr(C)` order: occupied, key and value. The entrypoint statically checks size
24 and alignment 8. Product admission checks tags, count, unique keys and probe
placement. Fresh install initializes the initial allocation; explicit growth
initializes only its new tail. Post-upgrade never zeroes unknown retained data.
Compact fixture methods pack two u32 operands into their wrapper argument.
The native entry receives separate u64 key/value arguments; the governed recovery
profile additionally exercises their full width on IC.

Memory starts with three pages and has a five-page maximum, separating the Rust
stack below 32768, metadata at 65536 and native Slots at 131072. The ten retained
u64 header words are magic
`NMAP0001`, format 4, base, extent, root A, root B, settlement, stride 24,
alignment 8 and hash/probe version 1. Each root packs count in bits 0..11,
log2(bucket count) in 12..15 and slot offset in 16..31. Higher bits must be zero.
The exponent must be in 3..=12. Full-width checks precede narrowing, and exact
page count must agree with the admitted extent. Buckets range from 8 to 4096;
count is at most
75% of buckets, both spans are in bounds and they do not overlap. Initial roots
have either 8 buckets each or 512 each. Older experimental formats are refused;
there is no stored-format compatibility promise.

Numeric checks do not create allocation provenance. The foreign claim import
loans the actual allocation after checking phase and settled custody. One
transient Pool reserves both root spans, then freshly issues each exact run
through Allocator::allocate without payload writes. Vec::from_raw_parts_in uses
that issuance; Map::admit validates each complete table. The Pool owns run
isolation, with at most 32 transient run records including growth candidates and
sealed roots. No old Rust owner, reference, allocator object or private Vec
header crosses the entrypoint boundary. The provider supplies initialization
for the whole loan; the allocator and product collection govern which values
are live and may be read.

The provider passes the current Slot extent in the upper 16 operation bits;
the lower 16 retain the closed selector. Rust admits only 1024, 2048, 4096 or
8192 Slots and validates both roots against that complete loan. This extends the
foreign allocation contract rather than making a numeric extent sufficient
authority. The root format change is deliberately incompatible with format 3.

Claim changes settled to claimed before returning the pointer. Both maps stay
owned during operations. Growth uses the product's ordinary reserve/insert path:
it allocates a disjoint candidate, builds it while the source stays live, then
retires the source. Freed runs become reusable without moving the other map.
Rust releases both actual Vec owners, seals their runs and finishes the Pool
before returning both packed roots in one u64. The wrapper validates them, writes
A, writes B, then settles. An unexpected failure traps the whole IC message.
Ordinary callback errors still finalize prior writes; forgetting a scoped View
does not skip release. Both source pre-upgrade and target post-upgrade reopen
and validate both maps before accepting retained state.

Claim now issues transient loan authority after validating the provider header,
root geometry and operation. Private Wasm globals bind the admitted end address,
both original root descriptors, physical page count and operation. Native code
cannot write those globals or the provider metadata. Each payload access checks
phase, custody, operation identity and bounds against this admitted loan instead
of rechecking every header field and both root descriptors. Staging callbacks
touch only their disjoint staging/cursor state and cannot reenter Rust.

Settlement revalidates the header and checks that the original roots, extent,
page count and operation still match the loan before publishing new roots. It
revokes loan authority before returning provider custody. Bounds never expand
merely because metadata or physical memory changes. Claim refuses an outstanding
loan; phase manipulation alone cannot issue one. Traps roll back authority and
payload together. This authority lasts for one native call, not across upgrades
or ordinary owner reconstruction; Map::admit still validates table contents.

Explicit fixture reset retires the selected owner before allocating a fresh
8-slot table. That permits reuse even when both initial 512-slot roots fill the
pool. Reset is an authorized destructive fixture operation; any failure during
its construction or publication traps and restores the prior message state.
It is never used as a fallback for failed admission.

Each root can represent up to 3072 records in 4096 buckets. Rehash needs a
contiguous candidate while both sources remain live; total capacity and
fragmentation can therefore refuse growth earlier. Deletion permits insertion
without growing. Filling a table uses one scoped native batch, with admission
once at the boundary, rather than reopening both maps for each record.

## Provider capacity growth

Only the idle provider may expand the pool. It first reopens, validates and
releases both maps, and requires settled custody with no active operation.
After Rust returns there are no live Vec owners, references, Pool controllers or
derived pointers. The provider can then call memory.grow and establish a larger
foreign allocation for the next loan. Under the supported wasm32 contract,
growth preserves existing linear-memory bytes; any engine-side relocation is
outside Rust and no earlier Rust pointer survives it. Fresh claim/issuance derives
all subsequent pointers anew. There is no in-loan reallocation or retained
allocator object whose address must remain valid.

Expansion from 24 to 48 KiB uses space in the existing third page. Expansion to
96 KiB adds a fourth page; 192 KiB adds a fifth. The provider initializes exactly
the new tail, publishes the larger extent, then independently reopens both maps
under a fresh loan. If memory.grow returns -1, it returns ordinary refusal
without publishing capacity or changing old bytes. Unexpected failures trap the
whole IC message. The 8192-Slot ceiling also refuses normally. A separate
three-page-maximum profile exercises an actual memory.grow refusal.

Compatible upgrade requires the admitted extent and exact page count to agree;
it does not perform growth automatically. A separate fault target expands during
post-upgrade then traps to test rollback. All growth is explicit and bounded;
freeing a map makes slots reusable but never shrinks the pool or physical pages.
This is a provider capacity mechanism, not a general elastic allocator or a
replacement for consumer recovery authority.

## Compiler and access qualification

The raw module has two `native_map_diagnostic` imports: `claim_region` returns the
exclusive pool pointer, and `staged_word(i32, i32) -> i64` reads a scalar or
acknowledges a completed operation. The latter is a trapping stub outside the
paged-recovery and batch-comparison compositions. There are nine defined
functions and a private five-argument scalar entry at function 8. Key and value
are separate u64 arguments; the compact fixture wrapper unpacks its u32 pairs.
Three i32
globals initialize to 32768. There are no data/start/table sections, indirect
calls, Rust-side memory growth or retained std headers. Its raw fixed memory
declaration is replaced by the reviewed provider's memory in exact composition;
only provider WAT can grow it. Scalar/vector profiles
reject this artifact; the map profile checks its own exact signatures, exports
and compiler shape.

Map-only stack analysis admits the emitted empty-result loops and integer
arithmetic. The call graph remains acyclic. Frame locals are immutable after
entry, all non-trapping exits restore their frame, and branches cannot bypass
restoration via an implicit function return. Only pure packing operations are
allowed after restoration; memory accesses and calls there refuse. Loops repeat
fixed frames without accumulating them. Peak simultaneous stack is 2688 bytes,
from 30080 through 32768, with 26 direct call edges. This is a space proof, not a
time bound. Counterexamples preserve older profiles' refusal of the added
operators and reject recursion, frame mutation, function-label escapes, memory
growth and effects after restoration.

All raw linear-memory accesses are rewritten to private provider guards:
170 i32 reads, 13 byte reads, 42 i64 reads, 82 i32 writes, 11 byte writes,
57 i64 writes and 17 copies. This compiler artifact emits no memory.fill;
its reserved fill helper traps. Copies include transient allocator bookkeeping.
Every emitted access must be accounted for after rewriting.

Guards allow the proven stack window or the complete exclusive payload loan
while phase=claimed, settlement=0 and the closed map/operation selector is valid.
The payload endpoint follows the admitted extent, never all currently available
linear memory. Rust-side memory.grow still fails artifact admission.
The closed selector now also admits command 12: reserve at most 64 additional
records, returning the unchanged root on allocation refusal. The paged recovery
wrapper uses it to end both native owners before deciding whether to grow and
retry; it does not grant native Rust authority to grow backing memory.
Command 13 processes 1–64 staged records in one admitted scope. Its value argument
selects insert-only (0), lookup with an expected value (1), replacement of an
existing key (2), or deletion with an expected value (3). Unknown modes refuse;
unexpected existence/absence or value disagreement traps the complete batch.
Each operation must finish before its acknowledgement. The two map owners and
the pool remain live until all records finish, then publish once. Paged recovery
continues to use only mode 0, refusing replacement of existing records.
The paged wrapper binds that callback
to complete staging and claimed custody; it cannot enter Rust or access the
loan. Native guards still exclude provider metadata, staging and unrelated memory. Offset/end arithmetic
checks wrap, and copies validate both complete spans before writing. Per-run
isolation and initialized-value access are obligations of Pool and Map, not a
second retained bitmap or guard-side allocator. A failpoint traps after an actual
candidate write outside both source spans. Guards execute outside LLVM, never
reenter Rust and do not grant pointer validity independently of the loan contract.

The linker resolves all native/guard imports, leaves only two IC reply imports
and hides native/helper exports. Final admission recomputes the exact composition
from the freshly admitted raw artifact and selected provider profile. Extra
exports or changed bytes refuse. Guard counterexamples add test-only methods to
the same provider; those artifacts fail normal final admission and are not
admitted profile candidates.

The [two-way comparison](../../docs/design/native-map-comparison.md#scoped-loan-and-batch-reuse-17-november-2026)
records the loan/batch optimization and current performance limits. Its installed
counterexamples fail after several mutations through an acknowledgement trap,
a wrong key or a forbidden metadata write, verify complete rollback and then
retry successfully. Standalone guard probes additionally reject forged phase,
metadata writes, capacity/root/operation substitution, wrong custody and reuse
after revocation. The compiler still proves the same stack bound; the additional
defined function is the compiler's factored removal path; imports and the native
entry function signature are unchanged.

## Installed checks and results

`make native-map-probe` runs artifact/stack checks without a server and joins CI.
`make native-map-test` uses the separately prepared Testkit server without
downloads and explicitly deletes its PocketIC instance after completion.

On 16 November 2026, cohort
`target/evidence/native-map-provider/attempt.92dP1f/` passed three artifact/stack
checks and the installed lifecycle test (8.85 seconds). It covers independent
and joint growth, lookup/replacement/deletion/reset/reuse, ordinary errors,
forgotten views, compatible upgrades in both executable directions, all six
capacity doublings for A and five for B, full-pool refusal and later reuse.
A separate initial 512+512 profile checks every one of 768 records and compatible
reopening. Both small maps also grow together in a single successful message.
The capacity-growth sequence reaches a 4096-slot map with 3072 records alongside
a 2048-slot map with 1536 records. Every logical entry is checked, both upgrade
directions preserve all bytes, and deletion/reinsertion remains usable.

Pool expansion injects traps after page growth, tail initialization, extent
publication and fresh native reopening. Each boundary is exercised in ordinary
updates; a post-upgrade expansion also traps after reopening. Observation covers
page count, header and all physical bytes from the payload base to memory end,
including space outside the old loan. Test-only canaries prove successful growth
initializes precisely the new tail, leaves still-unloaned bytes untouched and
restores both on failure. Dynamic end/copy guards are tested at every extent.
Growth while claimed refuses. A three-page maximum forces an actual memory.grow
failure, while the normal five-page profile checks the explicit pool ceiling.
Both refusals leave maps usable; freeing them does not reduce page count.

The fill operation uses one scoped native batch. The earlier per-record reopen
fixture exceeded the 40-billion-instruction message limit at larger sizes
(`attempt.AjXd2c`); its failed cohort is preserved. This is a correction to the
load fixture's access pattern, not removal of boundary admission or an installed
throughput result. Ordinary scopes still validate both complete tables.

Failure points cover an actual candidate write, release of A while B is still
owned, completed native release, publication of A only, publication of both roots,
completed settlement and a trap after normal completion. Reset fails after native
release and after the first root write. A failing upgrade mutates both maps
before trapping, including growth when capacity permits. Every failed message
compares the entire header, physical payload and page count; failed upgrades also preserve
source executable identity. Successful individual growth preserves the other
root and its complete allocated span, all source records and retired-source bytes.

Eighteen metadata/slot faults are exercised with and without the source hook,
independently checking target refusal. They include format, full-width root
encoding, capacity, extent, overlap, settlement, representation and logical-count
errors in either map. Guard counterexamples test both stack edges, both payload
edges, metadata exclusion, offset wrap, complete copy source/destination bounds,
unavailable fills, invalid selectors, unclaimed/settled access and overlapping
publication. Valid accesses and subsequent map operations still work.

The focused host regression passed 33 provider-region tests (one opt-in firehose
ignored), with strict Clippy. The shared scalar/vector artifact regression also
passed all 19 cases in `target/evidence/provider-link/attempt.3LLfOy/`.
The earlier format-2 two-bank cohort `attempt.XEuxtJ` and fixed format-3 pool
cohort `attempt.bMQt38` remain historical evidence, not supported formats in the
current implementation.

These results do not establish million-key IC throughput, adversarial hash
resilience, arbitrary record values, a production root catalog or cross-compiler
migration. The host pool separately qualifies three maps exceeding one million
records. A separate [governed recovery profile](native-map-recovery.md) now
qualifies two maps with at most 64 records each through whole-heap replacement.
It uses the same Map and Pool under its own fixed-domain protocol; it does not
yet recover the larger growing-pool workload. Larger recovery budgets, durable
job governance and consumer-facing custody remain integration work.
