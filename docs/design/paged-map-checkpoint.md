# Sequential paged map checkpoints

The product `ic_storage_native::map::paged` codec provides a portable checkpoint
for one u64-key/u64-value map. A 128-byte manifest commits to a forward chain of
1,104-byte pages, each containing at most 64 records. A sequential reader verifies
one page using its expected digest and previous key; it does not reread earlier
pages or load the complete checkpoint. This is not a random-access proof: reaching
a later page requires the verified preceding chain.

This is the storage boundary for larger recovery. The existing installed
`NKPAIR01` job keeps its lifecycle qualification. A new paged native profile
uses a 64-record staging call and admits 3,072 first-root and 1,536 second-root records, qualifying
combined root/continuation/progress transactions over multiple pages. The codec
also has host qualification through governed Storage 0.74.1 handles and a
no_std Wasm compile/lint check. Neither these tests nor a valid manifest
authorize a consumer cutover.

## Byte contract

All integers are unsigned and little-endian. Every size is exact; unknown magic,
length, geometry, epoch or root identity refuses. Root identity must be nonzero;
the caller selects the expected epoch/root and maximum record count.

| Manifest offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | `NKPAGED1` |
| 8 | 8 | Version 1 |
| 16 | 8 | Epoch |
| 24 | 8 | Root identity |
| 32 | 8 | Total records |
| 40 | 8 | Page count: ceiling(total / 64) |
| 48 | 8 | Page capacity, exactly 64 |
| 56 | 32 | First page digest; zero for an empty map |
| 88 | 8 | `PAGEDONE` |
| 96 | 32 | SHA-256 of bytes 0–95 |

A complete empty map has zero pages and a zero first-page digest. An all-zero
manifest is invalid, so an unsealed storage allocation cannot be mistaken for an
empty checkpoint. Manifest validation checks framing, identity, counts and its
digest, but does not establish that any referenced page is present or valid.

| Page offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | `NKPAGE01` |
| 8 | 8 | Epoch |
| 16 | 8 | Root identity |
| 24 | 8 | Zero-based logical page index |
| 32 | 8 | Records in this page |
| 40 | 8 | Total checkpoint records |
| 48 | 32 | Next page digest; zero on the last page |
| 80 | 1,024 | Up to 64 `(key, value)` pairs, 16 bytes each; unused tail bytes zero |

A page digest is SHA-256 of the eight bytes `NKPAGEH1` followed by all 1,104 page
bytes. The manifest commits to page zero; each validated page supplies the digest
required for the next. The digest binds page identity, total count, sequence,
contents, padding and the remaining chain. Every nonfinal page contains 64
records; the final page has exactly the remaining nonzero count. Its next digest
must be zero. Keys increase strictly within and across pages, including across
full-width u64 boundaries. Duplicate keys refuse regardless of their values.

Hashes detect corruption, not malicious replacement by an actor who can replace
the entire trusted manifest and chain. Authentication, checkpoint selection,
freshness and custody belong to the recovery authority.

## Export and admission

`Map::export_pages` checks source health and collects one logical pair per record,
then sorts by key. It encodes pages **last first** so each next digest is known.
The sink receives every logical index exactly once in descending order and must
store the page before acknowledging success. Only after every sink call succeeds
does export return a manifest. A failed or unwinding sink may contain partial
pages; the source map remains unchanged and no successful manifest is returned.
The exporter is synchronous and uses O(total records) sorting memory plus a
fixed page buffer. This does not yet provide bounded native export work per message.

`Manifest::reader` starts a transient verifier at page zero. Each `read_page`
checks exact size and digest, then fields, padding, chain termination and record
ordering. It allocates at most 64 returned pairs. Allocation or validation failure
leaves the reader's expected page, digest and last key unchanged. A successful
read advances once; replay, skipping and out-of-order delivery refuse. Completion
is reached only after the manifest's exact page count, or immediately for a valid
empty manifest. Extra page reads refuse.

Later corruption is discovered when its page is reached. The caller must keep
the reconstructed map private until the complete chain and all native application
invariants succeed. Completion proves logical verification, not that the caller
applied native writes. Never retain or serialize the reader's Rust layout.

### Verifier continuation

`Reader::checkpoint` supplies an explicit 144-byte format; `Reader::resume`
admits it against an already admitted manifest:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | `NKPCUR01` |
| 8 | 8 | Version 1 |
| 16 | 32 | Manifest's digest (its bytes 96–127) |
| 48 | 8 | Next logical page index, including page count when complete |
| 56 | 32 | Expected next-page digest; zero at completion |
| 88 | 8 | Last-key flag: zero initially, one after any page |
| 96 | 8 | Last verified key; zero when the flag is zero |
| 104 | 8 | `CURSDONE` |
| 112 | 32 | SHA-256 of bytes 0–111 |

Admission checks exact framing, checksum, manifest binding and progress bounds.
Initial progress must match the manifest's first digest and have no last key;
completed progress must have a zero next digest. Resumed readers enforce the
saved key boundary as well as the next page's hash, framing and identity. The
independent golden records the first page of the existing 65-record fixture:
epoch 17, root 9, next page 1, last key `u64::MAX - 1`.

