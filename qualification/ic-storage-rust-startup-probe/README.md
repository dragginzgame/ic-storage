# Controlled Rust startup probe

This page describes the default scalar startup/growth profile. The separate
`managed-graph` feature has its own [custody and evidence contract](managed-graph.md),
including a retained-memory kernel, denied global allocation and a bounded
transient function table. The scalar profile's restrictions below are unchanged.

This fixture tests the next boundary after the WAT retention probe: preparing a
new Rust module's transient memory without overwriting retained bytes. It is not
a persistent allocator, application storage API or general-purpose Wasm linker.
The source is independently authored; the design records the prior Motoko
implementation consultation and makes no clean-room claim.

## Selected contract

The canister uses Platform Runtime 0.23.12 with only `system-raw`, defaults disabled.
The fixture links std. This experiment therefore selects a std-linked,
allocation-free path rather than claiming a no_std dependency graph. Its source
uses scalar atomics, fixed stack buffers and replies, with no allocator, executor,
callbacks, timers, Rust references into retained storage or stable-memory access.
The scalar normal dependency graph contains only the fixture and Platform Runtime.
The raw module must import exactly the two reply functions. Platform's raw API
is Wasm-only; installed checks own this canister's execution evidence.

The only Rust unsafe constructs are narrowly expected export-name attributes:
the fixture owns each IC endpoint symbol, and admission checks the exact export
roster and signatures. No unsafe Rust block or persistent raw-pointer access is
used. The qualification workspace uses `unsafe_code = "deny"` to permit those
scoped expectations; both host test entrypoints retain `forbid(unsafe_code)`.
The product crate's prohibition remains unchanged.

