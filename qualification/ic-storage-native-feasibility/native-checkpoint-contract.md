# Native checkpoint bridge contract

This qualification-only bridge combines the reviewed single-vector native client
with the existing governed Storage checkpoint adapter and version-2 codec. It
extends the [vector allocation argument](vector-provider-contract.md) for the
specific source, compiler flags, callback bodies and final composition. It does
not admit arbitrary code, private std headers or a production migration service.

## Separate allocation domains

There is one unshared Wasm memory: initially 32 pages, at most 256. The provider
owns metadata at 65536..65608 and one 4096-byte native allocation at 131072 (A)
or 147456 (B). The latter is a real change to the destination allocation domain,
not a version label. These remain outside LLVM-created checkpoint allocations.
The native client retains only u64 elements and byte-defined descriptors; all
its Region/Vec owners end before the wrapper returns.

The checkpoint runtime is separately compiled with a 64 KiB stack, data base
196608 and its Global heap above the compiler's admitted data end. Every active
data segment must lie at or above that base and below data end. Heap base must
follow data end and precede initial physical memory. No data segment, Global
allocation or memory growth can claim the lower native region under the selected
Rust/allocator contract. The linker shares only memory, remaps functions/types/
globals and preserves table/element references and the retention marker.

The native 64-byte frame at 32704..32768 overlaps the checkpoint runtime's stack
reservation, but the two Rust runtimes are never live simultaneously. Native
operations run directly from WAT. Checkpoint code calls only scalar WAT functions,
whose transitive closure cannot call Rust or any host function. Trusted init and
recovery wrappers wait for one Rust runtime to return before entering the other.
WAT callbacks use locals, not linear-memory stack. Stack bytes are transient;
no private header, pointer, borrow or allocator state is transferred between them.

The checkpoint runtime's private Global allocator is **not admitted across EOP
upgrades**. The original source-profile post-upgrade refuses. Recovery-profile post-upgrade
checks zero native metadata in WAT before its first Rust call and accepts only
the explicit epoch/fault argument shape. Retained native headers therefore refuse
before opening that runtime. Tests use ordinary upgrades to obtain a fresh heap.
The separate native-vector fixture still owns compatible EOP lifecycle evidence.
Management requests explicitly select `wasm_memory_persistence = replace`; an
upgrade with that option omitted is rejected by the platform for EOP modules.
The harness uses the selected typed chunked-install API, not a reinstall (which
would discard stable memory), and requires actual trap error codes for refusals.

## Transient-domain reopening experiment

The additional reopen profile admits the native descriptor and completes its
native Rust call before discarding checkpoint runtime memory. A private WAT
helper zeros stack bytes 0..65536 and the complete transient region from 196608
through the current memory end, including grown pages. It restores the target's
active data from private passive copies in original segment order, then resets
the sole admitted mutable compiler global, the stack pointer. Other mutable
globals, passive input initialization or dynamic data/table operations refuse
this profile. Tables and immutable globals are freshly instantiated by Wasm.
The retained metadata/payload gap is never part of either reset range.

This relies on the reviewed bridge's synchronous entrypoints dropping all
Global-owned objects before returning: no application owner, reference, static
cache, pending callback or external resource can survive in that domain. Losing
allocator free lists is allowed because the entire domain is discarded; there
is no attempt to interpret or destroy an old allocator through new Rust. The
target begins with its own initialized data/BSS and a fresh heap. This extends
the ordinary foreign backing argument, not arbitrary heap-layout compatibility.
Whole-domain clearing is a bounded qualification mechanism, not a scalable
production allocator or evidence that physical pages are returned to the IC.

Reopening is explicit in the target artifact. Wrong native geometry, frozen or
unknown state refuses before reset; replacement recovery remains separately
selected and requires a fresh native region and checkpoint epoch. A compatible
upgrade neither reads nor writes the checkpoint. Installed tests must exercise
checkpoint allocation after reopening, retained native bytes, grown/poisoned
transient memory and subsequent export/replacement recovery before claiming
that these paths are connected. The [installed evidence](../ic-storage-retention-probe/native-checkpoint.md#compatible-reopening)
records that bounded proof separately from the original recovery cohort.

## Export, publication and reconstruction

Bridge metadata format 2 adds the u64 record counter at 65600. A fresh source
initializes it to 41. Freeze first checks the complete native descriptor, then
marks retained custody frozen (settlement 2, phase 3). The checkpoint runtime
receives only a bounded count and copied u64 counter/elements through checked
scalar callbacks. No reference or raw pointer crosses this boundary. No native
Rust owner remains while the provider reads its payload for export.

The existing codec writes only logical values, epoch and framed integrity data.
The shared provider adapter opens the same governed key, using fresh runtime and
lease objects. Source freeze and complete stable-cell publication are in one IC
transaction. Any failure traps; rollback must restore both native and stable
state. A frozen source cannot mutate or claim. Repeating export while frozen is
allowed; epoch authority is supplied by this isolated test coordinator, not an
authenticated production API. Cancellation/resumable export are not implemented.

Recovery checks the expected epoch, framing, schema, ownership and digest before
touching the target native region. It does not inspect or destroy source native
objects. A private WAT construction phase resets fresh target metadata, writes
successive initialized u64 elements, and requires the completed count before
publishing length/capacity. All payload stores remain within the new provider's
allocation. No native Vec exists during construction. Only after the checkpoint
runtime and decoded Global owners have ended does the wrapper enter native Rust,
issue the initialized target prefix through Region and settle it normally.

Imported capacity equals length; original spare bytes/addresses are not portable.
Traps after a stable write or during target construction abort the upgrade,
preserving the source module, frozen source state and checkpoint for retry.
The provider locator is independent of discarded native memory, and every
canister-side stable access remains through the same governed Storage handles.

## Limits

Artifact checks bind final bytes to the reviewed inputs; they do not authenticate
hostile source code or repair Rust undefined behavior. The compiler and pinned
Storage/standard-library implementations remain trusted within their ordinary
allocation contracts. This is one integer and a unique u64 vector, with no
application Drop, alias graph or concurrent/reentrant application code.
Relocating the provider and discarding the transient runtime is a bounded
incompatible-domain proof, not an actual compiler/SDK transition. Production
authority, durable epochs, resumable progress and arbitrary native heaps remain
separate gates.
