# Ordered storage for IC DB

R4 now has a product collection under `ic_storage::ordered`, enabled by the
`ordered` feature. It is an independently authored AVL tree over bounded byte
keys and values in a separately owned payload arena. It shares the scoped-owner,
allocator-custody and portable-recovery direction. It neither wraps retained std BTreeMap nodes nor
depends on legacy Storage. Host/provider-pool tests and a closed installed
64-row profile now cover compatible upgrades, row/index rollback and portable
recovery between two native windows. DB adoption and the complete R4 gate remain
open; this is a bounded qualification profile, not the reusable application runtime.

## Consumer boundary and supported data

Read-only review on 17 November 2026 used IC DB HEAD
`18ea9abd78d2fdef2293b4a03a9c5484ba4f6efb` plus its current dirty worktree. That
worktree is evidence for current requirements, not a certified released contract.
Relevant consumer interfaces are `src/store_key/mod.rs`, `src/codec.rs`,
`src/schema.rs`, `src/store/mod.rs` and
`src/store/query/aggregate/jobs/storage.rs` in `crates/ic-db`.
The sibling repository was not modified. No legacy or third-party tree
implementation was used as source material; tests were authored here. The host
standard BTreeMap is only an independent behavioral oracle, not a benchmark.

| DB requirement | Storage representation |
| --- | --- |
| Complete encoded primary/composite/secondary key, at most 512 bytes | Arena span with unsigned lexicographic byte ordering |
| Encoded row, at most 4,096 bytes | Independently allocated bounded bytes; exact opaque contents |
| Primary-key references in secondary entries, manifests and entity counters | Same bounded value surface, without padding entering logical output |
| Aggregate job keys and frames: 16 slots plus header, up to 16 KiB per job | Separate `Map<1, 16384>` profile, 17 live entries |
| Forward/reverse ranges and index-prefix scans | Borrowed double-ended range and prefix traversal |
| Entity batches expanded into row/index writes | Preflighted batches of up to 2,048 physical mutations |

DB keeps its existing encoding and validation for signed/unsigned integers through
128 bits, U256, ULID, dates/times/durations, decimals, finite floats, booleans,
principals, accounts/subaccounts, bounded text/blob, optional values and its
declared named/list/set/map shapes. Storage preserves their already encoded
bytes. It does not claim their Rust structs, String/Vec headers or external type
layouts are retained safely. Ordered numeric/composite semantics must already be
represented by the DB key codec; Storage does not reinterpret little-endian
integer payloads as numeric keys or normalize text.

`Map<512,4096>` is the qualified ordinary-row profile, not a hard-coded product
limit. Bounds are const generics. A node now occupies 56 bytes, with separate
key/value spans rather than reserving maximum-sized arrays. A nine-byte key uses
a 16-byte block; a 4-KiB row uses a 4-KiB block; an empty value needs no block.
The 16-KiB job profile does not enlarge ordinary row nodes. Representative churn,
fragmentation and installed scaling remain necessary before DB deployment.

## Native ownership and algorithms

Each `repr(C)` node contains seven u64 fields: left/right/height and two
offset/length spans. Links are one-based node indices; byte offsets address the
separate arena. Empty spans are exactly `(0, 0)`. Nodes contain no pointers,
owning Rust headers or destructors. The supported compiler remains Rust 1.100.0;
layout equivalence across compilers is not assumed. `Bytes<N>` remains an inline
transient copy for returned replaced/deleted values. Reads and iterators return
`ByteSlice` borrows confined to the admitted scope.

The transient map owns `Vec<Node, A>` and `Vec<u8, P>`, root/free indices and live
count. `new_in` uses one allocator authority; `new_in_allocators` accepts distinct
node/byte authorities. Both Vec headers and allocator objects are transient.
`release` returns both initialized allocations for provider settlement. `admit`
requires independently valid live custody, then checks payload bounds, zeroed
slack, disjoint ownership, complete live/free byte coverage and free-list counts,
followed by strict key ordering, unique node reachability, child bounds, exact
heights, AVL balance and complete live/free node coverage. Aliases, cycles, gaps,
orphan blocks and noncanonical free nodes refuse. Failure returns both unchanged
allocations. Admission does not repair bytes or establish pointer validity.

The byte arena begins with 64 `(head, count)` u64 little-endian pairs (1,024
bytes), allocated lazily when the first nonempty span is stored. Each size class
holds powers of two starting at eight bytes. A free block contains its next
offset in the first eight bytes and zeros afterward. Live unused tails are zero.
Allocation takes the smallest available fitting class, splitting larger blocks,
or appends a new block after fallible reserve. Deletion scrubs and returns both
spans to their size classes. Same-class replacement writes in place; a different
class reserves a replacement before retiring the old span. Tree rotations move
only scalar links; successor deletion transfers span ownership without copying rows.

