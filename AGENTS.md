# Repository instructions

This repository implements named stable-memory allocations and governed
`ic-stable-structures` collections in one `ic-storage` crate. On 17 November 2026
the user explicitly retired the OP experiment and selected the merged 0.64
allocation design with upstream collection engines. Do not resume OP, custom
collection engines or a separate `ic-memory` package without a new request.

Read `docs/DESIGN.md` before implementation. Keep 4,096 named allocations by
default and canonical string keys bounded to 128 bytes. Preserve namespace grants,
exclusive backing custody, non-cloneable allocation leases and refusal of unknown
or corrupt data. Applications own schemas and explicitly authorize migrations.
Do not modify the legacy sibling repository or reset consumer storage.

This repository adopts `TOKO-TOOLING.md` and its revision-bound references.
The canonical exporter owns Toko 0.45.0 under `vendor/toko-tooling/` and
`.toko-tooling.snapshot`. Access upstream collection engines and memory traits through
`ic_platform::stable_structures` with the Platform facade’s defaults disabled. Stable-memory
system calls continue through `ic-platform-runtime`.

Keep Rust 1.100.0 as pinned compiler and support floor. Check for active Cargo,
rustc and rustdoc processes before editing or compiling. Use focused locked
offline checks; setup is explicit. Keep Rust library code under `crates/` and
installed tests in the independent `qualification/` workspace. Use `foo/mod.rs`
when a module has children. Preserve source notices and historical evidence.

Publish only `ic-storage` when explicitly requested. `ic-storage-derive`, M0,
and the former OP qualification packages are no longer active products.
Do not commit, push, publish or deploy without the corresponding user request.
