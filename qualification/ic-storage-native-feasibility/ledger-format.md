# Experimental native allocation ledger image

This private host fixture tests a controlled representation of allocator metadata.
It is separate from the [portable logical checkpoint](checkpoint-format.md): the
ledger describes allocations, but contains no application values and cannot
recover them. It does not define a product format or authorize native reopening.

## Version 1 bytes

The exact image size is 2624 bytes. All integer fields are unsigned little-endian
u64s. The profile is fixed to 64 allocation slots and a backing domain of 64 u64
elements (512 bytes). Slot indices are retained; empty slots cannot contain old
metadata. No Rust padding, enum layout, pointer, allocator object or Vec header
is copied into the image.

The host transfer fixture can restrict a controller to a smaller usable prefix.
That transient selection is not encoded here: 64 remains the physical backing
extent. Controller admission separately rejects allocations beyond its limit;
successful image decoding alone never selects or admits a destination profile.

| Byte offset | Meaning |
| --- | --- |
| 0 | Eight-byte magic `ICNLED01` |
| 8 | Envelope version, exactly 1 |
| 16 | Domain extent in u64 elements, exactly 64 |
| 24 | Next allocation generation, nonzero |
| 32 + 40 × slot | Slot kind: 0 empty, 1 parked allocation |
| slot base + 8 | Start offset in u64 elements |
| slot base + 16 | Nonzero allocation capacity in u64 elements |
| slot base + 24 | Nonzero allocation generation |
| slot base + 32 | Initialized element count, at most capacity |
| 2592 | SHA-256 of bytes 0 through 2591, 32 bytes |

An empty slot is all zero bytes. Other kinds, versions, geometry, lengths or
trailing bytes refuse. Each extent must fit the domain using checked arithmetic.
Allocated extents cannot overlap, generations cannot repeat and every allocated
generation must precede the next generation. An exhausted next-generation value
does not require wrapping or invalidate existing allocations.

The occupied bitmap is derived from validated extents, not stored a second time.
The encoder checks its source ledger's bitmap and extents for consistency and
refuses any allocated block still held by an active Vec. It produces an image
only when all allocated blocks are parked. The decoder verifies exact framing
and integrity, decodes fields with checked integer conversions and applies the
whole-ledger invariants. Neither operation mutates the source arena or allocates
heap storage; the bounded arrays are local values.

## Custody is a separate prerequisite

A decoded ledger is ordinary untrusted metadata with structural checks applied.
SHA-256 detects accidental damage; it authenticates neither author nor domain.
Matching generations and bounds cannot prove that allocations exist, elements
are initialized, all owners are accounted for, or an image belongs to this heap.
In particular, coherently forged or replayed bytes may pass structural decoding.

There is deliberately no operation installing a decoded ledger into an Arena,
minting parking tokens, reading payloads or creating pointers from these bytes.
Only the original live arena and unique token authorize the existing raw-parts
reopen. Export does not consume that token or establish a migration transaction.
Forgotten owners and checkpoints from another domain require separate custody
and root-inventory treatment, not an inference from this format's digest.

The [live suspension experiment](suspension.md) adds sole-root accounting and
freezes its original arena; it does not restore allocation custody from the image.

Before using such metadata across an IC upgrade, specify a source/target domain
identity, retained payload geometry and lifetime, complete owner/root inventory,
snapshot publication protocol and allocation/pointer-creation mechanism. Neither
this format nor the existing host tests settle that compiler/runtime contract.

## Evidence and reproduction

The golden image has next generation 2, one allocation in slot 0 starting at 0,
capacity 4, generation 1 and initialized count 1; all remaining slots are zero.
Its independently assembled prefix has SHA-256
`e996a6efadf614041430e1af6ec7fb563a4698b66ce66967dff0e66ab324f8b0`:

```sh
perl -e 'print "ICNLED01", pack("Q<*", 1,64,2, 1,0,4,1,1), "\0" x (63*40)' \
  | sha256sum
```

Tests cover every truncated prefix, a trailing byte, one-bit corruption at every
byte position, resealed unsupported/noncanonical metadata, overlapping extents,
duplicate generations, active-owner refusal and sparse/full-domain round trips.
The Linux golden assembly command is an independent oracle, not a CI dependency.
Run the focused Rust checks on supported hosts from the repository root:

```sh
cargo clippy --manifest-path qualification/Cargo.toml \
  -p ic-storage-native-feasibility --target-dir target/qualification \
  --locked --offline --test allocation_tests -- -D warnings
cargo test --manifest-path qualification/Cargo.toml \
  -p ic-storage-native-feasibility --target-dir target/qualification \
  --locked --offline --test allocation_tests
```
