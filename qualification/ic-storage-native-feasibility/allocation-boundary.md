# Live allocation boundary experiment

This experiment covers live-host allocation custody, not IC retention. It uses
bounded arenas and allocator-backed u64 vectors on Rust 1.100.0. Fresh scalar-record
vectors now use the same layout-limited allocator, as described below. Its private integration
test kernel is separate from the unsafe-forbidden ownership model and M0 crate.
No application API, stored format or native reopening permission is introduced.

## Mechanism and unsafe argument

The arena owns a Box containing an UnsafeCell-wrapped array of 64 uninitialized
u64 slots. The Box is the original live allocation and stays allocated for the
arena's lifetime. Moving the arena does not move its backing allocation.
Metadata lives separately in Cell storage; no typed slice of the entire backing
array is ever created while allocations exist. The arena is not Sync.

The allocator accepts only nonzero multiples of u64 size with u64 alignment.
It returns exact-sized disjoint runs from the backing array. All offsets are
bounded before pointer arithmetic; pointers derive from UnsafeCell::get on the
original live allocation. There is no integer-to-pointer reconstruction.
Controllers may restrict allocation to a validated prefix of that backing;
whole-ledger admission and reusable-byte accounting enforce the same limit.
Allocation failure changes neither metadata nor an existing allocation. Methods
do not allocate their metadata, invoke application callbacks or unwind. The
standard Allocator grow/shrink defaults may move data between disjoint runs.

Allocation records bind offset, extent, generation and ownership state. The
monotonic generation counter refuses exhaustion rather than wrapping. A safe
allocation operation never touches or invalidates an existing client's block.
Deallocation validates the exact pointer/layout against live bookkeeping before
releasing the run; an impossible caller-contract violation aborts, never unwinds.
Pointers are compared numerically only for identity checks, not reconstructed.

Parking consumes the live Vec through its public raw-parts API. No Vec header
is serialized or reinterpreted. The allocation remains live, and its initialized
length is recorded separately from capacity. The unique parking token retains
the allocator borrow and carries a descriptor, not a saved raw pointer. It is
neither Clone nor a public constructor for arbitrary descriptors.

Reopening consumes that token and checks its generation, offset, capacity and
initialized length against the still-live allocation record before any typed
access. The pointer is derived afresh from the same live backing allocation.
`Vec::from_raw_parts_in` receives the original equivalent allocator, exact
capacity/alignment and a length known to have been initialized by the old Vec.
The old Vec and all its element borrows have ended; the new Vec is the sole
owner. Empty vectors need no block and reconstruct through Vec::new_in.

These conditions justify the private unsafe Allocator implementation, bounded
pointer arithmetic and from_raw_parts_in call **within this live-host model**.
Spare capacity remains MaybeUninit storage and is never read as initialized u64.
Only u64 elements are admitted for parking/reopening, so dropping those vectors
invokes no application Drop.
Default allocator growth copies only the byte spans its contract permits.
Here, u64 eligibility is enforced by the parking API and the fixture clients;
the allocator's layout restrictions alone are not a general type-admission test.

### Fresh scalar-record elements

The private `arena::record_elements` tests also allocate fresh `Vec<V2, &Arena>`
and `Vec<V3, &Arena>` using the existing concrete scalar declarations. Both are
repr(C), sixteen bytes with eight-byte alignment on this host; their generated
Copy requirement excludes application Drop and owning fields. These layouts fit
the allocator's existing disjoint, exact-sized runs without changing allocation
or pointer reconstruction code. No typed u64 access is made to runs occupied by
records. The backing's MaybeUninit storage permits uninitialized record padding;
std Vec moves records under its ordinary allocation contract, while export reads
only initialized fields through the generated logical codec. Neither spare
capacity nor native padding is serialized or hashed.

The construction/recovery tests use ordinary fresh Vec allocation. They never call
the u64-specific park/reopen API with records, retain a private Vec header or
reconstruct a record from raw saved bytes. Their candidate conversion/import path
keeps the existing root live until success, and ordinary destruction reclaims
failed candidates. A forgotten vector leaves its run unavailable; no destructor
is required to enforce that accounting. Reclaimed runs remain capacity of the
same Box, not capacity returned to a platform.

