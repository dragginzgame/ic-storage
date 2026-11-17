# Paged ordered recovery

Status: product codec, shared-job host qualification and a closed installed
four-root transaction binding. This extends the
[ordered collection](ordered-storage.md); it does not introduce another native
collection, allocation owner or job state machine. No consumer cutover is authorized.

## Bounded work

`Map::export_pages(epoch, root, sink)` visits the tree twice using reverse seeks.
The first pass checks each actual record fits and counts pages; the second writes
pages in descending index order. Scratch is one reusable 32-KiB byte buffer plus
constant traversal/hash state, independent of record population. It neither
sorts nor collects the full table.
Export is synchronous O(n log n), and requires an immutable admitted owner for
the call. Incremental export across messages is not implemented.

Pages pack complete key/value records, with limits of 32 KiB and 64 records.
The record limit also bounds the existing batch API's quadratic preflight cost.
One record's lengths plus key/value bytes must fit in 32,680 bytes; oversize
records refuse before the sink is called. The 512-byte key/4-KiB row and separate
16-KiB aggregate-job profiles fit. Short indexes consume their actual byte lengths.
The last page is filled first; the first page can therefore be short.

The sink owns page placement and publishes the manifest only after all pages
are stored. A failed sink can leave unusable partial pages, never an admitted
manifest. No source mutation occurs. The page encoding is distinct from the
unchanged `ORDMAP01` complete-buffer encoding; both carry logical records only.

## Byte contracts

Integers are little-endian. Hashes are SHA-256. All lengths are checked before
slicing or allocation; unknown versions, extra bytes and nonzero padding refuse.

| Object | Layout |
| --- | --- |
| Manifest, 128 bytes | `ORDPAG01`; u64 version 1, epoch, nonzero root, total records, page count, page width 32768; first-page digest; `ORDPDONE`; hash of preceding 96 bytes |
| Page, 32768 bytes | `ORDPG001`; u64 epoch, root, index, record count, total records; next-page digest; u64 used payload bytes; records; zero padding |
| Record | u32 key length, u32 value length, exact key bytes, exact value bytes |
| Continuation, 168 + last-key length | `ORDCUR01`; u64 version 1; manifest seal; u64 next page, records read; next digest; u64 last-key length, presence; `ORDCDONE`; u64 target K, target V; last key; hash of preceding bytes |

Page digests include the domain `ORDPGH01` followed by the entire padded page.
The final page's next digest is zero. An empty manifest has zero records/pages
and a zero first digest. Pages are nonempty, and keys increase strictly within
and across pages, including an optional empty first key. Bounds on page/record
counts are checked in manifests and continuations, including remaining progress.
An empty last key is distinguished from no previously consumed key by presence.
Continuations cannot be resumed under changed target K/V limits.

`Manifest::reader::<K,V>()` starts verification. `Reader::read_page` checks exactly
one expected page and returns borrowed records; it allocates no copies of values.
Only the last key is copied for the next step. `checkpoint` encodes the cursor;
`resume` admits that byte representation without retaining Rust, allocator or
hash implementation state. Refusal preserves the previous cursor bytes.
The source can be destroyed before reconstruction, and consumed pages need not
be reread. A fresh restart verifies the chain again from its manifest.

Integrity is not authentication. A party able to replace manifests or recompute
continuation checksums can invent progress. Trusted storage custody, current
job/epoch selection and private candidate ownership are separate obligations.
Resuming does not recheck previously consumed page contents or prove native writes.

## Shared job integration

The private qualification `OrderedJob` is a payload projection of the existing
`MigrationJob`, not another phase machine. It uses outer identity `ICJOR001`,
root 1, at most 128 row/index entries, and the installed ordered profile's 4-MiB
and 6-MiB native domains. Its payload is only the 128-byte manifest. Existing
104-byte metadata carries phase, domains, epoch and page offsets.

Shared transitions own sealing, cancellation, single-use import, bounded batches,
explicit unfinished restart and exact latest-request retries. `OrderedJob::reader`
requires an importing/consumed job and a cursor at exactly its committed page
offset. Neither the manifest nor job admission substitutes for page verification.
Completed retries acknowledge progress without replaying writes, preserving
application writes made after recovery.

