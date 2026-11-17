# Durable two-map recovery jobs

The `native-map-job` profile applies the shared migration job state machine to
[two-map checkpoint recovery](native-map-recovery.md). It qualifies atomic recovery
and staged reconstruction across messages, with at most 64 records in each map.
It does not qualify recovery of the growing pool's 4,608-record workload.

## Durable identity and authority

`MapJob` is `MigrationJob<MapPair>`: phases, epochs, cancellation, retries,
progress and consumption have one owner shared with scalar/record jobs. The
nested payload is the unchanged two-map `NKPAIR01` codec; outer magic is
`ICJMP001`. The [job envelope](migration-job.md#contract-and-bytes) has a 64-byte
header, payload, `JOBDONE1` and SHA-256, with a maximum of 2,360 bytes. Ready has
an empty payload, epoch zero and equal domains. A sealed empty pair is a real
checkpoint. Version 1 covers atomic jobs; version 2 uses the shared
[progress fields](staged-import.md#versioned-progress) at offsets 44/48/52 for
committed prefix length, latest batch start and requested limit.

Map domains are 131072 and 163840. Scalar/record domains remain 131072 and 147456;
map support does not widen their representation admission. Roots retain IDs 1/2
and all u64 key/value bits. Unknown identity, phase, geometry, length, epoch or
integrity refuses without resetting state.

Every open reconstructs transient governed Storage 0.74.1 handles under owner
`native-checkpoint` and authority prefix `native.checkpoint.`. The current adapter
uses two `StableCell<Vec<u8>>` allocations:

| Locator | Cell payload |
| --- | --- |
| `native.checkpoint.maps.job.metadata.v1` | Exactly 104 bytes: canonical job header (64), `JOBDONE1` (8), digest (32) |
| `native.checkpoint.maps.job.checkpoint.v1` | Unchanged nested `NKPAIR01` checkpoint, at most 2,256 bytes; empty only for Ready |

The metadata digest still covers **header + checkpoint + completeness marker**
in the original canonical order. The shared job codec splits/reassembles that
envelope and performs its existing complete validation. The evidence query returns
the validated canonical envelope, at most 2,360 bytes. Missing, oversized, corrupt,
truncated or mismatched parts refuse without replacement initialization. Recovery
requires both locators to exist before opening allocations; entirely empty backing refuses
before constructing a provider directory.

Export writes a changed checkpoint first, then metadata. Subsequent start,
progress, restart, cancellation and consumption write only the 104-byte metadata
payload. Identical writes skip both cells. The adapter relies on the IC transaction
for atomicity across the two cells and later native publication; write order alone
does not provide native host crash recovery. A trap between payload and metadata
publication must roll back both. Metadata encoding now hashes the existing
header/checkpoint/seal slices with fixed scratch space. Split decoding admits the
same slices directly, without allocating a temporary joined envelope. It still
allocates the validated job's owned checkpoint and logical candidates. Complete
checkpoint reads, digest work and validation remain proportional to the bounded
payload; this is not yet a paged reader or incremental prefix validator.
The product's [sequential page codec](../../docs/design/paged-map-checkpoint.md)
now has separate governed host qualification. Adopting its manifest and page
cursor into this installed two-root job remains explicit integration work.
Batch reads use the shared `decode_split_batch` operation: the logical maps from
complete admission feed the existing batch transition directly, eliminating a
second checkpoint decode and map reconstruction. Exact retries also validate
once per request. There is no cache across requests or caller-supplied validation
token, and the adapter still validates the complete retained prefix before new
native writes. Stored bytes, limits and publication ordering are unchanged.

Current native format **8** binds this two-cell job profile and distinguishes active
settlement 1, frozen-source settlement 2 and private-import settlement 3. It rejects
previous native formats, including atomic-job format 6 and single-cell staged
format 7; no old heap is silently reinterpreted. The previous
`native.checkpoint.maps.job.v1` locator and bare checkpoints are not enrolled or
rewritten automatically. Historical cohorts retain their matching artifacts;
transferring an existing older fixture requires separately qualified explicit work.
This is a closed qualification format, not a released consumer layout promise.

The fixed 24 KiB pool, root descriptors and scalar staging area retain the
[bounded bridge geometry](native-map-recovery.md#runtime-domains-and-sequencing).
All native owners end before checkpoint Rust starts; scalar callbacks never
re-enter native Rust. Actual native writes happen only after checkpoint Rust
returns. Exact composition hides private entries and admits only the reviewed
callback roster and call graph.

Export, cancellation, import batches and fault-injection methods require
`is_controller(msg_caller)`. Fresh recovery, staged start, resume and restart
are explicit management upgrades. A domain identity is not executable
authentication; trusted installation remains a separate obligation.

## Source and atomic recovery

Export freezes both native maps and seals the next sequential epoch for the
other admitted domain. Only an identical sealed export retries. Frozen compatible
resumption requires the exact job, epoch, domains and both logical payloads;
ordinary reopen refuses a frozen source. Cancellation atomically retires the
sealed epoch and thaws its original source. Its exact retry neither freezes nor
overwrites newer application writes; subsequent export requires the next epoch.

Atomic recovery requires management Replace and a fresh zero native header.
Checkpoint Rust consumes the sealed job and stages all records. After it returns,
the wrapper constructs both maps through the actual product API. Stable
consumption and native publication share one upgrade transaction; a trap at any
later point rolls back stable memory, heap and executable. Consumed jobs cannot
recover or cancel again. Their target becomes the next epoch's source.

## Staged import, interruption and restart

Staged start also requires Replace/fresh admission, changes Sealed to Importing
and initializes two empty private native maps after checkpoint Rust returns.
Application writes, source cancellation, export and ordinary reopen remain
blocked. Even an empty pair stays Importing until its first completing batch.

The cursor orders **root 1, then root 2, with ascending u64 keys inside each**.
Checkpoint decoding returns sorted logical records directly through the product
`Map::records_from_checkpoint` decoder, which checks framing, epoch, count,
integrity and duplicate keys without allocating native hash buckets. Atomic
recovery uses the same records. The staged adapter does not copy or re-sort them;
progress never depends on native buckets, allocation history or decoder iteration
order. Before a new batch, the adapter compares both native prefix counts and
every expected key/value against the independent checkpoint. A batch applies
1–64 records (or the remaining tail), including across the root boundary.

Prefix comparison reads each native record once through the existing scalar
cursor. `checkpoint::maps::verify_records` checks the count before reading,
finds each key by binary search in the admitted sorted prefix, compares its value
and marks its position in an eight-byte bitmap. A repeated position refuses;
equal counts and unique matching positions prove that no expected key is missing.
The comparison creates no transient maps or record copies. Scratch is fixed
for this 64-record profile; lookup work is O(n log n), in addition to the native
bucket scan. This is not a measured throughput gain or bounded total recovery
work, and it does not replace native ownership or allocator admission.

The adapter first writes the new durable cursor, then stages only the next
records through scalar callbacks. After Rust returns, the wrapper temporarily
admits native mutation, inserts the batch, verifies both resulting counts and
returns to private settlement, including for the final batch. Once every native
Rust owner has ended, a private checkpoint entry reopens the job and compares
the actual native counts and every key/value with its newly committed prefix.
The shared job owner admits staged Importing or Consumed progress at the exact
epoch/domains; Ready, Sealed, Cancelled and atomic-only Consumed jobs cannot use
this post-apply path. The verifier acknowledges completion through a scalar WAT
callback, which checks that the native publication flag agrees with the job.
Only after that acknowledgment may the wrapper publish active custody and reply.
The cursor, native allocations and settlement commit in one IC message. Failure
rolls back that message while preserving earlier committed batches.

Staged start and explicit fresh-target restart use the same post-apply boundary
to verify their new empty roots. Native apply, checkpoint verification and native
publication are sequential frames, never reentrant calls between Rust domains.
All sixteen map-job scalar callbacks retain a closed WAT-only call graph. The
new commit callbacks are private after composition, and artifact checks require
verification between application and publication in both upgrade and batch paths.
The transient acknowledgment is not persisted Rust state or a new native format.
This is the required post-write location for future paged continuation capture;
the installed profile still uses `NKPAIR01`, not the paged payload.

Only the exact latest `(start, requested_limit)` retries. An unfinished retry
validates the already committed prefix and makes no new writes. A completed
retry validates active native custody and acknowledges completion without
comparing against the old checkpoint or overwriting newer application changes.
Older/changed requests and stale epochs/domains refuse.

Compatible Importing resumption admits the private native maps, rebuilds the
checkpoint transient domain and validates the complete prefix against durable
progress. Corrupt or mismatched prefixes refuse. Explicit Restart accepts only
an unfinished Importing job at its existing target and epoch. It replaces the
partial heap, reopens the independent checkpoint and resets the cursor to zero;
old batch acknowledgments become invalid. Completed, sealed, cancelled and
corrupt jobs cannot restart. Header/settlement corruption additionally requires
explicit source-hook bypass; it never causes automatic reset.

## Fixture operations and bounds

Export/cancel/frozen resume use three little-endian u64 words
`(epoch, target_domain, fault)`. Atomic recovery, staged start, Importing resume
and restart use `(epoch, source_domain, fault)`. `import_batch` takes five:
`(epoch, source_domain, start, limit, fault)`. Zero fault is normal operation.
Start/restart faults 1–3 trap after stable publication, native initialization and
private settlement; fault 5 traps after post-apply verification. Batch faults 1–4
trap after the progress write, during scalar staging, during actual insertion
and after native insertion before verification. Batch fault 5 traps after
verification; fault 6 changes an applied native value so verification must refuse.
Fault 7 changes the native completion flag while retaining valid records; the
job-owned completion decision must reject that mismatch before publication.
Fault 8 replaces a second occupied native slot with the first slot's complete
key/value while preserving counts. The installed test requires the explicit
post-apply comparison trap, not merely any Wasm failure, and checks rollback.
Faults apply to new batches; retry acknowledges existing progress without writing.
Export fault 1 traps after complete job publication; fault 2 traps after the
checkpoint cell write and before the metadata write.

Export is still one bounded transaction. Each batch/resume decodes the whole
checkpoint and validates the whole committed prefix; a new batch additionally
decodes and checks its resulting prefix after native application. Sorting, admission scans
and transient-domain clearing are fixture costs. Native scalar copying uses the
[sequential reader](native-map-recovery.md#runtime-domains-and-sequencing), bounded
by the two bucket counts per read, including private-prefix validation. It does
not make complete prefix validation or checkpoint reads incremental. Progress
writes are fixed at 104 bytes; new exports still replace the complete checkpoint.
This is bounded-work evidence at 128 records, not a large-data throughput or
downtime guarantee. Larger state
needs matching checkpoint storage, incremental validation and measured budgets.
No real compiler transition, arbitrary graphs, hostile-controller protection,
whole-canister snapshot rollback protection or production recovery authority is
claimed. Consumer cutovers require their own authorization and data contracts.

## Qualification evidence

The 0.75.5 development [paged native adapter](../../docs/design/paged-map-checkpoint.md#installed-paged-native-recovery)
now qualifies combined retained-root/continuation/progress rollback within the
same bounded native staging ABI. Its separate `make native-map-paged-test`
cohort in `target/evidence/native-map-provider/attempt.ugEKzt` passed three native
artifact checks and two installed cases (11.90 seconds). The existing `NKPAIR01`
job profile, including cancellation and later epochs, passed again in
`attempt.edtOHJ` (three artifact checks, one ordering check and three installed
cases; 11.85 seconds for the job group). Eight governed host tests and strict
Clippy for the shared callbacks, adapters and caller also passed.

The earlier 0.75.5 development continuation observer uses the existing scalar callbacks
to fingerprint admitted frozen/private roots without rebuilding maps. The
qualification-only `native_prefix` query returns two count/fingerprint pairs;
its native admission frame ends before checkpoint Rust begins, and active roots
refuse. In `target/evidence/native-map-provider/attempt.PWFDBc`, three artifact
checks, the composition ordering check and three installed cases passed
(11.71 seconds for the job group). Independent raw-slot hashing agrees for
frozen sources and staged prefixes across root crossings and restarts. This
qualifies the observation boundary; that run used `NKPAIR01` and did not
persist paged continuations alongside native writes. The existing
callback roster, stored bytes and 64-record-per-root bound remain unchanged.

The 0.75.5 development scalar-prefix comparison passed four focused host tests
and strict host/Wasm Clippy on 17 November 2026. Installed evidence in
`target/evidence/native-map-provider/attempt.E0buFe` passed three native artifact
checks, one ordering check and all three map-job cases (12.44 seconds for the
job group). Duplicate-slot faults reach the post-apply verifier on partial,
root-crossing and final batches and preserve the previous native/stable state.
The initial diagnostic-only assertion failure remains in `attempt.4PyEjt`;
the final adapter uses an explicit trap message instead of a generic Rust panic.
Stored formats, scalar callback roster, installed limits and completed retry
behavior remained unchanged; that run preceded paged continuation integration.

Testkit 0.108.0 adoption passed strict Clippy for every retention test target and
the benchmark caller, plus dependency inheritance and PocketIC-only graph checks.
With the existing Runtime 0.23.16 lockfile selection, the rebuilt installed map
cohort in `target/evidence/native-map-provider/attempt.q6kklt` passed three native
artifact checks, one ordering check and all three map-job cases (11.79 seconds).
The installed scalar retention/failed-upgrade case also passed. These runs use
the separately prepared 0.106.0 launcher and PocketIC 16.1.0; optional held-reply
fixtures remain disabled and no Protocol 0.3 dependency is introduced.

The v0.75.4 delivery candidate passed the repository CI gate and strict lint on
17 November 2026. After the final Host Artifacts 0.80.3 lockfile update, affected
dependency, retention and benchmark checks passed again. Final installed cohorts
under `target/evidence/` passed with Runtime 0.23.15 and Testkit 0.107.4:
`native-map-provider/attempt.1tUWpo` (three native artifact checks, one ordering
check and three installed cases, 11.59 seconds for the job group),
`provider-link/attempt.zox0Zp` (19 artifact checks and seven scalar-job cases,
40.83 seconds), and `provider-link/attempt.yRIfoI` (19 artifact checks and five
record-job cases, 44.52 seconds). The separate paged recovery profile remains
host-qualified; installed jobs retain their existing payloads and limits.

The 0.75.4 development post-apply boundary passed on 17 November 2026 in
`target/evidence/native-map-provider/attempt.1sGans`: three native artifact
checks, one lifecycle-ordering artifact check, and all three installed map-job
cases (11.60 seconds for the job group). The new fault cases cover post-verification
traps during staged start/restart and batches, corruption after real native
insertion, and mismatched completion flags on partial and completing batches.
Refusals preserve complete native
observations and stable bytes; failed upgrades also preserve the old executable.
Exact completed retries preserve subsequent application mutations. Twenty-seven
shared host job tests, four governed-cell tests and focused host/Wasm Clippy
passed with Runtime 0.23.15, Testkit 0.107.4 and the prepared 0.106.0 launcher.

The v0.75.3 delivery passed the repository CI gate on 17 November 2026 after
the module-directory cleanup. Final installed artifacts passed under
`target/evidence/`: `native-map-provider/attempt.a1pvqs` (three artifact checks
and three map-job cases, 10.47 seconds), `native-map-provider/attempt.7VuLcD`
(three artifact checks and bare-map recovery, 3.18 seconds),
`provider-link/attempt.UcgQK4` (19 artifact checks and seven scalar-job cases,
39.78 seconds), and `provider-link/attempt.UUSKQJ` (19 artifact checks and five
record-job cases, 43.93 seconds). The separate paged format remains host-qualified;
these installed cohorts use the existing job/checkpoint format.

Direct logical-record decoding passed five product checkpoint tests, two paired
checkpoint tests, eighteen shared job tests and four metered-adapter tests.
Product tests compare sorted records with native reconstruction through 4,608
entries and reject nonadjacent duplicates with repaired digests, truncation,
invalid count/epoch and corruption. Focused host/Wasm Clippy and formatting passed.
Installed map-job checks passed in
`target/evidence/native-map-provider/attempt.ywl6IF` (three artifact checks and
three atomic/staged/restart cases, 10.91 seconds); bare-map recovery passed in
`attempt.8GXpgi` (three artifact checks and one installed matrix, 3.25 seconds).
This removes checkpoint-side hash-table reconstruction and the staged adapter's
record copy; native-prefix validation still reconstructs its independent
transient view. The 4,608-entry test establishes host codec behavior only; the
installed limit remains 64 per map. Unchanged scalar/record transitions retain
their previous installed evidence below.

The combined admission/batch path passed eighteen shared host job tests and four
metered-adapter tests on 16 November 2026. A test-only payload wrapper delegates
to the real map decoder and counts calls: separate admission/batch decodes twice,
combined admission once, and each retry validates once again. A corrupt outer
digest refuses before logical decoding; re-signing a corrupt nested checkpoint
still refuses. Transition comparisons cover empty/asymmetric/full pairs, invalid
offsets/limits/bindings, root crossings and completed retries. Adapter checks use
the combined operation and retain the 104-byte progress/zero-byte retry bounds.
Focused host/Wasm Clippy and formatting passed. Final installed artifacts passed
under `target/evidence/`: `native-map-provider/attempt.nbdC1g` (three artifact
checks and three map-job tests, 10.56 seconds), `provider-link/attempt.OEEipM`
(19 artifact checks and seven scalar-job tests, 39.91 seconds), and
`provider-link/attempt.DcFQ81` (19 artifact checks and five record-job tests,
43.85 seconds). These results include ordinary reads dropping transient decoded
values before allocating the owned checkpoint; they do not establish a throughput
improvement or increase the qualified recovery limits.

The v0.75.3 development codec passed sixteen shared host job tests (including
the independent staged-map golden), four metered-adapter tests and focused
Clippy/formatting checks. Installed regressions passed under `target/evidence/`:
`native-map-provider/attempt.rvYkE8` (three artifact and three map-job tests,
10.17 seconds), `provider-link/attempt.NTVfdW` (19 artifact and seven scalar-job
tests, 40.21 seconds), and `provider-link/attempt.8E8fxQ` (19 artifact and five
record-job tests, 43.45 seconds). These checks preserve the existing byte formats;
they do not qualify larger checkpoints or bounded-read recovery.

The v0.75.2 delivery passed the repository CI gate and then the affected installed
checks with Testkit 0.107.4: `attempt.Pg6AxM` (three artifact checks and three job
tests, 10.23 seconds) and `attempt.IetQrQ` (three artifact checks and bare-map
recovery, 3.13 seconds), under `target/evidence/native-map-provider/`. The Testkit
library sources match 0.107.3; 0.107.4 additionally restores its separately
selectable launcher binary. This qualification keeps the prepared 0.106.0 launcher.

The two-cell profile passed in
`target/evidence/native-map-provider/attempt.2QuAya/` on 16 November 2026:
three artifact checks and all three installed atomic/staged/restart tests
(10.72 seconds), including export rollback between the two cell writes and
independent metadata/payload corruption during sealed recovery and partial import.
The single-cell bare-map regression passed in `attempt.CJ79gh` (three artifact
checks and one installed matrix, 3.45 seconds). These cohorts use native job
format 8; earlier format-7 cohorts below retain their historical scope.

Four host adapter tests use the published Storage `MeteredMemory`, running the
same two-cell code as the Wasm fixture. At 0, 2 and 128 records, each new progress
update writes exactly 104 backing bytes in one write, with no growth; exact
retries write zero bytes. Cancellation, restart and atomic consumption preserve
the checkpoint. Reopening writes nothing. Missing either locator, the previous
single locator, an empty backing, corrupt/truncated/oversized parts and valid but
mismatched payloads refuse without writes. Fifteen shared host job tests include
canonical split/reassembly equality and corruption rejection. Focused host/Wasm
Clippy and formatting passed. Host IO counts do not establish IC instruction
costs or host crash atomicity.

After adopting the sequential scalar reader,
`target/evidence/native-map-provider/attempt.OMqVbt/` passed three artifact checks
and all three installed atomic/staged/restart tests (9.00 seconds) on
16 November 2026. Prefix checks independently compare bucket-order scalar output
and count actual bucket visits on empty, asymmetric and full partial imports.
Every read stays within the relevant bucket count. The complete checkpoint and
prefix are still decoded/validated for each batch. Fourteen host job tests and
focused Clippy also passed; unchanged scalar/record composer evidence below is
retained rather than rerun for this map-only callback change.

`make native-map-job-test` uses locked offline builds and the prepared PocketIC
server, retaining source/lock/compiler inputs, artifacts and hashes. On
16 November 2026, `target/evidence/native-map-provider/attempt.ZmZn2O/` passed
three artifact checks and three installed tests (8.76 seconds); fourteen shared
host job tests also passed. Markers `JOB_RECOVERED`, `STAGED_MAP_RECOVERED`,
`STAGED_MAP_RESTARTED` and `COMPLETE` are required by the collector.

The installed matrix covers empty/asymmetric/full pairs, full-width keys/values,
batches crossing roots, exact retries, authority revocation, incompatible entry
refusal, compatible interruptions, reverse-domain migration, all publication
fault points, completed retries after new writes, corrupt native values/headers/
settlement, healthy and damaged-prefix restarts, corrupt checkpoint refusal and
explicit source-hook bypass. Refusals compare native bytes and stable memory;
failed upgrades also preserve executable identity. Atomic lifecycle coverage
continues to run alongside staged cases.

Shared composer regressions passed in `provider-link/attempt.wM8HP7` (19 artifact
checks and seven scalar-job cases, 39.53 seconds) and `attempt.e60wH9` (19 artifact
checks and five record-job cases, 43.17 seconds). Bare map recovery passed in
`native-map-provider/attempt.cZX7fa` (three artifact checks and one installed
matrix, 2.33 seconds). Strict Clippy, formatting and diagnostic-collector checks
also passed. These paths are beneath `target/evidence/`.

Earlier atomic-only format-6 evidence remains in `attempt.RCttsN` (4.62 seconds),
with scalar regression `provider-link/attempt.whF5TW` and record regression
`provider-link/attempt.x21bYQ`; those historical results do not establish staged
map behavior. Current fixtures use Rust 1.100.0, Toko 0.44.11, Runtime 0.23.15,
Testkit 0.107.4 and the separately prepared 0.106.0 launcher. The next integration
step is recovery of larger grown pools with bounded storage/validation costs.
