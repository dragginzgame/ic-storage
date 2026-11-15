# Installed provider-custody protocol

This independently authored WAT fixture tests the custody portion of the
[foreign-provider investigation](../ic-storage-native-feasibility/provider-boundary.md).
It contains no Rust code, foreign call adapter or native vector. Returning a Wasm
integer from its private claim function does not grant Rust allocation validity.
It does not reuse or extend the frozen M0 storage format.

## Region and lifecycle contract

The module owns exactly three ordinary wasm32 pages, with initial and maximum
size both three. There are no data segments, start function, tables or growth
instructions. Page zero holds diagnostic replies; page one holds a controlled
header; a 4096-byte payload region begins at 131072 in page two. Only its first
u64 is used. No compiler allocations exist in this module. Keeping a future
Rust stack, data and allocator disjoint remains a separate artifact obligation.

All header fields are little-endian u64 words:

| Offset | Meaning | Admitted value |
| --- | --- | --- |
| 65536 | Format marker | `0x4e50524f56303031` |
| 65544 | Format version | 1 |
| 65552 | Payload base | 131072 |
| 65560 | Payload capacity in bytes | 4096 |
| 65568 | Initialized logical payload in bytes | 8 |
| 65576 | Settled custody | 1 |

Fresh installation writes this header and initializes the first payload word to
17. Upgrade never initializes unknown data: it checks every header field and
physical extent before becoming ready or accessing the payload. The incompatible
target deliberately expects a different capacity and refuses the source image.
These checks are not authentication or proof of an arbitrary writer's history.

The transient phase is cold, ready or claimed. Claim requires ready plus full
header admission, then clears the retained settlement word and changes phase to
claimed. A second claim traps. The private settlement operation requires claimed
phase and restores the retained word and ready phase. Its trusted caller must
have finished payload access; this WAT protocol does not enforce Rust lifetimes.

Pre-upgrade rejects an outstanding claim. Independently, destination admission
rejects an unset settlement word, even when the source has no pre-upgrade hook.
Thus resetting a transient phase on instantiation cannot erase lost custody.
An abandoned claim remains unavailable; there is no recovery/reset endpoint.
This is one whole-region claim, not a multi-allocation root inventory or allocator.

The source and target are trusted, reviewed fixtures. Test-only endpoints observe
raw words, corrupt the header or abandon a claim; these are not a proposed public
provider interface. The read-only observation query is WAT-only instrumentation,
not permission to access a future Rust-owned allocation while borrowed.

## Checks and observed result

The existing retention target runs the two new cases alongside the original
scalar retention test. It already runs in the repository's hosted CI matrix:

```sh
make retention-lint
make retention-test
```

Use the previously prepared Testkit server; these commands run locked and offline.
No new tool, dependency, Make workflow or qualification package is required.
All three tests passed locally on x86_64 Linux with Rust 1.100.0 and the admitted
PocketIC 16.1.0 server on 15 November 2026. This is not a production-subnet or
native macOS result. The new cases establish:

- Structural refusal of changed memory geometry, growth instructions, active
  data initialization and unexpected imports. Source review owns exact store
  addresses; the structural check is not an untrusted-code sandbox.
- Fresh initialization, repeated claim/settle updates and compatible A→B→A
  upgrades with continued writes and the expected installed module hashes.
- Duplicate-claim and write-then-trap update rollback, restoring phase, settled
  state and observed payload. A target that claims, writes and traps during
  post-upgrade restores the source module and observations.
- Incompatible target capacity, committed header corruption and abandoned
  custody refuse without silently resetting the source. A source missing its
  pre-upgrade guard still cannot pass unsealed state to the destination.

The query observes version, transient phase, settlement, the first payload word
and declared extent. It does not compare all memory bytes. Exact A/B and negative
Wasm artifacts are retained by SHA-256 under `target/evidence/provider-custody/`;
the current execution log is `target/evidence/provider-custody/current.log`.

## Remaining gate

The separate [vector descriptor protocol](vector-provider.md) extends this
investigation with variable initialized length/capacity and publication after
access ends. It preserves this scalar fixture and uses a distinct format marker.

This supplies an exercised custody protocol, not a Rust validity argument by
itself. The separate [combined scalar experiment](../ic-storage-native-feasibility/provider-boundary.md#installed-scalar-lifecycle)
now records its external-allocation contract, admits the final artifact and
executes native scalar accesses across installed upgrades. That argument and its
trusted entrypoints are additional obligations beyond this WAT-only evidence.
Fresh allocator issuance, native vectors, general initialization tracking and
reclamation remain unqualified.
