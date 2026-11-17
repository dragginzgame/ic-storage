# Repository instructions

This is a fresh research repository for Rust Orthogonal Persistence inspired by
Motoko. The user authorized this directory on 2026-11-15 and put development of
the [legacy Storage repository](https://github.com/dragginzgame/ic-storage-legacy)
on hold. Do not modify that repository as part of this experiment. This research
repository was renamed from `ic-storage-new` to `ic-storage` on 2026-11-16.
The public native package is `ic-storage`; qualification aliases the
frozen published oracle as `ic-storage-legacy`. Preserve M0's historical digest
domain bytes despite the package rename.

This repository adopts [TOKO-TOOLING.md](TOKO-TOOLING.md) and its revision-bound
references. The canonical exporter owns the Toko 0.44.11 snapshot under
`vendor/toko-tooling/` and `.toko-tooling.snapshot`. Product scope and the commands
below are consumer-owned choices within that baseline; no policy exceptions are
claimed. Installation alone does not establish native host or product
qualification. Do not copy Storage's release machinery.

## Product scope

- Develop this repository as the intended successor to `ic-storage`, aiming to
  subsume the storage capabilities our applications need, including those served
  by ic-stable-structures. Do not maintain two competing product architectures.
  Replacement requires capability and migration qualification; it does not imply
  current completeness, API compatibility or stored-format compatibility.
  Consumer cutovers require explicit authorization and evidence for their data,
  lifecycle and operational contracts.
- Read the [research architecture](docs/DESIGN.md) before implementation.
- Follow its selected native retained-collection direction with explicit schema
  conversions and a portable migration/recovery path for incompatible runtimes.
  Preserve M0 as a byte-defined evidence baseline; do not treat it as the new
  native ABI or continue a parallel M0 collection architecture. Initially support
  one explicit runtime contract: qualify the representations actually retained,
  pointer validity, scoped ownership and their lifecycle. Use planned portable
  migration for incompatible runtimes; universal layout certificates, automatic
  compatibility discovery and actual cross-compiler migration are not prerequisites
  for records and application converters within the supported contract. Each new
  stored type still needs its representation and tested portable path. Follow the
  design's explicit transitional checkpoint provider boundary without modifying
  the frozen sibling repository.
- Treat its [pinned cdk-rsx-store reference](docs/DESIGN.md#pinned-cdk-rsx-store-design-basis)
  as an additional design basis; preserve the recorded review scope and pin.
- Independently author the implementation from published behavior and interface
  specifications. Do not copy, translate or adapt Motoko implementation code.
- Do not claim arbitrary Rust values are upgrade-safe. Layout, references,
  allocator state, executable pointers and upgrade initialization need explicit
  contracts and evidence.
- Unknown or incompatible retained data must be rejected, never silently reset.
- Native models do not prove installed IC upgrades or rollback behavior.
- Publish only `ic-storage` and `ic-storage-derive`, as authorized on 2026-11-17.
  Keep `ic-storage-m0` and qualification packages private. Publication does not
  establish production readiness or adopt legacy APIs and stored formats.
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
