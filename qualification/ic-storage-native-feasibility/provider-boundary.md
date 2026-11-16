# Foreign allocation-provider investigation

Status: compiler, artifact and installed scalar evidence, 15 November 2026.
Scope: the scalar provider contract; later vector evidence is recorded in the
[vector provider contract](vector-provider-contract.md).
This investigates a concrete candidate from the
[allocation-domain note](../../docs/design/native-allocation-domain.md): an
independently implemented Wasm provider supplies one bounded foreign allocation
to Rust after source/target admission. The private scalar fixture now has an
explicit [foreign-allocation contract](scalar-provider-contract.md) and installed
lifecycle evidence. It does not qualify a native allocator or retained vectors.

## Candidate and alternatives

| Mechanism | Decision for the next investigation |
| --- | --- |
| Cast a retained numeric address to a Rust pointer | Insufficient: no allocation authority established |
| Reuse an old Global allocation or allocator reference | Rejected: the source allocator/controller does not survive instantiation |
| Retain an ordinary initialized Rust static by suppressing its data segment | Insufficient: changing initialization requires a separate compiler/runtime argument |
| Foreign provider implemented outside LLVM, returning a pointer into its exclusively owned region | Investigate with an explicit compiler and runtime contract; do not infer validity from the declaration |

Rust's pointer contract requires allocation provenance and valid access, not
just an in-bounds address. FFI declarations describe external behavior without
implementing it. [Rust pointer contract](https://doc.rust-lang.org/std/ptr/index.html#allocation),
[foreign declarations](https://doc.rust-lang.org/reference/items/external-blocks.html).

LLVM 23.1 permits a pointer returned by a function implemented outside LLVM to
be associated with externally allocated ranges, provided those ranges do not
overlap LLVM-created allocations. This is a relevant compiler mechanism, not a
complete Rust or IC persistence guarantee. The proposed provider must actually
own that external allocation and its lifetime. [LLVM 23.1 pointer aliasing rules](https://releases.llvm.org/23.1.0/docs/LangRef.html#pointer-aliasing-rules).

Our inference is that this boundary merits qualification before selecting a
compiler extension or abandoning native access. Merely moving an integer cast
into another file, declaring a symbol or emitting an opaque call does not
establish an external allocation. The provider implementation, linker and
startup rules must jointly satisfy the contract.

## Compile-only observation

[provider_abi.rs](tests/fixtures/provider_abi.rs) deliberately supplies **no**
provider. It declares one diagnostic import, then conditionally reads or writes
the first u64 returned by it. The unsafe functions assume a live, exclusive,
aligned 4096-byte allocation with an initialized first word. They must not be
linked to a dummy address-returning stub and executed as evidence of validity.
No vector, retained header, allocator ledger or application API is added.

Compiled on x86_64 Linux using Rust 1.100.0 (`0ef1bc386`, LLVM 23.1.5), targeting
wasm32-unknown-unknown at optimization levels 0 and 3, with abort-on-panic,
one codegen unit and fat-LTO requested. Both compilations passed with warnings
denied. The optimized LLVM IR shows:

- An opaque pointer-returning declaration with the diagnostic Wasm import name.
- Null refusal before either typed access; an aligned i64 load/store based
  directly on the returned pointer, without a Rust integer-to-pointer conversion.
- No `noalias`, `dereferenceable`, allocation-size or memory-purity assertion
  on the provider declaration. Those properties were not inferred or certified.

This IR-only step observes compilation under stated preconditions. It does not prove the
preconditions, final linked code, full link-time optimization behavior, provider
semantics or installed upgrades. The generic foreign import is not an IC system
API; a future final artifact must supply its reviewed implementation and reject
an unresolved import. The stock IC cannot be assumed to implement it.

Reproduce without fetching dependencies or installing tools:

```sh
mkdir -p target/evidence/provider-abi/opt0 target/evidence/provider-abi/opt3
rustc --version --verbose > target/evidence/provider-abi/compiler.txt
for level in 0 3; do
  rustc --edition 2024 --crate-type lib --target wasm32-unknown-unknown \
    -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level="$level" \
    -D warnings --emit=llvm-ir,obj \
    --out-dir "target/evidence/provider-abi/opt$level" \
    qualification/ic-storage-native-feasibility/tests/fixtures/provider_abi.rs
done
```

Inspect the emitted `provider_abi.ll` files; they are diagnostics, not a format
to accept as admission evidence. This standalone source is intentionally outside
Cargo's runnable test discovery. Raw output stays under target. The subsequent
linked-artifact experiment below has its own check. Execution additionally
requires the selected scalar contract and lifecycle adapter.

## Installed custody experiment

A separate [WAT custody fixture](../ic-storage-retention-probe/provider-custody.md)
now implements a fixed-region claim/settlement protocol and tests it on the local
IC server. It exercises compatible upgrades, abandoned claims, destination-side
refusal without a source pre-upgrade guard, and update/upgrade rollback. It has
no Rust code or compiler allocations and does not resolve the diagnostic import.
Its integer-valued claim result is not a pointer-validity proof. The combined
experiment below adds the scalar allocation argument and Rust lifecycle adapter.

## Linked artifact admission

`make native-provider-probe` now compiles the diagnostic as a wasm32 cdylib with
Rust 1.100.0, optimization level 3, fat LTO, abort-on-panic and one codegen unit.
The linker reserves a 64 KiB stack, exports stack/data/heap boundaries and fixes
memory to three pages. The fixture's panic handler traps; it is absent from the
admitted two-function output. No tools or dependencies are installed by this check.

The private host adapter admits exactly the observed scalar profile: one
pointer-returning import, two typed exports, three transient boundary globals
at 65536, and the reviewed null-check/load/store instruction sequences. Linker
padding is normalized through wasm-encoder before instruction comparison.
Data, startup, tables, extra code and growth are refused. This is intentionally
not a general compiler-output validator; a changed profile requires review.

The adapter combines those bodies with the existing WAT custody implementation,
sharing the admitted memory and remapping types, functions and globals. A temporary
named export locates each private claim/settlement function; both are removed
afterward. The raw Rust functions are also private in the final artifact.
Generated IC entrypoints settle custody only after Rust returns a scalar or unit;
post-upgrade admission runs before its first native read. The result has only the
two IC reply imports. Final admission checks Wasm validity
and exact agreement with the deterministic transformation of the admitted input
and trusted provider source. It is not an artifact signer or hostile-code sandbox.

Three diagnostic tests passed on x86_64 Linux. An independent check follows the
existing WAT client's claim call and verifies that both relocated Rust bodies
call that same function. It also checks private exports and the entrypoint
claim/settlement call ordering. Negative cases reject overlapping compiler bounds,
changed memory limits, active payload initialization, an unexpected import,
added growth, unresolved linkage and a changed provider. The raw module is
633 bytes and the combined A module 1250 bytes for this cohort; these are fixture
sizes, not a native-storage footprint measurement. Fresh source copies, compiler
identity, LLVM IR, Wasm and digest reports stay in per-attempt directories under
`target/evidence/provider-link/`. The new command is included in `make ci`.

## Installed scalar lifecycle

`make native-provider-test` builds a fresh artifact cohort, runs the admission
diagnostics and executes the admitted scalar module using the previously prepared local
Testkit server. Both commands run locked/offline; only the installed command needs
loopback access. Hosted CI selects the installed check after server preparation.

On 15 November 2026, all three artifact tests and the installed scalar test passed
on x86_64 Linux with Rust 1.100.0 and the admitted PocketIC 16.1.0 server. The
installed case covers:

- Native initialization reads and writes, independently cross-checked by WAT.
- A→B→A retaining upgrades with continued native writes; the target validates its
  header before reading through its own newly claimed pointer.
- Native write-then-trap rollback during updates and post-upgrade, preserving the
  observed source state and installed module hash on rejected upgrades.
- Incompatible extent, corrupted header and abandoned custody refusal. An
  abandoned claim blocks subsequent native reads, writes and upgrades.

The successful cohort is `target/evidence/provider-link/attempt.6gKlgH/`, with
compiler identity, copied sources, LLVM IR, raw and combined Wasm, artifact report
and `installed.txt`. Combined A has SHA-256
`6cd57712ab34e821cc99f10e4b0030b6f3b56314de60e49451b1a411bd50e54c`.
Each negative/B artifact is also retained by digest. Assertions compare selected
header/payload observations, not every memory byte. This is local installed-canister
evidence, not a production subnet or hosted macOS result.

The [scalar contract](scalar-provider-contract.md) records the selected external
allocation model, pointer creation, initialization, exclusivity and lifetime
argument. Installed success tests that bounded lifecycle; it does not establish
language soundness by itself. The fixture has no allocator, Vec header, retained
Rust pointer, arbitrary destructor or general application API.

## Remaining vector gate

The [provider-region experiment](provider-region.md) now exercises a single-block
controller over loaned backing on the host. The same implementation compiles into
a separate vector Wasm diagnostic under `make native-provider-probe`; an additional
test inventories it and verifies that scalar admission refuses it. Stack/control
regressions also check the transient frame and acyclic direct calls. These partial
checks do not execute the vector module or extend scalar admission.

The scalar integration decision is recorded; extending it to allocator-issued
blocks and a native vector must satisfy these requirements together:

1. Own one fixed, nonzero, aligned region disjoint from Rust data, stack and
   transient allocation. Keep the first experiment bounded; do not infer that
   adjacent memory.grow results are one Rust allocation or allow Global growth
   to consume the provider's reserved region.
2. Define how fresh installation creates the external allocation and how a new
   executable receives exclusive custody after retaining upgrade. Retaining
   bytes alone is insufficient. No source references, executable pointers or
   allocator controllers cross the boundary.
3. Run source/target, extent and complete-root admission before claim or payload
   interpretation. Failure must preserve rejection, never return an empty root.
   Validate payload initialization separately from a checksum or numeric bounds.
4. Inspect the optimized Rust IR and final Wasm: provider implementation outside
   LLVM, resolved calls, exact region ownership, transient initialization and
   complete growth authority. Reusing the M0 startup offsets is not admission.
5. Issue the retained span through the new allocator only after backing custody
   is valid, then construct the transient Vec header. Keep a unique claim and
   quarantine failures independently of guard destruction.

Before extending typed provider access, extend the actual implementation's
Rust/compiler argument and audit its complete artifact. If the allocation claim
cannot be justified, stop this candidate and choose explicit compiler/runtime
integration. Installed round trips then test lifecycle and rollback; they cannot
replace that argument. The independent portable checkpoint remains the recovery
path throughout this investigation.
