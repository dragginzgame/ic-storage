# Governed two-map checkpoint recovery

This bounded profile connects the actual product `Map`, the shared provider Pool
and the existing governed Storage checkpoint adapter. It exports two independently
identified maps, replaces the entire Wasm heap, and rebuilds both through product
map insertion in a different allocation domain. It is the map counterpart of the
[scalar/vector bridge](../ic-storage-retention-probe/native-checkpoint.md), not a
second collection implementation or a production migration coordinator.

## Portable envelope and custody

The [codec](src/checkpoint/maps/mod.rs) wraps two unchanged product `NKEYMAP1`
checkpoints in one complete envelope. Each map contains at most 64 u64 keys and
u64 values. Keys and values retain all 64 bits; native slot tags, bucket counts,
probe positions, offsets, pointers and allocator headers are never imported.

| Offset | Encoding |
| --- | --- |
| 0 | Eight-byte magic `NKPAIR01` |
| 8 | Little-endian u64 envelope version, exactly 1 |
| 16 | Little-endian u64 expected source epoch |
| 24 | Little-endian u64 identity of first unique map root, exactly 1 |
| 32 | Little-endian u64 identity of second unique map root, exactly 2 |
| 40 | Little-endian u64 first product checkpoint length |
| 48 | Little-endian u64 second product checkpoint length |
| 56 | Eight-byte completeness marker `SEALED01` |
| 64 | First product checkpoint, then second product checkpoint |
| End minus 32 | SHA-256 of all preceding envelope bytes |

Each inner checkpoint independently includes its epoch, count, complete logical
records and digest. The outer digest binds ordering, identities and framing;
validation requires exact lengths with no trailing data. Maximum complete size
is 2256 bytes. The decoder invokes `Map::from_checkpoint` for both parts, so the
product codec owns duplicate-key, count, epoch and entry validation. It returns
two fresh transient candidates only after both succeed. Allocation failure or
invalid input publishes neither root. The digest detects corruption; it does
not authenticate an untrusted checkpoint or establish freshness by itself.

The existing provider opens `native.checkpoint.maps.v1` under owner
`native-checkpoint` and prefix `native.checkpoint.` using governed MemoryRuntime,
AllocationGrants, MemoryAuthority and StableCell handles. The reader remains
published `ic-storage` 0.74.1, aliased as `ic-storage-legacy`; its repository is
unchanged. Every open reconstructs runtime/lease ownership. Recovery refuses a
missing locator and never creates one as a fallback. The target requires an
explicit expected epoch.

## Runtime domains and sequencing

This profile uses the same pinned compiler, product Slot representation, Pool
issuance and guarded native entry as the [installed map](native-map-provider.md).
The native entry now receives separate u64 key and value arguments; existing
compact fixture commands still unpack their u32 operands in the WAT wrapper.
Native Rust never owns checkpoint runtime objects.

The recovery profile has its own native protocol version 5. It retains a fixed
24 KiB pool at either 131072 or 163840, two nonoverlapping alternatives ending
below 196608. Metadata remains at 65536; a bounded scalar staging area occupies
66048..68096. Rust native stack remains below 32768. The checkpoint runtime uses
its separately admitted stack below 65536, data from 196608 and a transient Global
heap above its data. The complete module starts with 32 pages and has a 256-page
maximum. Checkpoint allocator growth is confined to that transient domain.
Provider pool-growth methods refuse in this profile. The separate growing-pool
profile and its 4608-record workload are not covered by this bounded recovery.

No Rust stack frames or owners from one runtime may survive entry into the other:

1. The export wrapper admits both native maps through the product implementation,
   releases both owners, then freezes the provider. Ordinary map updates refuse.
2. Checkpoint Rust reads copied logical scalars through WAT-only callbacks. These
   callbacks cannot call native Rust. It reconstructs bounded transient maps,
   exports them through the product codec, and publishes the single bundle through
   the governed StableCell. A trap rolls back both freezing and the stable write.
3. Explicit recovery uses management **Replace** mode. Before any native layout
   access, the target requires a zero native header and fresh lifecycle state.
   A Keep upgrade into the recovery entry refuses rather than overwriting data.
4. Checkpoint Rust opens the existing locator and validates the whole bundle and
   both maps. It stages copied scalar records through checked WAT callbacks;
   those callbacks do not initialize a native Map or invoke either Rust runtime.
5. After checkpoint Rust returns and all its owners end, the wrapper initializes
   the fresh native loan and inserts staged records through the actual Map API.
   Both logical counts must match before post-upgrade completes. Any unexpected
   construction/publication failure traps the complete upgrade.

