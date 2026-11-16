# Native feasibility

These experiments cover the supported runtime's scoped ownership, compiler
layout diagnostics and live allocation custody. They are fixture code, not a
product API or a retained heap implementation. The ownership-model library accepts
Global or explicit live allocators and forbids unsafe code. A separate private
integration-test kernel exercises an arena allocator under the explicit
[live allocation boundary](allocation-boundary.md); it reopens no retained data.

## Ownership boundary

`Host` owns the vector at all times. A frame offers scalar reads/writes,
append/removal, a borrowed slice of initialized `u64` elements and explicit
detach/attach/retire operations. It never exposes `&mut Vec`, an owning record,
`DerefMut`, a cloned owner or a consuming owner iterator. Owned transient copies
of scalar elements may escape. This does not generalize to owning elements.

Detaching moves the allocation to a private host slot and returns an invariant,
frame-bound ticket. The ticket contains no Vec, pointer or destructor-based
cleanup. Attach consumes it and restores the same live allocation; retire
consumes it and destroys the detached vector through its current runtime.
There is one detachable owner; [arena integration](scoped-ownership.md) uses this same API.

The host marks itself poisoned before invoking the callback and clears that
state only after normal return with no outstanding detached allocation.
Forgetting or dropping a ticket leaves the owner in its private slot and causes
quarantine. A business `Err` cannot hide that obligation. Forgetting only the
view, without an outstanding ticket, permits finalization: no independent loan
exists in this model. Panics propagate and keep the host poisoned; completed
writes are not rolled back. There is no unsafe recovery/reset-poison operation.

