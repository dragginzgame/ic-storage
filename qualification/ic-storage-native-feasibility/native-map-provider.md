# Closed installed native map contract

This profile executes the actual `ic-storage-native::map::Map` implementation
against the existing transient region allocator and a foreign WAT provider.
It extends the [product map evidence](../../docs/design/native-map.md) to
compatible installed IC upgrades, custody and rollback. It does not select
another collection architecture or make the fixed provider a general allocator.

## Scope and memory authority

The initial contract uses Rust 1.100.0, wasm32-unknown-unknown, panic=abort,
one codegen unit, optimization level 3 and fat LTO. The collector builds the
product `map` feature with the locked offline graph and records source, compiler
identity and resulting artifacts. No upstream implementation is copied.

The provider owns one exclusive live aligned allocation at 131072, covering
two disjoint banks of 512 Slots each: 24576 bytes total. Each Slot is three initialized native u64 words in
`repr(C)` order: occupied, key and value. The entrypoint statically checks size
24 and alignment 8. The product type's declaration fixes field order; its
logical admission separately checks tags, count, unique keys and probe placement.
The provider initializes the allocation to empty Slots only at fresh install.
Post-upgrade never zeroes or resets unknown retained data.
Closed fixture methods pack two u32 operands into the scalar call argument;
the product's full-width u64 key/value cases remain separately host tested.

The three-page fixed memory has separate domains: the bounded Rust stack below
32768, provider metadata at 65536 and native Slots at 131072. Retained root words
are magic `NMAP0001`, format 2, base, extent, live count, bucket count, settlement,
stride 24, alignment 8, hash/probe version 1 and active bank (0 or 1). Format 1
is refused; this experimental change makes no stored-format compatibility claim.
Full-width checks precede any
narrowing or multiplication. Bucket count is a power of two in 8..=512, count
is at most 75% of it, and every represented slot is initialized. The test profiles
start with 8 or 512 buckets; the full allocation belongs to the provider.

Numeric checks do not create allocation provenance. The foreign provider loans
its allocation through the only raw Rust import, after checking phase and settled
custody. Two transient controllers receive disjoint bank loans. The source
controller reserves the exact active slot span and freshly issues it via
Allocator::allocate without writing payload. Vec::from_raw_parts_in uses
that issued allocation; Map::admit validates the whole active table. The spare
controller starts with an empty allocation. No old Rust owner,
reference, allocator object or private Vec header crosses the entrypoint boundary.

The provider changes settled → claimed before returning the pointer. Rust
releases its Map and the actual Vec, then returns only count/bucket/bank bits. The
wrapper publishes the root and settles only after that return. All unexpected
failures trap the IC message; a normal callback Err still finalizes prior writes.
Forgetting a scoped View cannot skip release. The pre-upgrade hook checks both
provider metadata and the actual table; the target repeats admission before use.

Map growth keeps the source owner alive while `Map::rebuild_in` constructs a
candidate in the spare bank. Only after construction completes does Rust retire
the source owner, insert the new key and release the candidate. Publication
requires a full source, exactly doubled bucket count, opposite bank and one
additional entry. Same-bank publication cannot change bucket count. Retired bank
bytes remain unchanged until a later rebuild reuses that bank; only the selected
bank is authoritative. Fresh install initializes all Slots, including inactive
ones, and a candidate rebuild clears its own prefix before placing entries.

The 8-bucket profile grows six times to 512 buckets (384 live entries). At that
budget, further insertion refuses without changing the table; deletion permits
reuse. This qualifies growth within a preallocated loan, not memory.grow or a
general allocation pool.

## Compiler and access qualification

The raw module has one `native_map_diagnostic::claim_region` import, six
defined functions and a private four-argument scalar entry at function 5. Its
three i32 globals initialize to 32768; there are no data/start/table sections,
indirect calls, runtime memory growth or retained std headers. Scalar/vector
profiles reject this module, and the map profile has its own exact signature,
export and compiler-shape checks.

