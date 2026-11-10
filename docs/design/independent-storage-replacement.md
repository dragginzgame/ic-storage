# Independent Storage replacement inventory and delivery plan

Decision, 2026-11-10: replace maintained DFINITY-derived production code. ICSS
remains an external unpublished benchmark/test comparison. Fresh/reset backing
is authorized; incompatible images must be rejected without automatic reset.

## Implementation disposition

| Surface | Current owner and record |
| --- | --- |
| Memory and native/IC IO | Independently implemented; [restricted inputs and qualification](memory-replacement.md) |
| Named allocation backing | STORNAM1 version 1; [restricted inputs and qualification](backing-replacement.md) |
| Cell | STORCELL version 1; [restricted inputs and qualification](cell-replacement.md) |
| Vector | STORVECT version 1; [restricted inputs and qualification](vector-replacement.md) |
| Map, Set and map regions | Shared STORMAP1 version 1 ordered engine; [restricted inputs and qualification](ordered-map-replacement.md) |
| Copied tree, allocator, iterator, node format and address helpers | Deleted with the ordered-engine cutover, including their private format tests |
| Private copied codec trait and adapter codec wrapper | Deleted; all engines use public Storage `Storable` directly |
| Public adapters, custody and namespace integration | Remain Storage-owned; preserve checked leases and supported successful-call APIs |
| External reference codecs and collections | Registry/Git dependencies only in the unpublished comparison graph |

All files covered by the final ICSS import inventory were traced to the removed
engine directory and its callers. The old `docs/storage/icss-source.json` import
inventory is retired with those files. Earlier measurements and release records
retain their original source identities; Git history is unchanged. No second
production engine, old reader or fallback remains selected.

## Information boundary

The coordinating session has seen predecessor source. It prepared behavior and
interface specifications, integrated the results, authored separate public
acceptance tests and reviewed qualification. It did not author the replacement
engine algorithms. Separate unexposed implementation agents received restricted
handoffs for Cell, Vector, Memory, backing and ordered collections. Each record
identifies actual inputs, implemented format, tests and remaining limits.

The ordered implementation received only its new behavior/interface contract,
AGENTS.md, the selected baseline, pinned engineering rules and manifests. It
selected its own AVL/rank-cursor/size-class design. No predecessor layout or
implementation text was supplied. Existing behavioral tests remain integration
checks, not inputs to that implementation context.

## Qualification and release boundary

Replacement is not an optimization claim or deployment approval. The ordered
engine changes page footprints, allocation geometry and scan costs. Measure
Map/Set/region and all-collection application Wasm separately from benchmark
handlers. Compare logical workloads on separate Storage and external ICSS images.
Retain regressions and scope limitations rather than treating passing model
tests as production performance evidence.

Required checks include public model/range/codec/custody contracts, new-format
corruption and growth tests, actual external-image rejection, native/Wasm lint,
installed fresh/reopen/rollback, and old-image upgrade rejection. Product package
archives and both selected dependency graphs must retain the owned/external
boundary. Hosts must update framing checks and explicitly reset or replace
backing before using incompatible formats. No automatic migration is provided.

The [current handoff](../status/current.md) owns completed validation, comparison
results and outstanding release requirements. The canonical package metadata is
in the two workspace catalogs; every package inherits the root `license-file`.