There is no coalescing, in-place compaction or arena shrinking yet. Fragmented
free bytes can therefore fail to satisfy a larger allocation. Caller allocator
budgets bound that failure; portable reconstruction discards the old free-list
shape. A live-entry limit alone does not bound retained bytes as tightly as the
former inline representation. Sustained churn and a fragmentation policy remain
open qualification work, rather than an implied production allocator guarantee.

Lookup, insertion and deletion follow O(log n) tree paths. Deletion scrubs the
removed key/value and puts its node on the free list. Later insertion uses that
slot before growing. Growth can require old/new allocations to coexist, so peak
pool use exceeds retained size. Payload allocation examines at most 64 class
heads and touches the affected bytes; it does not scan unrelated records.
Full structural validation occurs on admission, not on each lookup or scoped
call. Admission sorts block intervals and checks unused bytes, requiring scratch
space proportional to block count. Untrusted admission bounds counts before
scratch allocation and limits recursion before a malformed chain exhausts the stack.

`with` lends a view whose owners and references cannot escape. Ordinary callback
errors keep earlier successful writes. An unwind poisons access/publication even
if the host catches it; only explicit validated checkpoint replacement recovers
that owner. Forgetting a view cannot bypass settlement. IC rollback and
cross-root transaction publication remain runtime obligations.

## Traversal and mutation contracts

Bounds compare exact bytes. Included/excluded/unbounded lower and upper endpoints
are supported; reversed or empty intervals return no entries. Prefix traversal
handles empty prefixes, embedded NUL, all-FF suffixes and the absence of a finite
successor prefix. Iterators are double-ended and fused; each step seeks from the
root in O(log n) without buffering the table or scanning unrelated prefixes.
Mutation is excluded while a range is borrowed.

`range(...).entries()` and `prefix(...).entries()` supply lazy `Entry` handles.
`key()` and `value_len()` inspect the key and scalar metadata; only `value()`
visits the value's byte span. The adapter preserves the remaining positions of
an already partially consumed range and supports `rev()`, mixed front/back calls
and fused exhaustion. It never collects or sorts the table. The original tuple
iterator remains available when both key and value are wanted immediately.
`contains_key` seeks using keys alone. These operations are also exposed through
the public native session's read scopes, allowing primary lookup during secondary
traversal and source/target reads within the same loan.

Between scoped calls, resume with `Excluded(last_key)`. Deleting the cursor key
does not invalidate the position. Keys inserted strictly beyond it can appear;
earlier keys are not replayed. Reverse continuation uses an excluded upper bound.
These are live key continuations, not snapshots or authenticated DB cursor tokens.
DB retains schema/revision/cursor identity policy.

A page must advance its owned logical cursor only after consuming a candidate.
If byte/work preflight rejects a lazy entry, retain the last consumed key; resuming
with that exclusive bound retries the candidate, even if the cursor key was
deleted between calls. Storage supplies traversal, not DB cursor publication or
filter/budget policy. Both directions are tested with full 512-byte keys and
4-KiB values. Test-only instrumentation confirms that preflight, membership and
key-only traversal do not invoke the value accessor; this is not an instruction
benchmark or a hardware memory-access measurement.

`remove_discard` returns only whether the key existed. It performs the same tree
repair, node recycling and payload scrubbing as `remove`, without materializing
an inline `Bytes<V>` result. Scrubbing still touches the retired bytes; this is
not constant-cost deletion of arbitrary payloads. Batch removal uses the discard
path, and its preflight reads old lengths from node metadata. Host tests cover
two-child deletion, absent keys, duplicate batch removals, reuse and readmission
with 16-KiB job values without calling the value accessor.

`apply` preflights all key/value bounds, duplicate-key sequential effects,
intermediate live-count limits, node capacity and a scalar simulation of payload
allocation/reuse before any record mutation. Expected refusal leaves all records
unchanged; an earlier successful reserve may still enlarge spare capacity. The
bounded preflight is O(batch² + batch log n); each accepted mutation then uses tree paths. DB's
32-entity limit is separate: one entity can generate many physical index writes.
The new tests apply 32 rows plus eight secondary entries per row as one 288-write
batch. Uniqueness, references, revision checks and final-state DB policy remain
consumer-owned. Multiple distinct roots are not made atomic by this single-map API.