**A continuation is trusted recovery state, not a proof of preceding pages.**
An actor able to replace and rechecksum it can invent intermediate progress.
It must never be admitted from an untrusted request. Its owner must select it
under exclusive storage custody, bind it to the current job, and commit it
atomically with native writes. Lost or invalid progress does not authorize
guessing a cursor or resetting data. Explicit fresh-target restart rereads the
checkpoint from its manifest.

Already consumed page bytes are not reread. If they later change, continuing
from the saved cursor still uses the previously verified records and next hash;
it is not an audit of the backup's current complete integrity. Restart detects
the damage when it reaches those bytes. Future-page corruption still refuses
before applying that batch, and the adapter separately checks native prefixes.

## Governed host qualification

The 0.75.4 development pair envelope in `checkpoint::maps::paged` binds both
root manifests before any checkpoint is published. Its fixed 328 bytes are:

| Pair offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | `NKPPAIR1` |
| 8 | 8 | Version 1 |
| 16 | 8 | Shared epoch |
| 24 | 8 | Exactly two roots |
| 32 | 128 | Product manifest for root 1 |
| 160 | 128 | Product manifest for root 2 |
| 288 | 8 | `PAIRDONE` |
| 296 | 32 | SHA-256 of bytes 0–295 |

Admission checks both nested manifests at the shared epoch and their expected
root IDs, rejects overflowing aggregate counts and applies the caller's record
limit to each root. Empty roots retain their identities. Export emits root 1's
pages followed by root 2's, descending within each, and returns the pair only
after both succeed. Host recovery keeps both candidate maps private until both
page chains finish. It cannot expose root 1 after failure in root 2.

The shared `provider::paged` owner in the existing checkpoint-provider package uses the published
provider API through fresh governed handles with owner `native-checkpoint`:

| Locator | Stored shape |
| --- | --- |
| `native.checkpoint.paged.maps.manifest.v1` | `StableCell<[u8; 328]>` |
| `native.checkpoint.paged.maps.pages.v1` | `StableVec<[u8; 1104]>` |

The fixture appends pages in exporter order, reversing logical order within each
root's segment. It checks exact vector length against the admitted
manifest. It publishes the manifest last and writes only to fresh storage;
partial exports stay unsealed. Missing locators, earlier checkpoint locators or
unknown/corrupt data are refused without initializing replacement state. No raw
stable-memory format is inferred or recreated, and the legacy repository is not
modified. Host execution does not supply crash transactions. Installed storage
rollback has the separate scope described below.
The earlier single-root `native.checkpoint.paged.map.*.v1` locators are not
automatically enrolled or converted. Their v0.75.3 evidence retains its scope.

Six product tests cover boundary sizes through 4,608 records, full-width keys,
source-independent reconstruction, every manifest/page truncation and single-byte
corruption position, mixed identities, missing/replayed pages and re-signed
semantic errors. The committed golden was independently encoded with Perl
`pack`/`Digest::SHA`: an empty manifest followed by a 65-record manifest and its
two pages, epoch 17/root 9, ascending keys `u64::MAX - 64` through `u64::MAX`, and
values `(u64::MAX - key) XOR 0xa55a0123456789ab`.

Four pair-codec tests cover empty/asymmetric pairs, bounds, aggregate overflow,
mixed epochs/roots, every manifest truncation and byte corruption, second-root
sink failure and independently encoded empty-pair golden bytes. Three governed
host tests cover reopening after source/handle destruction, incomplete pair
publication, cross-root substitution, absent/extra/corrupt pages and missing
locators. At counts `[0, 0]`, `[0, 65]`, `[65, 0]`, `[1, 1]`, `[64, 65]` and
`[2304, 2304]`, opening/admission reads 1,344 backing bytes. Every selected
nonempty page reads exactly 1,104 backing bytes, with no
writes or growth. Provider read-call splitting does not change that byte count.
These are measured fixture costs, not an IC instruction or throughput guarantee.
The page and pair codecs pass focused host and Wasm Clippy on Rust 1.100.0.
The v0.75.3 single-root fixture read 1,144 bytes on opening and 1,104 per page
at 0, 1, 64, 65 and 4,608 records; those are historical single-root costs.

Run the focused codec tests and governed fixture after the documented bootstrap:

```sh
cargo test -p ic-storage-native --features map --lib map::paged --locked --offline
cargo test --manifest-path qualification/Cargo.toml -p ic-storage-native-feasibility --target-dir target/qualification --lib checkpoint::maps::paged --locked --offline
make native-map-page-test
```

## Durable page offsets on the host

`MigrationJob<PagedMapPair>` reuses the existing job transition and split-envelope
owner. Its distinct magic is `ICJPG001`; the non-Ready payload is exactly the
328-byte pair manifest. Domain IDs are 131072 and 393216, matching the current
disjoint native windows. The former target 163840 refuses. Header, phase codes,
epoch sequencing, completeness marker and digest are the existing job format.
In the staged form, `next`, `start` and `limit` count **pages**, ordered root 1
then root 2, ascending within each root. Each request advances 1–64 pages; empty
pairs complete with the first valid request. Admission rejects total page counts
above `u32::MAX`, including on hosts whose `usize` is wider. All decoded counts
must also fit the target's `usize`.

