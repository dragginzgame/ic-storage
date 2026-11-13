# Current handoff — 2026-11-13

## Published work

Storage 0.73.6, Metrics Storage 0.76.3 and Logging Storage 0.72.3 are published
on crates.io and tagged on GitHub. Both published adapters require Storage
0.73.6. All 12 release stages passed for each group; no fresh hosted/macOS result
is asserted. Both locks retain Metrics 0.29.12 and Logging 0.24.17.
The [discard study](../measurements/storage-discard-mutations-2026-11-12.md) owns
presence-returning mutations, adapter adoption, instruction/IO and size evidence;
the [histogram study](../measurements/metrics-histogram-comparisons-2026-11-12.md)
owns full-width comparison improvements. Guards, returning behavior and stored
formats remain unchanged. [The published handoff](https://github.com/dragginzgame/ic-storage/blob/358b1ea/docs/status/current.md)
retains the detailed delivery narrative and earlier evidence.

## Consumer ownership and next actions

- [Storage #118](https://github.com/dragginzgame/ic-storage/issues/118) is closed
  as completed after publication. Actual adapter checkpoint calls are measured:
  changed histogram saves improve 0.31–0.48%, removals 0.20–0.58%, with 736 fewer
  benchmark Wasm bytes. No-op/rejection writes remain zero. The prefix-buffer
  trial was rejected and restored; the discard study owns its evidence.
- [IcyDB #635](https://github.com/dragginzgame/icydb/issues/635) owns key-only scans
  and bounded region existence probes using already published APIs.
- [Blob #129](https://github.com/dragginzgame/ic-blob-storage/issues/129) owns
  discard-mutation adoption; the supplier is now published.
- [Relay #24](https://github.com/dragginzgame/ic-relay/issues/24) owns measurement
  of bounded record rewrites before any state-layout redesign.
- Existing [Auth #132](https://github.com/dragginzgame/ic-auth/issues/132) and
  [Canic #764](https://github.com/dragginzgame/canic/issues/764) received adoption
  notes. Complete their existing ownership migration; do not create a second
  authentication engine. Their current installed regression ceiling is 1.3M;
  the earlier <1M request remains an optimization goal, not that release gate.
- [Storage #158](https://github.com/dragginzgame/ic-storage/issues/158) owns
  routing-key locality/blob density experiments and any explicit format decision.
  [Read-path attribution](../measurements/storage-identity-lookups-2026-11-12.md#access-path-attribution--2026-11-13)
  explains the coarse trial's extra 80k iterator event: two node reads share a
  4 KiB source page, triggering native stable mapping on the second. All 36
  complete read-only traces match the pinned IC path model. No extra IO or
  allocator work explains that event. Across 20 rank-cursor rounds, coarse
  raises whole-handler means 0.52–2.10%; the earlier ~84% inner median is not
  representative. The 6.32% churn median redistributes 19 costly events across
  three paths; their summed handler cost is nearly unchanged. Neither result
  establishes a universal default penalty. However, the resize-capacity tradeoff
  remains and 100k tagged/256-byte cold paths still incur 12 costly reads in
  both layouts. No policy is adopted; production remains byte-identical.
  Next assess checked routing/key read combination for contiguous allocations,
  preserving bounds, corruption checks and value-free traversal. No new format,
  reset, consumer change or publication is authorized. Auth remains unmeasured.
  Maintained diagnostics now support single-result rank cursors, bounded
  single-operation capture and explicit Binaryen type annotations. The locked
  .17 rustls-webpki archive was fetched separately; final driver checks and
  installed diagnostic runs pass with it, not a complete product release gate.

The maintainer authorized closing #118 and continuing #158 after deployment.
Sibling source stays read-only; sibling issues and delivery remain theirs.
Neither these trials nor discard mutations establish an Auth endpoint speedup.

## Current published baseline and suppliers

The authorized next deployment adopts published Logging 0.25.0, Metrics 0.30.0,
Platform 0.32.0 and Runtime/macros/guard 0.22.0. Both locked caches are prepared;
only those requested families and required Types 0.20.0 were changed from the
incoming selections. Native/Wasm production boundaries and isolated consumer
compilation pass. Testkit 0.101.0 remains the latest published client and owns its
separate native Platform 0.31/Types 0.19 graph. It exchanges opaque Wasm and Wire
protocols with the canisters; the new composition fixture has one Runtime/Types
identity. No collection format, checkpoint schema or allocation policy changes.
Evidence: `target/evidence/suppliers-deploy-20261113/`. Installed original/O3/Os/Oz
IO, corruption, custody, upgrade, rollback and Logging lifecycle qualification pass.
The release runner will qualify the three minor releases supplier first; this
preparation statement is not publication evidence.

`batch-fgFa7wx8` captured `a2552c509975a08b668614c0bc8d19acb3ee50ef`, published
supplier first and reconciled main to `4668fee8bac275d55bb0d5ef4c6b71725573e708`.
GitHub tags and downloaded registry/final-receipt archive hashes were verified.

| Package | Version | Release commit | Archive SHA-256 |
| --- | --- | --- | --- |
| Storage | 0.73.6 | `0ab2afdc7e482c3d0f352c44d533bab0d2a2cc09` | `99884d9c4cde4747cf7ccff910789033f3ee07c05b9d69ed77288df9fcdaeaee` |
| Metrics Storage | 0.76.3 | `39efb2b9f8424af1292d4bf948c20a6de255ba22` | `996a99e738f09792e2ac60a2e2a8e222a935d5be730a9fe101cd360028870c9b` |
| Logging Storage | 0.72.3 | `4668fee8bac275d55bb0d5ef4c6b71725573e708` | `ddf42f5459b78c0d7da24ab528e23c317615fca06341d85da1d54b82ebb094ab` |

Evidence: `target/evidence/deploy-storage-0736/verification.json`,
`target/deploy-0736-batch.log`, `target/release-frozen/batch-fgFa7wx8/` and
`.git/release-state/frozen/batch-fgFa7wx8/`. Installed Logging qualification was
refreshed against Logging 0.24.17: `target/qualification/runtime/attempt.Ge7c1K`
passes lifecycle, query non-persistence, update/upgrade rollback and reopen.

Toko 0.40.1, Rust 1.100.0, Host 0.76.5, locked Wire 0.21.5, Platform 0.32.0,
Runtime 0.22.0, Testkit 0.101.0 and PocketIC 16.1.0 are selected.
[The 0.40.0 migration](../adoption/storage-tooling.md#toko-0400-adoption) addresses
[#159](https://github.com/dragginzgame/ic-storage/issues/159): both workspace floors,
member lint inheritance, strict native/Wasm qualification and bounded private
protocol documentation debt. Pending benchmark work and supplier pins remain.
[The 0.40.1 refresh](../adoption/storage-tooling.md#toko-0401-adoption) adds deployment
sequencing and failure-specific recovery guidance. Installed executables and
product gates are unchanged; collector-only recovery remains unsupported.
Release-group metadata now selects the pending minor-line notes; the focused
native/Shared file-plan comparison passes for all three groups.
Earlier adoption evidence remains linked there; original installed evidence is
at `target/qualification/runtime/attempt.exkRp9`.

Pending Storage 0.74.0, Metrics Storage 0.77.0 and Logging Storage 0.73.0 notes
cover the increased consumer MSRV; Storage also retains the benchmark work.
Deployment is now authorized for these three groups; sibling source stays read-only.
The pre-existing qualification lock correction to Metrics Storage 0.76.3 is
preserved and used in both matched runs. Benchmark native tests (17 library,
28 driver), native/owned-Wasm lint and the earlier installed comparisons pass.
The isolated density/routing trials pass 50/59 engine tests, nine public contracts
each, lint and sixteen installed size consumers each. Map total/code growth is
413/307 bytes for density and 1,111/908 for routing. Native resize controls pass
but reject fine-grained density as the default; scalar regressions reject routing.
Temporary tests are removed with their trials. Those older optimization results
retain Rust 1.99.1 attribution; they are not Rust 1.100.0 performance measurements.
The compiler/lint adoption has separate evidence under
`target/evidence/toko-0400-adoption-20261113/`: native/Wasm lint and docs, unit and
integration tests, negative policy fixtures and all four size gates pass.
Installed original/O3/Os/Oz qualification passes at
`target/qualification/runtime/attempt.cDFmPe`. Publication and native macOS proof
remain separate; #159 stays open until integration and applicable CI evidence.
No release command was invoked for adoption.

## Performance and preservation boundaries

The [scalar study](../measurements/storage-scalar-scans-2026-11-11.md) owns shipped
scan/removal improvements. The [identity study](../measurements/storage-identity-lookups-2026-11-12.md)
owns node-first allocation (no demonstrated Auth win) and rejected refills/scratch.
Do not restore rejected root-cache, combined-read, compact-frame or decoder-outlining
trials. Measure first/second/third reads, mutations and application Wasm together.

Independent engines and current formats remain STORNAM1, STORCELL, STORVECT and
STORMAP1 version 1. ICSS is an external benchmark/test dependency only. Earlier
images require explicit reset and host framing admission; no automatic reset or
compatibility reader exists. A new format must not inherit an earlier consumer's
reset authorization. See [replacement records](../design/independent-storage-replacement.md).

Preserve failed attempts, archives, receipts, frozen sources, tool caches and
`.git/release-state/`, including earlier batch evidence linked from the prior
handoff. Keep stash `f652e8c7400f56f0cab6ad968d05c4c0aee6479d`. Trimming this
handoff authorizes no artifact cleanup. Target evidence can disappear under an
explicit cargo clean; durable recipes and decisions remain in measurement docs.
Preserve unrelated sibling work, including authorized IcyDB preparation at
`/tmp/icydb-control-layout`, branch `storage/control-allocation`, base `03c5713e`:
native checks passed, installed composition is unqualified, and durable relocation
or draft-PR approval remains pending.
