# Bounded staged native import

This qualification extension of the [governed job](migration-job.md) reconstructs
one native u64 vector across messages. It uses the same governed Storage cell,
two native allocation domains and 512-element logical checkpoint. It introduces
neither a product API nor arbitrary Rust graph migration.

## Versioned progress

Version-1 job bytes, initial golden and existing transitions are unchanged.
Explicit staged start converts a Sealed job into envelope version 2, phase
Importing=4. Only Importing and Consumed=2 are permitted in version 2. The header,
payload limit, nested checkpoint and integrity coverage remain the same, with
these formerly reserved fields given explicit version-2 meanings:

| Offset | Little-endian field |
| --- | --- |
| 44 | u32 next element / committed prefix length |
| 48 | u32 latest committed batch start |
| 52 | u32 latest committed batch requested limit |
| 56 | Eight zero reserved bytes |

Initial progress is `(0, 0, 0)`. A committed batch has limit 1–64 and
`next = min(start + limit, checkpoint_count)`. Its start precedes the total,
except that an empty checkpoint uses start zero. Importing requires an unfinished
prefix or initial progress; Consumed requires a committed final batch. All
cursor fields, nested framing, domains and epoch validate before returning a
job. Old version-1 readers reject version 2 rather than interpreting its cursor
as reserved bytes. No new stable locator is created during import.

## Construction and publication

The explicitly selected staged-start target requires fresh native metadata,
loads the exact Sealed epoch/source/compiled target, and initializes a candidate
counter and empty prefix. It commits Importing in the same upgrade transaction.
The old source heap is replaced, not retained as a cancellation fallback. A trap
restores the source module, frozen native state and stable record.

Native format 2 gains settlement 3 for an **unpublished construction prefix**.
Both length and capacity record the initialized prefix length, bounded to 512;
transient phase 5 blocks all native owner/Vec access. Older ready/frozen admission
rejects settlement 3. The prefix is scalar u64 storage under WAT custody, not a
partially constructed Rust Vec. Existing atomic recovery still uses its distinct
phase 4 and transient restore counter; staged import does not rely on that global.

`import_batch` requires a current controller and five u64 arguments:
`[epoch, source, offset, limit, fault]`. Domain and epoch must match the job and
the compiled destination. A new request must start at the committed cursor.
Before writing, the adapter compares native count, counter and every committed
element with the independent checkpoint. WAT callbacks append only at that
cursor and update retained length/capacity after each initialized scalar write.
The stable progress and native prefix commit together in one IC update; a trap
rolls back only this message, preserving earlier completed batches.

Only the exact latest `(offset, limit)` request can be acknowledged again.
Retries do not append, rewrite the cell or advance the cursor. A retry during
construction revalidates the committed prefix. An acknowledged final retry
checks ready custody without restoring checkpoint values: ordinary native writes
may already have occurred. Older/changing retries, skipped offsets, invalid
limits, wrong epochs/domains and malformed arguments trap. A later migration
retires the retry acknowledgement by publishing the next epoch.

The final batch alone switches to ready native settlement and Consumed in the
same message. An empty vector still requires that explicit final batch. No Rust
native owner exists while checkpoint Rust executes; subsequent application
operations use the original admitted fresh-controller/raw-parts path. Ready
upgrades and ordinary mutation work after completion. Importing forbids export,
source cancellation and atomic recovery; losing or corrupting a prefix does not
silently initialize or republish it. The explicit restart mode below can discard
an unfinished prefix and reconstruct from its independent checkpoint.

## Compatible interruption

The separately selected importing-resume target requires cold phase, idle
access, matching native geometry, equal length/capacity and settlement 3 before
entering either Rust runtime. Its WAT bootstrap restores phase 5, then the
existing admitted transient reset discards/reinitializes checkpoint runtime
memory. Fresh governed handles load Importing and compare epoch, domains,
counter, cursor and the complete initialized prefix. Success changes neither
native payload nor stable bytes. The next batch or exact retry can then proceed.

Ready/frozen/replacement modes cannot stand in for this resume mode. Its explicit
arguments are `[epoch, source, fault]`; fault 1 traps after verification. Staged
start uses the same arguments, with fault 1 after native/stable initialization.
Batch faults 1, 2 and 3 trap after stable progress, the first copied scalar and
native completion respectively (fault 2 needs a nonempty batch). The final
composer checks the WAT-only callback closure and bootstrap → transient reset →
checkpoint-verifier ordering. A digest/geometry still does not authenticate an
arbitrary executable.

## Explicit restart after cutover

A separately composed restart executable accepts only an Importing job at the
exact epoch, source and compiled destination. The management upgrade explicitly
selects Replace and `skip_pre_upgrade = true`. Skipping the old hook is deliberate:
even a malformed native header may make that hook refuse. This path uses no old
native descriptors, references, destructors or prefix values. Management controller
authorization still applies; there is no application update that resets the heap.
This fixture has no in-flight external effects and blocks native writes throughout
Importing. Skipping hooks in other application phases is not qualified here.

The destination's fresh-memory check runs before checkpoint Rust. Fresh governed
handles locate and validate the complete job/checkpoint independently. Only then
does it create an empty unpublished native prefix and reset durable progress to
`(0, 0, 0)` in the same upgrade. The epoch, domains, counter and logical checkpoint
remain unchanged. This adds a transition, not a new wire version or locator.
A trap after native/stable initialization restores the prior module, damaged
prefix and all stable bytes. Corrupt checkpoints refuse; restart cannot repair
the independent recovery record itself.

