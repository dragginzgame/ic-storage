# M0 retained layout, version 1

The [current architecture](../DESIGN.md) selects native retained collections and
explicit migration paths. This document continues to own the implemented M0
byte contract as a regression/evidence baseline; it is not the selected native
heap representation. No stored M0 bytes or admission rules change with that decision.

Status: frozen experimental specification, 15 November 2026. The library's
`m0::inspect` implements image admission and `m0::image` implements native-buffer
allocation and scoped mutation against this contract. A fixture-only live-memory
kernel now has local installed graph/upgrade evidence; production integration
is not implemented.
It is distinct from the scalar startup/growth fixture and does not admit that
fixture's bytes. Changes to this contract require a new layout identity and
explicit compatibility evidence; unknown layouts are rejected, never reset.

All integers are unsigned little-endian. Offsets below are bytes. No Rust struct
layout, pointers, padding, type names or executable addresses enter this format.

## Address space and metadata partition

There is one nonshared wasm32 memory with 64 KiB pages, initial minimum 129 and
maximum 256 pages. The volatile reserve is `[0, 8388608)`. The metadata page is
`[8388608, 8454144)`; the arena begins at 8454144 and ends no later than 16777216.
The arena cap is therefore 8323072 bytes. Offsets in the following tables are
relative to the metadata page unless stated otherwise.

| Half-open range | Meaning |
| --- | --- |
| 0–256 | Header |
| 256–512 | Two 128-byte schema descriptors |
| 512–576 | One 64-byte root entry |
| 576–1024 | Reserved, all zero |
| 1024–33792 | 1024 allocation-index entries, 32 bytes each |
| 33792–65536 | Reserved, all zero |

The ranges cover exactly 65536 bytes without overlap. Metadata cannot expand
into the arena. Every unlisted header/descriptor byte and every inactive index
entry is zero. Limits and offsets come from the module's admitted contract;
retained fields must equal them rather than redefine them.

## Header

| Offset | Width | Required value or meaning |
| --- | --- | --- |
| 0 | 8 | Magic bytes `ICOPM0\0\0` |
| 8 | 4 | Format version: 1 |
| 12 | 4 | Header bytes: 256 |
| 16 | 16 | Layout ID: ASCII `op.m0.layout.v01` |
| 32 | 8 | Volatile end: 8388608 |
| 40 | 8 | Metadata bytes: 65536 |
| 48 | 8 | Absolute arena base: 8454144 |
| 56 | 8 | Arena capacity limit: 8323072 |
| 64 | 8 | Committed arena bytes: physical memory bytes minus arena base |
| 72 | 8 | Append cursor: arena-relative end of the last indexed object |
| 80 | 8 | Next allocation identity: active count plus one |
| 88 | 4 | Active allocation-index count, 0 through 1024 |
| 92 | 4 | Index capacity: 1024 |
| 96 | 4 | Descriptor offset: 256 |
| 100 | 4 | Descriptor count: 2 |
| 104 | 4 | Descriptor stride: 128 |
| 108 | 4 | Root offset: 512 |
| 112 | 4 | Root count: 1 |
| 116 | 4 | Root stride: 64 |
| 120 | 4 | Index offset: 1024 |
| 124 | 4 | Index stride: 32 |
| 128 | 4 | Integrity algorithm ID: 1 (SHA-256 below) |
| 132 | 4 | Flags: zero |
| 136 | 24 | Reserved, zero |
| 160 | 32 | Metadata digest |
| 192 | 64 | Reserved, zero |

Committed arena bytes must be a multiple of 65536, must exactly match physical
memory, and cannot exceed the cap. Cursor is aligned to eight, at most committed
arena bytes, and recomputed from the index. Zero objects require cursor zero and
next identity one. Growth changes committed extent, not cursor or identities.

## Exact schemas and root

IDs are literal 16-byte values, not hashes. `CounterState` uses ASCII
`op.m0.counter.v1`; `Node` uses ASCII `op.m0.node.v1` followed by three zero bytes.
Descriptors appear in that order and must match every expected byte.

Each 128-byte descriptor has schema ID at 0 (16 bytes), payload length at 16
(u32), alignment at 20 (u32, exactly 8), field count at 24 (u32), zero at 28
(u32), then three 32-byte field slots starting at 32. An unused slot is all zero.
Each field slot has field ID, payload offset, kind and width (four u32s), then
16 bytes for a reference's target schema; scalar targets are all zero.
Kind 1 is u64 (width 8); kind 2 is nullable object reference (width 16).

| Schema / payload bytes | Field ID | Payload offset | Kind | Target |
| --- | --- | --- | --- | --- |
| CounterState / 40 | 1 | 0 | u64 | Zero |
| CounterState / 40 | 2 | 8 | Reference | Node |
| CounterState / 40 | 3 | 24 | Reference | Node |
| Node / 24 | 1 | 0 | u64 | Zero |
| Node / 24 | 2 | 8 | Reference | Node |

The root entry has root ID at 0 (ASCII `op.m0.root.v1` plus three zeros), target
schema at 16 (`CounterState`), reference at 32 (16 bytes), flags at 48 (u32 zero)
and twelve reserved zero bytes at 52. A null root is valid during explicit graph
construction; application access reports a missing root until it is published.
It never causes automatic fresh initialization during upgrade.

## Objects, allocation index and references

Each active 32-byte index entry contains an arena-relative object-header offset
at 0 (u64), allocation identity at 8 (u64), complete object span at 16 (u64), and
eight reserved zero bytes at 24. Entries are in allocation order, contiguous
with no holes: entry zero begins at arena offset zero; each later offset is the
previous offset plus span. Entry `i` has identity `i + 1`. There is no free,
reuse, compaction or wraparound. Reaching 1024 entries refuses before mutation.