The existing stack analysis now has a map-only extension for empty-result loops
and the emitted integer arithmetic, including i32.xor for bank selection. The call graph remains acyclic. The frame
local is immutable after entry, all non-trapping exits restore it, and branches
cannot target the implicit function return. Loops can repeat a fixed frame but
cannot accumulate stack frames. Peak simultaneous stack is 176 bytes, from
32592 through 32768. This is a space proof, not a time bound. Tests preserve the
older profiles' refusal of these new operators and reject recursive calls,
frame mutation, function-label escapes and memory growth.

All raw linear-memory accesses are rewritten to private provider guards. The
counts are 61 i32 reads (including one unsigned i64.load32), 4 byte reads,
47 i64 reads, 40 i32 writes, 7 byte writes, 63 i64 writes, zero copies and 1
zero-fills. The unsigned 32-bit load is guarded as i32 then zero-extended to i64.
The seven existing helper signatures remain unchanged; only this profile adds
the zero-fill helper. Every emitted access must be accounted for after rewriting.

Guards admit the bounded transient stack or the active initialized slot prefix
while phase=claimed, settlement=0 and the closed operation selector is valid.
For an insertion with a full source below 512 buckets, they additionally admit
the opposite bank's candidate prefix of twice the source bucket count. Metadata,
remaining spare capacity and other memory are inaccessible. A construction
failpoint traps immediately after an actual write into the candidate bank.
Offset/end arithmetic detects wrap. Fills require zero and validate the entire
destination; copy is unavailable because this compiler artifact emits none.
Guards execute outside LLVM, never reenter Rust and do not grant Rust pointer
validity independently of the foreign loan contract.

The linker resolves all native/guard imports, leaves only two IC reply imports,
and hides every native and helper export. Final admission recomputes the exact
composition from the freshly admitted raw artifact and selected provider profile.
Extra diagnostic exports, a different provider or altered bytes fail final
admission. Guard counterexamples deliberately add test-only entrypoints to the
same provider source; those instrumented artifacts are explicitly rejected by
normal final admission and are not deployment candidates.

## Installed checks and results

`make native-map-probe` runs artifact/stack checks without a server and joins CI.
`make native-map-test` uses the separately prepared Testkit server, without
downloads, and explicitly deletes its PocketIC instance after completion.

On 16 November 2026, cohort
`target/evidence/native-map-provider/attempt.XEuxtJ/` passed the three selected
artifact/stack tests and installed lifecycle test (3.56 seconds). That installed
test covers both initial sizes, all six growth steps and compatible upgrades at
each size in both executable directions,
lookup/replacement/deletion/clear/reuse, ordinary errors, forgotten views,
full-table refusal, duplicate claims, settlement without a claim and failed
updates/upgrades. Growth failpoints cover a destination write, completed rehash
with the source still owned, native release before publication, and completed
publication. A failing upgrade also grows in its post-upgrade hook before
trapping. Every failure compares the complete header and both banks and verifies
usable source custody; failed upgrades also preserve source code identity.
Successful growth preserves every old-bank byte, all entries and lookup/update
behavior. Business errors and forgotten views also complete growth and settlement.

Twelve metadata/slot faults are exercised with and without the source guard so
target refusal is independently tested. Guard counterexamples check valid stack
and payload endpoints, metadata exclusion, spare-space exclusion, both stack
edges, arithmetic wrap, unclaimed/settled access, invalid selectors, nonzero or
out-of-range fills and forbidden copies. Candidate-bank endpoints are checked in
both directions; inactive access without a growth operation and publication of
an unbuilt candidate are refused. Valid operations work after refusals.

The shared scalar/vector artifact regression also passed all 19 cases in
`target/evidence/provider-link/attempt.HMnJp1/`. These results add no claim about
million-key installed throughput, adversarial hash inputs, general record
values, multi-root composition or cross-compiler migration. Product map portable
recovery remains host qualified; its governed checkpoint integration on IC and
a general retained allocation pool remain next runtime work. Installed growth
is bounded by the preallocated 24 KiB region and 384 live entries.