Repeated explicit restarts while Importing are permitted and discard all partial
work again. Ordinary compatible resumption keeps that work. Restart invalidates
the previous latest-batch acknowledgement: the next batch must begin at zero.
An old offset-zero request can copy the same immutable checkpoint again after
reset; it is not acknowledged as a prior committed batch. There are no external
batch side effects or owning graph edges in this scalar fixture. A general job
with those effects would require a separately specified attempt identity.

Ready, Sealed, Cancelled and Consumed jobs refuse this mode. In particular,
successful publication and subsequent application writes cannot be undone by
replaying a restart. Restart remains within the already bound target domain;
redirecting recovery to another runtime/domain needs a separate protocol.
Keep mode refuses the nonempty retained header instead of clearing it.

## Bounds and qualification

At most 64 native values are copied per batch, but **total work is not O(batch)**.
Each request loads, hashes and decodes the complete bounded checkpoint and checks
the existing prefix. Resume does the same; compatible upgrade also clears the
whole admitted transient memory domain. The fixture remains capped at 512 values,
a 4320-byte cell payload and the existing 16 MiB linear-memory ceiling. Export
is still one atomic bounded operation. Larger checkpoints need chunked storage,
validation/work budgets and a different progress proof before scale claims.

`make native-job-test` includes codec/version/cursor refusals and installed
empty, 3-, 65- and 512-element imports in both directions. It checks retries,
mid-import compatible upgrades, controller refusal, blocked native operations,
wrong modes/bindings and batch/start/resume traps against native observations
and all stable bytes. Diagnostic `damage_import` is controller-protected and
deliberately changes native cursor, counter or first element without changing
the valid checkpoint; both resume and continued batches must refuse. These
diagnostics are qualification endpoints, not production recovery operations.
The restart suite additionally damages the header's length/capacity relation
(mode 3) and the independent job's integrity (mode 4). It tests empty, partial
and full-capacity checkpoints in both domains, explicit hook skipping, unchanged
state after refused/failed restarts, reconstruction and subsequent native reuse.

## Recorded evidence

On 15 November 2026, the focused gates passed on x86_64 Linux with Rust 1.100.0
and the prepared PocketIC server. Eight host job tests, twelve existing artifact
checks and five job checks (one artifact test and four installed suites) passed.
The job phase took 35.81 seconds excluding preparation. The staged suite performs
fourteen fresh-target starts, eight completed imports, twenty-four successful
importing-compatible resumes and eight later ready-compatible upgrades. Six
additional prefixes deliberately disagree with valid checkpoints and refuse
resumption, new batches and exact retries. The corruption diagnostic itself
rejects an unauthorized caller.

The cohort `target/evidence/provider-link/attempt.fsnTQv/` retains compiler,
codec, bridge, composer and contract inputs, candidate Wasm, `job.txt` and its
completion marker. Principal artifacts are:

| Target | Bytes | SHA-256 |
| --- | ---: | --- |
| Staged start A | 135946 | `329dff60365af717560f6942391eed2376f61d7991b7ed314be342684767c2e7` |
| Importing resume A | 148688 | `2b9b3edef91417f824c7ed7b956cafa77c3116d874936de00bed775d86ef077c` |
| Staged start B | 135946 | `985a75203127d48ba19e6ac80b54a9170f48ba7dd847d22eece32963b710a879` |
| Importing resume B | 148688 | `7aa59dd71e7b7642b644169b8febeadd7098aa8a4f401e8b12449ec6d1aa3371` |

The earlier native checkpoint/reopening gate also passed its twelve artifact
checks and four migration checks (3.92 seconds), with separate cohort
`target/evidence/provider-link/attempt.LlHLdG/`. Strict host/wasm Clippy,
formatting and shell checks passed. A preceding corruption-diagnostic attempt
stopped at WAT syntax validation before installed execution; that failed cohort
is preserved and is not migration evidence. Existing historical cohorts retain
their original meaning. This is local Linux qualification, not hosted CI,
macOS execution, production authority or large-data performance evidence.

### Restart evidence (0.1.11)

On 16 November 2026, `make native-job-test` passed on x86_64 Linux with Rust
1.100.0 and the prepared PocketIC server: nine host job tests, fourteen artifact
checks and six job checks. The job phase took 37.29 seconds excluding preparation.
The new suite completed fourteen explicit restarts and reconstructions in both
allocation domains, including four inadmissible-header recoveries. Two corrupt
checkpoint cases refused. Wrong mode/domain/epoch, unauthorized management
access, post-publication replay and injected initialization failure preserved
the observed native state, module hash and all stable bytes.

Cohort `target/evidence/provider-link/attempt.RuI0DK/` retains the exact compiler,
bridge, codec, composer, tests and candidate Wasm, plus `job.txt` and `COMPLETE`.
Restart A is 137087 bytes with SHA-256
`cd6f6503060514383949994ab2a3053466f0ce3bb36091eea8a308828c14a41e`;
restart B is 137087 bytes with SHA-256
`e48123fea3d09a1c06391700a33d045c326b32993e747aabc972c172c5dc1d77`.
This evidence retains the earlier bounds and does not establish production
recovery authority, arbitrary graph migration or hosted/macOS execution.