The 48-byte object header contains identity at 0 (u64), schema ID at 8 (16 bytes),
payload length at 24 (u64), flags at 32 (u32 zero) and twelve reserved zero bytes
at 36. The payload follows immediately. Both schema payload lengths are already
eight-aligned, so complete spans are exactly 88 bytes for CounterState and 72
for Node, with no extra padding. Headers are immutable after publication.

A reference is `(arena_relative_header_offset: u64, allocation_identity: u64)`.
Only `(0, 0)` is null. `(0, 1)` is the first object, not null. Nonzero offset with
identity zero is invalid. A non-null reference must exactly match an active
index entry, object header and field/root target schema; merely pointing inside
an object or inside physical memory is insufficient. References may share a
target or form cycles. All indexed objects, including unreachable ones, are
validated; bounded index and field scans do not recursively traverse cycles.

## Integrity and mutation coverage

The metadata digest is SHA-256 of the concatenation of:

1. ASCII `ic-storage-new/m0/metadata/v1` followed by one zero byte.
2. All 65536 metadata bytes, with offsets 160 through 191 replaced by zero.

Digest bytes are the standard 32-byte SHA-256 output, without integer endian
conversion. Coverage includes descriptors, root, counters, committed extent,
every active/inactive index entry and reserved bytes. Recompute after every
successful metadata mutation: fresh initialization, allocation publication,
memory growth and root publication. It does not cover arena object bytes or
ordinary scalar/reference payload changes; full representation checks remain
mandatory. An unkeyed digest detects corruption, not unauthorized modification,
module authenticity or application correctness.

Each operation validates spans, arithmetic, types and capacity before writing.
Only fresh installation creates the metadata page. Allocation writes a complete
header and initialized payload, then its index entry, count/cursor/next identity
and updated digest before returning. Growth publishes the new committed extent
and digest only after successful `memory.grow`; refusal preserves old metadata.
The root is published only after its target is valid. No host call or await may
occur between these steps. Unexpected faults after mutation begins trap on the
IC; a native model must poison access unless its harness restores a snapshot.
Ordinary errors do not undo earlier successful operations in the same message.

## Admission order and first graph vector

First check module-owned bounds and physical memory length, then read the fixed
metadata page. Validate constants/reserved bytes and digest, then exact schemas,
count/extent/cursor arithmetic and every index/header/payload span. Finally
validate all declared reference fields and the root against the admitted index.
No typed handle or writable capability is exposed before the complete scan
succeeds. Unknown flags, kinds, algorithms, IDs, invalid references or mismatched
metadata/physical extents refuse without changing retained bytes.

The canonical empty state has 129 physical pages, zero committed arena bytes,
zero cursor/count, next identity one, a null root, exact descriptors, and a
computed digest. Its metadata digest is
`944e725d1226789882410df6dbff62649034171bc5bdace98d8d72732049a5fa`;
SHA-256 of the complete page with that digest stored is
`62820b481cd4519884d2283fef29a53abb4cc0b49516c9a63cb5077300125a33`.
These are specification vectors, independently assembled from the tables, not
runtime qualification. The independently assembled page is retained as a
[test fixture](../../crates/ic-storage-new/src/m0/tests/fixtures/empty-metadata.bin).

For the first graph, grow one arena page, allocate Node at
offset 0 with identity 1 (span 72), then CounterState at offset 72 with identity
2 (span 88). Cursor is 160; next identity is 3. Node's value is 7 and its next
reference is `(0, 1)`. CounterState holds counter 41 and both references `(0, 1)`.
The root is `(72, 2)`. A later Node allocation begins at offset 160 with identity
3, leaving cursor 232. These values exercise offset-zero identity, sharing,
cycles and allocator continuation without a collection library.

The read-only inspector's native tests admit this graph and the canonical empty
image, and reject corrupted/resealed metadata, headers and references. It also
accepts a 1024-object forward cycle without recursive traversal and rejects
unreachable corrupt objects. The native mutation API reproduces the independently
assembled graph bytes, preserves sharing/cycles after reopening, and continues
allocation across a page boundary after explicit growth. Resource refusals leave
bytes and identities unchanged. Compile-fail examples reject escaping handles,
session-capturing futures and cross-image handle use; a runnable example exercises
valid access. Native fault injection checks poison after an interrupted write or
failed post-write admission, including when the callback catches the failure.

The image owner borrows only caller-provided retained capacity, beginning at
linear address 8 MiB, and tracks its physical prefix. The executing stack and
transient data must never be included in that slice. `inspect_retained` uses the
same admission rules and absolute error offsets as full-snapshot `inspect`;
growth clears only newly committed pages. Fresh initialization is explicit and
refuses nonzero retained bytes; reopening never initializes. Full admission runs
at session entry, after each write and at session exit. During exclusive custody,
reads reuse the last admitted summary and retain individual handle/schema and
reference checks. Every successful write refreshes the summary only after full
admission; write faults prevent its reuse. No cached permission survives the
session or an upgrade. This changes no persisted bytes or admission rules. Callback
errors preserve completed operations; panics and unexpected write faults withhold
further access without restoring bytes. Reopening after dropping the owner is
explicit readmission and does not restore business invariants.

The separate [managed graph qualification](../../qualification/ic-storage-rust-startup-probe/managed-graph.md)
now exercises this library through a private live-memory kernel. Its local
PocketIC cohort preserves full retained bytes across A→B→A, continues allocation
across a physical page boundary and rolls back selected trapped updates/upgrades.
Native tests alone do not supply that evidence. Neither result covers every
publication boundary, committed-corruption case or production custody obligation.
