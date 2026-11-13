# Current handoff — 2026-11-13

## Published work

Storage 0.74.0, Metrics Storage 0.77.0 and Logging Storage 0.73.0 are published
on crates.io and tagged on GitHub. Both published adapters require Storage
0.74.0. All 12 release stages passed for each group; no fresh hosted/macOS result
is asserted. The published locks select Metrics 0.30.0 and Logging 0.25.0.
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

The completed deployment adopted published Logging 0.25.0, Metrics 0.30.0,
Platform 0.32.0 and Runtime/macros/guard 0.22.0. Both locked caches are prepared;
only those requested families and required Types 0.20.0 were changed from the
incoming selections. Native/Wasm production boundaries and isolated consumer
compilation pass. That release selected Testkit 0.101.0, which owned its
separate native Platform 0.31/Types 0.19 graph. It exchanges opaque Wasm and Wire
protocols with the canisters; the new composition fixture has one Runtime/Types
identity. No collection format, checkpoint schema or allocation policy changes.
Evidence: `target/evidence/suppliers-deploy-20261113/`. Installed original/O3/Os/Oz
IO, corruption, custody, upgrade, rollback and Logging lifecycle qualification pass.
All three groups passed their complete gates and were published supplier first.

`batch-jkhHj3FX` captured `0c92858db24ede16b9800db3c3648187e637f184`, published
supplier first and reconciled main to `98e8d09504e8de2c668f7c7028180d86a5425752`.
GitHub tags and downloaded registry/final-receipt archive hashes were verified.

| Package | Version | Release commit | Archive SHA-256 |
| --- | --- | --- | --- |
| Storage | 0.74.0 | `5ec264a09ead28537fb405670df9d35e78bdfa9f` | `1b69f61ba55898f3b7eafd55a6a90ee271e05dc2105a1263d281b03e549cc81e` |
| Metrics Storage | 0.77.0 | `fd4f076df50ae11351d5e0abf0b9b4ab8c8e6c04` | `aa70edf01375057021950d52702bc29bcced5c2f0b9e6c7653967845d5ebff33` |
| Logging Storage | 0.73.0 | `98e8d09504e8de2c668f7c7028180d86a5425752` | `443986f115d088baffe41fec9d608f404059bbc818333b98efd0468faf1a26a4` |

Evidence: `target/evidence/suppliers-deploy-20261113/verification.json`,
its `deploy.log`, `target/release-frozen/batch-jkhHj3FX/` and
`.git/release-state/frozen/batch-jkhHj3FX/`. Installed qualification at
`target/qualification/runtime/attempt.Og2iy6` passes original/O3/Os/Oz IO,
corruption, custody, upgrades, rollback and Logging lifecycle behavior.
The initial reconciliation refused concurrent lock updates. They are retained on
`preserve/post-deploy-suppliers-20261113`, and restored unstaged after reconciliation.

Toko 0.40.1, Rust 1.100.0, locked Wire 0.21.5, Platform 0.32.0,
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

