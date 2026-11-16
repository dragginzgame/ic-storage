# Scoped native vector core

Unpublished experimental `no_std` + `alloc` library for operations over an
already valid `Vec<u64, A>`. It forbids unsafe code and has no runtime dependencies.
The caller retains the owning vector; `with_vector` lends a restricted scoped
view for scalar reads/writes, append, ordered removal, clear and shrink.

The view exposes neither `&mut Vec`, allocator access, owning iteration nor
Clone. References, views and capturing futures cannot escape the scope. Forgetting
a view does not forget the vector, so no guard destructor is needed for safety.
Ordinary returned errors preserve earlier mutations. The caller owns panic/trap
handling, native-host poisoning and successful-frame publication.

The installed single-vector provider uses this library after its own admission,
foreign-allocation claim and fresh allocator issuance. The library does not
validate retained bytes, reopen pointers, certify private layouts or implement
a persistence runtime. Its safe constructor accepts live Rust vectors, not saved
descriptors. Native owners and scalar record conversion remain runtime work.

Run `make native-core-test` for host behavior and ownership compile-fail tests;
`make native-provider-test` exercises the library through the installed provider.
The existing checkpoint/job gates additionally exercise portable reconstruction.
Installed scope cases cover normal callback errors, full-capacity refusal and
forgotten views across compatible upgrades. They check settlement before reply
and distinguish these outcomes from whole-message trap rollback.
The separate [provider-host fixture](../../qualification/ic-storage-native-feasibility/provider-region.md#host-poisoning-and-explicit-replacement)
tests caller-owned poisoning across caught unwinds and explicit checkpoint
replacement using the same core and region controller. The core itself does not
own that failure policy or make unwinding transactional.
