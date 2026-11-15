# Repository instructions

This is a fresh research repository for Rust Orthogonal Persistence inspired by
Motoko. The user authorized this directory on 2026-11-15 and put development of
`../ic-storage` on hold. Do not modify that repository as part of this experiment.

Use the [reviewed Toko engineering baseline](https://github.com/dragginzgame/toko-tooling/blob/626da7cd9626cc2e152594862a2d82ea82ef51da/TOKO-TOOLING.md)
and its revision-bound references. The canonical exporter owns the installed
Toko 0.37.2 snapshot recorded in `.toko-tooling.snapshot`; do not edit its files
or receipt manually. This installation does not establish native host or product
qualification. Do not copy Storage's release machinery.

## Product scope

- Read the [research architecture](docs/DESIGN.md) before implementation.
- Independently author the implementation from published behavior and interface
  specifications. Do not copy, translate or adapt Motoko implementation code.
- Do not claim arbitrary Rust values are upgrade-safe. Layout, references,
  allocator state, executable pointers and upgrade initialization need explicit
  contracts and evidence.
- Unknown or incompatible retained data must be rejected, never silently reset.
- Native models do not prove installed IC upgrades or rollback behavior.
- Keep this experimental package unpublished. Existing Storage package names,
  stored formats and release history do not belong to this experiment.
- Production stable-memory integration, if selected later, must use governed
  Storage handles. A retained main-memory design needs its own admission and
  custody contract before production integration.

## Development

Check for active Cargo, rustc and rustdoc processes before editing or compiling.
Keep Rust 1.100.0 as the pinned compiler and support floor. The virtual root owns
shared metadata and future dependency selections. Library code belongs under
`crates/`; keep tests beside the owning implementation. Do not create empty
qualification packages or speculative APIs.

Use focused locked offline checks. The bootstrap commands are documented in
README.md. Do not fetch dependencies implicitly or install tools during checks.
Do not commit, push, publish or configure deployment without the corresponding
user request. Preserve unrelated work and historical evidence.