An adapter must prepare the next job/cursor, admit the page, apply its records to
an exclusively owned private candidate and atomically commit all three. Prepare
fallible transient metadata before applying native mutations where possible.
The existing `View::apply` preflights expected allocation refusal for a whole
page; spare capacity can change, but records remain unchanged on refusal.
Ordinary errors, host unwinds and installed IC traps have distinct contracts.
The codec and host job tests alone do not prove an installed storage transaction;
the installed four-root binding below supplies separate evidence.

The candidate must remain unavailable to application writes until completion.
An adapter must bind custody to the exact active job, validate native admission
when reopening after a compatible upgrade, and explicitly discard an unfinished
candidate before restart. A count or last key alone does not prove native content
integrity. Do not add a full-table scan to every ordinary message as a substitute
for exclusive ownership. General application runtime custody remains R2 work.

## Qualification and next boundary

Focused checks exercise:

- Empty/exact-fit records, record-count and byte packing boundaries, 4-KiB rows
  and 16-KiB values, source destruction and cursor/native readmission at every page.
- Corrupt/truncated/replayed/out-of-order pages, correctly resealed malformed
  framing, duplicate keys, cross-page ordering, zero padding and trailing payload.
- Every manifest/continuation byte, truncated/extra bytes, changed target limits,
  independent empty-record manifest/page/cursor framing and epoch mismatch.
- Shared jobs through two opposite-domain epochs, interrupted decode/reopen,
  cancellation, explicit restart, stale/wrong requests, exact retries and preserved
  post-completion writes. These are host policy tests, not simulated IC rollback.
- Independent node and payload pool exhaustion after one successful page, with
  unchanged private records and committed cursor and complete loan reclamation.

Run the focused product filter `ordered::paged`, qualification library filter
`migration_job::ordered`, and provider-region filter `ordered_page_allocation`.
Host/Wasm Clippy and Rustdoc cover the new product API; existing core/feasibility
CI targets include these tests without a new dependency or qualification package.

The installed four-root binding below connects job, cursor and pages to product
checkpoint backing. It retains actual owners across calls and publishes the
completed family only after the final page. Complete-buffer recovery remains an
explicit separately tested mode; neither mode is an ordinary-write checkpoint mirror.

## Installed four-root page transactions

The pending 0.76.2 closed ordered binding now consumes public
`Read::export_pages` and `Write::apply` through the same four-root session.
Its `OrderedFamilyJob` is another payload of the existing `MigrationJob`, not
another transition machine. Shared qualification declarations bind main/source/
target/jobs to root and participant IDs 1–4, schema 1, entry limits
128/32/32/17, key bounds 512/512/512/1 and value bounds
4096/4096/4096/16384. The native owner and portable family use this one roster.

The job identity is `ICJOF001`. Its fixed 704-byte payload contains four parts,
ordered by root: six little-endian u64 declaration fields (root, participant,
schema, entries, key bytes, value bytes), followed by that root's 128-byte
`ORDPAG01` manifest. Existing job metadata seals the complete payload, producing
an 808-byte sealed/importing job. Progress counts pages across roots, skipping
empty roots. A root-bound cursor changes to the next root's initial verifier
only after the preceding chain completes. The last cursor remains bound to the
last nonempty root; an entirely empty family uses the first root's empty cursor.

The installed fixture owns one fresh checkpoint Store with four disjoint regions:
40 for the existing complete-buffer snapshot, 41 for the job, 42 for its verifier
cursor, and 43 for up to 64 fixed-width pages. Initialization issues all regions
from that fresh Store; reopening cannot initialize missing region framing.
The reserved stable backing is 49 pages (3,211,264 bytes). The page capacity and
shared native pool can refuse before all independently declared root maxima fit.
No ordinary update mirrors native records into these regions.

Native format 5 (`0x4f52444e41543035`) admits a private-import policy state in the
sealed four-root directory; format 4 refuses. Public record/row queries and
ordinary writes are unavailable while that state is active. The existing native
pools and generation metadata remain live across import messages. Compatible
upgrade closes all owners, admits native maps once under the selected contract,
then admits the trusted job/cursor at the same epoch and committed page offset.
It does not reread consumed pages or scan native records on every import call.
This relies on exclusive fixture custody; checksums alone do not prove a prefix.

A recovery step admits one expected page, prepares the next cursor and encoded
job, applies at most 64 borrowed key/value mutations to the private root, writes
the cursor, writes the job, then makes the family available if complete. Any
failure after mutation traps the enclosing IC message. Exact latest retries
validate the stored binding but read no pages, replay no mutations and write no
stable progress. Thus a completed retry preserves subsequent application writes.
Empty recovery still needs an explicit completion transaction.

