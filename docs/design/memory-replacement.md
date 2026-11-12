# Independent memory interface specification

This is the restricted behavioral handoff for replacing the memory interface and
its implementations. The integration reviewer has seen previous source; the
implementer must not read previous implementations, tests, diffs, history,
registry code or sibling code. Only this original specification, supplied
interface declarations, repository policy and manifests are implementation inputs.
Record actual files read. Parent owns integration and qualification.

Preserve these public interfaces, with implementations designed independently:

```rust
pub trait Memory {
    fn size(&self) -> u64;
    fn grow(&self, pages: u64) -> i64;
    fn read(&self, offset: u64, bytes: &mut [u8]);
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize);
    fn write(&self, offset: u64, bytes: &[u8]);
}
pub type VectorMemory = std::rc::Rc<std::cell::RefCell<Vec<u8>>>;
// Native: public type DefaultMemoryImpl = VectorMemory.
// Wasm: public DefaultMemoryImpl, with Clone + Copy + Default, implementing Memory.
```

Page size is `crate::WASM_PAGE_SIZE_BYTES` (65,536 bytes). Grow returns the
previous page count; refusal returns -1 with bytes and extent unchanged. Successful
growth preserves old bytes and adds zero-filled pages. Zero-page growth is a
no-op. Check representability and allocation refusal. RefCell<Vec<u8>> itself
implements Memory; native aliases share physical bytes. If an externally supplied
Vec is not page-aligned, never panic from unchecked growth subtraction or expose
its trailing partial page as advertised capacity; explicitly refusing growth is
acceptable. There is no shrink operation.

Safe reads/writes admit the entire logical range before accessing it, including
arithmetic overflow. Empty access at the end is allowed; empty access beyond the
end is refused. Rejected native range access must not partially copy/write.
Backings supplied by callers remain responsible for their trait implementation.
Implementing Memory is infrastructure capability, not allocation authority.

The raw-read contract accepts an exclusive writable destination of `count` bytes
valid for a Rust slice and nonoverlapping with the source/backing state. The
caller must satisfy pointer validity/alignment/exclusivity, including zero length.
Every byte is initialized on successful return. Failure may initialize a prefix,
but must not publish uninitialized bytes as ordinary u8 values. Provide a default
implementation so a custom Memory need only implement the four safe methods.
Do not assume the custom safe read accepts uninitialized u8 references. Choose
its implementation independently and document its safety. Preserve overrides
through forwarding implementations for `Rc<M>` and `&M`, including unsized M.
Native direct reads and the IC implementation should avoid unnecessary temporary
buffers. Scope unsafe lint expectations to the required operations.

Use the existing IC Platform Runtime `ic0` boundary on Wasm only. Available
signatures are:

```rust
stable64_size() -> u64
stable64_grow(pages: u64) -> u64 // failure is u64::MAX
stable64_read(dst: &mut [u8], offset: u64)
stable64_read_uninit(dst: &mut [std::mem::MaybeUninit<u8>], offset: u64)
stable64_write(src: &[u8], offset: u64)
```

These are safe functions under `ic_platform_runtime::ic0`; the uninit read
initializes its complete output or traps. Do not inspect its implementation or
add dependencies. The native build must not import Platform. Public successful
behavior, native backing identity representation, and persisted collection or
allocation formats remain unchanged by this component.

Write only `crates/ic-storage/src/memory/mod.rs` (overwrite without reading old
contents) and a new `crates/ic-storage/src/memory/contract_tests.rs`. Use ordinary
module discovery for the new tests; do not read previous tests. New tests should
cover growth, boundaries, initializing reads, fallback and forwarding behavior.
Do not run Cargo, formatter or Git operations; parent owns those effects and the
preserved old-test disposition. No old implementation remains in production.

## Implementation record — 2026-11-10

The fresh implementation agent read this specification, AGENTS.md, TOKO-TOOLING.md,
root/package Cargo.toml, rules/contributions.md, rules/engineering-details.md and
docs/principles/rust-code-hygiene.md. It read only its own new source thereafter;
no prior source, tests, history, registry or sibling implementation was supplied.
The source-exposed parent owns integration and qualification. This records the
information boundary, not a legal certification or whole-repository clearance.

The complete Memory module is replaced. The raw fallback uses a 1,024-byte
initialized stack buffer; native and Wasm implementations override it with direct
initializing reads. Forwarding preserves overrides for unsized Rc/reference
backings. Native capacity arithmetic and reservation are checked; trailing partial
pages are not exposed and positive growth refuses them unchanged. Wasm uses only
Platform's existing safe ic0 API, including MaybeUninit reads.

Eleven independently written tests replace the former Memory tests. They cover
boundaries, unchanged refusal, zero fill, partial pages, raw initialization,
chunked fallback and unsized forwarding. The native and Wasm Clippy gates pass.
Installed and benchmark evidence is recorded with the combined
[backing replacement](backing-replacement.md), since both components affect IO.

A measured refinement removed redundant size queries from nonempty Wasm IO.
The first refinement incorrectly relied on empty syscalls to check their offset;
installed qualification caught a successful invalid access. The retained version
explicitly checks empty ranges and returns, while nonempty requests rely on the
IC syscall's full-range trap. Installed tests now exercise all three direct
backing delegates at the end, beyond the end and through arithmetic overflow,
separately from governed bounds, and check rollback of preceding valid writes.
The implementer received this behavioral failure evidence only and changed its
own source; the parent did not provide the inspected Platform implementation.
