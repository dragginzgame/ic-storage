# Collection engine provenance

Storage owns allocation governance, backing custody, named directory and public
collection adapters. Its private `engine` module copies the collection algorithms
from published `ic-stable-structures` **0.7.2**. The maintainer explicitly selected
this source baseline instead of another independently maintained implementation.
There is no production dependency or public re-export of ICSS.

[The source inventory](icss-source.json) records original source SHA-256 hashes.
The relevant map/node/allocator/iterator, Vector, Cell, address and helper
code is copied under `crates/ic-storage/src/engine/`. Each copied file retains
DFINITY attribution and an Apache-2.0 notice; the package includes LICENSE-APACHE.
The bucket growth and translation mechanisms from `memory_manager.rs` are adapted
under `runtime/directory_model/persistence.rs`; that file retains the same notice.
Named identities, custody and integrity admission are Storage adaptations.
Storage-authored code remains MIT. This is adapted source, not an unchanged
vendored executable snapshot.

## The two collection layers

- Private `engine` owns one implementation of each algorithm. Set uses the same
  map engine with unit values; region views use that map over a bounded lease.
- Public `adapter` requires admitted `RuntimeMemory` handles. Ordinary operations
  return successful values directly; admission and region failures remain typed.
  Engines use Storage's `Memory` and `Bound` directly. The private codec bridge
  checks the public fallible codec contract before trapping on failure.

The old independent tree, block allocator, record envelopes, node cache, Vector
windows and mutation epochs are deleted. No alternative production engine stays
selected or maintained. The allocation backing now adapts ICSS bucket ownership and translation under
Storage custody. Its fixed 255-ID API and unrestricted constructors are not
exposed; names map to opaque u64 identities through admitted handles.

## Independent benchmark reference

The unpublished `testing/runtime-qualification` workspace selects published
`ic-stable-structures` 0.7.2 directly. It does not keep another source copy in
this repository or route the reference through the old Platform facade. The
benchmark support library uses ICSS traits and codecs, so this is a normal Cargo
dependency of that development-only package, not a `[dev-dependencies]` entry
(which would be unavailable to its library target). Production Storage and both
published adapters have no ICSS dependency. The optional pinned Git-head control
is also external and confined to the comparison workspace.

Keep the modified production engines here; do not overwrite them with pristine
reference code when updating a benchmark dependency. Map, Set and regions share
one B-tree source implementation. A shared private `TreeMemory<M>` now selects
whole-allocation or bounded-region IO at runtime, so identical key/value/backing
types share engine specializations. Different key/value types still specialize.
The whole-allocation variant owns its lease; each region operation clones the
existing Rc and drops that temporary reference with its freshly loaded tree.
Region bounds, growth refusal and owned scan snapshots remain unchanged.

## Deliberate adaptations

The algorithms, current node/overflow layouts, fixed-stride Vector slots and
scalar iterators follow 0.7.2, with the measured shortcuts described below.
Integration and safety adaptations:

- Private module paths and checked backing/codec views; existing application
  codecs remain unchanged. Storage unsigned codecs are little-endian, whereas
  ICSS's default unsigned codecs are big-endian. Same-image interoperability
  requires matching codecs, bounds, ordering and collection framing.
- Explicit create/load/reset admission. Remove upstream implicit initialization
  and old B-tree/node migration readers. Only current explicit-page-size BTR2,
  SVC1 and SCL1 images are admitted. No previous Storage image is read or reset.
  Map and region framing admission shares the engine's header read; foreign
  framing returns the existing typed error before allocator access.
- Storage's canonical `Memory::read_unsafe` contract passes through
  governed handles, preserving its ability to avoid preliminary zeroing.
  Callers admit a non-overlapping writable destination; implementations must
  initialize the complete range on success. Existing custom backings retain
  an initializing default. Governed and region views explicitly forward the hook
  so their optimized reads do not inherit that fallback. Public
  collection operations remain safe.
  Wasm uses published Runtime >=0.19.4 for the safe uninitialized stable-read
  export, with only its `ic0` feature selected.
- Cell admits the host's byte ceiling before buffer allocation. Vector checks
  a variable slot length against its declared bound before allocating payload.
- Vector reads/writes its active 17-byte header as one range and shares engine
  framing admission with its adapter. ICSS 0.7.2 uses five header reads/writes;
  Storage previously added another marker read. Reserved bytes, length reads,
  typed framing errors and bounds traps are unchanged. The
  [opening study](../audits/recurring/storage-vector-open-2026-11-08.md) records
  matched instructions, size and retained-handle controls.
- Cell writes its existing eight-byte header as one range, followed by the payload,
  instead of ICSS 0.7.2's three separate header writes. Layout and cached-value
  semantics are unchanged. The [write-path study](../audits/recurring/storage-write-paths-2026-11-07.md)
  records the measured savings and rejected B-tree experiments.
- B-tree node saves repair the overflow-chain terminator only when pages are
  removed; loaded and newly allocated tails already terminate. ICSS 0.7.2
  repeats that write on every save. See the [IO study](../audits/recurring/storage-btree-io-2026-11-07.md).