The shared transitions permit cancellation only while the original source remains
sealed. After import begins, an explicit unfinished restart replaces the whole
candidate heap, resets trusted progress and starts verification from page zero.
It cannot restart a consumed job. This qualifies bounded import and a deliberate
whole-heap restart, not an incremental export, in-place conversion scheduler or
an application-facing retained bootstrap API. The complete-buffer mode remains
separate diagnostic recovery; it does not resume a paged job.

### Installed evidence

`make ordered-native-test` passes all five installed suites and two artifact
checks, including strict host Clippy and strict Wasm Clippy for all five variants.
Final cohort: `target/evidence/ordered-native/attempt.qYIc7N/`; full log:
`target/ordered-paged-final-installed.log`; prepared-server evidence:
`target/evidence/testkit/run.VoWXzM/evidence`.

| Variant | Bytes | SHA-256 |
| --- | ---: | --- |
| A | 167,695 | `bac5e5ee431e3e5fa07cfb96846a795ed99a133f24de46264d3af87a67dd44f4` |
| B | 167,695 | `00b29704a15aa1ae44a1344fcda98513e256a95240f767b9d878a8dc78092d84` |
| Recovery | 167,695 | `ff2afcd5d96e99d9d1266678cd975412e1a40317d20a1ca07c730aa2f918ae22` |
| Omitted close | 167,664 | `6dfb3e86be4e512be07f8395b5f8d1e582c7f90521384c4d7b76e959a125d4b3` |
| Failed close | 167,695 | `ad50a991c5a33ac4d0f308740f63ee4dc39fdb19825d441327f713b08470bf33` |

The workload restores seventeen 512-byte-key/4-KiB records in each of main,
source and target, plus seventeen 16-KiB jobs, using 26 pages. Tests cover:

- Source export/cancellation rollback and exact cancellation retries; frozen
  compatible upgrade and refusal to import a cancelled or wrong-domain job.
- Traps during target setup, including actual session-metadata allocation refusal.
- Traps after native writes, cursor publication, job publication and late reply
  encoding, including the final page; native bytes, ownership and stable bytes
  remain unchanged. Query import attempts also leave committed state unchanged.
- Compatible interruption after a consumed page is deliberately corrupted:
  resumption reads only the next page. Explicit restart then rejects that corrupt
  first page until repaired, demonstrating that restart verifies from the beginning.
- Latest-request retries, stale epoch/offset refusal, private-candidate access
  refusal, completed-write preservation, two opposite-domain epochs, and empty
  recovery with explicit completion.

| Successful step sample | Body instructions | Scratch bytes | Stable reads | Stable writes | Page reads |
| --- | ---: | ---: | ---: | ---: | ---: |
| First page | 4,842,109 | 6,376 | 33,928 | 1,496 | 1 |
| After compatible interruption | 4,826,954 | 7,528 | 34,440 | 1,488 | 1 |
| Final page | 6,367,662 | 4,857 | 33,929 | 977 | 1 |
| Completed retry | 1,026,131 | 3,417 | 1,161 | 0 | 0 |
| Empty completion | 1,292,335 | 4,384 | 1,160 | 976 | 0 |

`paged-costs.tsv` retains every measured page and retry. Every nonempty successful
step reads exactly one 32-KiB page; metadata/manifest/cursor reads account for the
remainder. Across the sampled calls, scratch peaks at 7,528 bytes, stable reads
at 34,440 bytes and writes at 1,496 bytes. The page buffer lives on the stack;
scratch excludes stack, native allocations and the separate 456-byte session
metadata (64-KiB reservation). These are entry-body measurements for this closed
population, not end-to-end instruction budgets or general throughput evidence.

Focused public checks pass 29 ordered tests, six compile-fail examples, host/Wasm
Clippy and Rustdoc. Shared-job tests cover root transitions with empty intervening
roots, declaration/epoch refusal, complete/empty retry semantics and existing
single-root restart/refusal behavior. The installed profile runs on the prepared
Linux PocketIC host; no macOS execution is claimed.

Remaining Storage #4 work includes reusable supplier-owned retained allocation/
startup, DB's actual write/query/job/maintenance adoption witnesses, larger
populations and fragmentation, and incremental source export. This closed
fixture must not be copied into DB as a substitute for the missing runtime API.
