# Fixed-scalar foreign allocation contract

This is the implementation argument for executing the bounded scalar provider
fixture with Rust 1.100.0, LLVM 23.1.5 and wasm32-unknown-unknown. It authorizes
only this research fixture's admitted scalar accesses. It does not admit retained
Vec headers, allocator blocks, references, arbitrary records or production use.
The [provider investigation](provider-boundary.md) owns the surrounding experiment.

## Selected integration boundary

Treat `[131072, 135168)` as one mutable external allocation managed by the
independently authored Wasm provider, outside LLVM. The provider defines the
region, initializes its first word on fresh installation, and exclusively issues
its base through the foreign call. A newly admitted instance owns a new external
allocation over the retained physical bytes. No Rust allocation identity,
allocator controller or pointer is transferred from the previous instance.

This is our explicit compiler/runtime integration decision for this fixture,
not a claim that memory retention or an FFI declaration creates an allocation.
LLVM's pointer rules permit foreign-returned pointers to be associated with
external allocation ranges disjoint from LLVM-created allocations. Rust still
requires valid allocation-backed access. The concrete ownership, initialization,
disjointness and lifetime obligations are discharged below rather than inferred
from a successful test. [LLVM 23.1 pointer rules](https://releases.llvm.org/23.1.0/docs/LangRef.html#pointer-aliasing-rules),
[Rust allocation requirements](https://doc.rust-lang.org/std/ptr/index.html#allocation).

## Backing allocation and pointer creation

The complete module has one ordinary, unshared, fixed three-page Wasm memory.
The provider reserves its 4096-byte region independently of all Rust code and
keeps it alive for the instance. The base is nonzero and aligned to eight bytes;
the region is contiguous, mutable, within physical memory, smaller than isize::MAX
and cannot overflow wasm32 addresses. There is no growth, shrinking, remapping,
external memory writer or second memory in the admitted module.

Rust has no data segments, initialized statics, Global allocator or retained
object. Its exported stack/data/heap boundaries are exactly 65536. Admission
restricts its two bodies to a foreign call, null test and the first u64 load or
store; locals remain Wasm locals, with no linear-stack access. Provider metadata
starts at 65536, payload starts at 131072, and reply scratch is used only after
Rust returns. There is no concurrent or reentrant Rust execution in this fixture.

Rust's source of pointer provenance for each claim is the return from the opaque
foreign function. Rust neither manufactures it from an integer nor retrieves an
old pointer from memory. The implementation of that function remains outside
LLVM after the Rust optimization/link step. The combiner resolves it by name,
remaps indices and validates the final Wasm. The provider owns the entire range;
the emitted numeric Wasm base is its implementation, not independent evidence
that an arbitrary address is a valid Rust allocation.

## Initialization and exclusive access

Fresh installation writes the first u64 before marking the provider ready.
Only that word is accessed through Rust. Subsequent admitted writers store u64
values; every bit pattern is valid. Source and destination share the documented
eight-byte initialized-payload contract. Unused capacity is never interpreted.

This argument assumes the admitted, trusted source history. A matching header
does not authenticate arbitrary source code or certify arbitrary initialization.
The harness installs only the constructed and admitted A/B fixtures. Production
source identity, complete root inventory and general representation certificates
remain separate work.

Claim first checks the complete header and geometry, requires ready/settled
state, clears the retained settlement word and marks the transient phase claimed.
Only then does it return the pointer. A duplicate claim traps before access.
While claimed, the provider touches only its disjoint metadata; it does not read,
write or invalidate the payload on behalf of another client. The test-only WAT
observer runs in a separate call, after all Rust frames have ended.

The admitted Rust body has no callback, reference construction, pointer store,
pointer return, destructor or allocation. It returns either one copied u64 or
unit. Thus all pointer use ends when it returns to the trusted Wasm entrypoint.
The entrypoint then calls settlement exactly once before replying. This ends the
claim without relying on Rust Drop. The raw Rust functions and settlement helper
are not exported from the final module. Direct-call bodies and call ordering are
part of admission, not a convention applications are asked to follow.

## Upgrade and failure ordering

The source pre-upgrade hook requires settled custody. The target independently
checks its retained header and geometry before readiness, then its generated
post-upgrade entrypoint performs a native read and settles. No source Rust frame
or pointer crosses instantiation; the destination obtains a fresh pointer from
its own provider. The IC retains bytes and supplies transactional upgrade rollback,
not the Rust allocation identity used in this argument. [IC upgrade contract](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/#canister-upgrades).

The Rust panic strategy is abort; the fixture handler traps. There is no local
catch, poison reset or continuation with a partly completed claim. A trap after
native mutation relies on IC message/upgrade rollback. A deliberately abandoned
claim commits an unset settlement word; subsequent native access and upgrade
refuse. No safe recovery operation clears it. A business-level Err would not undo
writes; this scalar probe exposes no business-error API.

## Admission and scope of evidence

Before installation, the host checks the exact scalar instruction profiles,
memory geometry, boundary globals, signatures and import/export rosters. It
constructs only reviewed lifecycle glue and the trusted provider source, validates
the result, and requires exact final-byte agreement with that construction and
the selected profile. The independent artifact test follows the existing WAT
client's claim/settlement calls and verifies the Rust call chain and ordering.

This establishes a narrow, reviewable unsafe-code argument under the selected
foreign-allocation model. It is not a formal language proof, a hostile-module
sandbox or a general admission algorithm. Unknown compiler output refuses;
successful installed tests supply lifecycle evidence only. If later compiler,
runtime or collection requirements contradict this model, refuse that expansion
and revisit the integration decision rather than extrapolating from this scalar.

Next, an installed vector would additionally require fresh Allocator issuance,
owner confinement, initialization tracking, growth/reclamation and the independent
portable recovery route. This scalar fixture satisfies none of those by itself.
