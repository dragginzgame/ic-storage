# Vector provider linkage

The 0.1.8 artifact experiment composes the admitted compiler diagnostic with the
existing [vector metadata provider](vector-provider.md) and
[checked-access helpers](vector-access.md). The resulting Wasm has no unresolved
native imports. It now has an installed lifecycle under the separate
[vector allocation contract](../ic-storage-native-feasibility/vector-provider-contract.md).
This remains a closed qualification recipe, not a general linker, source
authenticator or native-persistence admission.

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
stack is now bounded to 80 live bytes with 76 guarded Rust access sites for the
shared scoped-core build; the original evidence below used 64 bytes and 49 sites. The
retention marker is preserved; pre-link function-name metadata is discarded
because its indices no longer describe the final artifact.

The trusted WAT glue selects one of the six reviewed operations. It checks the
operation selector is idle, admits the full-width metadata and only then narrows
length/capacity for Rust. Rust claims the same provider synchronously. No callback
can change the descriptor between admission and claim. The returned u64 contains
only length/capacity; the wrapper extracts these bits and settles after Rust has
returned, then resets the operation selector before replying. Failures propagate
as traps; there is no local catch or destructor-dependent custody recovery.

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

- All seven guard bindings and all 76 current access sites, with no direct memory
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
