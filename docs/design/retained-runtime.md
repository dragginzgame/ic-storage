# Public retained ordered runtime

`ic-storage`'s optional `retained` feature owns allocation, root custody and
startup admission for bounded ordered byte records. It closes the supplier
bootstrap blocker in [Storage #4](https://github.com/dragginzgame/ic-storage/issues/4).
The DB engine's own adoption and installed workload remain separate completion
gates. This is an experimental Rust 1.100.0 contract, not arbitrary Rust
persistence or production readiness.

## Application interface

Declare an ordered, nonempty list of at most sixteen `Root` values. Each declares
its root ID, participant ID, schema, entry limit and key/value byte limits.
IDs and schemas are nonzero; root IDs are strictly increasing. A participant can
own several roots. Maximum key/value lengths are 512/16,384 bytes; applications
can choose smaller limits for rows, indexes and job frames.

`Runtime::host(Geometry, &[Root])` owns process-local backing. On Wasm, use
`Canister::create` only during explicit fresh initialization,
`Canister::close` in the application's pre-upgrade hook, and
`Canister::reopen` in its post-upgrade hook. Reopen requires the exact bank,
physical capacities and declarations. It never falls back to creation. Creation
requires a zero directory and zero native backing, including both banks on Wasm.
The library exports no application endpoints or lifecycle hooks.

Obtain each participant's non-cloneable `Grant` once per generation. `read` and
`update` accept references to one or more grants and synchronously lend the public
ordered session views. They provide lookup, key-first ranges, prefixes, bounded
batches, deletion and portable export. Owners, allocators and borrowed records
cannot escape these callbacks. Ordinary operations reuse admitted owners; they
do not rescan tables. Admission walks native structure only at explicit reopen.

Closing revokes all grants, ends native owners, settles allocation custody and
then publishes the closed directory. Reopen creates new identities; stale,
foreign and duplicate grants refuse. Forgotten grants cannot keep allocation
owners alive. Active writes refuse reentry. Ordinary callback errors preserve
previous writes; a host unwind poisons the family. Host execution does not model
IC rollback. Installed IC traps roll back the entire message, including writes
and lifecycle metadata. Applications still own async quiescence, authorization,
checkpoint custody and deciding whether an error should trap.

## Mandatory artifact preparation

Enable `retained` in the canister and `retained-build` in a host build tool.
The retained feature rejects compilers other than Rust 1.100.0. Link the Wasm with:

```text
-C link-arg=--stack-first
-C link-arg=-zstack-size=1048576
-C link-arg=--global-base=37748736
-C link-arg=--initial-memory=41943040
-C link-arg=--max-memory=134217728
-C link-arg=--export=__stack_pointer
-C link-arg=--export=__data_end
-C link-arg=--export=__heap_base
```

Export the application's `canister_init`, `canister_pre_upgrade` and
`canister_post_upgrade` hooks with the IC's no-argument/no-result signatures.
Use the public host helper before installation:

```rust,ignore
let raw = std::fs::read("application.wasm")?;
let prepared = ic_storage::retained_build::prepare(&raw)?;
ic_storage::retained_build::admit(&prepared, &raw)?;
std::fs::write("application-installed.wasm", prepared)?;
```

`prepare` validates geometry, compiler data placement, lifecycle signatures and
imports. It turns compiler data into passive segments and installs a Wasm start
function that clears transient memory before replaying those segments. It resolves
the runtime's mandatory `ic_storage_retained_v1.admitted` import only together
with this transformation. Unprepared canisters retain an unsupported IC import.
`admit` compares the exact deterministic output with its raw input; later binary
changes require renewed admission. This is not an audit of arbitrary unsafe
application code or proof of compiler provenance from Wasm bytes alone. Preserve
compiler/build identity alongside the raw and prepared artifacts.

## Allocation and safety boundary

| Interval | Owner and upgrade behavior |
| --- | --- |
| 0 through 1,572,864 | Transient stack; cleared before Rust starts |
| 1,572,864 through 1,576,960 | Byte-defined 4-KiB root directory; retained |
| 2 through 18 MiB | Native bank A; retained |
| 18 through 34 MiB | Native bank B; retained |
| 36 MiB onward | Compiler data, private Rust headers, ordinary Global heap; reconstructed |

Compiler data and heap base must remain below 37 MiB. Linear memory starts at
40 MiB and is capped at 128 MiB. Each bank reserves its first MiB for nodes;
payload begins at the next MiB. `Geometry` selects 1–16,384 total node slots and
1 byte–15 MiB of shared payload. These capacities include coexistence during
Vec growth and fragmentation. Per-root logical maxima do not promise that every
root can reach its maximum simultaneously. Pools have 64 allocation-ledger slots;
normal roots need at most two allocations each across separate node/byte pools.
There is no automatic bank expansion or payload coalescing.

Only the private backing and pool modules use unsafe operations. Host backing
comes from aligned zeroed allocations, retained by `Rc` until the final owner
ends. Wasm backing uses initialized, exclusively reserved linear-memory windows
under the admitted linker/startup contract and the IC's retained-memory lifetime.
This is the explicit foreign-allocation and pointer-validity assumption of this
runtime. Application allocators must not claim those windows. Source close ends
Rust ownership before target startup; no private Vec, Rc, executable pointer or
allocator header is treated as retained Rust state.

The stored node is a checked 56-byte, 8-byte-aligned `repr(C)` record of seven
u64 fields; payload elements are u8. The allocator's sealed element set admits
only these scalar representations. It validates every span's length, capacity,
alignment-derived placement and disjointness before issuing any Vec. Collection
admission then checks tree links and payload ownership. A canonical SHA-256-sealed
`ICRET001` directory binds geometry, declarations, spans and closed state; the
seal detects corruption, not a malicious writer. The canister singleton and
non-Send/non-Sync ownership prevent competing authorities. M0, collection engines,
records and codecs continue to forbid unsafe code.

Native capacity refusals return errors. Transient Rust metadata can still exhaust
Global memory and trap on the IC (or abort a host process); it is not a fallible
allocator guarantee for all application allocations.

## Ordinary heap and portable recovery

Ordinary Vec allocations survive separate calls within a generation. Compatible
startup discards them and recreates Rust allocator metadata. Existing Wasm pages
cannot shrink, and this contract does not reclaim the previous generation's
ordinary heap for the new allocator. Repeated large transient allocations across
upgrades therefore increase the memory high-water mark toward the 128-MiB cap.
Monitor that operational limit and perform explicit portable recovery before it
is exhausted. Native deletion/reuse is independent of this Global-heap limit.

Export logical records through public grants before an incompatible change.
Complete `ORDMAP01` checkpoints or the bounded `ordered::paged` codecs provide
portable records. Applications own manifest/schema authorization, persistent job
progress, private candidates and atomic publication. Whole-heap replacement
followed by explicit fresh creation and logical reconstruction resets ordinary
heap growth and can select the opposite bank. A changed bank cannot be used as
a silent compatible upgrade. This does not claim cross-compiler migration has
been installed or that the public runtime already supplies a durable job engine.
The existing [paged installed job](ordered-recovery.md) remains a separate adapter.

## Qualification — 17 November 2026

`make retained-runtime-probe` checks the public transform and malformed memory,
imports, lifecycle signatures and startup. `make retained-runtime-test` installs
an application fixture that imports the public runtime without copying its
allocator, directory or startup. Both run strict Clippy first; CI includes the
artifact test and the installed lifecycle on its existing host matrix.

Six focused host cases cover overlapping sealed spans, corruption, occupied memory
behind a zero directory, physical capacity refusal, delete/reuse, generation and
participant authority, forgotten tokens, ordinary errors and poisoning.
Installed evidence covers four simultaneous main/source/target/job roots,
512-byte keys, 4-KiB rows and seventeen 16-KiB jobs; per-root and late trap rollback;
query isolation; four close/reopen cycles; compatible upgrades; missing/failed
source close; failed target startup; changed-bank refusal; and complete portable
reconstruction A→B→A with continued writes.

The final Linux cohort is `target/evidence/retained-runtime/attempt.isLgwP/` with
Testkit evidence under `target/evidence/testkit/run.sU6z69/evidence`. Two artifact
tests and the installed suite passed. A 12-MiB ordinary allocation survives across
messages; repeating it after a compatible upgrade grows memory from 833 to 1,026
Wasm pages. Portable replacement returns to 640 pages while preserving all four
logical roots. These are bounded installed results, not large-state performance
qualification or the DB engine's cutover witness.