The public session also exposes `apply(root, mutations)`, with the same 2,048
mutation bound and preflight contract plus the root's declared byte bounds.
Refusal preserves that root's logical records, but preceding writes to another
root still stand on an ordinary callback error. Applications must trap after a
partial commit on the IC; host code can quarantine the entire session.

## DB operation-surface evidence — pending 0.76.2

`make ordered-db-contract-test` runs ordered host tests, scope compile-fail checks
and strict host/Wasm lint. Four new DB-shaped cases live beside the implementation
in `ordered/db_tests.rs`; local output is `target/ordered-db-contract-test.log`.
They cover lazy payload preflight/continuation, all range bounds and prefix edges,
discard cleanup/reuse, and simultaneous main/source/target/job session roots.

The four-root case uses `Map<512, 16384>` carriers with explicit per-root bounds:
512-byte keys and 4-KiB values for main/source/target, one-byte keys and 16-KiB
values for 17 job entries. Compact nodes do not acquire inline 16-KiB payloads;
the wider carrier does not silently relax the three row roots' policy. The test
performs indexed primary lookup, source/target reads, row/index/count/revision
batch refusal and job cleanup without collecting a map.

This addresses concrete operation gaps from
[Storage #4 — IC DB ordered ownership](https://github.com/dragginzgame/ic-storage/issues/4).
Its principal remaining gate is still the supplier-owned retained allocation,
startup and publication binding described in the
[native session contract](native-sessions.md#metadata-lifetime-and-installed-boundary).
These host checks are not installed multi-root rollback, bounded maintenance,
the DB engine's actual witnesses or migration of old retained images. No sibling
code or selected registry dependency was changed for this increment.

## Portable format

`checkpoint(epoch)` produces a complete logical buffer. The first 24 bytes are
`ORDMAP01`, u64 little-endian epoch and u64 little-endian entry count. Each record
contains u32 little-endian key/value lengths followed by exact key and value
bytes. Records must be strictly increasing by byte key; duplicates refuse. A
32-byte SHA-256 digest seals every preceding byte. Unknown magic, wrong epoch,
bad seal, excessive count/length, truncated records, unordered keys and trailing
bytes refuse. Zero keys, zero values and empty maps are valid.

The format contains no node links, heights, padding, free-list order or allocator
state. Its `ORDMAP01` format is unchanged by the compact native representation.
Import builds a private candidate under its supplied allocators and only
returns it after full validation. Narrower byte bounds provide an explicit
conversion that rejects oversized data; semantic row-schema conversion remains
DB-owned. `restore` replaces an existing or poisoned owner only after the entire
candidate succeeds, requiring simultaneous old/candidate allocation capacity.
Failure preserves the original owner and its availability state. Checksums detect
corruption; trusted custody, schema identity and expected epochs remain separate.
This buffer path uses memory proportional to the logical checkpoint. The new
[paged recovery path](ordered-recovery.md) supplies bounded pages, byte-defined
verifier continuations and a host-qualified payload for the shared migration job.
Installed incremental recovery remains an R4 completion requirement.

## Qualification

`make native-core-test` includes thirteen ordered collection tests and lifetime
compile-fail examples, strict host/Wasm Clippy and Rustdoc. Coverage includes
12,000 deterministic mixed operations against the host ordered-map oracle,
4,096-key ascending/descending workloads, deletion/reuse, exact range-bound
matrices, prefix edges, mixed double-ended exhaustion, continuation under writes,
batch refusal, 512-byte keys/4-KiB rows, 16-KiB jobs, source destruction before
recovery, independent golden framing and corruption with recomputed seals.
Additional payload tests cover 2,000 six-mutation variable-size batches against
an independent oracle, splitting/reuse, every byte of the free-list directory,
overlapping spans, invalid offsets, orphan bytes, slack corruption and unchanged
allocations on refusal. The 64-row/index fixture checks its exact initialized size.

`make native-feasibility-test` includes five ordered tests using the existing
provider-pool implementation. They check concrete node size/alignment, real
allocator issuance, descriptor settlement and readmission, free-list reuse,
source-pool destruction followed by portable recovery, allocation refusal before
batch mutation and candidate-failure reclamation without losing either owner.
Node and byte allocator exhaustion are exercised independently; same-class
replacement and deletion/reuse succeed without growing the exhausted byte pool.
Session tests retain both actual loans across 256 row/index insertions and
separate remove/reinsert calls. Neither pool can be finished or reissued while
its owner remains live. After complete close, both backing allocations move to
new addresses and undergo fresh admission. A caught unwind quarantines both
allocations until explicit discard. `Map::allocation_usage` observes initialized
bytes and capacities without scanning records or closing the owner, and refuses
poisoned state.
The product adds no unsafe code. These are host custody proofs, not installed IC
upgrade/rollback or macOS execution evidence.

The next R4 boundary is installed incremental recovery jobs and general application runtime
integration, then qualifying fragmentation and larger measured
row/index workloads. Consumer cutover and legacy-format import remain separately
authorized work. No IC DB API or stored format was changed here.

## Installed ordered-root lifecycle

`make ordered-native-probe` builds five closed compiler artifacts: A, B, recovery,
an omitted-source-close variant and a failing-source-close variant. It admits
memory geometry, initializer ranges, imports and exports, then applies controlled
startup through the same reset builder used by scalar-map owner sessions. It joins CI.
`make ordered-native-test` runs the same admission and installed tests through the
prepared Testkit server. It does not install tools, fetch dependencies or touch DB.
The script captures sources, compiler identity, manifests, lockfiles, LLVM IR,
raw/final Wasm hashes, test output and `costs.tsv` in a unique evidence directory.

The fixture reuses the existing `region::pool::Pool` for both native allocations and
the product `ordered::Map<512,4096>` and `checkpoint::Record<Vec<u8>>` for data.
The map and both pool controllers now remain alive across ordinary messages.
Only initialization, explicit reopening and target startup establish fresh owners;
ordinary reads/writes operate on the existing admitted tree. Before a compatible
upgrade or explicit close, the map releases both allocations, both pools finish
and are dropped, and complete scalar descriptors are published last. While open,
the closed flag is zero and the stale root descriptors cannot be reissued.

Private Vec headers and allocator references live only within the current module
generation. The shared bootstrap clears `[0, 1,572,864)` before any target Rust
entrypoint, then replays admitted compiler data from passive segments. This clears
the old headers without touching the scalar header or either native window. The
target requires a closed source and admits fresh owners from complete descriptors.
A/B share one representation but differ in executable identity; recovery uses the
other native window after explicit whole-heap replacement. No caller-provided
validation flag bypasses structural admission. This remains a closed qualification
service, not a general production runtime API.

The fixed eight-MiB wasm32 memory contract is:

| Region | Meaning |
| --- | --- |
| 0–1,048,576 | Compiler stack |
| 1,048,576–1,572,864 | Compiler data and generation-local owners; cleared and reinitialized by controlled startup |
| 1,572,864–1,572,976 | Fourteen u64 descriptor words: magic, domain, node start/length/capacity, tree root, free head, live count, epoch, freeze flag, payload start/length/capacity and closed flag |
| 2,097,152–4,194,304 | Transient monotonic scratch window; no scratch owner survives an entry |
| 4,194,304–4,208,640 | Native A: 256 nodes × 56 bytes |
| 4,210,688–5,259,264 | Native A: one-MiB byte pool |
| 6,291,456–6,305,792 | Native B: 256 nodes × 56 bytes |
| 6,307,840–7,356,416 | Native B: one-MiB byte pool |

Other bytes are unused reserves. A compiled assertion fixes node size/alignment
to 56/8 for this wasm32 profile. These concrete fields have no padding.
The native header magic is now `0x4f52444e41543033`; the former inline-node
identity and compact per-message publication identity both refuse. The native
publication contract is incompatible with those fixtures, while
logical checkpoint framing remains unchanged. This is not an installed
old-binary-to-new-binary migration claim.
Artifact checks reject unexpected memory growth, initializer overlap, preexisting
start functions, missing linker bounds, extra entrypoints or unapproved system imports.
Nonstandard page sizes, computed data offsets and compiler-owned segment
initialization/drop operations also refuse before transformation.
Tests decode the generated start function and verify its exact clear extent,
bounded passive initialization and final-byte binding. Platform Runtime owns
the actual IC system calls. Source and final artifact identity are qualification
inputs: this geometry screen is not a general verifier for arbitrary Wasm code.

The foreign-memory boundary assumes the fixed IC linear-memory windows are live,
initialized and exclusively owned by the fixture. Node fields admit every Rust
bit pattern; structural/semantic admission occurs before interpreting links or
lengths. Header access, allocator issuance and generation-local static ownership
are the narrow fixture-owned unsafe boundaries. A phase guard excludes overlapping
frames, and admitted imports cannot reenter or suspend. Native references are
issued for the actual fixed generation allocation, never by extending a stack
borrow. The product remains safe Rust. The ephemeral allocator never issues
native addresses, has no memory growth path, and retains no allocation owner
across entrypoints. Its deallocation is deferred to the next entry reset, so its
high-water count includes all aligned allocations in that entry, not just live
requested bytes. Successful entries finish before another can enter; panic aborts
the IC transaction, including partial descriptor writes.

Stable memory is exclusive to one declared checkpoint region, ID 40, reserving
one MiB of payload and 17 stable pages including backing framing. Creation is
fresh-only; independent opening uses the exact product backing contract. Export
stores one complete ordered checkpoint and freezes the source in the same
message. Recovery mode requires a zeroed fresh header, the explicit expected
epoch and valid checkpoint bytes, constructs a private candidate, then keeps its
validated owners live in the target generation. Keeping an old heap cannot silently enter this fresh path;
replacement cannot silently enter compatible reopen. Explicitly bypassing a
corrupt source hook is tested only with an existing checkpoint and an authorized
management upgrade. This is an operator checkpoint path, not a consumed durable
job or automatic rollback/freshness policy for later application writes.

The installed workload has 64 full-size 4-KiB rows and 64 secondary entries in
one ordered collection. It checks full payloads, both index directions, five-item
forward/reverse pages, deletion/reuse at capacity, index-key replacement and
exclusive continuation after a deleted cursor. The diagnostic cursor has a
separate presence flag, preserving `u64::MAX` as an ordinary key. The logical
oracle checks every row and index after each lifecycle transition.

Faults occur between a row and its index, after native mutation before metadata
publication, during explicit/source close, after stable checkpoint write, and
during target construction/publication. Failed updates and upgrades preserve
complete native window bytes, stable memory and generation-owner status; failed
upgrades also preserve installed code. Queries, including an intentional query
write, leave both native data and transient owners unchanged. Explicit close
refuses ordinary access until reopening; double open/close refuses. Omitting
source close refuses target admission and preserves a usable source service.
Compatible A→B→A transitions preserve the native allocation. Two checkpoint
epochs recover A→B→A after actual heap replacement, with intervening writes and
deletion. Missing checkpoints, wrong epochs, corrupt native descriptors and
corrupt logical checkpoint framing refuse without replacement initialization.
The former inline/compact format identities and a truncated byte-allocation descriptor
also refuse; explicit operator recovery from a valid checkpoint remains possible.
Typed row-schema conversion, application authorization and DB cursor identity are
not supplied by this diagnostic endpoint protocol.

### Historical inline-node evidence — 17 November 2026

Rust 1.100.0, Runtime 0.23.17 and Testkit 0.109.0/PocketIC 16.1.0 passed both
artifact tests and both installed suites in
`target/evidence/ordered-native/attempt.g7bftN/`. The server evidence and complete
command log are identified in `target/ordered-native-qualified.log`. Final Wasm
SHA-256 values are:

- A: `e277e4fb6a8a519ad5234c58c0d5a40f7a83aad74cfb8ebce54f9853366e21cd`
- B: `cb21e69671ffc4de2bb985cc46105d21a7c6252c663a0dc94eb38f5e4e7040af`
- Recovery: `ae1d93b79ba4006b236c39343968ad23e8e8f81617184a6b8578fbd0fae60294`

Samples cover the final insertion at 64 rows, first export/recovery, second export
after mutation/deletion and reverse recovery. Each passes the selected measured
body budget of 50 million instructions and one-MiB transient high-water allowance.
Counter intervals include argument decoding, native admission, operation and
descriptor publication; they exclude the initial scratch reset and trailing
counter storage/reply/epilogue. They are **entry-body measurements**, not fully
instrumented whole-entrypoint counts, production throughput or an external-library
comparison. Stable reservation is 1,114,112 bytes; Wasm memory is fixed at
8,388,608 bytes. A native window reserves 1,189,888 bytes; 128 allocated nodes
occupy 594,944 bytes at 64 rows. Geometric growth temporarily needs old/new runs.

### Historical compact per-message evidence — 17 November 2026

The compact native representation passed both artifact tests and both installed
suites in `target/evidence/ordered-native/attempt.uMTutn/`, with server evidence
at `target/evidence/testkit/run.104e5A/evidence` and the full command log at
`target/ordered-compact-installed.log`. Compiler, Runtime and Testkit versions
are unchanged from the preceding cohort. Final Wasm SHA-256 values are:

- A: `7a63d1fae6b75605f4fdf3380d3bf639f595d5b571a5fe08da2c222a8631ab22`
- B: `c64aa6a555e35eafc3a63d954a2006c8cdcdb80a0b18cee019f195e21e017a41`
- Recovery: `b3272ff84eb908c2dd0b5d2789d9a17e74299c267c57faf1753355e06d9b6ddc`

Each native window now covers 1,064,960 bytes, including 16 KiB reserved for
nodes/gap and a one-MiB payload pool. At 64 rows plus indexes, initialized nodes
and payloads total 273,408 bytes, versus the preceding inline allocation's
594,944 bytes (54% less). The actual Vec capacities total 337,920 bytes after
insertion (43% less). These are distinct from reserved provider windows,
transient scratch, growth peaks and the unchanged eight-MiB Wasm memory.
`costs.tsv` reports initialized and allocated native bytes separately.

| Operation | Measured body instructions | Transient high-water bytes | Initialized native bytes | Allocated native bytes |
| --- | ---: | ---: | ---: | ---: |
| Final insert at 64 rows | 1,354,447 | 2,142 | 273,408 | 337,920 |
| First checkpoint | 25,027,169 | 267,224 | 273,408 | 337,920 |
| First replacement recovery | 27,882,839 | 265,048 | 273,408 | 335,872 |
| Second checkpoint, 63 rows | 25,822,434 | 528,132 | 273,408 | 335,872 |
| Reverse recovery, 63 rows | 27,284,274 | 260,908 | 269,152 | 335,872 |

The preceding inline cohort's final insert measured 7,770,734 body instructions.
This comparison includes cold admission and uses the small fixture's specific
shape; it is not a general throughput claim or an ic-stable-structures benchmark.
Full `make ci` passed; `target/ordered-compact-ci.log` records its checks.

That cohort reopened and validated both allocations per message and measures
the cold-admission boundary. It is preserved as historical evidence below the
current generation-owned profile.

### Owner-session evidence — 17 November 2026

Both artifact tests and all three installed suites passed in
`target/evidence/ordered-native/attempt.BWZnQ7/`; server evidence is at
`target/evidence/testkit/run.5xsXWG/evidence`, with the full log at
`target/ordered-session-installed.log`. Compiler/Runtime/Testkit versions are
unchanged. Final Wasm SHA-256 values are:

- A: `b5682b6878e511aae8e1a8d61bab056aa99819b3df52ee7eef85631207495d84`
- B: `9448a293de8a034770c9a9cca8715eaf7dd797f248408b62cf528794abb7ead7`
- Recovery: `8a11477871211a601d4e91d46fdd1d5c24f97f7d55c16d9736554b911fa381b7`
- No close: `b0b97441d31a5f984f80223face9f918973729730990da01665f9a02f35b5d8a`
- Failing close: `c510847311d2622e87f92b60308e06f04d6bdaca21df508b4968e26599f43da0`

The session suite verifies one opening across 32 distinct writes, no status
changes from queries, a second opening only after explicit close, and reset to
one fresh opening after upgrade. The 64-row suite separately checks that all
inserts and isolated lookups leave the opening count at one.

| Operation | Measured body instructions | Transient high-water bytes |
| --- | ---: | ---: |
| Final insert at 64 rows | 944,690 | 0 |
| Lookup key 0 | 563,567 | 0 |
| Lookup key 31 | 482,067 | 0 |
| Lookup key 63 | 643,891 | 0 |
| First checkpoint | 24,777,146 | 265,048 |
| First replacement recovery | 27,880,527 | 265,048 |
| Second checkpoint, 63 rows | 25,488,534 | 525,924 |
| Reverse recovery, 63 rows | 27,281,962 | 260,908 |

Ordinary insert/lookup intervals include request decoding, phase/policy checks
and the operation, ending before reply transmission. Checkpoint and recovery
intervals include their respective work; target startup clearing is outside the
post-upgrade interval. These are body measurements, not isolated tree instruction
counts, whole-lifecycle budgets, general throughput or a stable-structures comparison.
The final insert is about 30% below the compact cold-admission sample. Initialized
native bytes, allocated capacities and fixed provider/Wasm reservations remain
unchanged from that cohort. Lookups use tree paths; no per-message full validation
or transient allocation is hidden behind an unchecked admission shortcut.

The existing scalar-map session also passes installed regression qualification
after sharing its startup builder (`target/ordered-session-shared-startup.log`,
cohort `target/evidence/native-session/attempt.xMAo0p/`). General application
runtime integration, incremental recovery, larger populations, fragmentation,
installed 16-KiB job values and macOS execution remain open.
Full `make ci` passes on the session implementation, including the strengthened
startup refusals; its complete log is `target/ordered-session-ci.log`.

### Public-session installed evidence — pending 0.76.2

The closed ordered adapter now uses public `ordered::session` grants for updates,
read-only lookup, lazy index traversal, checkpoints and allocation observation.
Recovery and close first drop the grant and reclaim the session's map owners;
only then may native restoration or dual-pool settlement proceed. A dedicated
64-KiB region holds generation metadata, independently of per-entry scratch.
The grant, Rc and directory consume 144 bytes, reclaim to zero on close and reuse
that same amount after reopening. Neither ordinary calls nor query exports reset
live metadata. The fixed Wasm reservation remains eight MiB.

Both artifact tests and all three installed suites passed in
`target/evidence/ordered-native/attempt.NMbqQt/`; the log is
`target/ordered-public-session-installed-retry.log`, with server evidence in
`target/evidence/testkit/run.PkZdSG/evidence`. Five fixture variants also pass
strict Wasm Clippy (`target/ordered-public-*-clippy.log`). Final identities:

| Variant | Bytes | SHA-256 |
| --- | ---: | --- |
| A | 86,071 | `e4db4b0354e3641679a785335b21f994a0a6aec990383ee3e87df67d81b35307` |
| B | 86,071 | `d657a91113c623a044ecc9795199c243b8a0e4a025d0885b1bb8e4ef37518af4` |
| Recovery | 86,071 | `d58a609e0719597a52ea75d41a963772ea1253133b2c6461f515cb015b32ac09` |
| Omitted close | 86,040 | `3b4410cc4bf1ddd7059ffc1294888f1d353cf4ba3ec1861abbc72655ade2a268` |
| Failed close | 86,071 | `72d53d71678a3c98d43f8204afd0bfbdfda1fa3473907a7cd98b5e060cd45797` |

The extended session case forces a real allocation refusal while constructing
public metadata, then successfully reopens the original closed native roots.
Eight close/reopen cycles preserve records and metadata usage. A reply encoder
refuses after the public write scope and descriptor publication, and the IC
restores both logical/native state and working session ownership. Existing
query isolation, failed source/target upgrades, corruption refusals and both
portable-recovery epochs still pass. Successful `put` replies now encode the
submitted scalar as eight little-endian bytes in this diagnostic protocol.

| Operation | Measured body instructions | Scratch high-water bytes |
| --- | ---: | ---: |
| Final insert at 64 rows | 1,105,774 | 0 |
| Lookup key 0 | 723,644 | 0 |
| Lookup key 31 | 642,144 | 0 |
| Lookup key 63 | 803,968 | 0 |
| First checkpoint | 24,923,116 | 265,048 |
| First replacement recovery | 28,041,686 | 265,048 |

These complete body samples include public grant checks and exceed the earlier
private-owner samples; they are not a performance improvement claim. Metadata
usage/reservation is separate from the scratch column. Native initialized bytes
and capacities remain 273,408/337,920 at 64 rows. Full costs and the second epoch
are recorded in the cohort's `costs.tsv`. No general benchmark comparison or
consumer physical budget follows from this one-root fixture.

This advances [Storage #4 — IC DB retained ownership](https://github.com/dragginzgame/ic-storage/issues/4).
The native pool, fixed memory geometry and artifact startup remain closed
qualification machinery. At this cohort boundary, a reusable supplier-owned
allocator/bootstrap surface, installed simultaneous main/source/target/job roots,
bounded multi-message maintenance and actual DB adaptation remained required;
the following cohort addresses the simultaneous-root evidence. This fixture is not code
for DB to copy and does not establish migration of old Storage images.

### Four-root installed evidence — pending 0.76.2

The current closed adapter holds four ordered maps through public session grants:

| Root / participant | Entry limit | Key bytes | Value bytes |
| --- | ---: | ---: | ---: |
| Main / 1 | 128 | 512 | 4,096 |
| Source / 2 | 32 | 512 | 4,096 |
| Target / 3 | 32 | 512 | 4,096 |
| Jobs / 4 | 17 | 1 | 16,384 |

Each map uses the same 56-byte node carrier and shared node/payload pools, with
separate public byte bounds. Native format 4 (`0x4f52444e41543034`) binds this
closed declaration roster and allocation geometry. The main scalar header,
three auxiliary descriptors and `ORDROOT4` SHA-256 seal form the retained root
directory. Admission checks the seal and all disjoint spans before reconstructing
any owning vector, then validates each map and its public declaration bounds.
A checksum detects corruption; it does not independently grant allocation custody.
The existing closed artifact/startup review remains necessary.

Close ends all four grants and the session directory, settles every map's node
and payload allocation, finishes both pools and publishes the closed directory
last. Metadata consumes 456 bytes of its separate 64-KiB reservation and returns
to zero on close. Per-entry scratch never reclaims live metadata. Nodes still
have 256 fixed slots; the payload pool grows to 1.5 MiB, giving each disjoint
native window 1,589,248 bytes within unchanged eight-MiB Wasm memory. These are
shared physical limits, not a guarantee that every declared maximum fits at once
or that every growth request succeeds under fragmentation.

Explicit export now stores a complete `ORDFAM04` family: epoch, exact root/
participant/schema/bounds declarations, four nested `ORDMAP01` checkpoints and
an outer SHA-256 seal. Recovery validates the envelope and restores all four
maps before issuing the new public owner. Ordinary writes do not update that
checkpoint. Old native formats, a changed declaration with a valid outer seal,
a corrupt final nested checkpoint, and damaged native directory bytes refuse.
A final-map failure after earlier target maps were reconstructed rolls back the
replacement and preserves the source. This is an intentionally changed fixture
format, with no importer for older experimental native or checkpoint images.

`make ordered-native-test` passes strict host Clippy, strict Wasm Clippy for all
five variants, two artifact tests and all four installed suites. Final cohort:
`target/evidence/ordered-native/attempt.epL6f8/`; complete log:
`target/ordered-family-module-installed.log`; prepared-server evidence:
`target/evidence/testkit/run.ylM7fz/evidence`.

| Variant | Bytes | SHA-256 |
| --- | ---: | --- |
| A | 126,083 | `7bdaf85a954285792e62f40b1f0446d43a85e5d5e86ee344083b7ed4aaae0f84` |
| B | 126,083 | `df7f05b730fbbbb50df085055e4d012c49ae422e9fd4c1ef4f92e3a659f2938f` |
| Recovery | 126,083 | `d0e365c864d8daaa6cfb3813f6c523c5d939e1ede792323f1e041ca427630ac4` |
| Omitted close | 126,052 | `4c9211f5f77c2f881fcc8da53387263f9c46dbb9f7d483591770e2b74e36806b` |
| Failed close | 126,083 | `c89473d8de0e2a431c55276c7ccc16f9e3564d8a9871cc3f378d623452587221` |

The new suite fills seventeen records in each root: 512-byte keys and 4-KiB rows
in main/source/target, plus seventeen 16-KiB jobs. Independent grants reject
foreign-root access. Traps after each composed root write, an oversized main row,
an eighteenth job, partial cleanup and late reply encoding restore full native
window bytes, the scalar directory, working metadata and stable memory. Query
writes leave committed roots and session state unchanged. Repeated close/reopen,
real metadata allocation refusal and a compatible upgrade preserve every root.

Complete-family recovery succeeds in both allocation domains. Per-slot cleanup
removes one target/job pair while preserving main and source; a checkpoint with
empty target/jobs also recovers, and a second cycle reuses all seventeen slots
without changing source rows. This qualifies the storage operations for that
pattern, not DB's maintenance scheduler or durable progress state machine.

| Operation at seventeen jobs | Body instructions | Scratch high-water bytes |
| --- | ---: | ---: |
| Write four roots | 3,042,786 | 0 |
| Complete-family checkpoint | 90,768,302 | 1,028,946 |
| Complete-family recovery | 92,382,706 | 514,625 |

Initialized native storage is 521,576 bytes and reserved capacities total 834,816
bytes for this shape. The metadata allocation is separate. The existing 64-row
main/index suite also passes: initialized/capacity bytes remain 273,408/337,920,
lookup costs remain unchanged, and final insert costs 1,175,376 instructions.
Its complete-family checkpoint/recovery costs increase to 47,172,553/49,706,158
instructions because the envelope copies and seals nested checkpoints as well.
Full samples are in `costs.tsv` and `family-costs.tsv` within the cohort. These
are measured entry-body intervals, not end-to-end budgets or throughput claims.

Checkpoint/recovery remains a bounded one-message path with a one-MiB envelope
and two-MiB scratch reservation. Nested buffers and repeated checkpoint loads
consume scratch together; the declared root maxima are not a qualified combined
export capacity. At this cohort boundary, installed multi-message ordered recovery,
larger sustained-load and fragmentation evidence, reusable supplier-owned
allocator/bootstrap APIs and actual DB adaptation remained open for
[Storage #4 — IC DB retained ownership](https://github.com/dragginzgame/ic-storage/issues/4).
The subsequent [paged installed extension](ordered-recovery.md#installed-four-root-page-transactions)
now qualifies one-page four-root import transactions, compatible interruption,
explicit fresh restart and exact retries. It advances the native identity to
format 5 for private candidate policy and expands the fixed checkpoint namespace;
the complete-buffer cohorts above retain their original artifact/measurement scope.
