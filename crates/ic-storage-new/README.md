# IC Orthogonal Persistence

An unpublished research crate for independently implemented Rust orthogonal
persistence inspired by Motoko.

This crate currently implements the M0 evidence baseline. The
[selected native-collection and migration direction](../../docs/DESIGN.md)
is not implemented here; its native heap will require a separate admitted
representation. The existing M0 byte contract remains unchanged.

`m0::inspect(&memory_image)` performs read-only admission of the complete
experimental M0 v1 image and returns accounting/root identity or a typed refusal.
It checks the metadata digest, exact schemas, every indexed header and every
declared reference, including unreachable objects and cycles. It never resets
unknown data. The library is `no_std`, uses no heap allocation or unsafe Rust,
and obtains SHA-256 from `sha2` with default features disabled.

The result is a snapshot summary, not a live-memory capability.

`m0::inspect_retained` validates just the retained interval, with the same absolute
error offsets. `m0::image::Image` exclusively borrows that interval, starting at
linear address 8 MiB; its slice must exclude the stack and all transient bytes.
The physical-page argument still counts the whole linear memory. `initialize_fresh`
requires explicitly fresh, zeroed retained storage; `open` only admits existing
state. A physical-page count separates committed bytes from backing capacity.
Explicit `grow` zeroes new pages within that capacity without reallocating.
Sessions append Nodes and CounterState objects, publish roots, and read or mutate
scalars and Node edges. Sharing and cycles survive reopening, and allocation
continues from the retained index/cursor. Objects are never freed or reused.

`with_update` supplies handles bound to one session, with no public conversion
from a raw reference into an access capability. It is a borrow scope: an ordinary
callback `Err` preserves successful earlier writes and returns inside `Ok`.
Panic unwinding or an unexpected post-write failure poisons the owner; later
sessions refuse. Read-only bytes remain available for diagnosis. Explicitly
dropping and reopening the buffer is readmission, not rollback or recovery of
application invariants. The compiled Rustdoc example demonstrates the graph API.

Session entry, every successful write and session exit inspect the complete
image. Reads reuse the last admitted accounting while the session exclusively
owns the bytes; individual handle/schema and reference checks remain. The
summary is refreshed after writes and cannot authorize access after a mutation
fault or in another session. The library owns no IC bindings or raw memory access.
A [qualification kernel](../../qualification/ic-storage-rust-startup-probe/managed-graph.md)
now exercises the library directly against retained Wasm memory, including
installed upgrades and selected rollback cases. This API does not make arbitrary
Rust values persistent or provide production custody.

The [frozen layout](../../docs/design/m0-layout.md) owns the byte contract.
Run `make m0-test` for native golden-vector, corruption, graph, growth and poison
tests, plus the runnable example and compile-fail session-isolation checks, and
`make m0-lint-wasm` for the wasm32 compile/lint check. Neither proves installed
managed-heap behavior; `make managed-graph-test` supplies separate installed evidence.

See the repository's [research architecture](../../docs/DESIGN.md) for the
proposed persistence design, experiment plan and acceptance criteria.
