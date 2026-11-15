# Bounded governed migration job

This qualification profile adds controller-authorized export/cancellation and durable
single-use admission to the native checkpoint bridge. It uses published Storage
handles at `native.checkpoint.job.v1`; the original scalar checkpoint locator and
version-1/2 logical checkpoint formats are unchanged. Source and target share
the governed reader independently of discarded native memory.
The [staged import extension](staged-import.md) adds an explicit version-2 job
envelope and unpublished native construction across bounded updates/upgrades.

## Contract and bytes

The allocation-domain identity is the native provider's actual base, obtained
through a private scalar WAT callback compiled alongside its geometry. The two
admitted domains are 131072 and 147456, with the existing 4096-byte extent and
metadata/vector contracts. This is not an executable digest, compiler/layout
certificate or authentication of arbitrary source code. Exact artifact admission
and trusted-controller installation remain separate obligations.

The original version-1 envelope is at most 4320 bytes, all integers little-endian:

| Offset | Field |
| --- | --- |
| 0 | Eight-byte magic `ICJOB001` |
| 8 | u16 envelope version 1 |
| 10 | u16 phase: Ready=0, Sealed=1, Consumed=2, Cancelled=3 |
| 12 | u32 header size 64 |
| 16 | u64 epoch |
| 24 | u64 source allocation domain |
| 32 | u64 target allocation domain |
| 40 | u32 nested checkpoint length |
| 44 | Twenty zero reserved bytes |
| 64 | Complete version-2 checkpoint, or empty for Ready |
| after payload | Eight-byte `JOBDONE1`, then SHA-256 over all preceding bytes |

Ready has epoch zero, identical supported source/target domains and no payload.
Sealed/Consumed/Cancelled require a nonzero epoch, different supported domains and a fully
validated version-2 checkpoint with the same epoch. Unknown/corrupt/oversized
records refuse; no read failure initializes a replacement record. Integrity is
not authentication. This experiment does not use a production application codec.
Cancelled extends the phase vocabulary without changing existing phase bytes,
header fields or the nested checkpoint. Earlier readers reject phase 3 as unknown;
they are not qualified readers of cancelled jobs. The original initial-envelope
golden remains unchanged.

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
domain. Ordinary ready-state reopening does not read or rewrite the job.

### Explicit frozen-source resumption

The separately selected frozen-resume target preserves the frozen source rather
than enabling native ownership. Its private WAT bootstrap requires a cold
instance, idle access, matching native geometry, valid full-width length/capacity
and settlement 2. It restores only the transient frozen phase; it neither changes
retained metadata nor creates a Vec/Region owner. The closed bootstrap/callback
graph cannot enter Rust or host code. Fresh, ready, abandoned or incompatible
native state refuses before resetting or entering the checkpoint runtime.

After that bootstrap returns, the target rebuilds the complete transient domain
using the existing admitted reset contract, then enters a private Rust verifier.
Fresh governed handles must load Sealed with the requested epoch/destination and
the compiled source domain. It compares checkpoint counter, length and every
element with the still-frozen native source through scalar callbacks. Valid
digests and matching geometry alone do not establish that these are the same
logical state. The verifier neither writes stable memory nor consumes the job.

Success leaves native updates blocked and permits another identical export,
cancellation or explicit replacement recovery. Repeated frozen-compatible
upgrades follow the same checks. Any trap, including after complete verification,
must restore the source module, native observation and full stable bytes. Normal
reopening still refuses frozen data, and replacement into this profile refuses
fresh native memory. The upgrade artifact and arguments explicitly select the
path; unknown headers never trigger recovery or initialization.

### Cancellation before heap replacement

An authorized controller can cancel the exact Sealed job only on its original
source domain, with matching epoch and intended target. The adapter publishes
Cancelled through the same governed cell, then a private scalar WAT callback
validates frozen native custody and restores ready settlement/phase. It never
changes payload, length, capacity, allocation domain or the retained counter.
No native Rust owner exists during this callback; its closed call graph cannot
enter either Rust runtime or host code. The update is atomic: a trap after the
stable write or after thaw must restore both the sealed job and frozen source.

An identical cancellation retry is idempotent. It validates ready custody but
does not freeze, restore checkpoint values or rewrite the stable record: normal
native writes may have occurred since cancellation. Retrying after a compatible
upgrade follows the same rule. Abandoned loans, corrupt descriptors or unreadable
job bytes refuse; cancellation cannot repair lost custody or guess empty state.

