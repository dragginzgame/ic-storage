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
handles and a no_std Wasm compile/lint check. It has no installed recovery job,
persisted cursor or native retained representation. Neither these tests nor a
valid manifest authorize a consumer cutover.

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
applied native writes. Reader progress is not a stable format: do not retain or
serialize its Rust layout. Lost progress currently requires starting verification
again from the manifest.

## Governed host qualification

The fixture in the existing checkpoint-provider package uses the published
provider API through fresh governed handles with owner `native-checkpoint`:

| Locator | Stored shape |
| --- | --- |
| `native.checkpoint.paged.map.manifest.v1` | `StableCell<[u8; 128]>` |
| `native.checkpoint.paged.map.pages.v1` | `StableVec<[u8; 1104]>` |

The fixture appends pages in exporter order, so physical vector order is the
reverse of logical page order. It checks exact vector length against the admitted
manifest. It publishes the manifest last and writes only to fresh storage;
partial exports stay unsealed. Missing locators, earlier checkpoint locators or
unknown/corrupt data are refused without initializing replacement state. No raw
stable-memory format is inferred or recreated, and the legacy repository is not
modified. This fixture does not supply host crash transactions or establish IC
rollback across the two allocations.

Six product tests cover boundary sizes through 4,608 records, full-width keys,
source-independent reconstruction, every manifest/page truncation and single-byte
corruption position, mixed identities, missing/replayed pages and re-signed
semantic errors. The committed golden was independently encoded with Perl
`pack`/`Digest::SHA`: an empty manifest followed by a 65-record manifest and its
two pages, epoch 17/root 9, ascending keys `u64::MAX - 64` through `u64::MAX`, and
values `(u64::MAX - key) XOR 0xa55a0123456789ab`.

Three governed host tests verify reopening after source/handle destruction,
incomplete publication, absent/extra/mixed/corrupt pages and missing locators.
At 0, 1, 64, 65 and 4,608 records, initial opening/admission reads 1,144 backing
bytes. Every selected nonempty page reads exactly 1,104 backing bytes, with no
writes or growth. Provider read-call splitting does not change that byte count.
These are measured fixture costs, not an IC instruction or throughput guarantee.
The new page codec passes focused host and Wasm Clippy on Rust 1.100.0.

Run the focused codec tests and governed fixture after the documented bootstrap:

```sh
cargo test -p ic-storage-native --features map --lib map::paged --locked --offline
make native-map-page-test
```

The regular native-core tests own codec coverage; the governed page target also
runs in repository CI. Existing installed jobs retain their previous evidence.
Integrating this format requires the shared job owner to bind durable page
progress to the manifest and native writes, explicit two-root sealing and restart
semantics, and installed rollback/corruption tests. Native-prefix validation still
scans imported state; this page format alone does not make total recovery work
independent of map size.