- Empty node fields inside the already allocated initial page skip backing
  calls, unlike ICSS 0.7.2. Logical extent and overflow reservation are preserved;
  governed backing already suppresses physical empty writes, so this saves CPU
  overhead rather than page charges. The IO study records the measured tradeoff.
- Loading/extracting an empty B-tree value returns an empty vector before
  constructing its payload reader; ICSS 0.7.2 still requests a zero-byte read.
  Nonempty values, keys and routing retain their original read paths. The
  [empty-value study](../audits/recurring/storage-empty-values-2026-11-07.md)
  records this shortcut and the rejected general-reader experiment.
- Insertions search non-full roots once through `insert_nonfull`; ICSS 0.7.2
  searches again there after an initial root miss. Full roots retain the
  pre-split lookup. See the [save/search study](../audits/recurring/storage-save-search-2026-11-07.md).
- A persisted allocator allocation flag is read as u8, checked, then interpreted,
  preserving valid bytes without constructing an invalid Rust bool on corruption.
- Native mutation unwind guards poison the shared owner after a panic. They add
  no Wasm journal or runtime work; IC rollback remains the atomicity mechanism.
- Private shared-node iterator entries project into public owned `MapEntry`
  values. Skipping traverses routing before materializing the selected entry;
  it does not read discarded value payloads.
- Vector has one engine module; copied single-variant runtime version and lazy
  object wrappers are removed without changing persisted headers.
- Remove unused APIs and upstream tests; qualify the copied subset using Storage
  models and the separately published reference. Rust 2024 unsafe blocks, checked
  conversions, lint fixes and narrowly explained expectations are mechanical
  adaptations, not performance enhancements.

Vector `last()` directly reads the final remaining live slot instead of scanning
and decoding every preceding slot as ICSS 0.7.2 does. It clips the iterator's
original range to the current length after truncation and never includes later
appends outside that range. Like `nth()`, it does not validate discarded records.
No collection layout or successful result changes.

Map entry iterator `last()` uses the existing reverse cursor, including bounded
and partially consumed ranges; Set inherits that shortcut. ICSS 0.7.2's entry
iterators still walk forward. Its direct Map `last_key_value()` and Set `last()`
are already efficient alternatives. Key/value projection iterators remain
unchanged. This shortcut does not validate discarded keys or nodes; it preserves
successful results and requires no image migration. The backing study records
instruction gains, stateful-trace regressions and benchmark Wasm-size growth.

Vector direct Get/Pop reads of fixed-width values up to 32 bytes without a
destructor use a small stack buffer instead of ICSS's temporary heap allocation.
A separate helper keeps stack preparation off the out-of-range path. Larger,
variable-width and destructor-bearing values retain owned decoder input;
iterators retain their reusable buffer. The cutoff is internal and introduces
no format or API change. The [matched comparison](../audits/recurring/storage-icss-backing-parity-2026-11-06.md#step-8-remove-small-fixed-vector-read-allocations)
checks both loop and full-handler costs, including unchanged controls.

B-tree child-address batches read into their final vector instead of allocating
ICSS's intermediate byte vector and copying into a second allocation. Packed
addresses retain little-endian decoding, including on native big-endian hosts;
the vector length becomes visible only after a complete initializing read.
Already-eager fixed-width keys up to 16 bytes are read as one contiguous range
per node, with the same per-key decoding and overflow-page translation. Larger
and variable-width keys keep their existing paths. These changes apply to Map,
Set and region views without changing images, geometry, bounds or custody.
The [backing study](../audits/recurring/storage-icss-backing-parity-2026-11-06.md)
retains before/after and whole-handler comparisons with published ICSS 0.7.2.

Lazy B-tree key reads transfer their existing buffer to codecs for key types
with destructors, allowing String and byte-vector decoders to reuse it. ICSS
0.7.2 passes this buffer as borrowed bytes. Keys without destructors retain the
borrowed path: transferring every buffer regressed fixed-array iteration in the
matched matrix. Eager keys still borrow their shared read buffer. The choice
folds at compile time and changes neither images nor successful decoding semantics.

Codec and engine failures trap. Framing admission and explicit reservations can
still return typed errors. Ordinary operation and iterator results are direct.
There are no collection checksums. Images predating the copied ICSS baseline
require an explicit host rebuild/reset or fresh allocations; the architectural
simplification preserves current images. The separate allocation backing now uses the authorized fresh-image bucket hard
cut; snapshot-directory images cannot be reopened. See [the collection contract](../../crates/ic-storage/README.md).

## Qualification and research controls

The independent workspace retains published ICSS 0.7.2 directly and optional
Git `b5d1650` as separate research controls. Neither
enters production. Same-image tests exchange current collections using matching
application codecs and governed Storage backing. Installed lifecycle tests and
matched IC benchmarks establish behavior and costs separately.

Historical audits and measurements describe the engines selected when they ran;
they do not establish current performance. The copied baseline deliberately
removes earlier local optimizations so later changes can be compared cleanly.
No universal improvement or ICSS default-codec compatibility is claimed.

IC Platform supplies Wasm stable-memory system access; IC Wire supplies optional application codecs. Testkit owns server lifecycle;
Shared Tooling owns its reviewed engineering snapshot, unchanged by this port.
Hosts retain lifecycle, comparators, schema and recovery decisions. Sibling code
and retained images have not been migrated by this change.