The shared store adds `native.checkpoint.paged.maps.job.metadata.v1` as a
`StableCell<[u8; 104]>` and `native.checkpoint.paged.maps.job.continuation.v1` as
a `StableCell<[u8; 512]>` beside the pair manifest and pages. Fresh publication
writes both chains, then the manifest, then job metadata; the continuation stays
zero until explicit import begins. Each importing transition writes 512 bytes
of continuation then 104 bytes of metadata, for **616 bytes**, independent of
import length. Exact retries write zero. Write order detects mixed records but
does not provide host crash transactions. Missing metadata or continuation
cannot implicitly enroll an earlier host job namespace or standalone pair.
The independent consumed-empty-job golden has 432 canonical bytes: epoch 1,
source 131072, target 393216, progress `(next=0, start=0, limit=2)`, and two empty
root manifests. Its bytes were assembled with Perl `pack`/`Digest::SHA`.

The job admits the manifest and transitions, **not the referenced page data**.
The qualification `migration_job::paged_continuation` module binds its two
product continuations to the job and private target maps in this fixed envelope:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | `NKPCNT01` |
| 8 | 8 | Version 1 |
| 16 | 104 | Exact canonical split job metadata, including its digest |
| 120 | 144 | Root-1 reader continuation |
| 264 | 144 | Root-2 reader continuation |
| 408 | 32 | Root-1 native-prefix fingerprint |
| 440 | 32 | Root-2 native-prefix fingerprint |
| 472 | 8 | `CONTDONE` |
| 480 | 32 | SHA-256 of bytes 0–479 |

Admission checks complete integrity, exact current job metadata, per-root reader
positions projected from the shared progress offset, and native-prefix counts.
Each fingerprint is SHA-256 of `NKPNAT01` followed by the product map's complete
streamed `NKEYMAP1` checkpoint at epoch zero, including its digest. This uses
constant hash scratch and the current native bucket iteration order. A logically
equal reconstruction with different bucket order refuses; matching fingerprints
still do not prove allocation custody. Explicit fresh-target restart owns
reconstruction. These fingerprints admit no raw native pointers or allocator state.

`NativePrefix::from_records` observes an admitted scalar stream directly, using
the same `export_records` byte encoder as `Map::export`. It preserves input order,
checks the declared count and uses constant encoder/hash scratch. Native custody,
unique keys and an immutable scan remain caller obligations; passing an arbitrary
record list does not establish them. Count mismatch or sink failure leaves an
incomplete checkpoint that must not be published. Portable import still rejects
duplicate keys independently.

The shared governed store accepts these transient observations for capture and
a lazy observer for resumption. `decode_observed` validates framing, checksum,
exact job metadata and cursor geometry before calling that observer once for an
importing job. Invalid binding and completed retries never call it. Existing
map-based convenience calls delegate to the same codec; the 512-byte format and
golden remain unchanged. Host tests compare both paths after every page through
4,608-record pairs, reject changed counts/values/order and verify lazy refusal.

Before new writes, the adapter checks both current native fingerprints and
stages only newly requested page records through the resumed readers. After
applying them, it captures the new fingerprints/cursors and publishes the bound
records. **Native scans still cost O(map capacity) per batch**, both to check the
old prefix and to capture the new one. Hashing and reallocation can still dominate
recovery CPU. This improves checkpoint I/O without claiming bounded total work.
Scratch holds at most 64 new pages of records plus the current verifier page.
Both candidate maps stay private until completion. A completed exact retry
validates job/cursor binding but does not compare or overwrite later native
application writes. Explicit restart discards only an unfinished target and
replaces its continuation at offset zero, preserving checkpoint selection and
retired-epoch rules. It can recover from a corrupt continuation or private prefix
without reading them; subsequent batches still validate checkpoint pages.

Four page-job codec tests cover transition binding, retries, cancellation,
restart, progress corruption, count overflow and independent golden bytes.
Three product continuation tests and four job-continuation tests cover byte
truncation/corruption, resealed invalid geometry, stale/mixed parts, native changes,
exact job binding, completed retries and independently encoded golden bytes.
Five governed job tests reopen all storage handles between batches,
recover pairs through 4,608 records, check fixed progress writes and preserve
later application writes on completed retries. Fault cases include a wrong
native value or unexpected second-root prefix, corrupt continuations, corruption
after a valid newly staged page, missing pages and mixed/unsealed metadata.
They refuse before target and stable writes. Two-page batches read at most
2,208 page bytes regardless of imported length; exact retries read zero page
bytes. These measurements exclude the fixed handle/cell reopening reads.
An explicit case checks damage to an already consumed page when restart reads
it again. Fresh-target
restart also recovers damaged private prefixes and continuations. These checks
use live host maps, not
retained native heaps or IC transactions. The host adapter admits at most 4,096
records per root; this is a fixture resource bound, not a new product limit.

Run `cargo test --manifest-path qualification/Cargo.toml -p ic-storage-native-feasibility --target-dir target/qualification --lib migration_job --locked --offline`
for the shared job cases and `make native-map-page-test` for both governed page
groups. These cases run through existing repository CI selections.

## Installed storage transaction boundary

