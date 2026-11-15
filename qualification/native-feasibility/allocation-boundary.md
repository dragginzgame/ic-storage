# Live allocation boundary experiment

This experiment covers live-host allocation custody, not IC retention. It uses
one bounded arena and `Vec<u64, &Arena>` on Rust 1.100.0. Its private integration
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
Only u64 elements are admitted, so dropping a vector invokes no application Drop.
Default allocator growth copies only the byte spans its contract permits.
Here, u64 eligibility is enforced by the parking API and the fixture clients;
the allocator's layout restrictions alone are not a general type-admission test.

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
reconstructs logical values after source teardown. Provider-backed export/import
and installed recovery remain prerequisites for persistent state admission.

The 15 November 2026 contract review found no bridge in the consulted sources
from IC memory retention to Rust allocation-domain validity. The
[IC upgrade contract](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/)
preserves Wasm memory in keep mode while creating the new instance; Rust's
allocation/provenance and Allocator lifetime requirements remain separate.
This is an unresolved integration obligation, not a claim that native reopening
is impossible. No cross-instance unsafe cast was added on that assumption.

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
