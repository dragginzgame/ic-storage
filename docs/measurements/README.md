# Selected measurements and ownership

Experimental evidence follows the [shared retention contract](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/rules/engineering-details.md#changelogs-and-artifact-preservation):
one current summary and at most two small data files per topic, 100 KiB combined,
within the 5 MiB repository budget. These are limits, not storage targets.

| Topic | Summary | Selected data |
| --- | --- | --- |
| Allocation admission | [Integrity checks, compact index and tradeoffs](../audits/recurring/storage-admission-2026-11-09.md) | [Matched admission/reopen/append matrix](../audits/recurring/storage-admission-2026-11-09.csv) |
| Vector terminal buffer | [Ownership transfer and matched ICSS comparison](../audits/recurring/storage-vector-last-2026-11-09.md) | Bulk, whole-handler, scalar and size controls inline |
| Bucket backing | [Hard cut, tradeoffs and ICSS comparison](../audits/recurring/storage-bucket-backing-2026-11-09.md) | Depth, bulk and eight-selection Wasm matrices inline; [native IO attribution](../audits/recurring/storage-bulk-attribution-2026-11-09.csv) and [bulk controls](../audits/recurring/storage-bulk-controls-2026-11-09.csv) |
| Cell replacement | [Cached encoded length and ICSS comparison](../audits/recurring/storage-cell-length-2026-11-09.md) | 73-type, depth, bulk and small-consumer matrices inline |
| Overflow initialization | [Combined write and matched ICSS comparison](../audits/recurring/storage-overflow-init-2026-11-09.md) | Bulk/depth and eight-selection size matrices inline |
| B-tree prefix writes | [Combined write and matched ICSS comparison](../audits/recurring/storage-prefix-write-2026-11-08.md) | Depth and eight-selection size matrices inline |
| Shared B-tree prefix | [Retained prefix/child readers and rejected lease-index trials](../audits/recurring/storage-shared-prefix-2026-11-08.md) | All-collection depth and eight-selection size matrices inline |
| Architectural hard cut | [Selected cuts, rejected iterator and matched sizes](../audits/recurring/architectural-reduction-2026-11-08.md) | Inline eight-selection size matrix and qualification scope |
| Storage/ICSS type matrix | [Current coverage/results](../audits/recurring/benchmark-full-catalogue-2026-11-05.md) | [565 selected rows](storage-comparison-selected.csv), [compact identities](storage-comparison-identities.json) |
| Storage 0.68 follow-through | [Three-step comparisons](../audits/recurring/storage-068-followup-2026-11-06.md) | [Instruction matrix](storage-068-followup-instructions.csv), [IO/codec matrix](storage-068-followup-io.csv) |
| Governed backing parity | [Attribution and routing-read comparisons](../audits/recurring/storage-icss-backing-parity-2026-11-06.md) | [Step/round and diagnostic counters](storage-icss-backing-steps.csv), [payload costs](storage-icss-backing-payloads.csv); published Runtime ≥0.19.4 |
| Ordered terminal iteration | [Retained entry shortcut and rejected projection/caller trials](../audits/recurring/storage-ordered-iteration-2026-11-07.md) | Selected rows in the summary and shared [step matrix](storage-icss-backing-steps.csv) |
| Write-path experiments | [B-tree rejections and retained Cell header writes](../audits/recurring/storage-write-paths-2026-11-07.md) | Selected instruction, IO and Wasm-size matrices in the summary |
| B-tree IO follow-through | [Retained tail/empty-write shortcuts and rejected prefix reads](../audits/recurring/storage-btree-io-2026-11-07.md) | Per-step Storage/ICSS instruction, IO and Wasm-size tables |
| Empty B-tree values | [Retained value shortcut and rejected general-read branch](../audits/recurring/storage-empty-values-2026-11-07.md) | Matched all-collection depth matrix and reduced Wasm size |
| B-tree save and search | [Retained root lookup and rejected overflow-list transfer](../audits/recurring/storage-save-search-2026-11-07.md) | Per-step depth matrix plus matched 64-KiB/1-MiB payload study |
| B-tree node loads | [Rejected delayed-sibling and borrowed/owned split trials](../audits/recurring/storage-node-loads-2026-11-07.md) | Three-step all-collection Storage/ICSS handler matrix and Wasm sizes |
| Byte and Boolean codecs | [Borrowed encodings and decoder control](../audits/recurring/storage-byte-codecs-2026-11-07.md) | Two-revision all-collection Storage/ICSS matrix, individual-round regressions and Wasm sizes |
| Vector opening | [Combined header IO and shared framing admission](../audits/recurring/storage-vector-open-2026-11-08.md) | Reopen/initialization and all-collection controls versus ICSS; benchmark and consumer-fixture Wasm sizes |
| Cell writer sharing | [Codec-independent writes and size tradeoff](../audits/recurring/storage-cell-outline-2026-11-08.md) | Matched Cell/ICSS instructions, all-collection controls and consumer-fixture Wasm sizes |
| Matched small-consumer Wasm size | [Primary size study](../audits/recurring/storage-wasm-gap-2026-11-08.md#primary-size-benchmark-matched-small-consumer-crates) | [Eight paired selections](storage-consumer-wasm-size.csv), [artifact identities and sections](storage-consumer-wasm-size-identities.json) |
| Storage versus ICSS Wasm gap | [Full and compact attribution](../audits/recurring/storage-wasm-gap-2026-11-08.md) | Matched optimizer, compact depth and governed-ICSS controls; duplicated tree specializations |
| Consumer memory sizing | [Declaration inventory and historical controls](../local-memory-audit.md) | [Consumer declarations](consumer-memory-inventory.json), [selected historical controls](storage-memory-study.json) |
| Current engine reset | [Compatibility, safety and selected baseline](../audits/recurring/icss-engine-baseline-2026-11-05.md) | Inline selected table |
| Historical engine experiments | [Decisions and limits](../audits/recurring/storage-icss-hotspots-2026-11-04.md) | Inline selected table |
| IC stable-read charging | [Mechanism and selected calibration](../audits/recurring/vector-stable-read-charges-2026-11-05.md) | Inline selected tables |

The 2026-11-06 cleanup retires bulk CSV/JSON exports, repeated run metadata and
patches for removed collection engines. It does not create another permanent
archive. Existing target run directories are untouched and transient; hashes or
summaries alone cannot verify raw artifacts after explicit cargo clean removes
them. New comparison output uses `target/evidence/storage-comparison/`.

Small runtime/ledger/allocation/compiler/Wasm qualification tables remain linked
from their historical release/support records. The `canic162/` qualification
subdirectory, release receipts, active recovery state, source-provenance records,
tool caches and pending code are outside this experimental-export retirement.
Do not treat historical measurements as qualification of current collection
formats or an optimum for all applications.