The `paged-store` profile now uses the same `provider::paged::Jobs` owner as the
host fixture. It opens all four locators through fresh governed handles, exports
pages before their manifest and metadata, and captures a continuation before
writing progress metadata. The owner refuses publication over an existing
checkpoint and refuses absent locators; it never converts a failed load into
initialization. `commit_progress` binds the supplied job, cursors and actual map
counts before any write. Its caller still owns transition authority and semantic
verification of native writes; this storage method cannot establish those facts.

The ordinary Wasm probe recreates transient maps by rereading the committed prefix
on every request. It exercises the real continuation codec and storage operations,
not retained native custody or bounded recovery I/O. Domain IDs belong to the
shared job fixture and are not proof of native arena ownership in this probe.
Controller checks protect its publication/progress/fault updates; management
upgrade authority owns its optional post-upgrade progress operation.

Installed tests independently check both root manifests, every page hash and
full-width record values. They cover `[0, 0]`, `[0, 65]`, `[65, 0]`, `[129, 65]`
and `[2304, 2304]` pairs, reopening all handles after ordinary heap replacement,
root-crossing and final progress writes, exact retries, and corrupt metadata,
manifest, continuation or future-page refusal. Removing stable memory cannot
silently initialize a replacement store.

Fault points follow the actual governed writes: first page, first page of root 2,
manifest, continuation and metadata. Failed update calls preserve complete stable
bytes and the preceding snapshot. Failed ordinary upgrades after continuation or
metadata writes additionally preserve the old executable. Thus the IC message
supplies atomicity across these storage allocations; cell ordering alone does not.
Both target roots remain outside this qualification boundary.

On 17 November 2026, `make paged-store-test` passed both installed cases in
`target/evidence/paged-store/attempt.ftDAhy` (11.64 seconds) using Rust 1.100.0,
Runtime 0.23.16, Testkit 0.108.0, the prepared 0.106.0 launcher and PocketIC 16.1.0.
The collector preserves both admitted Wasm artifacts, source inputs and completion
markers. Eight existing governed host tests retain their 616-byte progress and
zero-write retry measurements after extraction into the shared owner. Strict
host/Wasm Clippy passed for the changed probe and the existing ordinary/native-map
callers. Hosted CI runs the new installed target separately from the host gate.

After the existing explicit setup, use `make paged-store-build` to compile both
ordinary variants, or `make paged-store-test` to build and run the installed cases.
These commands use locked offline dependencies and the admitted prepared server.

## Installed scalar observation

The bounded native-map job has a qualification-only `native_prefix` query. Its
native wrapper admits the retained roots, finishes the native Rust frame and
restores frozen-source or private-import custody before the checkpoint domain
scans scalar callbacks. Each key is immediately followed by its value, in native
bucket order. No temporary Rust map is built; each root returns its count and
32-byte fingerprint. Active roots refuse. The scalar callback roster remains
unchanged, and no observer Rust value is retained or stored.

On 17 November 2026, `native-map-provider/attempt.PWFDBc` passed three artifact
checks, the composition ordering check and three installed map-job cases
(11.71 seconds for the job group). Independent observations hash the actual
retained slot records, including their portable checkpoint digest; frozen
sources, partial imports, root crossings and restarts agree. Completed roots
refuse observation and remain unchanged. Seven product checkpoint tests, seven
continuation tests and eight governed host tests pass, with strict host/Wasm
Clippy. The shared store's installed cases also passed again in
`paged-store/attempt.LTdsCr` (12.21 seconds) with completion markers, using the
same Rust 1.100.0/Runtime 0.23.16/Testkit 0.108.0 toolchain described above.
Both evidence directories are under `target/evidence/`.

## Installed paged native recovery

`native-map-paged` selects the paged adapter in the existing checkpoint-provider
fixture. It uses native format 11, the closed scalar staging ABI, the shared job
state machine and all four governed paged locators. Its fresh namespace stores
an explicit Ready job envelope. Export freezes both source roots and seals sorted
logical page chains. Repeated exports of the same epoch refuse; cancellation or
completed recovery permits the next sequential epoch. Source cancellation is
controller-authorized and atomically restores native custody. Whole-checkpoint
consume remains outside this profile; no old locator is implicitly converted.

The profile limits the first root to 3,072 records and the second to 1,536,
with one 64-record page per batch: at most 48 + 24 pages. Empty pairs complete on
their first batch. Recovery starts in a 24 KiB pool and grows through 48, 96 and
192 KiB. The sequential construction keeps the first root's 4096-bucket table
live while constructing the second root's 2048-bucket table. Those asymmetric
limits admit the tested rehash candidates; they are not a general capacity claim
for any two-root allocation history or the reversed distribution. Export above
either root's bound refuses.
Earlier paged native formats 8–10 are not silently reopened as format 11.
Page, manifest, job, continuation framing and domains are unchanged from format
10; its earlier target-domain transition remains recorded below. No native
layout compatibility or automatic conversion between experimental formats is implied.

The composed memory contract reserves these half-open byte ranges:

| Owner | Range |
| --- | --- |
| Shared transient stack | `[0, 65536)` |
| Native header and scalar staging | `[65536, 68096)` |
| Source native pool window | `[131072, 327680)` |
| Target native pool window | `[393216, 589824)` |
| Checkpoint data, allocator and heap | `[655360, memory.size * 65536)` |

