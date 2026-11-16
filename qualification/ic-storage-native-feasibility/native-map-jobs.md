# Durable two-map recovery jobs

The `native-map-job` profile adds the existing migration job state machine to
[two-map checkpoint recovery](native-map-recovery.md). It qualifies one atomic
reconstruction of up to 64 records per map. It does not qualify multi-message map
imports or recovery of the growing pool's 4,608-record workload.

## Durable identity and authority

`MapJob` is `MigrationJob<MapPair>`: phase transitions, epoch allocation,
cancellation, retry and consumption have one owner shared with scalar and record
jobs. The map payload wraps the unchanged two-map `NKPAIR01` codec. Its distinct
outer magic is `ICJMP001`, using the [version-1 job envelope](migration-job.md#contract-and-bytes):
64 header bytes, nested checkpoint, `JOBDONE1` and SHA-256. Maximum size is 2,360
bytes. Ready has an empty payload, epoch zero and equal domains. A sealed empty
map pair is a real checkpoint, distinct from Ready.

Allocation domains belong to each closed payload contract. Map jobs admit only
131072 and 163840; scalar and record jobs still admit only 131072 and 147456.
Map admission does not widen their native-layout contract. Map roots retain their
independent IDs 1 and 2 and all key/value bits. Unknown magic, phase, domain,
length, epoch or integrity refuses without resetting the job.

The governed locator is `native.checkpoint.maps.job.v1`, under the existing
`native-checkpoint` owner and `native.checkpoint.` authority prefix. Every open
reconstructs transient handles through frozen published Storage 0.74.1. A missing
locator refuses. Neither a bare map checkpoint nor an older native heap is
silently enrolled. The native provider uses format 6, distinct from bare recovery
format 5 and growing-pool format 4. Geometry and scalar staging remain those of
the bounded recovery profile.

Export and cancellation require `is_controller(msg_caller)`. Recovery and frozen
resumption are explicit management upgrades and inherit management authorization;
the job's domain identity is not executable authentication. Shared controller and
argument checks also serve existing scalar/record jobs. The test-only corruption
entry is controller-gated.

## Lifecycle and transaction boundaries

- Installation creates Ready. Export freezes both admitted native maps and seals
  the next sequential epoch with a different admitted target domain. Only an
  identical sealed export can retry. Source mutation and skipped/stale epochs
  refuse.
- Frozen compatible resumption requires the exact sealed job, epoch, domains and
  both logical payloads. Native admission ends before checkpoint Rust starts.
  An ordinary reopen of a frozen source refuses; the dedicated frozen entry
  validates continuity and leaves application writes blocked.
- Cancellation atomically marks the job Cancelled and thaws its original source.
  Its exact retry verifies active custody without freezing, restoring or
  overwriting newer application writes. The cancelled epoch stays retired;
  another export must use the next epoch.
- Recovery requires management Replace and a fresh zero native header. It
  validates and consumes the sealed job, stages scalars through WAT-only callbacks,
  and returns from checkpoint Rust. Only then does the wrapper construct both
  native maps through the actual product API and publish their roots. Consumption
  is written before construction, but both belong to the same atomic upgrade:
  a later native trap must roll back the stable write, heap and executable.
- Consumed jobs retain their checkpoint but cannot recover or cancel again.
  After successful recovery, normal writes and compatible reopening resume. The
  current target becomes the source for the next epoch, permitting repeated
  migrations in both directions without replaying earlier application state.

The fixture methods `export_checkpoint` and `cancel_checkpoint` take three
little-endian u64 words `(epoch, target_domain, fault)`. Frozen post-upgrade takes
the same tuple; recovery takes `(epoch, source_domain, fault)`. Zero fault is the
normal path. Export has one failure point after the stable write; cancellation
has two, after stable publication and native thaw. Recovery faults 1–4 trap after
consumption, during scalar staging, during actual native construction and after
both root publications respectively. No native Rust frame is live while any
checkpoint Rust frame is live.

A corrupt native source can be recovered only by an explicitly selected Replace
operation that skips its failing source hook, after an independent valid export.
Job integrity, expected epoch, domain and single-use checks still apply. This is
not an automatic reset, production operator policy, protection against a hostile
controller or protection against restoring an entire historical canister snapshot.

## Evidence and remaining work

`make native-map-job-test` runs locked offline host transition checks, Wasm
provider Clippy/build, raw-map artifact admission and an installed PocketIC matrix
using the already prepared server. Evidence includes source/lock/compiler inputs,
raw/composed artifacts, hashes and `JOB_RECOVERED`/`COMPLETE` markers.

On 16 November 2026, `target/evidence/native-map-provider/attempt.RCttsN/` passed
three artifact checks and the installed job matrix in 4.62 seconds. Thirteen
shared host job tests also passed. The installed matrix covers controller refusal
and revocation, empty and full 64+64 maps, full-width records, capacity refusal,
exact export retries, frozen resumption and changed-payload refusal, cancellation
and preservation of subsequent writes, Keep refusal, stale epochs/wrong domains,
all publication fault points, consumed replay refusal after new writes, repeated
domain migrations and explicit recovery after native-header loss. Refused updates
compare native bytes and stable memory; refused upgrades also compare executable
identity. Discarded source payload bytes are zero after Replace.

The scalar job regression in `target/evidence/provider-link/attempt.whF5TW/`
passed 19 artifact checks and seven lifecycle/staged/restart tests (40.96 seconds).
The record-job regression `attempt.x21bYQ` under the same provider-link evidence
directory passed 19 artifact checks and five record-job tests (45.25 seconds).
The bare map-recovery regression `target/evidence/native-map-provider/attempt.rjJO2M/`
passed three artifact checks and its installed recovery matrix (2.39 seconds).
The map cohort uses Rust 1.100.0, Toko 0.44.11, Platform Runtime 0.23.14 and the
existing Testkit 0.107.3 lock selection with the prepared 0.106.0 launcher.

Next qualify durable bounded batches, interruption/restart and complete-root
publication for maps, then a recovery workload matching the larger growing pool.
The shared job engine already has staged transitions for scalar/record profiles;
that does not establish installed map prefix validation or restart safety.
