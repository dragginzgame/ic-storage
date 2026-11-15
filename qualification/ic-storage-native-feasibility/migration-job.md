# Bounded governed migration job

This qualification profile adds controller-authorized export and durable
single-use admission to the native checkpoint bridge. It uses published Storage
handles at `native.checkpoint.job.v1`; the original scalar checkpoint locator and
version-1/2 logical checkpoint formats are unchanged. Source and target share
the governed reader independently of discarded native memory.

## Contract and bytes

The allocation-domain identity is the native provider's actual base, obtained
through a private scalar WAT callback compiled alongside its geometry. The two
admitted domains are 131072 and 147456, with the existing 4096-byte extent and
metadata/vector contracts. This is not an executable digest, compiler/layout
certificate or authentication of arbitrary source code. Exact artifact admission
and trusted-controller installation remain separate obligations.

The byte-defined envelope is at most 4320 bytes, all integers little-endian:

| Offset | Field |
| --- | --- |
| 0 | Eight-byte magic `ICJOB001` |
| 8 | u16 envelope version 1 |
| 10 | u16 phase: Ready=0, Sealed=1, Consumed=2 |
| 12 | u32 header size 64 |
| 16 | u64 epoch |
| 24 | u64 source allocation domain |
| 32 | u64 target allocation domain |
| 40 | u32 nested checkpoint length |
| 44 | Twenty zero reserved bytes |
| 64 | Complete version-2 checkpoint, or empty for Ready |
| after payload | Eight-byte `JOBDONE1`, then SHA-256 over all preceding bytes |

Ready has epoch zero, identical supported source/target domains and no payload.
Sealed/Consumed require a nonzero epoch, different supported domains and a fully
validated version-2 checkpoint with the same epoch. Unknown/corrupt/oversized
records refuse; no read failure initializes a replacement record. Integrity is
not authentication. This experiment does not use a production application codec.

## Transitions and authority

The source checks the current IC controller set before export or fault injection,
then opens the governed record, freezes the native root, builds the checkpoint
and publishes Sealed in one update. Only the next epoch is accepted; the initial
epoch is 1. An identical frozen retry is idempotent. Changed epoch, destination
or payload refuses. Epoch exhaustion refuses instead of wrapping. Consumed
records retain their checkpoint until the next authorized export replaces it.

Recovery runs only in the explicit replacement-upgrade profile, whose WAT gate
requires fresh native metadata before checkpoint Rust. It requires Sealed,
the requested epoch and predecessor domain, and its own compiled destination
domain. The transient job changes to Consumed before construction; stable
publication follows native settlement. All of this is one upgrade transaction,
including the final native post-upgrade check. A trap must restore source module,
native state and sealed job for retry. A successful recovery permanently refuses
another import of that job within this cooperating protocol, including after
a compatible upgrade. Later migration requires the next epoch from the current
domain. Compatible reopening does not read or rewrite the job.

Test entrypoints use three u64 arguments: export `[next_epoch, target, fault]`,
recovery `[epoch, source, fault]`. Faults exercise export after stable publication,
import after a Consumed write, partial target construction and completed native
publication. Queries expose evidence bytes. Native mutation and diagnostic
endpoints remain qualification-only; this is not application access control.

Run `make native-job-test` with cached dependencies and the prepared server.
Host tests check an independently assembled golden envelope, transitions,
exhaustion, malformed/resealed envelopes and full capacity. Installed tests check
live controller revocation, retries, domain/epoch refusal, consumption across
compatible upgrades and rollback. Evidence is recorded below, separately from
the original bridge's unauthenticated epoch fixture.

## Limits

Controllers can install arbitrary code or restore an entire older stable-memory
snapshot; this protocol does not defend against that authority. The counter is
durable within the governed canister history, not an external anti-rollback anchor.
Cancellation, frozen-source compatible reopening, cross-message progress,
production source/target authentication and actual compiler transitions remain
open. No consumer cutover or public migration service is introduced.

## Observed evidence

On 15 November 2026, the focused gate passed on x86_64 Linux with Rust 1.100.0
and the prepared PocketIC server. Three host codec/transition tests, twelve
existing artifact checks and the installed job suite passed. The installed suite
took 6.08 seconds, excluding preparation: six replacement recoveries, three
compatible upgrades, thirty refused imports and twenty-five refused exports.
Coverage includes empty/full vectors, identical retries, live controller
revocation, consumed-job replay after reopening and all three import trap sites.

The job cohort `target/evidence/provider-link/attempt.MXrCKH/` retains the raw
compiler inputs, codec/golden sources, bridge/contract, composed Wasm and
`job.txt`. Principal artifact identities are:

- Source A (ordinary root), 137912 bytes: `34f73a75c482bb7fc742b80ae0a2063584393027ce80ee014ccaa8a7ad0aaa7f`.
- Recovery B, 126231 bytes: `1a3fd17e60cba3c5eb6f6b5505f90d9e9589e1f47c0c15295bdfaeeae8e52b46`.
- Recovery A, 126231 bytes: `80296f9ae56b26e7d168391237a6a3385207d6d1210fe0b1980a369b9ef95adb`.
- Reopen B, 137912 bytes: `424ad6f2b0a3aa283fcc0e2a37208b3d46f08738ff2e9d4ec9d224ec0a043296`.

The affected ordinary checkpoint suites also passed (2.47 seconds), along with
the four earlier native migration/reopening checks (4.00 seconds) in the separate
cohort `target/evidence/provider-link/attempt.Gog8SI/`. These rebuilt artifacts
reflect the shared provider refactor; previous cohorts remain historical evidence.
Strict host/wasm Clippy, formatting, shell and workflow validation passed. Hosted
CI and native macOS execution have not been observed for this local change.