Compatible reopening admits native state before rebuilding the discarded
checkpoint stack/data/heap with the existing transient reset plan. Frozen roots
remain frozen across compatible upgrades. Neither private Rust headers nor a
Global allocator image are treated as retained objects.

The scalar reader uses one transient forward cursor. `map_count(root)` starts or
restarts that root; `map_word(root, index, field)` requires consecutive indices,
each key (field 0) immediately followed by its value (field 1). Switching roots
requires another count call. Unstarted, wrong-root, skipped, repeated, invalid-field
and past-end reads trap. Every call rechecks readable custody and root bounds.
The key read advances through buckets once; the value read reuses that occupied
slot. A complete read visits at most the root's bucket count instead of rescanning
from bucket zero for every scalar. No cursor is stored in the checkpoint or native
header; compatible upgrade and retry reconstruct it. Logical checkpoint bytes,
native formats and recovery limits are unchanged.

If native metadata is corrupted after a valid independent export, ordinary source
admission refuses. An explicitly selected recovery operation can skip the source
hook and Replace the heap. It still validates the complete checkpoint and expected
epoch; it neither reads nor drops the corrupt old native objects. This is an
explicit salvage path, never an automatic reset on admission failure.

## Qualification and limits

The sequential reader passed installed recovery in
`target/evidence/native-map-provider/attempt.m3VZJj/` on 16 November 2026
(three artifact checks, one installed matrix, 3.18 seconds). Independent native
slot decoding verifies copied record order, all value bits and the bucket-visit
bound. The 64+64-record case visited 126+125 buckets; the previous per-scalar
rescan would visit 9,232+8,986 on that same layout (a calculated baseline, not a
wall-clock comparison). Empty maps visit none. Deleting 62 entries from each
map leaves two records in each 128-bucket table; both sparse checkpoints recover
correctly. Invalid callback sequences trap without changing native/stable state,
and a new count restarts a partial read. Compatible frozen reopening preserves
the same observations. Host codec tests, provider/integration Clippy and formatting
also passed. The [job profile](native-map-jobs.md#qualification-evidence) exercises
the same reader on unfinished import prefixes.

`make native-map-checkpoint-test` builds the dedicated `native-map` provider
feature with locked offline inputs, uses the prepared local PocketIC server,
and records compiler/source/artifact evidence. Final admission reconstructs the
exact native/provider/checkpoint composition; changed bytes or a different
reopen/recovery mode refuse. Only IC imports and public canister methods remain;
native, bridge and scalar callback exports are hidden.

Host cases cover empty/full maps, all-width values, independent mutation after
source destruction, every truncated prefix, trailing data, framing and root
identities, outer/inner digest failure, wrong inner/outer epochs, duplicate keys
with repaired digests and the 64-record ceiling.

On 16 November 2026, `target/evidence/native-map-provider/attempt.RnN98a/`
passed the two host codec tests, three raw-map artifact checks and installed
recovery matrix (2.78 seconds). It covers empty and 64+64-record export/recovery, full-width boundary values,
compatible reopening, frozen update refusal, over-limit export refusal, repeated
replacement in both allocation directions, lookup/deletion/insertion after
recovery, absent/corrupt checkpoints and epoch refusal. Failure injection spans
export after stable publication, recovery after a stable write, scalar staging,
actual native construction and completed publication. Failures compare native
bytes and stable memory; failed upgrades also preserve executable identity.
Discarded allocation-domain bytes are checked as zero after heap replacement.
Explicit source-hook bypass recovers from corrupt native metadata and still
refuses a wrong epoch without changing the source or checkpoint.

The final cohort uses locked Platform Runtime 0.23.14. Cached source comparison
against 0.23.13
found an additive synchronous replicated-execution observation, unused by this
bridge; its existing raw system calls are unchanged. Optional macro/guard pins
also advanced but are not enabled here. The earlier 0.23.13 cohort `attempt.e9BcFT`
also passed the recovery matrix. The growing-pool regression passed in
`attempt.92dP1f`; shared scalar/vector artifact and installed checkpoint regressions
passed 19 and four cases respectively in
`target/evidence/provider-link/attempt.3LLfOy/`. Strict codec/provider Clippy,
all-features provider Clippy, formatting and dependency-policy checks passed.

This is bounded planned recovery evidence, not a real cross-compiler migration,
a durable multi-message import job, checkpoint authentication or production
operator authorization. The separate [durable map-job profile](native-map-jobs.md)
adds consumption, cancellation and frozen resumption under its own locator/native
identity; this bare checkpoint profile does not gain those rules. A management caller
can explicitly restore an older checkpoint; this fixture supplies no general
replay prevention. Consumer cutover requires that governance and a recovery budget
for the consumer's actual state. Larger grown pools need a matching export/import
workload before claiming their installed recovery is qualified.
