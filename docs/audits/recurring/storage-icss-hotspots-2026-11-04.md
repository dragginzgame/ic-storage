# Historical Storage collection experiments

Scope: 2026-11-03–05, primarily Storage 0.65–0.67 source based on `7c5cf83`
and `4dea092`. The [copied-ICSS baseline](icss-engine-baseline-2026-11-05.md)
retires the independent tree/block/record engines, checksums, node cache and
Vector windows studied here. These results explain past choices; they are not
current Storage performance or instructions for restoring those implementations.

## Conclusions worth retaining

| Experiment | Selected historical result | Decision and limit |
| --- | --- | --- |
| Inline encoded replacement | u64/u64 Replace: 198,145 → 186,718 instructions for one; 1,379,545 → 991,852 for 32 | Avoided complete node decode/re-encode; not a whole-call 2× claim |
| Smaller implementation of that fast path | Module 1,023,670 → 1,027,724 bytes, versus first candidate 1,032,987 | Kept most savings with less code; unrelated Last regression +2.66% remained |
| Smaller external blocks | 33–44-byte values: 27 → 15 physical pages in the native prototype | Capacity saving alone did not justify a new allocator format |
| Corrected bulk-growth trace | 1 MiB → 1.5 MiB Grow: Map 37,720,652 Storage / 76,026,494 ICSS; Cell 18,585,336 / 5,739,124 | Different read/write/integrity contracts; neither engine universally wins |
| Six mixed-size replacement rounds, 64 allocations | Storage 238.31 MiB / ICSS eight-page 168.06 MiB; Storage writes 97,577,577 bytes versus ICSS 966,683,123 in the final round | Lower writes traded against higher reserved capacity; not a leak/plateau proof |

The bulk trace previously capped growth at 64 KiB. Expanding it to 256 KiB and
1.5 MiB fixed coverage. A 2 MiB returned old value exceeded the IC reply ceiling;
the final recipe kept bounded replies. Agreement with a host model cannot expose
a missing scenario when both implement the same mistaken trace policy.

Borrowed entry decoding had cloned complete encoded values unnecessarily.
Passing borrowed bytes removed that copy for compatible codecs, without proving
that consuming scan timings would improve. Current copied engines have their
own lazy-entry behavior; no obsolete patch is needed for architectural symmetry.

## Two-way cache experiment and the ICSS comparison

Published ICSS 0.7.2 has no persistent decoded-node cache. Its single-entry
virtual bucket-translation cache is a different mechanism. The pinned research
source `b5d1650463f477affb3d358e86b1113953fc9937` has a direct-mapped node cache
with 16 default slots; its index uses node geometry, not Wasm page size.
This is a version-specific source comparison, not a claim about future Git HEAD.

The old Storage two-way cache removed one retained64 collision pathology:
a complete seed-one, 80-byte, single-update trace fell from 11,291,880 to
7,027,774 instructions (−37.76%). Seed two changed +0.20%; the original allocator's
seed two changed +0.13%. Cold ordinary traces ranged −0.60% to +1.50%, including
Map First +4.25%. It was selected for the earlier implementation, not promoted
as a universal speedup or as an equal-budget comparison with the Git cache.
The stronger mixed hash and retained64 allocator remained research-only.
The current copied baseline has no persistent node cache.

## RSX source comparison and Vector follow-through

RSX source was pinned to
[757bf506](https://github.com/Neutrinomic/cdk-rsx/tree/757bf506bb84c6487d6b367d56bb7f7bed96aea3).
Its bounded Hylix Queue visitor suggested fewer backing reads per prefix scan;
it did not establish that all RSX iterators batch IO. Storage's later window
experiment needed mutation epochs because set/push/pop use shared references.
Those windows and epochs were removed in the copied-ICSS reset.

RSX's compiled map capacities, deferred frame flushing and retention arenas are
different contracts. A fair comparison must include final flush/publication,
metadata, initialization, alignment, lifetimes and actual working sets. No RSX
speed ranking, new production fanout or general allocator affinity was established.

Removing old Vector checksums changed prefix/payload addresses and exposed
80k/160k source-charge reversals. The cause was pinned IC stable-read dispatch,
not unexplained timing noise. The independent
[page-charge explanation](vector-stable-read-charges-2026-11-05.md) retains that
mechanism and selected absolute observations; it does not select a current layout.
Packed encoding, checksum removal and buffering each had code-size, copying and
heap-charge tradeoffs. No historical prototype is a current optimization mandate.

## Reproducing useful work now

The maintained benchmark supports control/records/bulk, boundaries, ordered,
iterators, Auth lookups and the full type catalogue. Use
[the current guide](../../../qualification/ic-storage-benchmarks/README.md)
and matched ICSS buckets, populations, histories and execution modes. Rebuilding
measures current source, not these retired formats. Native capacity/IO and IC
instruction results must remain separate; never sum unrelated traces into one score.

## Evidence retirement

The 2026-11-06 cleanup retires bulk tables, repeated identity JSON, diagnostic
exports and patches targeting removed engines. Useful decisions and selected
absolute results remain above; tracked prior material remains in ordinary Git
history, without history rewriting or a second permanent archive. Untracked
experimental exports are intentionally retired, not promised as durable evidence.
Existing target run directories were not touched and may contain local raw data;
explicit cargo clean can remove them. A retained summary or digest alone does not
prove that retired raw measurements remain independently verifiable.
Release/recovery receipts, tool caches, preservation stashes and current source
work are outside this cleanup.
