# Architectural hard cut — 2026-11-08

## Selected implementation

This implements the approved reduction audit within Storage's two workspaces.
It does not publish packages or alter sibling repositories. Existing pending
lockfile selections are preserved. Pending release lines are Storage 0.69,
Metrics Storage 0.71 and Logging Storage 0.67; manifests remain at delivered versions.

| Line | Authoritative survivor and deletion | Result |
| --- | --- | --- |
| Benchmark catalogue | Structured collection/key/value cases replace 58 named cases and parallel dispatch; the 73-type catalogue supplies both libraries | Retained, prior concrete cases and boundary payloads preserved |
| Memory/codec bridge | Public Memory and Bound replace private duplicates and the Backing wrapper | Retained; bridge checks actual Bound directly, not overridable codec helper methods |
| Collection results | Direct successful values replace impossible Result wrappers and error variants | Retained; constructors, reservations and region admission retain genuine typed errors |
| Cache controls | Delete ineffective Storage node-cache settings, CLI fields and tests | Retained; actual optional Git ICSS cache experiment remains separate |
| Engine shells | One Vector implementation, direct lazy objects and page geometry | Retained; no persisted header changes |
| Admission | Carry one canonical admitted-key set into binding and directory mutation | Retained; deterministic IDs, policy and failure behavior preserved |
| Iterator ownership | Inline nodes and owned raw pairs replace shared-node cursor entries | Rejected after Wasm measurement; original shared-node iterator remains |
| Historical probes | Delete old stable-read probe executables and retired Vector geometry predictions | Retained; historical evidence links pin original source |
| Test infrastructure | Existing MeteredMemory replaces adapter counters; remove redundant fixture Cell | Retained; independent failure tests remain |
| Helper gate | Exclude all repo_tool tests from ordinary Storage gate; dedicated helper gate remains | Retained; all 88 helper tests pass |

Ordinary Map/Set/Vector/Cell operations and iterator items now return values
rather than an always-successful Result. Workspace callers migrate together.
Consumers remove only the obsolete error layer: an Option returned by lookup
still represents absence. Allocation, admission and reservation errors remain.
Metrics and Logging adapter checkpoint results retain their genuine errors.

Private engine, bridge and runtime changes preserve BTR2/BTN2, SVC1, SCL1,
directory images, metric frames and discriminator-3 logging frames. This batch
requires no reset of current images. Native unwind poisoning, Wasm rollback,
exclusive leases, backing custody and bounds validation remain necessary.
Malformed codec lengths trap before payload writes; new hostile-codec tests
verify overriding public helper methods cannot bypass geometry validation.

## Wasm selection

Raw application fixture bytes, same compiler and build settings. Before is the
retained delivered fixture `attempt.tvXumh`; final is `attempt.rELoGp` under
`target/evidence/storage-consumer-size/`. The earlier lock differs only in local
Storage's package version; this is retained baseline evidence, not a fresh HEAD build.

| Selection | Previous Storage | Final Storage | Published ICSS 0.7.2 |
| --- | ---: | ---: | ---: |
| Base | 16,998 | 16,998 | 16,994 |
| Map | 134,314 | 133,400 | 90,264 |
| Set | 134,028 | 132,844 | 90,064 |
| Vector | 109,531 | 108,366 | 64,616 |
| Cell | 106,460 | 105,255 | 63,195 |
| Region | 135,361 | 134,461 | 89,989 |
| Map and region | 137,877 | 136,955 | 110,914 |
| All five | 150,062 | 147,911 | 119,378 |

All-five saves 2,151 bytes; the remaining ICSS gap is 28,533 bytes. These are
application fixtures with allocation governance, not the broad benchmark harness.
All 16 final artifacts pass 1,888 installed calls and 32 upgrades.

The rejected inline-node iterator made the broad Storage module 12,667,392
bytes. Reverting only that experiment in the same harness produces 11,532,762:
**1,134,630 bytes smaller**. Code-section encoded size falls from 10,908,367 to
9,766,784 bytes. Repeated typed Map/Set specializations caused the growth;
it was not merely debug names. Small-consumer results alone missed this problem.
The final broad ICSS module is 11,091,029 bytes. Harness sizes are not application
size estimates. Older broad results also changed catalogue or directory code,
so they cannot isolate this batch's savings.

## Qualification and reproduction

