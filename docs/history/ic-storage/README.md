# IC Storage

IC Storage exposes governed stable collections through `ic_storage::adapter`.
All public collections require IC Memory runtime handles; memory extraction is
not exposed. Governed region views support retained trees sharing an allocation
with application frames. Collections retain the upstream stable structures' persisted formats.
Read the [package guide](crates/ic-storage/README.md) for the supported API and
allocation ownership boundary.

The independent B-tree and comparison benchmark have been removed. Their design
and performance investigation is retained as [historical research](docs/archive/private-btree/README.md).

## Development

The reviewed [Shared Tooling snapshot](.shared-tooling.snapshot) pins version
0.25.8 at commit `0a2d2ee3b5e0ae77755c2915ac28836d4d5ba359`.
Read [AGENTS.md](AGENTS.md) for local contributor instructions and
[the setup guide](docs/local-setup.md) for Linux/macOS prerequisites.
Rust 1.99.1 is the development toolchain. The Rust 2024 library lives in
`crates/ic-storage`, inherits root workspace metadata, and supports Rust 1.91.0.
See [upstream ownership](docs/upstreams.md) for dependency responsibilities.

```sh
make install-tools
make tools-check
make install-hooks
make help
```

When the shared Cargo tool build policy changes, rerun `make install-tools` and
`make install-release-tools` to prepare the new selection.
Prior installations remain retained; offline checks never repair selections.

Common setup prepares host and Cargo tools. IC executables are optional product
selections. Make selects the repository's tools automatically. For direct shell use:

```sh
export PATH="$PWD/.tools/host/bin:$PWD/.tools/rust/common/bin:$PATH"
```

Focused checks include `make shared-tooling-check`, `make check-doc-links`,
`make check-license`, `make check-pins` and `make check-storage-boundary`.
Rust checks are `make fmt-check`, `make check`, `make clippy`, `make docs-check`,
`make test`, `make wasm-check` and `make msrv`. Use the shared validation cadence
for development; `make ci` is the complete release gate. Linux and macOS are
configured in CI. Historical engine benchmarks do not qualify a future storage
implementation.

Authorized releases use `make release-patch`, `make release-minor` or
`make release-major`; `make release-resume VERSION=x.y.z` resumes the exact
recorded candidate. Authorized release preparation installs and checks the selected
published IC Build 0.18.0 metadata editor alongside common tools. Separate setup
uses `make install-release-tools`; `make release-tools-check` verifies it offline.
Read-only preflight checks never install tools. Release preparation supplies the
editor's absolute path and a 1 MiB per-document
input limit to Shared Tooling. This host tool is separate from library dependencies
and common development setup. Publication uses `make publish-check` and `make publish`
from clean source at an annotated release tag. See [the release contract](docs/releases.md).
Release validation runs the existing Clippy gate before builds, tests and packaging;
publication entrypoints also stop on lint failure before creating an attempt.

Historical licensing statement: “Licensed under MIT; see LICENSE.” This applies
to the archived revision identified in the [history index](index.md), not current
releases. For current terms, see the [root licence](../../../LICENSE.md).