The [record collection experiment](records.md#fresh-record-vector-elements)
owns the bounded test-only portable format, lifecycle checks and next gate.
This is live-host evidence, not admission for retained record vectors on the IC.

The later [V3 custody extension](records.md#same-live-domain-record-custody) consumes
a live record vector into a private one-use token. It keeps the original arena
borrow and captures the exact live allocation generation, initialized length and
capacity, plus the concrete schema/layout contract. Reopening compares a supplied
descriptor with that private witness and the whole validated live ledger before
deriving a pointer from the same Box and calling `Vec::from_raw_parts_in` for V3.
The allocation never ended, no old Vec/borrow remains, and its exact type, allocator,
capacity, alignment and initialized prefix are unchanged. No saved integer pointer,
private std header or new executable is involved.

Record tokens leave the existing ledger's u64 `parked_len` unset. That field cannot
describe record initialization: V3 padding may be uninitialized, so interpreting
its occupied words as initialized u64s would be invalid. The private typed token
is the sole record initialization witness; the existing u64 reopening path refuses
that allocation. Allocation accounting continues to mark the entire run occupied.
The token is neither Clone nor constructible from an external descriptor. Successful
reopening consumes it; failure, dropping and forgetting leave the run unavailable.
No destructor is needed to prevent duplicate owners or premature reuse. This adds
one narrowly scoped raw-parts reconstruction to the private host experiment; it
does not change the allocator implementation or supply IC cross-instance authority.

The [scoped-owner clone tests](scoped-ownership.md) add a private forwarding
allocator. Each wrapper borrows one unchanged arena and forwards allocation and
deallocation to it; the arenas outlive all wrappers and blocks. Clone can choose
another arena or panic, without invalidating the original blocks. No
AllocatorClone equivalence is asserted. The narrowly allowed unsafe forwarding
implementation preserves the arena's contracts; the ownership library remains safe.

Reopening now first validates the entire live ledger: checked nonempty extents,
disjoint runs, exact agreement with the occupied bitmap, unique nonzero generation
identities below the next generation, and parked lengths within capacity. Invalid
metadata refuses without mutation, including corruption outside the requested
block and attempts to reopen an empty vector. Four focused tests include fourteen
fault injections plus live siblings, holes, exact-end blocks and exhausted
generation counters. The fixed 64-slot diagnostic uses a bounded quadratic
generation check; this is not a product allocator performance choice.

This validates structural consistency, not the truth of arbitrary metadata.
The original arena and unique token still supply the evidence that allocations
are live and elements initialized. A consistently forged ledger cannot become
trusted through this check. Its typed host representation is not a persisted
format. The separate [ledger-image codec](ledger-format.md) now exercises
versioned metadata bytes with integrity and structural checks, including active
owner refusal on export. Decoding never installs trusted bookkeeping or creates
pointers. The image cannot replace the original allocation and ownership evidence.

The [single-root suspension experiment](suspension.md) accounts for every allocated
block before freezing the arena. Its independent flag blocks allocation and
ordinary reopening until the original token is consumed by a checked resume.
Losing that token never clears the flag; no destructor is required for safety.
This adds live inventory evidence, not a retained allocation-domain contract.

## Refusal and forgetting

A malformed or stale parking descriptor refuses before creating a vector. Its
allocation remains accounted for; refusal does not guess ownership or free it.
Forgetting a vector or parking token likewise leaves its run unavailable for
reuse. There is no public reset or forced-reclaim operation. The whole arena
can be discarded once its borrow lifetime ends: no safe reference may outlive
that domain, and its MaybeUninit backing has no element destructors to run.

Allocator-reusable bytes are reported separately from the fixed backing size.
Releasing a run does not return pages to the IC or release the Box's capacity.
The older ownership experiment still owns business-error/poison/frame rules;
this kernel is a private allocator experiment, not a replacement public scope API.

## Limits and next admission decision

The allocation domain never ends during parking/reopening. This does not prove
that retained bytes after an upgrade are a currently allocated block in the new
runtime, or establish pointer provenance across instantiations. Do not treat a
copied descriptor, matching numerical address or matching layout as a token.
The arena's Cell/Box/UnsafeCell metadata is transient and is not a retained ABI.

Reconstructing ephemeral std headers through public raw-parts APIs is a candidate
alternative to retaining those private headers for this small vector subset.
It still requires a controlled retained descriptor, allocation ledger and a
target-specific allocation-domain contract. It does not establish that arbitrary
maps or collections with private allocated nodes can use the same mechanism.
No std-header strategy is selected for the product by this experiment.

Before installed native work, specify how an admitted IC memory region becomes
a live allocation domain in the new executable, how its retained ledger is
validated without typed access, and how each retained initialized span obtains
valid pointers under the selected compiler/runtime contract. If that argument
cannot be established, choose an explicit runtime integration or a supported
representation. The [host checkpoint experiment](checkpoint-format.md) now
reconstructs logical values after source teardown. The separate
[installed provider fixture](../ic-storage-checkpoint-provider-probe/README.md)
now exercises fresh-heap recovery; native admission remains unresolved.

The 15 November 2026 contract review found no bridge in the consulted sources
from IC memory retention to Rust allocation-domain validity. The
[IC upgrade contract](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/)
preserves Wasm memory in keep mode while creating the new instance; Rust's
allocation/provenance and Allocator lifetime requirements remain separate.
This is an unresolved integration obligation, not a claim that native reopening
is impossible. No cross-instance unsafe cast was added on that assumption.

The [next allocation-domain experiment](../../docs/design/native-allocation-domain.md)
separates valid backing ownership from fresh allocator issuance. Its host
[controller-transfer fixture](transfer.md) now replaces the controller over the
same live Box; the actual IC provider contract remains outstanding.

Contract sources: [Allocator](https://doc.rust-lang.org/std/alloc/trait.Allocator.html),
[Vec raw-parts reconstruction](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.from_raw_parts_in),
[UnsafeCell aliasing](https://doc.rust-lang.org/std/cell/struct.UnsafeCell.html#aliasing-rules)
and [pointer provenance](https://doc.rust-lang.org/std/ptr/index.html#provenance).
This is a repository-owned safety argument for review, not an independent audit.

## Observed checks

`make native-feasibility-test` includes strict Clippy and fourteen live-arena tests.
On 15 November 2026 these passed locally on x86_64 Linux with Rust 1.100.0.
They exercise empty and cleared vectors, disjoint allocations, same-block
parking/reopening, growth/shrink/removal, exhaustion without mutation, release
and reuse, failed partial construction, stale/malformed descriptors, forgotten
owners, fragmentation, allocator moves and generation-counter exhaustion.
Two additional checkpoint tests exercise source teardown and candidate import.
No IC installation, cross-instance
pointer restoration, native macOS execution or Miri verification is claimed.
Miri is not installed in the pinned toolchain; no toolchain change was made.
