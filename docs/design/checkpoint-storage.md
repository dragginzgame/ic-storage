# Owned checkpoint backing

The optional `ic-storage` feature `checkpoint` provides bounded `Record` and
`Pages` containers over a caller-supplied `Memory`. This is portable checkpoint
backing for the retained native collections, not another collection architecture.
It is independently implemented here; legacy Storage is no longer a dependency.
The new format does not read or automatically migrate legacy checkpoints.

## Ownership and failure contract

The caller exclusively owns the complete backing namespace for every issued
handle's lifetime. `Memory` clones serve disjoint regions; they do not authorize
concurrent stores, external mutation or reopening while old handles remain live.
`Store::take` issues each declared region once, including when a handle is
forgotten. Regions and their containers are not cloneable. Cross-store exclusivity
is a caller obligation, not an automatically enforced global registry.

Creation requires zero allocated 64-KiB pages, validates the entire declaration,
reserves the complete capacity once and initializes the directory. Opening
requires the exact allocated size and every expected directory byte, with no
writes. Regions cannot be freshly initialized through a reopened store, even if
their framing is zero. Missing, foreign, truncated or incompatible data refuses;
there is no repair or reset fallback.

Backing IO must be complete or stop execution. Growth failure must leave memory
unchanged. Multi-write operations are not crash-atomic by themselves: the owner
must roll back an interrupted transaction, including growth and subsequent native
publication. The installed fixture uses IC message rollback. Host tests establish
bounded IO and refusal, not disk-crash durability. Logical checkpoint codecs own
payload integrity, completeness, schema, epochs and recovery-job identity.

## Byte format

All integers are little-endian u64. A fixed 128-byte directory contains:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | `ICKPT001` |
| 8 | 8 | Region count, 1–4 |
| 16 + 24 × index | 8 | Nonzero caller-assigned region ID, strictly increasing |
| 24 + 24 × index | 8 | Region offset |
| 32 + 24 × index | 8 | Region capacity including framing, at least 16 bytes |

Unused directory bytes are zero. Regions are packed consecutively starting at
128; total size is checked for overflow, then rounded up to whole backing pages.
IDs and capacities are part of the format contract. Changing a declaration needs
explicit migration. There is no allocator, extent directory or table scan.

`Record<T>` uses `ICREC001` at region offset 0, payload length at 8 and bytes at
16. Supported values are `Vec<u8>` and fixed byte arrays. Loading checks framing,
capacity and the caller's logical length limit before allocating/reading payload;
arrays additionally require an exact length. The value is cached after loading.
Setting checks region capacity, writes the payload and writes the eight-byte
length only if it changed. The load limit is admission policy; callers remain
responsible for validating newly written logical values.

`Pages<N>` uses `ICPAG001` at 0, count at 8, nonzero page width at 16 and packed
payload starting at 24. Opening verifies width/count/capacity without reading any
page payload. `get` and `set` touch exactly one N-byte page. Push writes one page
and its count; pop reads one page and updates count. Clear changes only the count.
Capacity is fixed and can be reused after trimming. No operation scans the table
to locate a page. Payload hashes and sequencing belong to the map page codec.

## Qualification binding

The private provider binds `Memory` to Platform `stable64_*` functions. Each
closed fixture owns the entire stable address space. Its region declarations are:

| Profile | IDs |
| --- | --- |
| Scalar/vector and scalar record | 1 |
| Two-map checkpoint | 2 |
| Scalar migration job | 3 |
| Record-vector checkpoint/conversion | 4 |
| Record-vector migration job | 5 |
| Split map job | 10 metadata, 11 checkpoint |
| Paged map job/storage | 20 manifest, 21 pages, 22 metadata, 23 continuation |

Records reserve 16 KiB payload each; their codecs impose smaller load limits.
The page region reserves 128 × 1,104 payload bytes plus its 24-byte framing.
Each declaration is independent; one fixture profile cannot open another's layout.
The host binding meters actual IO and bounds allocation. Production namespace
admission, shared-memory integration and a legacy-format importer are not supplied
by these fixture bindings.

`make native-core-test` covers format corruption, bounds, growth refusal,
forgotten claims and read-only reopen on host, with strict host/Wasm lint and docs.
The checkpoint, record, job, map-job, paged-map and paged-store installed targets
exercise independent reopening, upgrade recovery and transaction rollback through
the same product backing. Existing historical measurements made with 0.74.1
remain historical; they do not measure this implementation.

## Replacement qualification — 17 November 2026

Locked offline checks used Rust 1.100.0, Platform Runtime 0.23.17 and Testkit
0.109.0 with its prepared PocketIC 16.1.0 server. Both the legacy Storage package
and its transitive Runtime 0.22.5/`ic0` entries are absent from the qualification
lockfile. The product checkpoint feature adds no runtime dependency.

| Gate | Evidence cohort under `target/evidence/` |
| --- | --- |
| Ordinary scalar checkpoint upgrades/refusals | `testkit/run.ESG43E` |
| Native scalar recovery | `provider-link/attempt.gxT6mo` |
| Scalar record conversion/recovery | `provider-link/attempt.0ekdQf` |
| Record-vector conversion/recovery | `provider-link/attempt.UEAD9N` |
| Scalar durable jobs | `provider-link/attempt.Tcc9zx` |
| Record-vector durable jobs | `provider-link/attempt.Zf2WFK` |
| Bounded map checkpoint recovery | `native-map-provider/attempt.bWZWfh` |
| Split map durable jobs | `native-map-provider/attempt.lV6kup` |
| Paged native recovery, lifecycle costs and allocation checks | `native-map-provider/attempt.uNFdXS` |
| Ordinary paged storage upgrades/refusals | `testkit/run.g6YOC2` |

All these gates passed. The paged native cohort passed 11 tests and the ordinary
paged storage cohort passed two, including failed publication/upgrade rollback,
corrupt/missing storage refusal, exact retries and continued later epochs.
Host provider tests passed all 14 cases. Core framing tests cover every byte of
the 128-byte directory as well as wrong width, excessive lengths and allocated
size mismatch. Logs are `target/owned-checkpoint-*.log`.

The successful paged cohort recorded 225 instruction/IO samples across initial
and later epochs, plus 119 allocation samples. Its largest measured entrypoint
used 55,342,308 instructions (4,608-record shape, final batch). All cost samples
used exactly three stable pages (196,608 bytes). A batch opens the backing twice:
each open reads 128 directory + 48 record-header + 24 page-header + 944 record
payload bytes. Thus empty batches read 2,288 bytes; nonempty batches read 3,392,
including exactly one 1,104-byte page. Progress writes remain 616 bytes and exact
retries write zero. These are complete fixture-entrypoint observations, not a
general service throughput guarantee or a comparison against another library.

Requested checkpoint-heap allocation peaked at 197,264 bytes; all measured calls
returned to zero live requested bytes. The unchanged 150-million instruction and
256-KiB requested-allocation ceilings passed. Allocator overhead, fragmentation
and native-pool allocation are outside the requested-byte metric. TSVs and
artifact hashes remain in the named cohort.