Cancelled retains its checkpoint and retired epoch. Recovery accepts only Sealed,
so neither that checkpoint nor its epoch can be replayed; the original source
must export its current values at the next epoch. A stale cancellation cannot
cancel a newer export. Consumed jobs refuse cancellation, since successful heap
replacement has already discarded their original source. A later export from
the recovered domain is a new job and may itself be cancelled before cutover.

Test entrypoints use three u64 arguments: export `[next_epoch, target, fault]`,
recovery `[epoch, source, fault]`, cancellation and frozen resumption
`[epoch, target, fault]`.
Cancellation faults 1 and 2 trap after the stable write and native thaw respectively.
Faults also exercise export after stable publication,
import after a Consumed write, partial target construction and completed native
publication. Queries expose evidence bytes. Native mutation and diagnostic
endpoints remain qualification-only; this is not application access control.
Frozen-resume fault 1 traps after complete verification. Governed damage modes
1–3 construct a correctly encoded epoch-1 job with a changed counter, element or
length to test logical continuity; mode 0 retains the original digest-corruption
case. These are diagnostic endpoints, not migration operations.

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
Cancellation after cutover, general graph migration, production source/target
authentication and actual compiler transitions remain open. Staged import is
bounded to the fixture's single 512-element vector; export and checkpoint reads
still process that complete bounded record. No consumer cutover or public
migration service is introduced.

## Original job evidence (0.1.9)

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

## Cancellation evidence

On 15 November 2026, `make native-job-test` passed four host job tests, twelve
existing artifact checks and both installed job suites on x86_64 Linux, Rust
1.100.0 and the prepared PocketIC server. The installed phase took 7.77 seconds,
excluding preparation. The added cancellation test covers seven newly cancelled
jobs, six successful identical retries, six replacement recoveries, three
compatible upgrades and fifty expected refusals. It exercises empty, ordinary
and full native vectors, live controller revocation, traps after stable write
and thaw, cancellation after failed recovery, retired-epoch refusal, stale
cancellation, corrupt job bytes and abandoned native custody.

The cohort `target/evidence/provider-link/attempt.LE712E/` retains source inputs,
contracts, compiler identities, composed artifacts and `job.txt`. Principal
identities are source A with ordinary root (139371 bytes), SHA-256
`cf7afb35f96a8ef50a667df1c3f3ed88d8fa6d60d87c743a9d247378be379ba1`,
and recovery B (127538 bytes), SHA-256
`20c33ce32e4b5a9af9630d9c778135d96d050c8b616a3f7a984656c855dc204c`.
The first cancellation run stopped on an assertion expecting a specific controller
denial message from a generic panic trap. That run is not cancellation evidence;
the guard now emits an explicit controller-denial trap and the full suite passed.
The shared composer regression also passed its twelve artifact checks and four
installed native migration/reopening checks (3.88 seconds) in
`target/evidence/provider-link/attempt.biDYcA/`. Strict host/wasm Clippy,
formatting and diff checks passed; this is local Linux evidence, not hosted CI
or macOS qualification.

## Frozen-source evidence

On 15 November 2026, `make native-job-test` passed five host job tests, twelve
existing artifact checks and four job checks (one artifact-ordering test and
three installed suites) on x86_64 Linux with Rust 1.100.0 and the prepared server.
The job phase took 6.57 seconds, excluding preparation. The added installed
suite completed twelve frozen-compatible upgrades, six later replacement
recoveries and three cancellations. It rejected thirty-four upgrades and twelve
attempted native writes while frozen. Cases include empty/full vectors, repeated
resumption, both allocation domains, wrong modes/bindings, post-validation traps,
corrupt jobs and correctly digested jobs with wrong logical values.

Refusals preserve the source module, full native header/payload observations and
all stable bytes; a subsequent checkpoint query exercises the old transient
runtime again. Successful resumes leave the checkpoint bytes and frozen native
observation unchanged. Artifact admission checks bootstrap → transient reset →
job verifier order and rejects a valid same-signature substitution of native
initialization for frozen admission. These checks are not source authentication.

The cohort `target/evidence/provider-link/attempt.dmEMIF/` retains compiler,
codec, bridge and bootstrap sources, contracts, all candidate Wasm and `job.txt`.
Frozen-resume A is 141795 bytes, SHA-256
`3247f609959c455239572f3c50a13b676844fef47d5269bc44636f7ee9eb3c2c`;
frozen-resume B is 141795 bytes, SHA-256
`b5c2b0a5ed0ed02548d06afede18d0dba954162e927a0818cc2dbd2ef24fbff7`.
The four original native checkpoint/reopening checks also passed (3.91 seconds)
in `target/evidence/provider-link/attempt.xhxIJ8/`. Strict host/wasm Clippy and
formatting passed. Earlier cohorts retain their original scope and identities.
