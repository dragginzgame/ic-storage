# Vector provider linkage

The 0.1.8 artifact experiment composes the admitted compiler diagnostic with the
existing [vector metadata provider](vector-provider.md) and
[checked-access helpers](vector-access.md). The resulting Wasm has no unresolved
native imports. It now has an installed lifecycle under the separate
[vector allocation contract](../ic-storage-native-feasibility/vector-provider-contract.md).
This remains a closed qualification recipe, not a general linker, source
authenticator or native-persistence admission.

## Finalization failure evidence (0.1.12)

The trusted WAT wrapper now has a closed failpoint after native Rust releases
the Vec and returns its descriptor, before publication, custody settlement or
operation-selector reset. Normal endpoints pass a fixed zero; only the diagnostic
`vector_error_finalize_trap` passes one. The artifact test checks both bindings
and the complete instruction order. Callers still cannot supply a descriptor.
The change adds no native Rust code, retained representation or persistent
injection global.

The installed scope suite adds twenty finalization traps across the same five
roots and A/B/A versions: five before the first successful update, then fifteen
following ordinary error replies. Empty, spare, growing and full roots compare
the complete provider header and payload before/after each trap. Subsequent
forgotten-view operations and compatible upgrades verify usable custody and idle
selector restoration. This complements the fifteen existing post-settlement
traps. A business error cannot suppress fatal finalization failure. Frozen and
staged-import fixtures also reject the diagnostic endpoint without state changes.

On 16 November 2026, with Rust 1.100.0 on x86_64 Linux,
`make native-provider-test` passed fourteen artifact checks and all three
installed suites (2.04 seconds) in
`target/evidence/provider-link/attempt.Uinp3r/`. The raw Rust vector and guarded
intermediate are byte-identical to the 0.1.11 artifacts: eight bodies, eleven
internal edges, 80-byte peak stack and 85 guarded memory sites. New linked artifacts:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Linked A | 4177 | `b8391a437ba0e1efe23cbd8e489135e7a1a8a817d02e957619c91e7db13c0f41` |
| Linked B | 4177 | `002076db46249ea3fcc9d72fea4aa2b341dbe0ab0816b2a790ec5df231d10850` |

