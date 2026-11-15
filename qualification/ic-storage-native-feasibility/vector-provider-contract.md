# Single-vector foreign allocation contract

This extends the [fixed-scalar argument](scalar-provider-contract.md) only for
the reviewed `provider_vector.rs`, `region.rs`, scoped
[`ic-storage-native`](../../crates/ic-storage-native/README.md) core, provider WAT and final linker on
Rust 1.100.0 / LLVM 23.1.5 / wasm32-unknown-unknown. It permits the bounded
installed qualification fixture; it is not a public safe API or general retained
Rust heap contract. Passing a machine-code profile alone does not authorize an
unknown source executable.

## Backing, representation and initialization

The provider owns one external allocation `[131072, 135168)` outside LLVM, live
for the Wasm instance and disjoint from Rust allocations. This is the same
explicit foreign-allocation integration decision as the scalar fixture. On each
new instance the provider establishes its own allocation over retained physical
bytes; it does not resurrect Rust pointers or allocation identity from the source.
The opaque foreign return supplies the pointer. Rust does not cast a decoded
address into a pointer. The independently linked provider remains outside LLVM
optimization. [LLVM pointer rules](https://releases.llvm.org/23.1.0/docs/LangRef.html#pointer-aliasing-rules).

The complete retained inventory is seven byte-defined u64 metadata words and
at most 512 u64 elements. The provider format owns metadata offsets and the
initialized element count. The native element representation is the wasm32
little-endian u64 with eight-byte size/alignment and no invalid bit patterns;
the provider base satisfies that alignment. No Vec, Region, Cell, reference,
executable pointer or allocator header survives a call. Their private layouts
are therefore not inputs to reopening. Equal addresses or matching sizeof
would not justify retaining any of those headers.

Fresh installation initializes the entire payload before publishing metadata.
Subsequent admitted operations preserve the initialized prefix by construction.
No spare capacity is interpreted as elements, even when old bytes remain there.
The provider checks the complete header, `length <= capacity <= 512`, readiness
and settlement before issuing a pointer; the wrapper admits those same words
before narrowing. A matching header is not proof of source history: the harness
installs only the reviewed source fixtures and their explicitly injected metadata
faults. Faults never authorize reading a fabricated prefix.

## Fresh issuance and ownership

The claim marks the provider unavailable before returning the base. Its lifetime
covers the controller and every client. While claimed, provider helpers read
only disjoint metadata and implement the current Rust client's checked accesses;
they never access payload on behalf of another owner. The complete helper closure
has no host callback, Rust re-entry or concurrent writer. Memory is unshared and
fixed at three pages; no growth or remapping invalidates the backing.

`Region::claim` borrows this live backing; it does not manufacture allocation
authority from the descriptor. For nonzero capacity, `restore` issues one exact
eight-byte-aligned prefix through `Allocator::allocate`, without altering its
initialized bytes. The allocator returns exactly the requested size, so the
fitting deallocation layout is exact. `Vec::from_raw_parts_in` receives that
freshly issued block, the same allocator, exact capacity and initialized length.
There is no intervening fallible operation or callback. Zero capacity requires
zero length and uses `Vec::new_in`, with no payload dereference.
[Allocator contract](https://doc.rust-lang.org/std/alloc/trait.Allocator.html),
[Vec reconstruction](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.from_raw_parts_in).

Only one block can be live. Growth/shrink reissue its prefix within the backing,
preserving the common initialized bytes; callers use the returned block after
success. Failure leaves the old block unchanged. Deallocation makes the prefix
available to this controller again and touches no payload. It does not return
Wasm pages to the platform. Controller moves do not move the backing; there is
no Clone equivalence, callback or unwinding allocator method.

The six operations use ordinary Vec methods on u64 through the shared scoped
core: mutate first, reserve/push,
remove first, shrink, clear/shrink, and destroy/reallocate/push. Push writes before
publishing a longer prefix; removal preserves the surviving elements; clear and
destruction invoke no application Drop. Partial allocation failure traps. Source
code initializes all controller/Vec fields and stack values before reading them;
retained stack bytes are never treated as old live Rust state.

Release verifies the controller, pointer and exact capacity, consumes the only
Vec through its public raw-parts API and seals the controller. The returned value
contains only length/capacity. No reference, pointer, owner, iterator, future or
closure can escape this fixed private entrypoint. The allocator instance ends
before the wrapper settles. A leaked owner would keep issuance unavailable;
memory safety does not depend on a guard destructor. This closed entrypoint does
not expose arbitrary canister callbacks. Its shared safe `with_vector` interface
owns only a scoped borrow and exposes scalar slices and restricted operations,
never the owning Vec or allocator. Compile-fail tests reject escaping views,
references, capturing futures, owner extraction and consuming iteration. Forgetting
the view leaves the actual Vec with this entrypoint; it has no Drop-based
settlement obligation. This safe layer admits no retained pointer or descriptor.
Arbitrary application callbacks, owning elements and frame poison/publication
remain outside the installed closed-entrypoint contract.

## Machine boundary and failure semantics

The final linker keeps raw Rust functions private, resolves all claim/access
imports, shares the admitted memory, and checks the bounded stack/call profile.
The Rust linear stack is bounded to `[32688, 32768)`, disjoint from metadata/payload.
The current eight-body profile has a 48-byte entry frame and nested 16-byte helper
frames, with at most 80 bytes simultaneously live. Each frame must reserve and
restore the same amount through an unmodified local. An acyclic direct call
graph supplies the maximum sum; early return, implicit-function-label branches,
effects/control after restoration and unknown operators refuse. Plain numeric
Select is admitted but cannot replace or bypass frame reservation/restoration.
All 76 emitted memory sites call guards for this stack or the operation's payload
window. Helpers use Wasm locals, not Rust stack slots. These post-codegen guards
enforce machine bounds; they cannot fix earlier Rust undefined behavior or prove
initialization, aliasing or source identity. The argument above must hold first.

The wrapper settles only after Rust returns and resets its operation selector
before replying. Source pre-upgrade requires idle/settled custody. Destination
admission precedes its first native operation, using a fresh controller/pointer.
Traps abort the whole IC message or upgrade; no local continuation, poison reset
or destructor cleanup is attempted. Fault tests compare the complete header and
payload and the installed module hash after refused upgrades. They do not claim
that all transient scratch bytes are directly observed.

Returning a business Err would not undo earlier writes. This fixture has no
business-error API: reserve failures and invalid metadata trap. Native-host
poisoning, arbitrary Drop, shared/cyclic ownership and general installed application
callbacks stay outside this subset. Capacity-reuse evidence is not platform-memory reclamation.

## Remaining migration boundary

The logical portable value is the ordered initialized u64 sequence, excluding
capacity, addresses, controllers and spare bytes. The existing
[checkpoint format](checkpoint-format.md) now has an explicit 512-element envelope
and installed governed-provider recovery at that bound. Version-1 bytes and its
16-element limit remain unchanged. The separate
[native checkpoint bridge](../ic-storage-retention-probe/native-checkpoint.md)
now performs that connection under an expanded memory/custody contract, including
fresh reconstruction at a different native base. This original fixture does not
itself run that path. Application-type admission and a genuine compiler/runtime
transition remain separate proofs.