This implements the [Rust requirement that forgetting be safe](https://doc.rust-lang.org/std/mem/fn.forget.html)
without relying on ticket or frame destructors. In this all-safe, live-host
model, discarding the entire host can safely destroy its own allocations; that
does not justify reclamation under forgotten raw/provider loans in a future
kernel. Those loans, shared owners, arbitrary application Drop and async
execution remain excluded.

The separate [provider-host fixture](provider-region.md#host-poisoning-and-explicit-replacement)
now applies poison-before-entry and explicit checkpoint replacement to the actual
region controller and shared scoped core used by the installed vector. It checks
caught unwinds, failure after owner release or root publication, and partial
candidate failures without reopening poisoned state.
Its backing and checkpoints are host-owned; no durable provider or process-restart
contract follows from this test.

## Layout and pointer gate remains open

| Representation | Evidence available here | Retained admission |
| --- | --- | --- |
| Integer and vector elements (`u64`) | Rust scalar representation and live initialized values | Still needs actual target/domain contract |
| Host and frame | Private ordinary Rust records with transient references and poison state | Transient only |
| `Vec<u64>` header and Global allocation metadata | Live std operations; [Vec has no stable ABI/field order](https://doc.rust-lang.org/std/vec/struct.Vec.html#guarantees) | Unresolved; must not be retained |
| Ticket | Transient invariant lifetime marker; host owns accounting | Never persistent |

There is deliberately no layout certificate or reopen API. Size/alignment,
matching addresses or passing these tests would not supply complete private
representation evidence. No lifetime or provenance is carried across executable
instantiation. The compiler diagnostic experiment below exposes private header
offsets but does not select a certified std build, custom collection format or
compiler extension. These limitations describe the earlier live ownership model;
the later [provider contract](vector-provider-contract.md) qualifies a bounded
installed vector by retaining scalar payload and controlled metadata, with a
fresh transient std header. The [runtime policy](../../docs/DESIGN.md#supported-runtime-contract)
permits controlled records and application conversions to build on that evidence.
Universal layout certificates and cross-compiler migration are deferred; every
new retained representation still needs its own safety and portable contract.

## Compiler layout diagnostics

`make native-layout-probe` builds an ordinary integer/Vec record for the host
and wasm32 with the pinned stable compiler and `-C debuginfo=2 --emit=llvm-ir`.
The recipe fixes one codegen unit, optimization level zero and no stripping.
It also builds a reordered record with the same size/alignment but different
field offsets. This is a debugger-metadata experiment, not an installed canister
or an optimized production build. It uses Global, not a persistent allocator.

The narrow test reader follows the root's by-value structure/member graph,
including Vec, RawVec, RawVecInner, NonNull, capacity wrapper, Global marker and
PhantomData. Pointer nodes are leaves, not descriptions of their allocations.
It retains unknown scalar alignment as unknown and refuses missing/duplicate
nodes or roots, unsupported reachable node kinds, malformed lists, forward
declarations, recursive by-value shapes, wrong member scopes and inconsistent
member bounds/sizes. It is not a general LLVM parser or a certificate verifier.

The experiment compares the two field graphs, checks the private header paths
exist, and cross-checks host outer offsets/sizes against running `offset_of!`,
`size_of` and `align_of` observations. No private field is read by casting bytes.
The wasm32 measurements come from compilation, not execution.

Observed on 15 November 2026 with Rust 1.100.0 (`0ef1bc386`, LLVM 23.1.5):

| Target / variant | Record size / alignment | Counter offset | Vec offset |
| --- | --- | --- | --- |
| x86_64 Linux / base | 32 / 8 | 0 | 8 |
| x86_64 Linux / swapped | 32 / 8 | 24 | 0 |
| wasm32 / base | 24 / 8 | 0 | 8 |
| wasm32 / swapped | 24 / 8 | 16 | 0 |

All table values are bytes. The observed Vec header is 24 bytes on x86_64 and
12 bytes on wasm32. Capacity/pointer/length offsets relative to that header are
0/8/16 and 0/4/8 respectively. These are diagnostic observations of these exact
types and compilation settings, not a stored contract or an admission recipe.
The experiment detects the reordered record despite equal total size/alignment.

Stable compiler output can therefore supply useful private-field measurements.
It does not supply a complete persistence argument: capacity validity ranges,
initialized length versus spare storage, allocation ownership/provenance,
allocator metadata and executable interpretation/destruction still need explicit
contracts and qualification. A zero-sized Global marker does not imply that its
allocator has no external state. Debug metadata fidelity, std-build custody and
binding measurements to the final optimized executable are also unresolved.

Sources: [rustc debug information](https://doc.rust-lang.org/rustc/codegen-options/index.html#debuginfo),
[LLVM debug metadata](https://llvm.org/docs/LangRef.html#dicompositetype) and
[offset_of visibility and layout limits](https://doc.rust-lang.org/std/mem/macro.offset_of.html).
Do not turn these diagnostic measurements into native reopen permission.

## Checks

The [live allocation experiment](allocation-boundary.md) uses a fixed 512-byte
arena and real `Vec<u64, &Arena>` allocations. Parking consumes a vector while
keeping its block live. Reopening validates ledger consistency, extent, generation
and initialized length, then reconstructs a header using the original live domain
and allocator. It neither retains a private Vec header nor restores a domain
after an upgrade. Its reviewed operations and limits are recorded beside the
private unsafe kernel; no unsafe permission is added to the ownership library
or product crate.
The [ledger image](ledger-format.md) avoids Rust struct layout. [Suspension](suspension.md)
checks sole-root custody; [transfer](transfer.md) replaces the controller over live backing.

```sh
make native-feasibility-test
make native-layout-probe
```

The first command runs strict Clippy, nine ownership tests, five checkpoint
tests, sixty-one arena/allocation/import tests, six metadata-reader tests, ten
compile-fail and one positive documentation case, locked and offline. The compiled
metadata test is ignored in ordinary Cargo tests: the second command supplies
fresh inputs and explicitly runs it. `make ci` includes both on the host matrix.
Cases cover return/capture/future escape, cross-frame tickets, take/replace,
iterators, borrowed escape and ticket cloning. No shared-owner type is admitted.

Runtime cases cover in-frame moves, same-allocation reattachment, retirement,
continued allocation, capacity refusal, detached access refusal, ordinary Err,
panic quarantine, forgotten/dropped tickets, a forgotten frame and independent
transient copies. Live-arena cases additionally cover same-allocation reopening,
disjoint clients, growth exhaustion, shrink/removal, run reuse, partial
construction unwind, malformed/stale descriptors, forgotten live/parked owners,
empty/cleared vectors, unsupported layouts and generation exhaustion. This does
not establish retained allocator reuse, physical page reclamation, durable
checkpoint recovery or installed native upgrades. Host checkpoint tests cover
source teardown, failed-import retry, cancellation and native extent refusal;
explicit portable recovery preserves the existing target on failed import.

Both commands passed on x86_64 Linux with Rust 1.100.0. Raw IR, source snapshots,
compiler identity, host observations and reports remain
under `target/evidence/native-layout/attempt.*/`. COMPLETE marks successful
diagnostic collection and comparison, never persistence admission. These diagnostic
results establish neither native macOS execution nor installed native persistence.
