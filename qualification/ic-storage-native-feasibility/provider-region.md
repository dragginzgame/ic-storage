# Transient allocator over a provider loan

This private single-vector experiment bridges the existing live-Box transfer
tests and the fixed-scalar foreign provider. It does not extend the installed
scalar admission profile. Installed vector use has its own narrow
[foreign-allocation contract](vector-provider-contract.md).

## Allocation and ownership argument

The controller accepts one exclusive, live backing allocation through an unsafe
constructor. A pointer, numeric extent and descriptor are insufficient: the
caller must establish provenance, initialization and custody for the entire loan.
Host tests supply those facts through an exclusive borrow of initialized storage.
The Wasm fixture now has an admitted provider and bounded startup/artifact
contract extending [the scalar argument](scalar-provider-contract.md).

The controller does not own a Box, Global allocation or retained Rust reference.
Its pointer, lifetime marker and Cell bookkeeping are transient. The loan is
bounded to 512 u64 words, with checked alignment, length and capacity. It reserves
the root's exact capacity, blocks ordinary allocation, and issues that prefix
through Allocator::allocate only during restoration. Issuance performs no payload
writes. Vec::from_raw_parts_in then receives the newly allocated pointer, exact
capacity, initialized length and this allocator; no fallible operation or callback
intervenes. No private Vec representation is retained or decoded.

