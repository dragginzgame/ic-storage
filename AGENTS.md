# Repository instructions

This repository implements named stable-memory allocations, an owned Cell and an
owned multi-entry B-tree in one `ic-storage` crate. The 25 November 2026 hard cut
retains the merged allocation design and selects the independent Storage engines.
OP and a separate `ic-memory` package remain retired.

Read `docs/DESIGN.md` before implementation. Keep 4,096 named allocations by
default and canonical string keys bounded to 128 bytes. Preserve namespace grants,
exclusive backing custody, non-cloneable allocation leases and refusal of unknown
or corrupt data. Applications own schemas and explicitly authorize migrations.
Do not modify the legacy sibling repository or reset consumer storage.

This repository adopts `TOKO-TOOLING.md` and its revision-bound references.
The canonical exporter owns Toko 0.46.4 under `vendor/toko-tooling/` and
`.toko-tooling.snapshot`. Use Storage-owned Memory and Storable contracts. Keep `ic-stable-structures` out of product and normal canister dependency graphs.
The qualification matrix may use the released crate as a dev-dependency. Stable-memory
system calls continue through `ic-platform-runtime`.

Keep Rust 1.100.0 as pinned compiler and support floor. Check for active Cargo,
rustc and rustdoc processes before editing or compiling. Use focused, locked checks and leave Cargo network mode to the caller.
Setup is explicit. Keep Rust library code under `crates/` and
installed tests in the independent `qualification/` workspace. Use `foo/mod.rs`
when a module has children. Preserve source notices and historical evidence.

Publish only `ic-storage` when explicitly requested. `ic-storage-derive`, M0,
and the former OP qualification packages are no longer active products.
Do not commit, push, publish or deploy without the corresponding user request.
