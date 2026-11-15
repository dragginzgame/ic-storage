# Connected native checkpoint recovery

This bounded experiment connects the real retained `Vec<u64>` fixture to the
existing governed Storage checkpoint adapter and logical version-2 codec. Export
and import execute inside the canister. The host does not copy observed values
into a target, transfer stable bytes or substitute a parallel checkpoint reader.

## Source and target

The [bridge contract](../ic-storage-native-feasibility/native-checkpoint-contract.md)
owns allocation, stack, initialization, scalar-callback and failure obligations.
Native metadata and payload are below 196608; checkpoint static data and Global
heap are above that boundary. All live native owners end before checkpoint Rust
runs, and scalar callbacks cannot call either Rust runtime or host services.
No Rust pointer, Vec header or allocator owner crosses between runtimes.

Source A has its 4096-byte native allocation at 131072. Target B uses 147456.
Both reconstruct transient std headers under the existing Region contract. The
bridge-specific metadata format is 2 and includes one u64 counter at 65600.
The counter is initialized to 41; the vector supports the existing six operations.

Checkpoint export validates and freezes the source, reads its logical values
through checked scalar callbacks and publishes the complete envelope through
fresh governed Storage handles. Freeze and stable writes share the IC transaction.
Normal mutation refuses while frozen. Failed export restores both domains.

The bridge runtime's private Global allocator is not admitted across EOP. Source
post-upgrade refuses; a recovery target checks fresh zero native metadata in WAT
before entering checkpoint Rust. Keeping source memory therefore refuses. The
recovery request explicitly selects heap replacement through the typed management
API, preserving stable memory while discarding the source Wasm heap.

The target opens the same provider locator independently, validates the expected
epoch and complete envelope, and builds a new initialized native prefix through
private sequential-write callbacks. Only after checkpoint Rust has returned does
the wrapper enter native Rust and settle the reconstructed Vec. Capacity becomes
length; spare bytes and old addresses are not imported. Unknown/corrupt native
state does not trigger automatic checkpoint recovery.

## Qualification

`make native-checkpoint-test` builds the separately selected bridge profile,
runs strict Clippy and the existing native artifact checks, combines the modules
and runs the installed migration test. The existing CI workflow selects this
target after preparing the server. All dependencies and tools remain pinned;
checks are locked/offline and do not install anything implicitly.

Artifact admission checks fixed memory bounds, stack/data/heap separation,
active initialization ranges, callback roster and its non-reentrant closure.
Final exports expose only the intended test entrypoints; raw bridge functions
and native callbacks remain private. Exact final comparison rejects changed
lifecycle calls or a different allocation domain. Negative cases remain valid
Wasm, including swapped same-signature lifecycle calls and data initialization
over the retained region. These checks are not authentication of arbitrary inputs.

The installed test covers empty, ordinary and full 512-element roots. It proves
retained-heap refusal, explicit replacement recovery A→B→A, logical checkpoint
bytes/digest, destination base and root publication, continued native allocation,
shrink/removal/reuse, frozen mutation refusal and retries after failure. Failure
cases include missing checkpoints, wrong epochs, truncation, corruption, unknown
schema, export traps after stable writes, import traps after stable writes and
traps during native target construction. Refusals compare the source module hash,
complete provider header/payload and stable bytes, with actual trap codes checked.

The initial harness attempt used an omitted persistence option and failed at the
platform contract. That attempt is not migration evidence. The corrected harness
explicitly selects replacement and distinguishes platform rejection from the
expected traps. Historical attempt directories remain intact.

## Observed evidence

On 15 November 2026, strict Clippy, the twelve existing compiler/artifact checks,
the bridge artifact test and the installed native migration test passed on
x86_64 Linux with Rust 1.100.0 and the prepared PocketIC server. The migration
phase took 3.80 seconds, excluding preparation, and included six successful
replacement upgrades and twenty refused upgrades. The two affected ordinary
checkpoint suites also passed after the shared-provider refactor (2.30 seconds).
Formatting, shell and workflow validation were checked separately.

The final migration cohort is `target/evidence/provider-link/attempt.5rhMNr/`,
with raw compiler inputs, bridge source, native admission source, contract,
constructed Wasm variants and `migration.txt`. The unchanged codec's source and
golden evidence remain in the version-2 cohort `target/evidence/checkpoint-v2/attempt.MUq1l6/`;
future migration cohorts also copy that codec directly. The principal identities are:

- Source A, 121240 bytes: `27696a048820f3de5f0a5409d9a30b55a9fc24b4dfdddc19ef0881b5f972aab8`.
- Recovery B, 121247 bytes: `85c7a3d3a8519ba816969a36b5b4826417abd53b621e0c4fcccfdeebfacb406c`.
- Recovery A, 121247 bytes: `2d90d981358635920e6d7bcd0fc8405ac3dade853c2348af5a9021345fe8e23e`.

## Scope

This establishes a bounded incompatible-allocation-domain migration. It is not an
actual compiler/SDK transition or permission to retain private std layouts. The
existing ordinary checkpoint and compatible native-vector fixtures retain their
separate meanings. Source/target authentication, durable epoch authority,
cancellation, resumable migration and resource-exhaustion recovery remain open.
No application converters, broader collection library or consumer cutover are
introduced by this test. Further work should make those admission and migration
controls concrete before expanding the supported state surface.
