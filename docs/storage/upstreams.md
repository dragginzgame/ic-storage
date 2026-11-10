# Collection engines and external comparison

Storage owns the public codec and memory interfaces, named backing, governed
leases and all collection implementations. Production has no ICSS dependency.

| Component | Current implementation | Implementation/qualification record |
| --- | --- | --- |
| Memory | Owned native and Platform-backed IO | [Memory](../design/memory-replacement.md) |
| Named backing | STORNAM1 version 1 | [Backing](../design/backing-replacement.md) |
| Cell | STORCELL version 1 | [Cell](../design/cell-replacement.md) |
| Vector | STORVECT version 1 | [Vector](../design/vector-replacement.md) |
| Map, Set and regions | STORMAP1 version 1, one shared AVL engine | [Ordered map](../design/ordered-map-replacement.md) |

Each replacement was authored through a restricted specification handoff. The
records identify the inputs and qualification scope. The remaining copied tree,
allocator, iterator, address types and private codec helpers have now been
removed together. Their predecessor-layout tests and import inventory are also
removed. Existing public behavior tests remain; they do not select an alternate
production implementation. Integration uses Storage's `Storable` directly.

The independently designed formats reject earlier images without writes or
implicit initialization. Hosts must explicitly provide fresh/reset backing and
update framing admission, including map-region callers. There is no compatibility
reader or automatic migration. Source replacement is not deployment approval;
see the [current handoff](../status/current.md) for performance and remaining gates.

## External benchmark reference

Published `ic-stable-structures` 0.7.2 remains an external dependency of the
unpublished `qualification/ic-storage-benchmarks` and `qualification/icss-size-consumer`
packages. A pinned optional upstream-head control is also confined to that graph.
Reference codecs used by the type catalogue remain external comparison inputs.
No reference engine is copied into the production tree or used as a fallback.

The two backends run the same logical operations on separate images. Native
comparison tests verify refusal of actual ICSS images without changing them;
installed qualification verifies old-image upgrade rejection and rollback.
Same-image interoperability is no longer a supported test contract.

Matched small consumers measure application Wasm separately from the broad
benchmark handler. Performance measurements must distinguish Storage governance,
backing translation and collection algorithm costs. The ordered tree keeps bounded
forward/backward traversal paths; rank-based skipping avoids value decoding.
Small keys/values fit inside nodes, and read-only reopening admits a short header
and root without loading allocation free lists. It reuses exact allocation size
classes without splitting or coalescing. These are design tradeoffs to measure,
not assumed improvements.

Historical baseline decisions and measurements remain in the
[measurement index](../measurements/README.md). They describe their recorded
source versions and do not qualify the replacement engines.