Storage 0.74.0, Metrics Storage 0.77.0 and Logging Storage 0.73.0 cover the
increased consumer MSRV; Storage also retains the benchmark work. Sibling source
stays read-only. Benchmark native tests (17 library,
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

## Pending Logging and Host adoption

Toko Tooling 0.42.6 is selected locally in both workspaces and the authenticated
snapshot. `make deps-list` replaces unused display-only adapter aliases; all three
groups retain their members. Both workspaces apply the common Cargo lints and
strict Rustdoc table validation. Product and independent native/Wasm Clippy pass
after narrowing Metrics dependencies to examples and checking actual Principal
exchange in the composition fixture. Strict Rustdoc and the existing private-debt
gate pass. See [the adoption record](../adoption/storage-tooling.md#toko-0410-adoption)
for evidence and the new deployment isolation rule. No full release or installed
runtime gate is claimed for this tooling/qualification-only follow-up.
The published 0.41.1 bootstrap observer removes the conflicting requirement for
unused member aliases. The local bootstrap fixture now reflects the real catalog,
retaining every release member without adding unused dependencies. Both the
launcher refusal/recovery fixture and real three-group lock-first/manifest-first
recovery pass. Snapshot, pins, tool admission and formatting also pass. See
[the patch adoption](../adoption/storage-tooling.md#toko-0411-adoption).

The [0.42.1 migration](../adoption/storage-tooling.md#toko-0421-adoption) moves
installed helpers to `vendor/toko-tooling/` and migrates Make, CI, hooks, release
callers and their fixtures. Release and publication remain explicit selections;
validation callers pass the consumer root. The shared retention probe initially
assumed root-installed helper paths and blocked its consumer check. The gate
remained enabled. The [0.42.2 patch](../adoption/storage-tooling.md#toko-0422-adoption)
passes its changed root-file admission caller, snapshot/pin checks, tool admission
and formatting, but does not change or fix the retention helper. The
[0.42.4 adoption](../adoption/storage-tooling.md#toko-0424-adoption) adds Cargo-tool
maintenance recovery and clarifies domain/endpoint ownership; Storage's existing
codec boundaries need no change. The [0.42.5 fix](../adoption/storage-tooling.md#toko-0425-adoption)
clears [Toko #381](https://github.com/dragginzgame/toko-tooling/issues/381):
Storage's unchanged retention suite now passes, including completion/failure
probes and the archive round trip. Snapshot/pin checks, tool setup/admission and
formatting also pass. The [0.42.6 refresh](../adoption/storage-tooling.md#toko-0426-adoption)
changes producer tests only; Storage's installed payload is unchanged, so the
passing retention evidence remains applicable. No deployment is included in
these adoptions.

Select Logging 0.25.1 in both catalogs and locks, and Host Artifacts/FS/Process
0.77.0 in the product workspace's native development graph. Build 0.84.0 is a
required companion: its archive API exchanges Host `OutputLimits` with Storage.
The 0.83/0.77 combination fails compilation; the matching release avoids a shim
and leaves one Host family in that graph. The selected metadata CLI also advances
to 0.84.0. The subsequent Testkit adoption below advances its native Host graph
through published requirements; no Host dependency enters product Wasm.

Preserve the incoming Metrics 0.30.1 lock update. Correct qualification's stale
Metrics Storage 0.76.2 requirement to the current 0.77.0 package: this mismatch
survived the preceding minor release and blocked independent Cargo resolution.
Logging 0.25.1's production sources match 0.25.0 byte for byte. Collection APIs,
checkpoint schemas, retained formats and product versions are unchanged.
Pending notes were Storage 0.74.1 and Logging Storage 0.73.1; the Wire adoption
below moves the unpublished adapter work into its next minor. This adoption is not
another deployment. Evidence: `target/evidence/logging-host-adoption-20261113/`.
The release guard handles Build's typed build/environment compiler declarations;
direct and included overrides still refuse before effects without leaking values.
Qualification passes 89 native helper tests, 13 Logging unit tests and two
doctests, strict native helper/Wasm Logging Clippy, and the actual Build 0.84
editor's lock-first/manifest-first recovery fixtures for all three groups.
Both locked metadata graphs, snapshot/pins, production dependency boundaries,
formatting and documentation links pass. No full release suite or fresh installed
runtime run is claimed for this local adoption. Product source is unchanged;
the affected native tool and compatible Logging callers own the focused checks.

## Pending Testkit 0.102.0 adoption

The qualification catalog/lock and selected server CLI now use published Testkit
0.102.0. PocketIC remains 16.1.0. The targeted update advances Host 0.76.5 to
0.77.0 and removes Platform 0.31.3/Types 0.19.5 in favor of the existing
0.32.0/0.20.0 selections. The product lockfile is unchanged from this adoption's
starting state; pending supplier, tooling and native guard edits are preserved.

Installed Linux original/O3/Os/Oz qualification passes at
`target/qualification/runtime/attempt.qEgdWm`, including allocation IO,
corruption, custody, all collections, Logging, upgrades and rollback. Runner and
benchmark Clippy, destination dependency boundaries and composition checks pass.
No new macOS proof, full release validation or publication is claimed.

The Cargo 1.100 binary-test blocker is resolved locally by adopting published
Toko 0.41.1 and moving the runtime qualifier's tooling declaration to the admitted
`target.'cfg(any())'.dev-dependencies` source-only marker. Exact inherited
selection and the cargo-machete exemption remain; no lint is suppressed and no
dummy import is added. All five runtime runner unit tests pass in this checkout.
The two explicitly installed tests remain ignored in that invocation; the
separate installed suite above already passed. The earlier reproduction and
producer qualification stay in `target/evidence/tooling-unblock-20261113/`.
Testkit adoption evidence remains in `target/evidence/testkit-adoption-20261113/`;
the new snapshot and focused consumer proofs are under
`target/evidence/toko-0411-adoption-20261113/`. No Storage commit, publication or
fresh hosted/macOS qualification is included in this adoption.

## Pending Wire 0.22.0 adoption

Both catalogs and locks select published IC Wire 0.22.0, archive checksum
`89cf2de2b642d47bfbc1d4c886cc9351f3f897e35252cd3dc18cb3bffb0293a6`.
Only Wire's version/checksum changed relative to this adoption's incoming locks;
both locked caches are prepared. The newer 0.22.1 notes in the supplier checkout
were unpublished at selection and are not this adoption's source.

Core Storage uses Wire only in key-codec tests and diagnostic size probes. Its
production collection engines retain their own codecs. Metrics Storage and
Logging Storage use bounded Wire encoding for persisted checkpoint values; the
independent runner/benchmark uses it for test-canister protocols. No production
Platform Types, endpoint or derive feature is introduced by this update.

Both adapters now expose the unit variant `StoreError::Codec`, with no public
Wire error payload, conversion or error source. Malformed and oversized record
categories remain distinct; codec implementation details are intentionally hidden.
Wire remains a private implementation dependency for the existing checkpoint
formats. Prepare minor notes for Metrics Storage 0.78.0 and Logging Storage
0.74.0 for this API hard cut, carrying the pending
Logging 0.25.1 work forward. Core's pending 0.74.1 remains a patch. Package
versions are unchanged until release. No persistence schema or reset changes.

Focused native checks pass: 30 Metrics tests, 13 Logging tests and five adapter
doctests; five core key-codec tests; five runtime runner tests (two installed-only
tests ignored); and 17 benchmark library/protocol tests. These include literal
checkpoint bytes, reopening retained rows, malformed/trailing input and no-write
refusals. Adapter Wasm Clippy and production-graph policy pass. No installed
runtime, performance or Wasm-size result is claimed for the new Wire selection.
Evidence: `target/evidence/wire-022-adoption-20261113/`. No commit, deployment or
sibling source change is included.

The subsequent public-error hard cut passes all 43 adapter unit tests and five
doctests, native all-target and Wasm library Clippy, formatting and documentation
links. Literal checkpoint-byte and retained-row tests remain unchanged and pass;
the codec change affects error reporting, not encoding or admission. Existing
arithmetic error sources remain available; `Codec` has no nested source.
Evidence: `target/evidence/adapter-codec-errors-20261113/`.

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