One block may be live at a time. Growth and shrink change its issued prefix in
place, within the same exclusively loaned backing allocation. Failed allocation
or growth leaves the current block and bytes intact. The allocator returns exact
sizes, accepts only nonzero u64-aligned multiples, and never unwinds. Its default
grow_zeroed cannot acquire a second block and refuses without changing the first.
Ordinary allocate_zeroed is allowed only on a free controller, never a reserved
root. These restrictions satisfy a bounded allocator, not arbitrary allocation
requests or element-type admission. [Allocator contract](https://doc.rust-lang.org/std/alloc/trait.Allocator.html).

Release consumes the vector through its public raw-parts API after checking the
issuing controller, pointer and exact block. It seals further access and returns
only length/capacity. No pointer or private header is retained. Refusal returns
the original vector; forgotten owners keep their block unavailable. Dropping a
vector deallocates its block for reuse but cannot replay the old root descriptor.
No application destructor runs because the only reconstructed element is u64.

The provider must not settle or reissue the loan until the controller and all
clients have ended. This private unsafe boundary is not the safe application
scope API: source release, forgotten-loan quarantine and source/target identity
must still be enforced by the eventual provider wrapper. A copied descriptor
alone cannot authorize another claim. The host replacement test retains its
original backing allocation, so it proves no cross-instance allocation identity.
Allocator reuse does not return backing capacity or Wasm pages to the platform.

## Validation and next boundary

`make native-feasibility-test` selects the host tests alongside existing ownership,
transfer and portable-checkpoint tests. They cover fresh issuance without payload
writes, controller replacement, repeated restoration, reserved/duplicate allocation,
malformed roots, empty/cleared roots, wrong-controller release, forgotten owners,
growth failure, shrink/removal, zeroing and reuse.

On 15 November 2026 the eight new host tests passed on x86_64 Linux with Rust
1.100.0. The complete native-feasibility target passed 100 tests (including
doctests), with its existing compiler-layout-input test intentionally ignored by
the ordinary test invocation. Strict Clippy passed. Spare-capacity coverage also
uses genuinely uninitialized slots and reads only the admitted initialized prefix.

`make native-provider-probe` now also compiles
[provider_vector.rs](tests/fixtures/provider_vector.rs) using the same controller.
It exercises compilation of reconstruction, mutation, append, removal, shrink,
clear and reuse through ordinary Vec operations. Linking alloc requires a Global
allocator even though the vector explicitly selects Region; the fixture supplies
one that refuses every request and traps on invalid deallocation. It grants no
default-heap allocation or growth authority.

The diagnostic reserves a 32 KiB transient stack, fixes memory at three pages,
and exports compiler boundaries for inspection. The successful raw cohort is
`target/evidence/provider-link/attempt.Y75Ei5/vector/`. Its 1628-byte Wasm has
SHA-256 `3d40321d831bd7cc82aba77a6e16e97b44399e095eeefba6dab7f9eeda7d20af`:
two defined functions, no data bytes, one unresolved diagnostic provider import,
stack/data/heap boundaries at 32768, two global writes and one memory.copy.
Optimized IR retains the foreign pointer call, transient controller/header stack
storage and the removal memmove. Empty vectors use std's non-dereferenced dangling
pointer; that is not a retained-region pointer-creation mechanism.

Four artifact diagnostic tests passed, including refusal of this vector module
by the exact scalar admission profile. The inventory checks bounds, imports,
startup/growth exclusions and selected compiler structure; it does not establish
every load/store address or stack bound on every path. **This cohort neither linked
nor executed vector Wasm.** No private layout or installed-vector qualification
follows from compilation, the reported artifact size or the host lifecycle tests.

## Transient stack and direct-call proof

The 0.1.7 diagnostic adds a deliberately narrow control-flow check to the actual
vector Wasm. Only the exported entry may change the stack global. Its entry
sequence reserves one positive, aligned constant frame in a protected Wasm local;
that local cannot be reassigned. The only later stack write must restore exactly
that amount from the saved local. The normal return follows restoration through
a straight-line sequence without calls, effects or control-flow entry points.
All other entry exits must trap. Local block branches may not target the implicit
function label and bypass restoration.

Helpers cannot read or change the stack global. The complete direct-call graph
must be acyclic; indirect/tail calls, loops, exceptions, growth and unrecognized
operators refuse. The foreign provider remains a conditional non-reentrant leaf:
its future linked implementation must establish that assumption. This check
starts from validated Wasm and the inspected import/export/global roster.

Nine diagnostics passed on x86_64 Linux with Rust 1.100.0 on 15 November 2026,
including five tests using valid Wasm counterexamples for stack mutation,
restoration mismatch, early returns, branch escapes, recursion and unsupported
control. The actual vector reports a 64-byte frame, stack-pointer floor 32704
from initial 32768, and two direct helper-call sites. The raw vector digest is
unchanged from the cohort above. New raw evidence and the full operator listing
are under `target/evidence/provider-link/attempt.YrVrf1/`.

This proves the admitted shape's **stack-pointer discipline**, not that every
load/store falls inside its frame or the provider payload. Helper pointer
arguments, initialized vector span, memory.copy bounds, complete retained metadata
and the final linked provider remain independent admission obligations. The
vector still cannot pass scalar admission; that cohort did not execute it.

## Installed boundary and remaining recovery

The [installed vector metadata protocol](../ic-storage-retention-probe/vector-provider.md)
now checks full-width length/capacity before narrowing, refuses invalid source
headers and publishes replacement metadata only through settlement. Its WAT-only
tests compare the entire header and payload after rollback. The accompanying
source-level access audit records the Rust candidate's initialized-span and copy
obligations; it does not yet bind all compiled accesses or execute that candidate.

Installed use now combines final vector memory-access and entrypoint checks with
the explicit allocation/initialization argument. The scalar combiner still rejects
additional code. Numeric descriptor checks alone do not establish backing custody
or initialization. The portable scalar/vector format remains a separate required
recovery contract. Its new 512-element envelope and governed provider are tested,
and the separate [native bridge](../ic-storage-retention-probe/native-checkpoint.md)
now connects retained values to fresh reconstruction in a relocated domain.
The [checked-access experiment](../ic-storage-retention-probe/vector-access.md)
now routes all 49 compiled memory operations through unresolved helper imports.
Installed WAT tests exercise the helpers' operation-specific access windows;
the intermediate rewrite remains unresolved. The subsequent
[link experiment](../ic-storage-retention-probe/vector-link.md) binds those imports
and checks trusted entrypoint ordering. Its subsequent installed lifecycle now
executes the bounded native vector under the separate allocation contract;
general runtime migration remains outside that original lifecycle fixture.