Each native loan starts at 24 KiB and may grow only while active and idle, after
all native Rust owners have returned. The existing provider initializes only
the newly admitted tail, publishes capacity, then readmits both roots. Frozen
sources and private import prefixes refuse ordinary growth. Both complete
windows fit within the composed module's initial 32 Wasm pages, so these growth
cases expand the loan without invoking physical `memory.grow`. The separate
native provider evidence still owns actual page-growth/refusal claims.

During a new private import batch, the closed wrapper temporarily activates the
roots after checkpoint Rust has returned. Native command 12 reserves at most 64
additional records through the product `Map::reserve`. Only allocation refusal
is recoverable: it preserves the old root and records. Both native owners then
end and release the loan. If the resulting table capacity is insufficient, the
provider expands the loan and retries reservation before inserting any staged
record. A maximum-capacity refusal traps the batch. Native frames never span
pool growth; private custody is restored before verification and progress writes.
No application endpoint can observe the temporary active state. Growth faults
11–14 select the provider's four existing rollback points (backing expansion,
tail initialization, extent publication and readmission); they fire only when
growth is actually needed. Exact retries neither reserve nor grow.

Page application now uses native command 13: admit both maps once, insert all
staged records under one scoped view, then release both actual owners. The
`staged_word(index, field)` callback returns only scalar u64 values from disjoint
provider staging. It cannot read the loan, enter Rust or retain a pointer. Its
closed protocol requires claimed custody, a pending uncommitted batch, the
matching root/operation, complete staging, per-root and total bounds of 64, and
sequential key/value/acknowledgement calls. Each acknowledgement follows an actual
insertion; fault 3 now traps there while both owners remain live. Duplicate
insertions refuse. The native memory guards still exclude staging: Rust receives
scalars through the provider rather than borrowing those bytes. The new raw
import is admitted by exact name and signature; native format 11 and all portable
framing remain unchanged. Reservation is skipped when the already admitted table
has room; actual page application still freshly admits both maps. No native owner
spans growth or any checkpoint Rust frame.

Composition admits checkpoint data and heap placement above 655360 before
selecting that reset floor. Compatible upgrades first admit native state, then
clear the checkpoint domain through the current memory end, restore pristine
initialization and reset the shared stack. Neither native window is cleared.
Whole-heap replacement reconstructs the target from logical pages and discards
the entire old source window. The older bounded map/vector profiles keep their
196608 checkpoint floor and existing domains.

Every new batch has six closed wrapper steps:

1. Admit native state and restore private-import custody after native Rust ends.
2. Admit stored metadata/continuation and the retained fingerprints. Capture the
   admitted old records in transient checkpoint memory; read only the requested
   new page and stage its scalar records. Do not write durable progress yet.
3. End checkpoint Rust, reserve the page (growing only between ended native
   loans), apply native writes and suspend both roots privately.
4. Return to checkpoint Rust. Compare every resulting native record against the
   old records plus the newly verified page, rejecting duplicates and missing or
   changed values. Capture actual bucket-order fingerprints, then write the
   512-byte continuation and 104-byte job metadata through the shared store.
5. End checkpoint Rust and publish both roots only when the verified batch is
   complete. Intermediate batches retain private-import custody.
6. Reply after all commits finish.

The between-frame plan uses ordinary transient checkpoint allocations outside
the native pool, with no live borrow while native Rust runs. It is taken and
dropped within the same message. No such Rust plan survives as recovery state;
resumption uses byte-defined governed data. A trap rolls back both domains and
stable allocations together. Write ordering and host unwinding alone do not
supply this transaction guarantee.

Fresh-target start and explicit unfinished-target restart use the same post-apply
verification/commit boundary with empty expected roots. Restart discards only an
unfinished target; it can recover corrupt native state or a damaged continuation
from valid independent job/manifest/pages, but cannot repair corrupt job metadata
or checkpoint pages. Skipping a corrupt old pre-upgrade hook is explicit in the
qualification management call. Compatible interruption validates the retained
fingerprints and stored cursors. Completed retries validate binding without
scanning or rewriting active roots, preserving newer application writes.

Installed cases cover empty, asymmetric, 129+65, 192+192 and 3,072+1,536-record pairs, independent
manifest/page/native-slot checks, frozen resumption, compatible interruption,
exact retries and restart. Faults after native insertion, semantic verification,
continuation writes, metadata writes and final native publication preserve the
preceding native/stable bytes; failed upgrades also preserve the old executable.
Value/duplicate corruption and completion-flag substitution refuse. Missing
stores and invalid metadata/manifests/continuations/pages never reset data.
Corrupting an already consumed page does not affect continuation to the next
page; explicit restart encounters it again. A corrupt later page refuses only
when reached, preserving prior native roots and progress. Between the first and
second pages of a root, tests observe the actual 128-to-256-bucket relocation
and check rollback after rehashing. Restart also covers a complete first root
and incomplete second root while both remain private. Controller checks guard updates.

The shared logical prefix verifier uses a caller-selected fixed bitmap: one
u64 for the older 64-record job and 48 u64s for the paged profile. It checks
counts and sorted unique expected keys before scanning, and rejects duplicate,
missing or wrong-value native records across all bitmap words. The 384-byte
bitmap adds no allocation. Expected-record capture, sorting and fingerprint
scans still grow with imported records; the change is not a bounded-CPU claim.