The consuming workspace selects release optimization (Cargo's default level 3),
abort on panic, LTO and one codegen unit. `make rust-startup-build` records the
explicit linker command: stack first, 1 MiB stack, 129 initial memory pages,
256 maximum pages, and exported stack/data/heap boundaries. The target is
`wasm32-unknown-unknown` under Rust 1.100.0. There is no post-link optimizer or
production optimization claim. Environment flag/linker overrides are refused;
final layout checks also apply to effects from Cargo configuration.

The transient region is `[0, 8388608)`. The next page is reserved for this probe's
retained anchor: a little-endian magic word at 8388608, scalar sentinel at
8388616 and committed page count at 8388624. These offsets and the small record are an experiment contract, not the
managed heap ABI. The initial memory extent includes the complete record.

## Artifact admission and transformation

The host runner uses bounded Host Artifacts reads/validation and wasmparser /
wasm-encoder for the experiment-specific transformation. The latter two are
direct dependencies because the existing Host Artifacts structural view does not
expose the memory, globals, initialization expressions and instruction rewriting
needed by this policy. It does not duplicate a general build or optimizer tool.

Before transformation, it requires one bounded nonshared wasm32 memory, the exact
reply imports and endpoint exports, an initial 1 MiB stack pointer, and data/heap
boundaries below the retained region. Active data must fit between stack top and
data end. Existing start functions, passive source data, unsupported sections,
populated tables, indirect calls and memory-growth instructions are rejected.

The transformer preserves the admitted Rust function bodies and appends:

- A Wasm start function that clears the complete transient reserve, then restores
  admitted data through passive `memory.init` instructions. It executes before
  any Rust frame exists; the new instance supplies the fresh stack global.
- Protocol fixture helpers to write/read the retained scalar using fixed numeric
  Wasm addresses. They reuse the existing Platform-supplied reply imports.
- Controlled helpers for one-page growth, growth to the declared limit, sampled
  zero-initialization checks, and deliberate growth-and-trap failures. They write
  sentinels at the first/last words of each newly grown range. Only these generated
  helpers may grow memory; growth instructions in Rust compiler output remain
  refused. This is an extent probe, not an allocator.
- A post-upgrade wrapper that rejects a missing/wrong anchor before calling the
  Rust post-upgrade export. It also requires the retained page count to equal the
  physical memory extent. The fixture's fresh initialization does not guess
  whether an upgrade occurred; the test explicitly writes its retained sentinel
  through the fixture helper before successful upgrade attempts.

Final bytes are fully validated and admitted only when they exactly match the
deterministic transformation of the reviewed raw input, including the start body,
passive segments, lifecycle wrapper and EOP marker. Raw/final hashes and linker
boundaries are recorded together. The source and transformer remain trusted:
these checks do not prove arbitrary Rust/Wasm code cannot store into the retained
region. They are not an untrusted-module sandbox or a production artifact signer.

## Run

Prepare the pinned compiler/target and dependencies explicitly as described in
the root README, then prepare the Testkit server with `make retention-setup`.
Checks do not install tools, fetch dependencies or repair missing raw artifacts.

```sh
make rust-startup-lint
make rust-startup-build
make rust-startup-test
```

The first command lints both canister variants and the host admission/transform
code. The build creates raw A/B modules in `target/qualification/rust-a` and
`rust-b`. The test reads those exact files, constructs and admits final modules,
and runs native negative cases plus the installed PocketIC scenario. Final
artifacts and reports go under `target/evidence/rust-startup/`.

## Observed result, 15 November 2026

All three tests passed on x86_64 Linux with Rust 1.100.0 (`0ef1bc386`, LLVM 23.1.5)
and the previously admitted PocketIC 16.1.0 executable. The qualification lock
SHA-256 is `57aade60b1d264b23a979e3ccefc373ac30cb0fb13c701f0fb226ce8a8be285f`.
Native macOS and hosted CI results have not been observed for this change.

| Observed property | Result |
| --- | --- |
| Rust A data end / heap base | 1048600 / 1048608 |
| Rust B data end / heap base | 1049120 / 1049120 |
| A / B active segments before conversion | 1 / 2 |
| A→B and B→A retaining upgrades | Sentinel unchanged; installed hash matches destination |
| Initialized atomic after A→B | Dirty 999 replaced by B's 202 |
| BSS atomic after A→B | Dirty 888 replaced by zero |
| Lifecycle dispatch | Fresh install reports 1; successful upgrades report 2 |
| Nested fixed stack buffers | Expected checksum before and after upgrades |
| Missing anchor | Typed trap; A's code, dirty statics/BSS and zero anchor survive |
| Host negative cases | Unsafe layouts and altered or mismatched final artifacts refused |
| Growth across A→B→A | 129→130→131→256 pages; sampled old/new sentinels survive |
| Grow beyond maximum | Returns -1; physical extent, recorded extent and samples unchanged |
| Zero-page growth at maximum | Returns 256; no out-of-bounds access or state change |
| Trap after growth in an update or upgrade hook | Old memory extent, metadata and sampled state restored |
| Omitted retention option / stripped destination marker | `CanisterContractViolation`; old code and observed state preserved |
| Retained claim of 257 pages with 256 physical pages | Post-upgrade trap; corrupt claim preserved, never reset |

The generated fixture is now `rust-startup-probe-v2`; it extends the original
scalar fixture with extent metadata and growth probes. It is not compatible with
the M0 managed-record layout. Final A SHA-256:
`4a8705fd3e4f849f3f82a53a1635a4184f0a1fac66869c3aca6ead210f05b796`.
Final B SHA-256:
`9681c9ce63fee85d58cee87c19797abb3545c68aab1d0db8ed83ef1fded59240`.
The current local execution log is `target/evidence/rust-startup-growth-test.log`;
the earlier v1 run remains in `target/evidence/rust-startup-test.log`. Individual
artifact reports retain raw hashes and linker bounds. These identify this local
cohort, not reproducibility across all hosts or arbitrary future toolchains.
The negative upgrade-growth and stripped-marker modules are retained beside the
A/B artifacts, with their hashes in the current log. Growth-trap probes reply
or complete normally if growth is refused, so the expected trap establishes
that growth succeeded before rollback was exercised.

The [M0 layout contract](../../docs/design/m0-layout.md) is now frozen for the
managed-graph implementation. Its image inspector, native append-only allocator
and scoped graph mutation tests live in the product crate; the separate managed
graph profile now provides installed graph/allocator continuation evidence.
This scalar growth
test samples selected boundary words, not every byte of
every new page, and does not inject host resource exhaustion. General allocation,
bounds for arbitrary recursion, migration, reclamation, callbacks and production
capacity/performance remain outside this result.
