# Scoped ownership over live arena allocations

The existing safe ownership model now accepts `Vec<u64, A>` with an explicit
allocator. Its default remains Global. This connects the original scoped-access
experiment to the private live-arena kernel without introducing another view API
or granting retained-layout admission. The library still forbids unsafe code.

## Ownership and allocator lifetime

`Host::from_vec` consumes the counter and live vector without reallocating or
changing its allocator. It returns an oversized vector unchanged rather than
truncating it. The coordinator owns Host; an application callback receives only
its frame, scalar operations, borrowed scalar slices and tracked ownership
tickets. It receives no owning vector, allocator or mutable Host reference.

Host is generic over `Allocator + Clone`. Detach clones an allocator only to
create the empty replacement root; the detached vector always retains its own
allocator. Attach moves that same vector back, while retire destroys it and
leaves the empty replacement. No existing allocation is handed to a cloned
allocator, so the generic mechanism does not assume clone equivalence. This is
not admission of arbitrary allocators into persistent state. Most arena tests
use borrowed `&Arena` allocators, whose copies refer to the same live domain.
Two additional cases use a private forwarding allocator whose Clone selects a
different live arena or panics. It does not implement `AllocatorClone` or promise
equivalence. Attachment retains the original domain; retirement releases that
domain, and subsequent growth uses the empty replacement's allocator. A clone
panic occurs before detachment, preserves the source allocation and leaves Host
poisoned even after the injected failure is disabled. Dropping Host reclaims the
original run without touching another domain's allocations.

The allocator borrow travels with the vector into Host. Its lifetime prevents
discarding the allocator while Host still uses it. A positive documentation case
and an E0505 compile-fail case check this with a borrowed standard allocator;
the integration cases use the actual private arena. Existing ticket/reference
escape cases continue to exercise the same higher-ranked frame interface.

## Mutation, quarantine and reclamation

The host poisons itself before running callback code and clears that state only
after normal return with all detached ownership settled. Business errors preserve
completed writes. A panic, including one from allocator cloning, leaves the host
poisoned. Forgotten tickets retain their actual vector in the private owner slot;
there is no destructor-based authorization to reclaim its run or reopen access.

Discarding the whole live host drops its owned vectors through their original
allocators. This reclaims arena runs even when the host was poisoned. It is safe
for the admitted u64 subset because no reference or raw/provider loan escapes
the frame. It does not establish rollback, arbitrary application Drop safety,
shared-owner handling, page return or destruction through an incompatible runtime.

A record freshly reconstructed by the transfer fixture can enter Host without
another allocation. The test coordinator consumes the entire record, operates
through frames and discards Host before inspecting destination reclamation.
There is no new inverse operation extracting owning values from a frame or a
poisoned Host, nor a combined persistent lifecycle/publication API.

## Evidence

Five arena cases cover unchanged allocation on entry, ordinary Err followed by
logical export, actual capacity refusal, retirement/reallocation, forgotten
authority, panic quarantine and oversized-input refusal. A sixth case connects
fresh native allocation issuance to scoped ownership and final reclamation.
Run the allocation target for these cases and the original arena regressions;
the library's ownership tests and documentation tests cover its generic interface.

This is host-only evidence for one live counter/vector. It does not settle the
IC backing-provider or cross-instance pointer-validity gate.
