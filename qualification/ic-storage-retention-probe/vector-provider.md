# Vector descriptor custody and payload obligations

This independently authored WAT experiment supplies the next provider protocol
for the [native vector controller](../ic-storage-native-feasibility/provider-region.md).
It executes no Rust. Its metadata checks do not establish Rust allocation validity
or complete the compiled vector's payload-address admission.

The later [record-element profile](record-vector-provider.md) reuses these custody
transitions with explicit V3 schema metadata and initialized-record admission.
Its WAT-only evidence remains separate from the installed native u64 vector.

## Retained representation

The fixture has exactly three fixed wasm32 pages: transient reply scratch in
page zero, metadata in page one, and a 4096-byte provider region beginning at
131072. There are no data segments, start function, tables or memory growth.
The scalar provider's original format and regression cases remain unchanged.
The vector uses a distinct marker; it cannot silently reopen a scalar image.

All header fields are explicitly written little-endian u64 words:

| Offset | Field | Admission |
| --- | --- | --- |
| 65536 | Marker | `0x4e5650524f563031` |
| 65544 | Format | 1 |
| 65552 | Payload base | 131072 |
| 65560 | Backing extent, bytes | 4096 |
| 65568 | Initialized element count | `length <= capacity` |
| 65576 | Allocator capacity, elements | `capacity <= 512` |
| 65584 | Settled custody | Exactly 1 before claim or upgrade |

Unsigned comparisons operate on the entire u64 words before multiplication,
wasm32 narrowing or any payload access. Zero capacity therefore requires zero
length; zero length with nonzero capacity is valid. A source with values such as
`2^32 + 3` must refuse, even though narrowing would make its length look small.
These fields describe one root at offset zero, not arbitrary allocation records.

Fresh installation explicitly zeros the backing region and writes initial scalar
values before publishing the header. An upgrade never initializes or resets it.
Afterward, trusted writers preserve the initialized prefix. Valid numeric fields
do not prove initialization, authenticate source history or authorize arbitrary
writers. Final native admission must bind the actual source and target artifacts.

## Claim and publication

Claim requires the ready transient phase, complete header admission and settled
custody. It clears the retained settlement word, marks the phase claimed and only
then supplies the region base. No query, callback or asynchronous work can join
that claim. Source pre-upgrade and destination admission independently refuse an
outstanding claim; resetting transient globals does not recover abandoned custody.

Settlement requires a claimed phase and an unset settlement word, rechecks the
static header and validates the returned length/capacity. It then writes both
descriptor fields, publishes settlement and returns to ready. Its trusted caller
must have ended every payload access and owner first. WAT tests emulate this
ordering; the future Rust entrypoint must enforce it after Rust returns.

The fixture exercises append, overlapping removal, truncation/shrink and clear
followed by reuse. Clearing changes the logical descriptor without clearing the
backing. None of these operations returns physical memory to the IC. Capacity
failure and invalid settlement trap; the IC transaction restores metadata and
payload together. A future business Err must not be mistaken for rollback.

## Source-level payload audit for the Rust candidate

Let `B` denote the claimed base, `L` the admitted initialized length and `C` the
capacity. Before reconstruction, `0 <= L <= C <= 512`; fresh allocator issuance
must cover exactly `8*C` bytes while the provider loan covers 4096. The current
Rust candidate uses only u64 elements and the public raw-parts constructor.
The following are obligations for its final artifact, not an execution receipt:

| Candidate operation | Required payload access and resulting prefix |
| --- | --- |
| Reconstruct | Issue without clearing; only `[B, B+8*L)` is initialized |
| Mutate first | Access `[B, B+8)` only when `L > 0` |
| Append | Reserve first; refuse at the region limit; initialize the word at `B+8*L` before publishing length `L+1` |
| Remove first | When nonempty, read the removed word and move `[B+8, B+8*L)` to `[B, B+8*(L-1))` with overlap support; publish `L-1` |
| Shrink | Keep the initialized prefix and reduce issued capacity; no payload relocation in this controller |
| Clear/reuse | Publish zero length before any later initialization claim; a replacement vector initializes its new first word |

Empty Vec headers may contain a dangling pointer but may not dereference it.
Spare capacity must never be read as initialized elements. Scalar wrapping
addition does not change address bounds. The pinned Rust 1.100 Vec source was
reviewed for reconstruction, reserve, remove, shrink and clear; its public
[reconstruction](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.from_raw_parts_in)
and [removal](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.remove)
contracts describe the relevant caller obligations and behavior.

The future link wrapper must take length/capacity from the same admitted header,
not caller-supplied arguments, and keep the raw Rust function private. It may
decode and publish the returned packed descriptor only after the Rust frame and
its owners have ended. The provider must remain non-reentrant throughout.

The compiled candidate still needs its helper pointers and actual load/store/copy
sites bound to these ranges, alongside the existing transient-stack check. A
successful WAT protocol test or matching source/artifact digest cannot substitute
for that argument or the foreign allocation contract. No native vector has yet
been installed, and the independent portable recovery gate remains required.
The [checked-access experiment](vector-access.md) now supplies WAT range helpers
and an unexecuted compiler rewrite covering every current memory operation. Final
linkage, source identity and the native allocation argument remain separate gates.

## Installed evidence

The new case is selected by `make retention-test` and the existing hosted CI
retention step; it uses the prepared server and locked/offline dependencies.
Strict Clippy uses `make retention-lint`. No new package, dependency or test-server
setup is introduced.

On 15 November 2026 the focused vector metadata test passed on x86_64 Linux with
Rust 1.100.0 and the admitted PocketIC 16.1.0 server. It exercises A→B→A retaining
upgrades, continued writes, empty and full regions, abandoned claims, settlement
errors and traps after payload/descriptor mutation. Thirteen committed header
faults include oversized words and invalid geometry/format/settlement. Each
corrupt source omits its pre-upgrade guard, independently exercising destination
refusal. Failures preserve the installed module hash where relevant, all seven
header words and all 4096 payload bytes; this is stronger than observing only
the logical initialized prefix, but is not a comparison of all Wasm memory.

Raw A/B and negative Wasm artifacts are retained by digest under
`target/evidence/provider-custody/`; the passing log, source copies and compiler
identity are under `target/evidence/vector-metadata/attempt.XQwCyz/`.
A and B are each 1480 bytes, with SHA-256:

- A: `5cca7f6d3d9b842f84b6ceb89770dc6d26709d0ab53ab22ff30288bab9cb94c5`
- B: `2f02f7ababa54df119c74cdf299ea58b50ae41eb3dc363b16e042814dd9d13f3`

These are local installed WAT results, not native Rust, production-subnet or
hosted macOS execution claims.