- Core native: 89 passing, one ignored library test; four runtime tests and the
  compile-fail suite pass. Adapter libraries: 29 Metrics and 13 Logging tests pass.
- Benchmark catalogue and driver: 48 tests pass, including all supported catalogue
  cases across four access patterns and three mutation rounds. Prior 58 named combinations
  remain representable, including boundary payloads and full Session coverage.
- All package doctests, strict native/all-target benchmark lint, feature builds,
  production dependency graph, pins and
  formatting pass. All 88 repository-helper tests also pass. Production still excludes ICSS and native Platform Runtime.
- Final installed IO, collection reopen/upgrade, decoder/update/upgrade rollback,
  six directory failure checkpoints, custody and logging checks pass. The large
  reopen scenario retains 1,031 named allocations.
- Source reduction is approximately 1,150 Rust lines net, including 278 production
  Storage lines. This is source reduction, not a corresponding instruction claim.

Reproduce application size with `make benchmark-storage-size`; reproduce the
all-type comparison with `make benchmark-storage BENCH_WORKLOAD=types BENCH_SAMPLES=1`.
Both commands capture source, lockfiles, compiler, flags, modules and checksums.
Raw artifacts live under target and can disappear after authorized cleanup.
Final broad comparison evidence is `storage-comparison/attempt.FAkSnk`.

## Complete type comparison

The final run passed all five shards: **565 cases, 51,768 measured calls and
53 expected refusal records**, followed by final source/artifact checksum checks.
The catalogue has 73 types. Unsupported roles are explicit; no decoder or
ordering capability is invented to make them benchmarkable. There are 26,214
summary rows including control/refusal rows.

Paired non-control timing cells below use one sample, 32 base records (capped
for tiny domains), batches 1/8 and the catalogue's payload boundaries. Ratios
are Storage/ICSS; lower is better. A median across heterogeneous cells is a
coverage summary, not a workload-weighted throughput estimate.

| Collection | Paired cells | Storage faster / equal / slower | Median ratio | Range |
| --- | ---: | ---: | ---: | ---: |
| Map | 13,806 | 6,937 / 3 / 6,866 | 0.9999 | 0.0306–2.0899 |
| Set | 1,322 | 605 / 0 / 717 | 1.0022 | 0.0291–1.1515 |
| Vector | 2,352 | 550 / 0 / 1,802 | 1.0013 | 0.0481–1.9800 |
| Cell | 962 | 583 / 86 / 293 | 0.9992 | 0.5863–1.2400 |
| Region | 5,384 | 337 / 2 / 5,045 | 1.0285 | 0.7858–1.7054 |

This is final Storage versus ICSS, not an isolated before/after instruction
experiment. It demonstrates no universal performance superiority. Region and
warm-lookup overhead remain candidates for a separately measured follow-up.
The reduction itself is justified by less duplicate code and smaller matched
application Wasm. After capture, only documentation and native test coverage
changed; the measured runtime implementation remains the selected one.

## Session lookup control

The final modules also pass AuthLookups at 1/512/1,024/2,048 records in update
and query modes (`target/evidence/architectural-reduction/auth/`). One sample
per cell is a controlled comparison, not a distribution or complete Auth endpoint.
Selected 2,048-record update, first record, 256-byte Session, batch one:

| Timed phase | Storage instructions | ICSS instructions |
| --- | ---: | ---: |
| Get | 83,162 | 82,511 |
| WarmGet | 3,162 | 2,511 |
| Decode | 1,734 | 1,727 |
| Missing index | 80,664 | 80,587 |

Across the six 2,048-record update Get cells Storage/ICSS ranges 1.0001–1.0079;
WarmGet ranges 0.9865–1.2593. Warm operation overhead is visible even when first
access charges dominate an ordinary Get. These are final-library comparisons,
not before/after evidence attributing a speedup to this hard cut. Configuration
preparation and Auth's inner session decoding remain outside the fixture.

## Remaining boundaries

Canic and IcyDB were inspected but not modified. Their existing published
Storage dependencies remain valid; adopting this breaking line requires caller
migration. IcyDB's overlay iterator error types need explicit migration, not
blind removal of every Result. Concurrent sibling work remains untouched.
No crate merger, new dependency, persisted-format reset or governance deletion
is justified by this reduction. Full release/archive qualification and independent
publication remain separate from the development checks recorded here.
