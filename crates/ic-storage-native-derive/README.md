# Scalar Store declarations

Unpublished experimental derive, re-exported by `ic-storage-native` when its
`derive` feature is enabled. The existing qualification V1/V2 records are its
first consumers. It generates safe Rust only; it creates no retained allocation,
root registration, unsafe marker implementation or permission to reopen memory.

```rust
use ic_storage_native::Store;

#[repr(C)]
#[derive(Clone, Copy, Store)]
#[store(id = "RECU64V1", version = 2, validate = Self::valid)]
struct Counter {
    #[store(field = 1)]
    counter: u64,
    #[store(field = 2)]
    ceiling: u64,
}

impl Counter {
    fn valid(&self) -> bool { self.counter <= self.ceiling }
}
```

The initial subset requires exactly repr(C), one to sixteen named `u32`/`u64` fields,
unique nonzero u32 field IDs, an eight-byte non-NUL ASCII schema ID and a nonzero
u32 version. The record must implement Copy, excluding arbitrary application
Drop. Generated code requires actual primitive values of the declared width, so shadowing a name
cannot hide a borrowed or owning representation. Generics, enums, tuple records,
other field types, alternate repr options and skipped fields refuse. Field types
accept bare `u32`/`u64` or absolute `::core::primitive::u32`/`::core::primitive::u64`
paths. The explicit paths are useful in modules that shadow primitive names.
Relative paths, user-defined aliases and generic path arguments are excluded;
these spellings do not add a new retained representation or change codec bytes.

Generated items are:

- `STORE_ID`, `STORE_VERSION`, `STORE_BYTES`, `STORE_SIZE`, `STORE_ALIGN` and
  `STORE_FIELDS`: explicit identity, logical byte bound and concrete public-layout
  metadata. Field descriptors contain ID, native offset and scalar kind, ordered
  by ID. Equal record sizes and offsets do not imply equal field representations.
- `store_encode` and `store_decode`: an exact-size, allocation-free payload of
  little-endian fields ordered by field ID, using four bytes for u32 and eight
  for u64. Native padding is omitted; the payload is at most 128 bytes. Decode constructs fresh values;
  an optional application `validate` function checks both decode and encode.
  The result distinguishes length refusal from invariant refusal.
- `CounterView` with scalar getters, and `store_with` with a higher-ranked scope.
  Each getter preserves its field's concrete integer type. The view exposes no
  mutable record, owning carrier or Deref. Application-specific
  methods can extend the view in the declaration's module; the qualified V2
  supplies checked addition this way. There are no generated arbitrary setters.

The schema ID/version are not repeated in this field payload. An enclosing
checked envelope must select the matching declaration before decoding. Existing
checkpoint v1/v2/v3 envelopes retain their exact bytes and framing; the fixture
adapts the generated scalar payload to their established header slots. Reordering
declarations changes native offsets while preserving the logical field order;
that does not authorize direct reopening of the changed native layout.

Owners still freeze declarations and bind expected metadata to each admitted
runtime/schema. The macro does not maintain a global ID registry, compare old
executables, prove private collection layouts, establish pointer validity, enforce
validator purity or choose migrations. V1→V2 conversion remains hand-written.
`store_with` assumes an already valid live record; callers own admission,
poison/trap policy and finalization. Validation functions and added view methods
are reviewed application code, not an escape from those obligations.

The `ic_storage_native` dependency must retain its canonical name for generated
paths. This first subset supports scalar declarations only; the design's future
owning collections, nested records and sharing are not admitted by this derive.
Scalar width changes require explicit conversion, never a cast of retained bytes.
The [V2→V3 narrowing probe](../../qualification/ic-storage-native-feasibility/records.md#checked-scalar-width-conversion)
checks the u64→u32 range and preserves the ceiling. The installed qualification
profile now exercises conversion, publication rollback and reconstruction through
checkpoint envelope v4. Explicit scalar V3→V2 widening now also has
[installed admission, rollback and recovery evidence](../../qualification/ic-storage-native-feasibility/records.md#installed-exact-widening);
it leaves the vector unchanged and reuses schema-2 checkpoint encoding.
These schemas are test cases, not maintained pre-1.0 product releases.

Run `make native-feasibility-test` for macro refusal tests, generated layout and
codec checks and the host lifecycle. Run
`make native-record-test` for the installed conversion/update/recovery matrix.
Both legacy checkpoint/job suites remain regression gates for the shared codec.
`make native-core-test` exercises the feature directly in a `no_std` application,
including field order, exact-length decoding, invariant refusal, missing implicit
prelude, explicit primitive paths and application names shadowing generated-code
dependencies. It also owns the compile-fail tests for unsupported fields, custom
Drop, misleading scalar aliases, escaping views and unrestricted record mutation.
Generated standard-library paths, primitives, result constructors and documentation macros
are fully qualified; callers need no supporting imports.