Connected checkpoint checks passed in `attempt.IyWGew` (four checks, 3.96 seconds).
Job qualification passed nine host tests, fourteen artifact checks and six job
checks (39.19 seconds) in `attempt.PtEZ7K`. All cohorts use the evidence parent
above. The separate [native-host proof](../ic-storage-native-feasibility/provider-region.md#host-poisoning-and-explicit-replacement)
keeps access poisoned on finalization failure and requires explicit replacement;
host unwinding is not IC rollback. These remain fixed u64 fixtures, not general
application callbacks, arbitrary Drop, complete native admission or production
checkpoint custody.

## Installed scope outcomes (0.1.11)

On 16 November 2026, the closed native fixture added two scoped operations:
mutate/try-append then return a normal callback Err, and mutate then forget the
non-owning view. Rust verifies the expected outcome and releases the actual Vec;
the WAT wrapper settles custody before replying. The error reply is diagnostic
code 1, not an IC trap. A separate endpoint traps after the same settled operation
to demonstrate whole-message rollback. No new retained representation is added.

The installed scope suite covers five roots: empty, empty with spare capacity,
growth from exact capacity, existing spare capacity and full 512-element capacity.
It checks fifteen normal-error replies, twenty forgotten views, fifteen rollback
cases and twenty compatible upgrades. It compares the complete payload and root,
then clears, reallocates and reopens the vector. At full capacity, refused append
preserves the preceding scalar write. Frozen/importing job tests refuse both new
operations before exposing ordinary native access.

The raw profile still has eight bodies, the same signatures and an 80-byte peak
stack; the acyclic graph now has eleven direct internal edges. All 85 memory
sites are guarded. Exact access inventory and added WAT boundary cases cover the
new read/write windows, including spare reads, writes beyond the single append,
full capacity and forbidden copies. The first artifact-only attempt stopped at
the old access-count assertion; `attempt.gdgGaf` is preserved as failed evidence.

On x86_64 Linux with Rust 1.100.0, `make native-provider-test` passed fourteen
artifact checks and three installed suites (1.99 seconds) in
`target/evidence/provider-link/attempt.bmbqrN/`. Principal artifacts are:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Raw vector | 4891 | `6f217d37a40b20f947e1f86f7b75165ce801a529e61d49e059f2e46d24d47bd3` |
| Guarded intermediate | 2094 | `74fbc13cac80ab51474dfd6a0af31cf170fa183d7d94429fe55e03fd304af638` |
| Linked A | 4096 | `eaaf59a0bc887d7ec0e2b754213da85765c2e0acb5ff84c887287b555362ebca` |
| Linked B | 4096 | `e9744d2d9015ce687177d87e1d65c9cef87355db5347ad1ec781e2600b1eaa5d` |

All five installed retention/helper tests passed (1.47 seconds). Connected
checkpoint checks passed in `attempt.x8CTtq` (four checks, 4.13 seconds). The final
job cohort `attempt.CbyT3P` passed nine host job tests, fourteen artifact checks
and six job checks (40.04 seconds), including the new frozen/importing refusals.
Host/wasm lint and formatting passed. These results qualify fixed synchronous
u64 operations, not arbitrary application callbacks, native-host poisoning,
owning elements, general layout admission or hosted/macOS execution.

## Shared scoped core evidence (0.1.10)

On 16 November 2026, the native fixture began using the unpublished
[`ic-storage-native`](../../crates/ic-storage-native/README.md) library for
scoped u64 operations. Its allocator claim, reconstruction and release remain
in the separately reviewed provider boundary. No private header or new native
representation is retained. Host checks passed three behavior tests, one usage
doctest, eight ownership compile-fail cases and nine provider-region tests,
including an allocator refusal that preserves earlier writes and custody.

The generated Wasm contains eight Rust bodies with an acyclic nine-edge internal
call graph. Entry reserves 48 bytes; nested helpers reserve 16 each, for an
80-byte peak. Admission checks balanced helper frames and the maximum path sum,
including rejection of nested exhaustion, early helper exits and effects after
restoration. All 76 memory sites are rewritten and checked. No frame check was
replaced by a source hash or an observed successful upgrade.

`make native-provider-test` passed fourteen artifact checks and both installed
scalar/vector suites (1.80 seconds) in
`target/evidence/provider-link/attempt.DNozEc/`. Raw vector Wasm is 4739 bytes,
SHA-256 `1445119c0d5fb7ebf1efd8bd377af4b07d13881bc14aa6a3f3fe203cf81f643f`;
the linked ordinary A vector is 3759 bytes,
SHA-256 `46d082e262c1a2b0550fa818db325097b375fecf64f0e6f99730873c91ceef38`.
The cohort includes the library source/manifest and root workspace/lock inputs
alongside the existing raw/compiler/provider evidence.

The connected checkpoint suite passed four checks (4.00 seconds) in
`attempt.4NFnuO`; the governed job suite passed five checks (38.39 seconds) in
`attempt.tG9wAg`, both under the same evidence parent. These cover compatible
upgrades, relocated recovery, cancellation, frozen resumption and staged imports
through the shared core. Installed WAT helper/retention checks also passed all
five tests, including both edges of the revised stack window. Host/wasm Clippy,
strict Rustdoc, dependency policy, formatting and shell checks passed locally.
The initial core build was refused by the earlier single-frame profile before
installed execution; its failed cohort is preserved. These are Linux results
with Rust 1.100.0, not hosted CI, general application callbacks or record conversion.

## Composition and custody

The recipe accepts raw compiler output only through the existing vector profile
and deterministic access rewrite. It does not accept arbitrary rewritten modules.
It compiles the provider and access helpers from the same sources used by their
installed WAT tests. Temporary named exports identify the private claim and seven
guards; all are removed after binding. A temporary provider import identifies the
Rust entrypoint and is also removed. Provider function indices shift around that
removed import, while Rust types, globals and direct calls are remapped into the
same module. Only the two existing IC reply imports remain.

The fixed three-page memory is shared. Provider globals precede the Rust globals,
and the raw Rust functions and compiler globals remain private. The transient
stack is now bounded to 80 live bytes with 85 guarded Rust access sites for the
shared scoped-core build; the original evidence below used 64 bytes and 49 sites. The
retention marker is preserved; pre-link function-name metadata is discarded
because its indices no longer describe the final artifact.

The trusted WAT glue selects one of the eight reviewed operations. It checks the
operation selector is idle, admits the full-width metadata and only then narrows
length/capacity for Rust. Rust claims the same provider synchronously. No callback
can change the descriptor between admission and claim. The returned u64 contains
only length/capacity; the wrapper extracts these bits and settles after Rust has
returned, then resets the operation selector before replying. Normal callback
errors preserve writes and still settle; unexpected failures propagate as traps.
There is no local catch or destructor-dependent custody recovery.

Pre-upgrade checks idle operation state before the existing provider checks.
Post-upgrade first runs provider admission, then a zero-add native operation and
settlement. Existing WAT-only diagnostic endpoints remain available for independent
cross-checks; this fixture is not a production canister interface. Fault profiles
can omit the source guard, corrupt individual metadata words, change the expected
extent or trap after native post-upgrade mutation. Every profile is part of final
byte comparison and is recorded with the artifact digest.

## Artifact checks and evidence

`make native-provider-probe` builds fresh compiler inputs and runs the new link
test alongside the existing diagnostics. The test independently follows the WAT
provider's claim/settle calls and checks:

- All seven guard bindings and all 85 current access sites, with no direct memory
  instructions remaining in the Rust bodies.
- Private Rust/claim/settle/access functions, correct global remapping, fixed
  memory and the sole retention marker; no data initializer, table or start.
- The exact admission, narrowing, Rust return, settlement and idle-reset sequence,
  each operation selector, and the post-upgrade call order.
- The entire claim/guard call closure has no Rust re-entry or host callback.
- A same-signature guard swap remains valid Wasm but fails final comparison;
  an artifact prepared for version B also fails admission as version A.

Final comparison reconstructs the expected artifact from the supplied raw input,
reviewed provider sources and selected version. It detects changed bindings or
provider bytes relative to those inputs. It does not authenticate those inputs or
prove a malicious compiler output safe merely because its shape matches.

In the initial artifact-only cohort on 15 November 2026, strict Clippy and all
twelve compiler/artifact checks passed
on x86_64 Linux with Rust 1.100.0. The final cohort is
`target/evidence/provider-link/attempt.8kPtYn/`. It includes compiler identity,
raw/rewritten/linked Wasm and source copies for both allocator and provider.
The linked artifact is 3146 bytes with SHA-256
`2f61814b111fffe3bb11b3e7e0f66bea113693af6feb4b87b3051456390b36fc`.
The scalar artifact hashes and prior WAT evidence remain unchanged. This cohort
does not run the installed scalar test or any native-vector execution.

## Installed vector lifecycle

`make native-provider-test` now runs both the scalar and vector installed tests,
after the compiler/artifact checks. The vector test executes the same reviewed
Region controller and ordinary std Vec operations, with its raw function private.
No private Vec layout is retained. The written allocation contract precedes this
execution; passing these tests does not retroactively establish pointer validity.

The native test covers A→B→A retention, continued mutation and growth, overlapping
removal, shrink, destruction/reuse, clear and fresh allocation. It upgrades empty,
reserved-empty, one-element and full 512-element roots. The independent WAT client
appends after Rust using the same descriptor and backing. Clear changes the root
without wiping payload. Reuse reissues allocator capacity, not platform pages.

An explicit trap after native settlement restores the complete header and payload.
At full capacity, a two-operation update first mutates and settles, then fails
append inside Rust; rollback restores both calls' effects. The shared core now
rejects this append at its logical limit before asking the allocator to grow.
A post-upgrade target
performs native work before trapping, preserving the source module hash, header
and payload. Incompatible extent, 13 malformed-header variants and abandoned
custody refuse. Corrupt sources omit pre-upgrade guards so destination admission
must reject independently before native interpretation. No test compares all
transient scratch bytes or establishes a production-subnet guarantee.

On 15 November 2026, strict Clippy, all twelve artifact checks and both installed
tests passed on x86_64 Linux with Rust 1.100.0 and the prepared PocketIC server.
The installed test phase took 2.00 seconds, excluding preparation. Evidence is
under `target/evidence/provider-link/attempt.2ohnxt/`, including the allocation
contract, source copies, compiler identity, installed log and all variant Wasm.
The A artifact is 3247 bytes with SHA-256
`4e84cfb9d666c16b3cc91656e470746f599a0d16925dad72d53f21c160e85b4e`;
B is 3247 bytes with SHA-256
`d0abfbcf7035301b869bf277d5532c2e12bf6116719368321e6b0725e56ead05`.
Earlier scalar and artifact-only cohorts remain historical evidence.

## Remaining recovery gate

The separate [native checkpoint bridge](native-checkpoint.md) now connects this
retained-state mechanism to the governed provider and reconstructs under a
relocated allocation domain. Its expanded memory/custody contract and installed
evidence are independent of this original compatible-lifecycle cohort. Actual
compiler/SDK transitions, migration authority, resumable recovery, application
converters and consumer cutovers remain outside these bounded results.
