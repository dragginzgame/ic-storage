# Native allocation-domain feasibility

Status: candidate experiment, 15 November 2026; gate 1 remains open. This note
narrows the next native investigation after the live-arena suspension and
checkpoint proofs. It is not permission to interpret retained bytes as Rust
objects, a selected product ABI or a completed unsafe-code argument.

## Separate backing memory from allocator-issued blocks

The original host suspension fixture keeps the Box, allocator and unique token
alive. It therefore does not test replacement of the allocation authority.
Its [suspension](../../qualification/ic-storage-native-feasibility/suspension.md)
and [checkpoint](../../qualification/ic-storage-native-feasibility/checkpoint-format.md)
evidence stays valid within that boundary.

The next mechanism must establish two things independently:

1. **Backing-region validity.** The destination has a live, mutable, aligned
   allocation covering the complete region, with a pointer derived through its
   actual owner and a lifetime covering every client. Bounds, address equality
   and memory retention are insufficient. Rust defines pointer operations in
   terms of allocations and their provenance. [Pointer contract](https://doc.rust-lang.org/std/ptr/index.html#allocation).
2. **Allocator authority.** Each vector's block is currently allocated through
   the destination allocator or a justified equivalent. The Rust 1.100 contract
   ties that status to allocation operations and invalidation rules, including
   allocator destruction and expiring lifetimes. Installing decoded bookkeeping
   is not one of those operations. [Allocator contract](https://doc.rust-lang.org/std/alloc/trait.Allocator.html).

This distinction was checked against the installed Rust 1.100.0 rust-src
(`core/src/alloc/mod.rs`, `alloc/src/vec/mod.rs`) as well as the published docs.
The existing same-domain raw-parts reconstruction continues to use its original
allocator. No equivalence across Wasm instances is inferred from that test.

## Candidate: externally owned region, fresh allocation issuance

Investigate a bounded region whose storage owner exists independently of the
transient Rust allocator controller. The provider must supply an actual backing
allocation and exclusive custody to the destination. A Rust foreign declaration
would describe that contract; it would not implement or prove the provider.
External statics have initialization and mutability obligations of their own.
[External-static contract](https://doc.rust-lang.org/reference/items/external-blocks.html#statics).

For the first experiment, keep one counter and one uniquely owned u64 vector.
Retain controlled metadata and initialized scalar payloads; rebuild the Vec
header and controller. Do not retain Box, Cell, references, parking tokens or
private std headers. This candidate deliberately does not resolve private map
nodes, owning elements, shared pointers or arbitrary native records.

Subject to a valid provider, investigate issuing each admitted span through the
new allocator's ordinary `allocate` operation, without clearing or relocating
its existing initialized payload. The destination then owns a newly issued block
at that span; it need not assert that an old allocator reference survived. Keep
all spans reserved from ordinary clients until the complete reconstruction is
admitted. Never use `allocate_zeroed` for this operation or let normal allocation
overwrite a span awaiting reconstruction.

This is a proposed implementation argument, not a guarantee supplied by the IC
or Rust documentation. Fresh allocation issuance cannot repair an invalid backing
pointer, missing source custody, invalid element values or incomplete ownership.
Only after issuance and those checks could raw-parts construction receive the
new pointer, allocator, exact capacity and initialized length.
[Vec reconstruction requirements](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.from_raw_parts_in).

## Required provider and bootstrap contract

| Obligation | Required evidence |
| --- | --- |
| Backing object | Concrete provider, extent, alignment, lifetime and pointer-creation mechanism; no pointer invented from a numeric base |
| Source release | All retained owners accounted for; no old borrow, callback or executable pointer transferred; an explicit transfer distinct from losing a token |
| Destination exclusivity | One claim of the region per admitted instance; failure or forgetting cannot produce a second owner |
| Initialization | Admitted source establishes initialized u64 spans; spare capacity stays unread; a digest does not establish initialization |
| Allocation issuance | Each span returned once with its exact layout; failed issuance cannot expose partial roots or make reserved spans available |
| Startup isolation | Final artifact proves transient reset/data/stack/allocator activity excludes the provider region |
| Retention binding | Admitted source, destination, memory geometry, lifecycle mode and root inventory agree before native interpretation |
| Failure policy | Refusal before mutation preserves custody; partial reconstruction traps on IC or quarantines on host; no implicit empty-state reset |

The IC creates a new instance during upgrade and retains Wasm memory only under
the selected keep mode; it does not preserve other Wasm state. Its atomic upgrade
rules supply the platform rollback boundary, not a Rust allocation provider.
[IC upgrade contract](https://docs.internetcomputer.org/references/ic-interface-spec/canister-interface/#canister-upgrades).

A normal Rust static represents an allocation with an initializer. Declaring one
at a convenient address, or suppressing its initialization in a post-link pass,
does not by itself justify receiving an earlier instance's bytes. The proposed
provider must establish initialization and mutation semantics under the selected
compiler, and the final artifact must preserve them.
[Static-item contract](https://doc.rust-lang.org/reference/items/static-items.html).

The existing [startup probe](../../qualification/ic-storage-rust-startup-probe/README.md)
resets its entire transient reserve and uses generated Wasm helpers for retained
scalars. It declares no Rust backing object for that retained region. Reusing its
numeric boundary or successful upgrade result cannot fill this missing contract.
Do not change the frozen M0 kernel into a native provider by relabelling it.

## Next experiment and stop conditions

The private [live backing-transfer fixture](../../qualification/ic-storage-native-feasibility/transfer.md)
now keeps one real backing allocation live while explicitly transferring custody,
ending the source controller and constructing a fresh controller. It exercises
nonzero-offset spans, empty/cleared vectors, fresh issuance without payload writes,
duplicate claims, wrong extent/alignment, issuance failure, forgotten authority
and growth/shrink/reuse. Lost ordinary suspension remains quarantined; transfer
consumes a distinct release authority. Its single allocation has no fallible step
after successful issuance; general partial reconstruction remains unimplemented.

Destination admission now enforces a usable prefix of the backing and refuses a
source span outside it. The [checkpoint integration](../../qualification/ic-storage-native-feasibility/checkpoint-format.md#native-refusal-and-explicit-portable-recovery)
discards that refused native source explicitly and restores its logical record
into a fresh smaller arena. This exercises both routes without claiming complete
runtime compatibility from a capacity check.

That fixture can establish controller replacement over a known live domain only.
Before any installed native vector, name and justify the actual IC region provider
and implement the artifact admission above. Test startup overlap and incompatible
profiles as refusals before typed access; then run the compatible lifecycle and
portable migration gates already ordered in [the architecture](../DESIGN.md#8-evidence-and-next-implementation-gates).

If the provider cannot establish a live Rust allocation, stop pointer-based
reopening and make the explicit runtime/compiler integration decision. If fresh
issuance cannot preserve custody and initialization, reject that candidate rather
than treating decoded ledger entries as allocations. Continue to preserve the
working portable recovery route. Neither more same-domain tests nor an installed
round trip can substitute for the missing unsafe argument.
