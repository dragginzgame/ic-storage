# Live backing transfer and fresh allocation issuance

This private host fixture replaces the allocator controller while preserving its
original backing Box. It exercises the first step of the
[allocation-domain investigation](../../docs/design/native-allocation-domain.md).
It supplies no IC region provider, retained Rust pointer or persistent ABI.

## Custody transition

An admitted single-root suspension can either resume or prepare transfer.
Preparing transfer validates the same image, ledger and original token as resume,
then consumes the token into a private source release record containing the
vector descriptor and counter. The source remains frozen. A second preparation
or resume refuses; losing an ordinary suspension never creates a release record.

`Transfer::take` consumes the complete source Arena, rechecks the release against
its whole ledger, then moves only the actual backing Box and released root into
a unique Transfer object. Refusal returns the source unchanged. Moving the Arena
requires its safe outstanding borrows to have ended. Its controller and ledger
end before the destination is created; the Box's allocation stays live throughout.
No allocation, copy of payload bytes or new raw pointer is needed for this move.

Transfer has no cloning, serialization or raw-address constructor. Dropping it
discards its backing; forgetting it leaks that backing rather than authorizing
another claimant. The consumed Rust owner is the authority, not a matching image,
address or descriptor. This proof assumes the private trusted constructors, not
arbitrary internally forged release records.

## Fresh destination controller

Destination admission accepts a usable prefix of 1 through 64 words within the
fixed backing Box. It checks the complete source span, including spare capacity,
against that prefix before creating a controller or reading payload. Invalid
geometry returns `Domain`; a span outside it returns `IncompatibleExtent`. Both
return the original Transfer unchanged. There is no automatic checkpoint fallback.

Consuming an admitted Transfer creates a destination with an empty allocation ledger, fresh
generation counter and the released span reserved separately. It starts frozen.
Reserved capacity is excluded from reusable-byte reporting even though no new
allocation has yet been issued. No normal client can acquire the reserved span.

Restore checks controller state, empty-ledger consistency and the reservation
against the released root. It briefly enables the allocator and calls its normal
`allocate` operation with the exact retained layout. The reservation selects the
original offset, including nonzero offsets, and is cleared only after successful
issuance. Wrong size/alignment, malformed extent and generation exhaustion refuse
without changing the ledger or reservation. Allocation derives a new pointer
from the same still-live Box and never reads or writes its payload.

Only then does the private raw-parts boundary build a fresh Vec with that pointer,
the destination allocator, exact capacity and source-established initialized
length. The old Vec was consumed before transfer; no old owner survives. Moving
the Box does not move its allocation, and all destination pointers derive afresh
from its current owner. No historical pointer is dereferenced or reconstructed
from an integer. A zero-capacity root uses Vec::new_in without allocation.

There is no callback or fallible step between successful issuance and returning
the completed counter/vector record. The returned allocator borrow keeps the
destination alive. Consuming the root prevents a second restore, including after
the returned record is forgotten. Subsequent growth, shrink, removal and release
use ordinary allocator operations; freed runs become reusable, not returned pages.
Normal allocations and reusable-byte reporting honor the destination's prefix;
having physically larger backing does not permit allocations beyond that limit.
The reconstructed record can now enter the existing [scoped owner](scoped-ownership.md)
without reallocating; dropping that host returns the vector's run to this arena.

## Failure and scope

Pre-issuance refusal preserves source or destination custody. Allocation failure
restores the frozen flag and retains the reserved root for quarantine. No partial
Vec is published. The single-span sequence has no recoverable failure point after
issuance; adding another allocation or fallible construction step would require
an explicit partial-construction protocol. No general rollback is implied.

The fixed Box is the independently owned backing region in this experiment.
An IC upgrade still needs its own provider, initialization, exclusivity and
startup contract before it can supply equivalent backing validity. Neither this
transfer object nor its release record is retained across Wasm instances. The
source counter is explicitly supplied by the fixture coordinator; this is not a
durable publication or application quiescence protocol.

The [checkpoint integration](checkpoint-format.md#native-refusal-and-explicit-portable-recovery)
now contrasts native transfer within the full region with explicit recovery into
a smaller region that cannot contain the old allocation. This is an enforced
host allocator-geometry difference, not a compiler or IC runtime transition.

## Evidence

Twelve transfer tests cover nonzero offsets, single issuance, continued mutation
and complete run reuse, empty/cleared roots, lost suspension authority, unchanged
live-source refusal, corrupted source metadata, wrong allocation layout,
allocation failure, altered destination spans and forgotten reconstructed owners.
They also check invalid/incompatible prefixes, exact fits and whole-ledger
admission against the usable region. Two checkpoint tests exercise the native
and explicit portable routes. Scoped-owner integration brings the allocation target
to 59 tests:

```sh
cargo clippy --manifest-path qualification/Cargo.toml \
  -p ic-storage-native-feasibility --target-dir target/qualification \
  --locked --offline --test allocation_tests -- -D warnings
cargo test --manifest-path qualification/Cargo.toml \
  -p ic-storage-native-feasibility --target-dir target/qualification \
  --locked --offline --test allocation_tests
```

The unsafe argument adds only fresh Vec construction after actual allocation;
the allocator retains its original [live-domain argument](allocation-boundary.md).
No unsafe permission is added to the ownership library or product crate.
