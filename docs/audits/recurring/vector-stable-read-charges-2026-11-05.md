# Vector stable-read charge diagnosis — 2026-11-05

The last-record regressions come from stable-memory read dispatch and source
alignment. They are not unexplained noise. Removing checksums changed the
addresses of the length prefixes and payloads, exposing a conditional IC fast
path that the old power-of-two population samples happened to favor.

Historical diagnosis over pre-0.68 Vector images. It changed no production code
or format. The copied-ICSS reset later replaced those collection layouts; the IC
dispatch explanation is distinct from a current-layout performance claim.

## Verified mechanism

PocketIC 16.1.0's [release metadata](https://github.com/dfinity/pocketic/releases/tag/16.1.0)
names IC commit `e9e42a6dd74acb9049017863e944cd5799be5b2b`.
Its [stable64-read replacement](https://github.com/dfinity/ic/blob/e9e42a6dd74acb9049017863e944cd5799be5b2b/rs/embedders/src/wasm_utils/system_api_replacements.rs#L627)
selects the first-access helper only when **every requested 4 KiB stable source
granule is untouched in that execution**. Otherwise it uses native memory copy
for the complete requested range. The
[host helper](https://github.com/dfinity/ic/blob/e9e42a6dd74acb9049017863e944cd5799be5b2b/rs/embedders/src/wasmtime_embedder/system_api/stable_memory.rs#L30)
reads through the stable-memory buffer. The native path enters the
[deterministic tracker](https://github.com/dfinity/ic/blob/e9e42a6dd74acb9049017863e944cd5799be5b2b/rs/memory_tracker/src/deterministic.rs#L333),
which charges a newly accessed 64 KiB region. The installed measurements below
observe 80,000 instructions per such native region; heap destination charges
are kept outside these per-read counters.

A variable-length Vector get first reads its length prefix, then its payload.
If those occupy the same 4 KiB granule, that prefix read disables the payload's
bypass. If the payload also crosses a 64 KiB boundary, native copy can charge
two regions. If the prefix ends immediately before a 4 KiB-aligned payload,
the payload's granules remain untouched and its first read bypasses that charge.

## Actual original images, not assumed addresses

The [host diagnostic](https://github.com/dragginzgame/ic-storage/blob/3c4734d9bd413b8a902f23458ebdc222255cd9e7/qualification/ic-storage-benchmarks/benches/stable_read_probe/main.rs)
installs each original Wasm from the checksummed baseline `attempt.c6YVkt` and
compact candidate `attempt.xIWOoI`. It fully populates 512/1,024/2,048 rows with
256/2,048-byte inner payloads through their original governed setup endpoints.
It captures the stable image and locates each unique complete Wire envelope.
It checks the preceding length field, then passively upgrades to the
[probe](https://github.com/dragginzgame/ic-storage/blob/3c4734d9bd413b8a902f23458ebdc222255cd9e7/qualification/ic-storage-benchmarks/benches/stable_read_probe/wasm.rs)
without resetting stable memory.

The probe performs no collection access. It fills every heap destination with
nonzero bytes before the counters, then times only individual stable64 reads.
Prefix/payload/repeat and combined-prefix-plus-payload/repeat are separate fresh
query executions. Every returned byte is checked against the actual captured
stable image. Thus these measurements isolate source-read charging without
assuming the original Vector's heap placement or translation-cache state.

The extended matrix includes neighboring last indices and the compact layout's
favorable index 2,015 with both neighbors. All **96 queries / 240 system calls**
pass byte validation. An independent source-granule/native-region replay fits
every observed call exactly:

`measured instructions = payload bytes + 220 fixture/transfer overhead + 80,000 × newly natively accessed stable regions`.

The 220 residual is specific to this probe and counter boundary; it is not a
general Vector or endpoint overhead. Derived path/region columns are model
predictions checked against measured costs, not a server fault-event export.

## The 1,024-row, 2 KiB last-record regression

The outer stored envelope is 2,054 bytes. Its actual physical locations are:

| Layout | Prefix start | Payload start | Payload start modulo 4 KiB | 64 KiB regions intersected |
| --- | ---: | ---: | ---: | ---: |
| Checksummed | 4,403,188 | 4,403,200 | 0 | 1 |
| Compact | 4,388,926 | 4,388,928 | 2,112 | 2 |

The old 12-byte prefix ends just before a fresh source granule. The compact
two-byte prefix shares the payload's first granule, and the payload extends
70 bytes beyond its first 64 KiB region.

| Measured system call | Checksummed image | Compact image |
| --- | ---: | ---: |
| Prefix read | 232 | 222 |
| Payload read | 2,274 | 162,274 |
| Repeat payload read | 82,274 | 2,274 |

The payload regression is **exactly 160,000 instructions**. The original whole
Get changes by 142,988 instructions (25,613 → 168,601): that is 160,000 extra
source charging offset by 17,012 fewer other instructions in this matched cell.
At 256 bytes the new payload occupies one native region, producing the 80k step.
At population 2,048 the new last-record payload also occupies one region.
These source arrangements account for all eight previously regressing Get cells.

The alignment advantage reverses at index 2,015 in the 2,048-row image: the old
2 KiB payload read costs 82,274 while the compact one costs 2,274. Its compact
payload begins at physical offset 8,454,144, a 64 KiB boundary, with its length
field in the preceding region. Both neighboring indices are included as controls.
Power-of-two populations plus first/middle/last positions are insufficient to
establish a general winner for this layout.

## Why WarmGet can mislead

In the old last-record case, Get costs 25,613 but WarmGet costs 105,613. The
untimed pre-read takes the bypass and marks the payload granules as read without
paying native admission. The timed second get then pays the deferred 80k.
In the compact case, the untimed pre-read already pays 160k, leaving the timed
get at 8,601. WarmGet is therefore a **pre-read experiment**, not proof that all
relevant charges were paid or that the reported difference is ordinary CPU work.
Include the complete sequence when judging an execution's total cost.

## What this suggests, and what it does not prove

A combined compact-prefix-plus-payload read at the same physical offset costs
2,276 instructions in the probe; its subsequent repeated payload read costs
162,274. The first combined read preserves the untouched-granule bypass. This
demonstrates a mechanism for a single cold read, not a free charge reduction for
all subsequent reads in the same execution.

The next bounded experiment is reading a complete small slot once and parsing
its length from the returned bytes. A real get does not know the payload length
before reading the prefix, so it must fetch padding or introduce a different
representation to coalesce the reads. Extra copying, heap capacity/charges,
large declared bounds, neighboring accesses and repeated reads must be measured
before selecting that strategy. No new threshold, format, cache or public API
is selected here. Reintroducing the checksum is unnecessary to explain the result.

This diagnosis does not qualify Canic configuration preparation, caller-index
lookup, Auth's inner decoder or whole authorization throughput.

## Evidence and reproduction

See the [probe command](../../../qualification/ic-storage-benchmarks/README.md#stable-read-alignment-diagnostic).
Original inputs, source-bound IC files, two diagnostic runs and raw returned
bytes remain under `target/vector-first-access-study/`. The initial run contains
48 queries / 120 reads; the extended run contains 96 queries / 240 reads.

The authorized cleanup retired the three per-run CSV/JSON exports from docs.
The tables above retain selected absolute observations. Original local artifacts
under `target/vector-first-access-study/` were left untouched; explicit cargo
clean may remove them. No durable raw archive or independent verification from
a digest alone is promised. The probe source and reproduction command remain.

The largest legal combined reply passes its native Wire budget test. Native
driver and Wasm probe lint pass. Both lock selections and all product source
remain unchanged by this diagnosis. No release or publication was performed.
