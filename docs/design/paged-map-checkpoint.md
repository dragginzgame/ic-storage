# Sequential paged map checkpoints

The product `ic_storage_native::map::paged` codec provides a portable checkpoint
for one u64-key/u64-value map. A 128-byte manifest commits to a forward chain of
1,104-byte pages, each containing at most 64 records. A sequential reader verifies
one page using its expected digest and previous key; it does not reread earlier
pages or load the complete checkpoint. This is not a random-access proof: reaching
a later page requires the verified preceding chain.

This is the next storage boundary for larger recovery. The current installed
two-map job still uses its existing `NKPAIR01` checkpoint and 64-record-per-map
limit. The new codec has host qualification through governed Storage 0.74.1
handles and a no_std Wasm compile/lint check. A host adapter now persists page
offsets and verifier continuations through the shared migration job. There is
no installed paged recovery job. Neither these tests nor a valid manifest
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

The fixture in the existing checkpoint-provider package uses the published
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
modified. This fixture does not supply host crash transactions or establish IC
rollback across the two allocations.
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
328-byte pair manifest. Domain IDs remain 131072 and 163840. Header, phase codes,
epoch sequencing, completeness marker and digest are the existing job format.
In the staged form, `next`, `start` and `limit` count **pages**, ordered root 1
then root 2, ascending within each root. Each request advances 1–64 pages; empty
pairs complete with the first valid request. Admission rejects total page counts
above `u32::MAX`, including on hosts whose `usize` is wider. All decoded counts
must also fit the target's `usize`.

The host job fixture adds `native.checkpoint.paged.maps.job.metadata.v1` as a
`StableCell<[u8; 104]>` and `native.checkpoint.paged.maps.job.continuation.v1` as
a `StableCell<[u8; 512]>` beside the pair manifest and pages. Fresh publication
writes both chains, then the manifest, then job metadata; the continuation stays
zero until explicit import begins. Each importing transition writes 512 bytes
of continuation then 104 bytes of metadata, for **616 bytes**, independent of
import length. Exact retries write zero. Write order detects mixed records but
does not provide host crash transactions. Missing metadata or continuation
cannot implicitly enroll an earlier host job namespace or standalone pair.
The independent consumed-empty-job golden has 432 canonical bytes: epoch 1,
source 131072, target 163840, progress `(next=0, start=0, limit=2)`, and two empty
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

The next integration must replace the installed bounded-map payload path with
this page adapter, qualify native custody and atomic publication of progress and
both roots, and exercise installed rollback/restart. The installed bounded job
now provides a qualified post-apply verification boundary before publication;
continuation capture must run there after actual native writes, not while the
checkpoint adapter is only staging them. Host unwinding does not roll
back partially applied native allocations or writes. It supplies no installed
controller checks, frozen-source resumption, crash transaction or cross-compiler
proof. Bounded native-prefix validation and the installed continuation custody
contract remain open; the host checks do not establish either.
The current installed job's limits and historical evidence remain unchanged.