Use `make native-map-paged-build` for the admitted Wasm build and
`make native-map-paged-test` for installed qualification. Hosted CI calls the same
target, and the collector preserves source inputs, all six composed executables
and `PAGED_NATIVE_RECOVERED`, `PAGED_NATIVE_REFUSED`, `PAGED_NATIVE_MULTIPAGE`,
`PAGED_NATIVE_GROWTH`, `PAGED_NATIVE_LARGE` and `COMPLETE` markers.

On 17 November 2026, the initial 64-record native paged cohort in
`target/evidence/native-map-provider/attempt.ugEKzt` passed three native artifact
checks and both installed cases (11.90 seconds), including explicit confirmation
that value and duplicate-slot faults reach post-apply verification. The existing
map-job cohort passed again in `attempt.edtOHJ` (three artifact checks, one
ordering check and three installed cases; 11.85 seconds for the job group).
Eight governed host tests and strict host/Wasm Clippy passed for the affected
adapters and caller. These runs used Rust 1.100.0, Runtime 0.23.16, Testkit 0.108.0,
the prepared 0.106.0 launcher and PocketIC 16.1.0. Preliminary successful runs
`attempt.WTUpkb` and `attempt.Ij35Cq` remain preserved with their narrower tests.

The subsequent 192-record/root qualification passed in
`target/evidence/native-map-provider/attempt.rU6zdL`: three native artifact checks
and all three installed cases (32.38 seconds), including same-root rehashing,
later-page corruption, partial last pages and restart after crossing roots.
The older `NKPAIR01` profile passed in `attempt.xehH5r` (three artifact checks,
one ordering check and three installed cases; 11.83 seconds for the job group).
Five prefix-verifier host tests cover bitmap boundaries and every duplicate
placement across 192 records; eight governed-store tests also pass. Strict
host/Wasm Clippy passed for the changed code and the bare-map, map-job and
ordinary paged-store callers. The compiler and dependencies are unchanged from
the preceding evidence. Earlier source-fixture diagnostic failures remain in
`attempt.TKUUnq` and `attempt.4rmPgc`; neither is completion evidence.

The format-10 window/growth qualification passed in
`target/evidence/native-map-provider/attempt.9xD1Hd`: three native artifact checks,
one checkpoint-placement refusal check and four installed cases (30.46 seconds
for the paged group). The new case grows both source and completed-target loans
through 24/48/96/192 KiB, checks all four growth fault points against the full
reserved window, and retains a real 2048-bucket allocation containing records
beyond the former 196608 reset boundary. Compatible source/frozen upgrades
preserve that allocation; replacement recovers its 192+192 logical records into
the separate target and leaves the discarded source window zero. Frozen/private
growth and growth beyond the maximum refuse. Completed retries after target
growth preserve native state and durable progress.

The same cohort rejects valid Wasm whose checkpoint initialization starts at
196608, 393216 or 655359. Thirty job/continuation host tests, including updated
independent domain-bound golden bytes, and eight governed-store host tests pass.
The ordinary paged-store cohort passes in
`target/evidence/paged-store/attempt.S91iGu` (two installed cases, 12.08 seconds).
The older bounded map job passes in `attempt.YdOwFg` (three artifact checks,
one ordering check and three installed cases; 11.57 seconds for the job group).
Strict host/Wasm Clippy passes for the affected callers. The earlier successful
window run `attempt.YtYI20` remains preserved; it preceded the explicit compiled
data-placement refusal test. Toolchain and dependency versions are unchanged.

### Format-11 large recovery evidence

The 17 November 2026 cohort in
`target/evidence/native-map-provider/attempt.cVLsDu` passed three native artifact
checks, one checkpoint-placement refusal check and five installed cases (45.54
seconds for the paged group). The new case recovers 3,072 + 1,536 records in
72 batches and both empty-root variants. The full pair expands its private loan
before zero-based pages 6, 12 and 24, reaching 48, 96 and 192 KiB. Faults at each
growth step and after record/progress publication restore the full native window
and stable state. Compatible interruption preserves private prefixes; a corrupt
prefix refuses and explicit restart reconstructs it from the independent pages.
Mixed replacement, lookup, deletion and insertion run before export; replacement,
lookup and deletion run after recovery. Consumed retries preserve those newer
writes. Independent slot/page/manifest/fingerprint checks verify the records
after every batch. Ordinary private growth still refuses.

Six prefix-verifier host tests pass, including all 3,072 expected records across
48 bitmap words, duplicate/missing records at word boundaries and oversized
input refusal before observation. The new reservation command preserves the
admitted native function roster, memory-access coverage and 2688-byte stack
bound. Strict Wasm and installed-harness Clippy pass. The older bounded map job
passes in `attempt.drexPp` (three artifact checks, one ordering check and three
installed cases; 11.92 seconds for the job group). The standalone native map
profile passes in `attempt.p1taOM` (three artifact checks and its installed
growth/custody/refusal cohort, 8.95 seconds). Its unknown-operation counterexample
now uses command 13, preserving refusal beyond the new closed selector.
Toolchain and dependency
versions are unchanged from the preceding cohort. Earlier diagnostic attempts
remain preserved and are not completion evidence.

