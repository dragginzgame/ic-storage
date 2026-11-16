# Installed M0 managed graph

The `managed-graph` feature uses the product crate's frozen M0 layout and safe
image API directly in retained Wasm memory. It is a qualification fixture, not
a production memory owner or SDK. The default scalar startup/growth profile and
its historical evidence remain separate and use an incompatible record format.

## Custody and startup

The product image API now accepts only retained bytes beginning at 8 MiB.
Borrowing a complete live memory image would overlap Rust's executing stack and
transient objects. `inspect` still accepts complete native snapshots;
`inspect_retained` and `Image` omit that prefix, retaining absolute error offsets
and the existing byte format. `Image` physical-page arguments include the 128
transient pages; `Image::bytes` returns only the retained physical prefix.

The fixture's private kernel creates one bounded mutable slice over
`[8388608, memory.size * 65536)`. A busy guard excludes nested access; synchronous
callbacks cannot return its borrow. There are no outgoing calls, executor,
timers or concurrent memory users. The admitted stack/data bounds stay below
8 MiB. No slice or session remains live across `memory.grow`: the kernel admits
the old bytes, ends that borrow, grows one page, then reopens the old prefix with
the new physical page as capacity and publishes extent/digest through M0.
Failure after system growth traps, invoking IC rollback.

The kernel and IC export attributes use narrow unsafe expectations in the
qualification crate. Product code still forbids unsafe Rust. This custody proof
depends on the selected module, source review and artifact checks; it does not
authorize constructing slices from arbitrary addresses in other programs.

The controlled Wasm start clears/restores transient memory before Rust executes.
Only `canister_init` initializes fresh retained bytes and builds the first graph.
`canister_post_upgrade` admits all retained metadata, indexed objects and
references, requires the expected non-null root, then enables the transient
capability. Missing/incompatible data is never initialized on upgrade.

The fixture links std with Platform's `system-raw` path. The M0 decoder retains std panic
formatting and its function table. A fixture global allocator traps on every
allocation/deallocation request, preventing dlmalloc from acquiring retained
pages. The graph artifact profile admits exactly one 18-slot nonshared funcref
table, initialized at slot one with 17 module functions. It rejects table writes,
growth/copy/fill/init instructions and reference/tail-call extensions. Ordinary
indirect calls can use only table zero. The table and its static vtables are
transient executable state; no persisted M0 field is used as an executable
pointer. This does not qualify arbitrary trait objects or general std allocation.

Artifact admission still requires exact reply imports, endpoint roster, memory
bounds, passive-data bootstrap and final bytes derived from the reviewed raw
module. It admits exactly one compiler-output `memory.grow`, owned by the kernel;
additional growth sites refuse. These structural checks complement trusted source
review, rather than sandboxing arbitrary hostile compiler input. No scalar
sentinel helpers are installed in the graph profile.

## Commands and observed evidence

After the existing explicit tool/dependency/server setup:

```sh
make managed-graph-lint
make managed-graph-build
make managed-graph-test
```

The build uses the same Rust 1.100.0 wasm32 linker bounds as the scalar profile.
The test runs locked/offline with the prepared Testkit/PocketIC server. CI selects
it on Linux and both macOS architectures; hosted results are not yet observed.

Local x86_64 Linux / PocketIC 16.1.0 evidence on 15 November 2026:

| Scenario | Observed result |
| --- | --- |
| Fresh graph | Node `(0,1)` points to itself; CounterState `(72,2)` references that Node twice |
| A→B→A | Complete retained bytes match across upgrades; transient values reset for destination code |
| Mutation under B | Counter becomes 42; writing Node through left yields 99 through right |
| Continued allocation | Next Node uses offset 160, identity 3; cursor becomes 232 |
| Trapped update | Growth, payload writes, index, cursor and digest roll back together |
| Incompatible-version injection before post-upgrade admission | Upgrade traps; old code hash, transients and complete retained bytes survive |
| Growth/mutation trap after successful post-upgrade admission | Same rollback checks pass, followed by successful upgrade and mutation |
| Near-boundary population | 909 objects occupy 65464 bytes; after upgrade, allocation fills to 65536 bytes and then refuses |
| Growth and continuation | Explicit growth to 131 total pages permits object 911, ending at byte 65608; later downgrade preserves all retained bytes |
| Host admission | Wrong profile, final-byte tampering, extra growth sites and table mutation refuse |

Negative upgrade modules are deliberately submitted only by the rejection test
after the positive artifact validator refuses them. The incompatible case injects
version 2 before normal admission; it is not a supported schema migration. The
post-admission trap is inserted at normal function exits so an explicit compiler
return cannot bypass it. The deliberate graph trap returns normally if system
growth is refused, preventing a resource refusal from being mistaken for rollback
of successful growth. The fixture traps on ordinary operation errors; the product API's
ordinary callback `Err` semantics remain distinct from IC trap rollback.

Final modules, raw/final SHA-256 hashes, linker bounds and negative modules live
under `target/evidence/managed-graph/`; execution output is
`target/evidence/managed-graph-test.log` records the initial run. Its final A SHA-256 is
`72afba87c754d45f288220f789f23e60c7408b18b1216297b900d6e52a721750`;
B is `6c90b87c7b0c43e0cdcd6a45a5d609d1c824ae42f8dbc21d86e70191e961f9cc`.
Both use stack top 1048576; A data/heap ends are 1050357/1050368 and B's are
1050869/1050880. The scalar regression retains its earlier final hashes.
These are local cohort evidence, not a
production subnet claim. Full byte comparisons cover the exercised populations,
not all possible heaps or interruption points. Remaining work includes deliberate
interruption of each publication stage, committed-corruption upgrade cases,
resource/instruction cost measurement, and a production admission/custody contract.
Reclamation, migration, async execution and arbitrary Rust values remain excluded.

The session-read optimization was rebuilt and reran both graph tests on
15 November 2026. All admission, upgrade, growth and rollback assertions passed.
The current artifact directory contains A
`03e0994d32e1b1fc9fd425c55ab70a76e362fdea4155311d28e3e326da6f246c`
and B `88b8d9ff2b3bd08985f770cca6f52c096e34008414a288f235fb472e7a4a7c21`.
A data/heap ends are 1050341/1050352 and B's are 1050853/1050864; stack top and
retained boundary are unchanged. The
[benchmark evidence](../ic-storage-simple-benchmarks/README.md#session-read-reuse-15-november-2026)
records the measured instruction improvement and remaining validation overhead.
