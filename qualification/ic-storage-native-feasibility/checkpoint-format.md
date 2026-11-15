# Scalar/vector checkpoint experiment

This fixture specifies a logical checkpoint for one u64 counter and one uniquely
owned vector of at most 16 u64 values. It is an experimental contract, not a
production Storage format or the complete migration protocol. It contains no
pointer, capacity, native header, allocator layout or executable identity.

## Bytes, version 1

All integer fields are unsigned little-endian. The fixed header is 80 bytes.

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 8 | Magic: ASCII `ICNCP001` |
| 8 | 2 | Envelope version: 1 |
| 10 | 2 | Flags: 0 |
| 12 | 4 | Header size: 80 |
| 16 | 8 | Logical schema identity: ASCII `RECU64V1` |
| 24 | 4 | Logical schema version: 1 |
| 28 | 4 | Vector element count, 0 through 16 |
| 32 | 8 | Source epoch, supplied and checked by the migration caller |
| 40 | 8 | Checkpoint-local root object ID: 1 |
| 48 | 8 | Checkpoint-local owned vector object ID: 2 |
| 56 | 8 | Counter value |
| 64 | 8 | Payload size, exactly count times 8 |
| 72 | 8 | Reserved: zero |
| 80 | count × 8 | Ordered initialized vector values |
| 80 + count × 8 | 8 | Completeness marker: ASCII `SEALED01` |
| 88 + count × 8 | 32 | SHA-256 of every preceding byte, including the marker |

The complete size is 120 + count × 8: 120 through 248 bytes. No trailing bytes
or alternate encodings are accepted. The root uniquely owns object 2; no other
objects, aliases, cycles, weak edges or unreachable allocations are represented.
The IDs are checkpoint-local, not native addresses or application business IDs.
The schema identity is opaque; its separately encoded version is authoritative.

The encoder appends the seal and digest only after all logical values. Decoding
checks bounded size, canonical framing, schema, object IDs, lengths, seal and
digest before allocating decoded values. It also requires the caller's expected
source epoch. Unknown versions, mismatched epochs, incomplete input and failed
integrity refuse; there is no fallback or empty-state initialization.

SHA-256 detects corruption against the supplied digest, not malicious authorship:
an attacker can replace both bytes and digest. The outer migration authority
must bind the complete checkpoint digest, source epoch, authorized target and
provider locator independently. This standalone fixture has no authenticated
provider, durable publication or write-freeze protocol. An explicit schema
version or new collection kind needs its own decoder and migration policy.

## Ownership and reconstruction

Export reads logical scalars under the source's existing exclusive custody.
It does not consume a parked owner or run destructors through unknown layouts.
The returned bytes are an independent transient allocation. For the host proof,
the source vector and arena are then dropped before a fresh target arena exists.

Import decodes only those bytes, allocates a new target vector and populates its
initialized elements through safe Vec operations. The target must remain a
candidate until complete validation/construction succeeds. If allocation fails,
discard that candidate and preserve the checkpoint; never publish a partial root.
The counter and vector order must be preserved. Native addresses and capacity
are intentionally not preserved. The test also changes the enclosing record's
field order to demonstrate that the logical bytes do not depend on that layout.

This is neither compatible direct reopen nor the installed incompatible-runtime
proof. It does not qualify independent opening of governed Storage handles,
canister upgrade modes, rollback, interruption/resume or persistent checkpoint
storage. The required published Storage 0.74.1 provider boundary remains unchanged.

## Checks and evidence

`make native-feasibility-test` includes checkpoint round trips, an independently
assembled golden vector, empty/max bounds, every truncated prefix, trailing
bytes, oversized counts, altered fields with recomputed digests, corruption,
epoch mismatch, source teardown and fresh-arena reconstruction/allocation.
The golden fixture uses counter 42, epoch 7 and values 1, 0x0102030405060708 and
u64::MAX. Header/payload/seal were assembled separately and their digest computed
with the host SHA-256 utility; tests compare the complete bytes.
Its digest is `9722da7a78445e4fa829f1ebee9c129705916e529152ef2674b9ebbba39c39db`.

On 15 November 2026, five codec tests and two arena reconstruction tests passed
on x86_64 Linux with Rust 1.100.0, alongside the existing ownership/allocation
checks. The fixture uses the already-selected SHA-256 implementation sha2 0.11.1,
inherited from the qualification workspace; no dependency versions were upgraded.

No type becomes admitted for persistent application state from passing this
codec experiment. Native allocation-domain establishment and the installed
provider-backed migration proof remain open.
