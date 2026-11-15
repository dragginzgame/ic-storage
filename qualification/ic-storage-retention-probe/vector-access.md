# Checked vector memory operations

This qualification-only experiment adds checked access helpers to the
[vector provider protocol](vector-provider.md) and rewrites the compiled vector's
memory instructions to call them. The helpers have installed WAT tests. The
rewritten Rust module still has unresolved imports and is **not executed**.
This is not a product allocator, a hostile-code sandbox or complete native
admission. The foreign allocation and initialized-owner argument remains required.

## Effective addresses and access windows

The private helpers widen the wasm32 address, add the original memory offset in
u64 arithmetic, and check addition and end-of-range overflow before narrowing.
They check the entire byte interval, not only its starting address. An offset
that would wrap into the provider after i32 addition therefore refuses.

The only transient-memory window is `[32704, 32768)`, exactly the 64-byte frame
established by the current compiler's stack/control check. The helpers themselves
use only Wasm locals and globals, with no additional linear-memory stack. Stack
access does not require a provider claim because Rust entry may initialize its
transient frame before claiming backing. This checks bounds, not initialization
of stack slots or their Rust representations.

Payload access requires a claimed phase, an unset settlement word and a selected
operation in 0..=5. The header's full-width length/capacity is checked again before
length arithmetic. All other linear memory, including provider metadata and reply
scratch, is inaccessible through these helpers. For old admitted length `L`, the
payload windows start at 131072 and have these lengths:

| Operation | Read bytes | Write bytes |
| --- | --- | --- |
| 0: mutate first | `8*L` | `8*L` |
| 1: append | `8*L` | `8*(L+1)`, refusing a full 512-word region |
| 2: remove first | `8*L` | `8*L` |
| 3: shrink | 0 | 0 |
| 4: clear/shrink | 0 | 0 |
| 5: destroy/reuse | 0 | 8 |

These are conservative windows for the reviewed u64-only operations, not proof
that every operation writes its intended element. New writes do not enlarge the
read window during the same call. Source ownership and successful allocation
must still justify the block being accessed; a numeric range is insufficient.

Copy is permitted only for removal and only within the payload. Both ranges are
checked before the overlapping Wasm memory.copy executes. Stack-to-payload copies
are refused. Zero-byte copies at an admitted boundary are allowed without reading
elements; nonzero reads of spare capacity are refused.

## Compiler transformation

The rewriter first validates the bounded wasm32 module, exact memory/global and
import/export rosters, helper/entry signatures and the existing stack/control
profile. Initializers, tables, growth and unsupported operations refuse. It keeps
the same transient frame and replaces every supported access:

| Raw operation | Required imported helper |
| --- | --- |
| i32.load / i32.load8_u / i64.load | load_i32 / load_u8 / load_i64 |
| i32.store / i32.store8 / i64.store | store_i32 / store_u8 / store_i64 |
| memory.copy | copy |

Loads receive `(address, original_offset)`; stores receive
`(address, original_value, original_offset)`. Offsets are preserved as u64 values,
without i32 truncation. Ordinary function calls and exports are remapped around
the seven added imports. The output is validated again. Raw custom metadata is
omitted; the smaller diagnostic byte count is not a native-storage size result.

The closed forwarding roster prevents newly supported memory instructions from
silently passing through unguarded. Unsupported access widths, memory.fill and
initialization are negative cases. Independent test inventory compares raw access
kinds with emitted helper calls and verifies there are no original direct memory
operations left in the rewritten bodies. Synthetic cases check store value/offset
ordering and direct-call/export remapping.

For the current Rust 1.100.0 output, all 49 sites are replaced: 19 i32 loads,
one byte load, one i64 load, 21 i32 stores, two byte stores, four i64 stores and
one copy. The unresolved output is 1429 bytes with SHA-256
`9330264d5230f7b23358eef05e7cba4c2a5611390358d96df9b7affecb1db7ac`.
The passing eleven-check cohort and `vector/guarded.wasm` are under
`target/evidence/provider-link/attempt.JIJ4Hv/`.

## Installed helper evidence and remaining gate

The installed WAT helper test covers exact stack/payload boundaries, overlapping
and zero-byte copies, append/reuse write windows, operation-specific read refusal,
metadata/spare-capacity access, unclaimed access and offset overflow. Refused
updates preserve all provider-header words and all 4096 payload bytes, including
a valid write followed by a forbidden read in the same transaction. It is selected
by the existing `make retention-test`; Clippy uses `make retention-lint`.

On 15 November 2026 the focused installed helper test passed on x86_64 Linux
with Rust 1.100.0 and the admitted PocketIC 16.1.0 server: seven successful
probe cases and 25 refusal cases. Source copies, compiler identity and the log
are under `target/evidence/vector-access/attempt.oXlJG2/`; each WAT variant is
retained by digest under `target/evidence/provider-custody/`. Strict Clippy and
the eleven compiler/artifact diagnostics also passed. These are local results,
not native-vector, production-subnet or hosted macOS execution claims.

The compiled-vector diagnostics run through `make native-provider-probe`. Passing
either command does not link the guard imports or authorize execution of the
rewritten Rust module. Before that step, final artifact admission must bind each
import to these exact helper semantics, keep the raw Rust function private and
establish the complete provider loan and source initialization history.

The trusted entrypoint must derive length/capacity from the admitted header,
select the operation, call Rust, and settle the returned descriptor only after
all Rust ownership has ended. It must reset the transient operation selector
before replying. Failure must trap without local recovery. Guards cannot repair
Rust undefined behavior before code generation, certify aliases or replace the
allocation contract. Their role is to enforce bounded machine accesses and make
unexpected accesses fail during the eventual installed lifecycle experiment.