### Complete-entrypoint cost evidence

`make native-map-paged-test` also preserves `metered-paged-*.wasm`, their hashes,
`paged-costs.tsv` and a `PAGED_NATIVE_COSTS` completion marker. These are instrumented
copies of the admitted executables, with unchanged native layout and logical data.
The ordinary uninstrumented recovery cohort still runs alongside them. A separate
calibration canister checks exact read/write/grow counters, per-call reset, query
noninterference and counter/memory rollback after a trap.

The measurement wraps the complete successful exported Wasm entrypoint, including
native admission, checkpoint Rust, native apply, post-write verification, durable
progress, publication and reply. It reads
[`performance_counter(0)`](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/#performance-counter)
after the original entrypoint returns. Counts include instrumentation overhead
and exclude only the final counter store and wrapper return. They are IC execution
counter units, not wall time or a cycle-price quote. Stable I/O counters wrap the
actual `stable64_read/write/grow` calls, including provider metadata work. Grow
pages count requests; successful end-of-call stable size is recorded separately.

On 17 November 2026, `target/evidence/native-map-provider/attempt.6qQnfa` passed
three artifact checks, one placement-refusal check and six installed cases
(44.69 seconds for the paged group), with strict Clippy. The cost TSV is identical
to the preceding `attempt.zrTGNu` run. Rust, Runtime, Testkit and PocketIC versions
are unchanged from the format-11 cohort.

| Records | Export instructions | Maximum batch instructions | Batch count | Sum of batch instructions |
| --- | ---: | ---: | ---: | ---: |
| 0 + 0 | 1,710,826 | 2,555,237 | 1 | 2,555,237 |
| 192 + 192 | 4,771,685 | 103,859,740 | 6 | 384,335,250 |
| 3,072 + 1,536 | 40,409,832 | 1,412,361,148 | 72 | 51,070,508,424 |

The large pair's first batch costs 21,210,864 instructions; its final batch is
the maximum. Each nonempty batch reads 6,656 backing bytes in 27 calls (including
one 1,104-byte page), writes 616 bytes in two calls, and requests no stable growth.
Empty completion reads 5,552 bytes. Sampled retries write/grow nothing; the largest
sampled retry costs 28,558,937 instructions. Native admission at the full source
costs 21,452,430; frozen post-upgrade costs 43,922,236; the largest sampled importing
post-upgrade costs 33,143,793; active post-upgrade costs 26,495,947. Pre-upgrade
admission is measured through a dedicated update alias. Post-upgrade values cover
that Wasm hook, including transient reset; they exclude management-canister work,
compilation and the separate source pre-upgrade hook.

Every sampled entrypoint in this baseline stayed within its qualification ceilings of
2 billion instructions, 2 MiB Wasm memory, 50 stable pages (3,276,800 bytes) and a
192 KiB retained native loan. All three shapes occupy 2 MiB of physical Wasm memory
and the same 50-page governed backing allocation; these sizes include reserved
space and provider overhead. Since Wasm/stable memory cannot shrink within these
calls, ending page counts also bound their physical high-water marks. They do not
measure peak live allocator payload or distinguish all transient allocations.
The loan grows from 24 to 192 KiB for the large import. These are regression
ceilings for this fixture, not production service budgets or general IC limits.
Failed-call costs, allocator-level peaks and whole management-upgrade costs remain
unmeasured; failed transactions preserve the preceding counter sample.

These baseline measurements exposed a scaling problem. `$build_map` invoked the
native bridge once per inserted record, and each invocation admits both complete
maps. The full-source admission cost multiplied by 64 was close to the final
batch cost; this is evidence for targeting repeated admission, not an exact
stage attribution. The following change addresses it while retaining staging
authority, both-root validation and atomic publication. Fixed page I/O still does
not make recovery CPU independent of the imported prefix.

### One-scope page application evidence

The 17 November 2026 cohort in
`target/evidence/native-map-provider/attempt.YNnfnO` passes four artifact checks,
one checkpoint-placement refusal check and seven installed cases (32.82 seconds
for the paged group). It retains the earlier growth, corruption, interruption,
restart, exact-retry and publication rollback checks. Two successful query probes
and twenty refusal modes independently check the scalar callback's root binding,
custody, complete staging, bounds (including overflow attempts), field ordering,
duplicates and reads after completion/release. These query probes synthesize
provider state without a Rust loan; real installed batches separately qualify
the callback with live native owners and mid-insert rollback.

| Records | Maximum batch instructions | Sum of batch instructions |
| --- | ---: | ---: |
| 0 + 0 | 2,555,237 | 2,555,237 |
| 192 + 192 | 10,171,882 | 43,811,397 |
| 3,072 + 1,536 | 101,728,323 | 3,565,146,839 |

Against the preceding same-shape baseline, the large pair's worst batch is
**13.9× lower** and its 72-batch total is **14.3× lower** in instruction-counter
units. The new maximum occurs at zero-based page 60, when the second root
rehashes; the final batch costs 91,016,586. Export/admission/post-upgrade costs,
6,656-byte reads, 616-byte writes and physical memory sizes are unchanged. The
fixture now gates every sampled successful entrypoint at **150 million
instructions**, keeping the 2 MiB Wasm, 50-page stable and 192 KiB native budgets.
Capacity remains 3,072 + 1,536 records. Instrumentation scope and unmeasured costs
are unchanged from the baseline above.

The raw map now has two exact non-reentrant imports and entry index 7, with eight
defined functions, 22 direct native call edges and the same 2688-byte stack bound.
Every memory access remains guarded; the additional i64 load brings that count
to 35. A valid Wasm module with a renamed staging import refuses admission.
Twelve shared vector/access/link/stack tests pass, including two-import leaf
accounting and defined-cycle refusal. The standalone installed map passes in
`attempt.ibTHia` (four artifact checks and the installed cohort, 8.39 seconds),
including invalid command-13 arguments and unbound staging refusal. The older
bounded map job passes in `attempt.ThlUFp` (four artifact checks, one ordering
check and three installed cases, 11.51 seconds for the job group). Strict Clippy
passes; compiler and dependencies are unchanged. Preliminary successful runs
`attempt.j28ufx`, `attempt.bUVTlG` and `attempt.6k40SM` remain preserved with their
earlier checks; the final cohort owns the current results.

## Cancellation and repeated epoch evidence

The paged adapter now uses the shared job's cancellation and next-epoch rules.
Cancellation writes only the 104-byte job metadata, then thaws the source in the
same IC message. An exact cancellation retry writes nothing and preserves later
native mutations. Ready, importing, consumed, wrong-domain and stale-epoch
cancellations refuse. A partial target must use the existing explicit restart
path; cancellation does not publish or thaw incomplete target roots.

Next-epoch export admits the old envelope and continuation before governed
writes. A completed continuation must retain its checksum and cursor binding,
but its old native fingerprints are deliberately not compared with newer
authoritative application values. Ready/cancelled jobs require an empty
continuation. Unknown or zeroed job metadata is never treated as initialization.
The adapter calculates the new manifest, admits it through `PagedMapJob::seal`,
then exports pages into existing vector slots, appending or popping as needed.
It writes the manifest, clears the continuation and commits the new metadata.
The two export passes sort logical records; they do not buffer every page.
IC message rollback, not host write ordering, protects replacement. Trimming
reduces the logical vector length; it does not release physical stable pages.

`target/evidence/native-map-provider/attempt.Ydm3nq` passes four artifact checks,
one checkpoint-placement refusal and eight installed cases (32.97 seconds for
the paged group), including `PAGED_NATIVE_LIFECYCLE` and `COMPLETE`. The new
history exports 72 pages and cancels, exports/reconstructs five pages A→B,
exports/reconstructs an empty pair B→A, then exports/reconstructs six pages A→B.
It checks controller authorization, compatible reopening, intervening application
writes, exact cancellation/completion retries, and refusal of older jobs after
later publication. Traps after cancellation metadata and native thaw, and after
replacement page writes/pops, manifest, continuation and metadata all preserve
the complete prior stable bytes and native observations. Independent manifest,
page-chain and record observations cover each epoch. Reverse-direction artifacts
and hashes are preserved with the usual six composed executables.

All ten governed host tests pass, including malformed-state refusal, zero-write
transition refusals, page-vector length changes, and completed-continuation
corruption before replacement. The initial-epoch cost cohort still passes its
150-million-instruction, 2-MiB-Wasm and 50-stable-page gates. The largest export
now costs 49,835,869 instructions; the worst batch is 101,728,776 and all 72
batches total 3,565,179,073. Nonempty batch I/O remains 6,656 bytes read and 616
written. The TSV contains 110 successful-call rows. Cancellation and later-epoch
costs are not yet instrumented; the lifecycle tests establish behavior and
rollback, not a service budget for those calls.

The older bounded map job also passes in `attempt.GNi8Od` (four artifact checks,
one ordering check and three installed cases; 11.72 seconds for the job group).
The ordinary paged-store profile passes both installed cases in
`target/evidence/paged-store/attempt.wmYzlc` (12.14 seconds). All 30 shared
job/continuation tests and four bounded-store host tests pass. Strict host/Wasm
Clippy passes with the pinned Rust 1.100.0 and unchanged dependencies.

The Git-only 0.75.5 release repeats the full offline `make ci` gate and rebuilds
the installed paged artifacts with the versioned workspace. Its
`native-map-provider/attempt.5taToy` passes all four artifact checks and all nine
paged checks (34.15 seconds), with the lifecycle/cost markers and `COMPLETE`.
The earlier directories retain the focused implementation and regression evidence.

## Remaining capacity and lifecycle work

See [roadmap R1](../DESIGN.md#r1--larger-installed-recovery-next) for the next
implementation slice and completion gates; this section owns the protocol limits.

The next work is measuring live allocation peaks and costs across repeated
lifecycle epochs before expanding the supported size. Private import growth now
recovers the
3,072 + 1,536-record workload from an initial 24 KiB loan. Export still sorts complete
source maps. Prefix hashing, expected-record capture and
post-write comparison still scan the imported records; these are not bounded
total recovery CPU or memory guarantees. Larger workloads,
consumer-facing recovery authority and actual cross-compiler migration remain
unqualified by this bounded profile. Historical evidence is unchanged.
