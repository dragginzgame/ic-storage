# IC Storage New

A fresh experiment in **Rust Orthogonal Persistence**, inspired by Motoko's
persistence model on the Internet Computer.

The objective is to keep application state available across canister upgrades
without application-authored save/load routines. This repository currently
contains an unpublished Rust library scaffold and a research architecture, not an
implemented persistence system.

The existing `../ic-storage` repository remains separate and unchanged. This
experiment does not replace its packages or admit its stored data formats.

## Development

Use the pinned Rust 1.100.0 toolchain (also the support floor). The library has
no runtime dependencies. The root selects Toko Tooling 0.37.2 exactly as a
development dependency, matching the reviewed baseline revision in AGENTS.md.
Its authenticated snapshot is installed through the canonical exporter.

Explicit setup and offline admission are separate:

```sh
make install-tools
make tools-check
```

Common tools are selected through `.tools/host/bin` and
`.tools/rust/common/bin`; Make sets PATH automatically. Add those directories
to PATH for interactive use. Compiler setup remains explicit; checks never
download a compiler or dependencies.

Focused development commands:

```sh
make check
make fmt-check
make deps-tree
make deps-boundaries
make toko-tooling-check
```

`make deps-check` aggregates snapshot admission, declaration/pinning checks and
dependency boundaries. `make deps-pins` requires Cargo.lock to be tracked by Git.
`deps-boundaries` checks
direct IC dependency policy and excludes tooling from the normal/build graph
across all targets. Neither check proves installed IC behavior.

`make deps-update` explicitly permits network access and updates Cargo.lock
within root requirements. Review that diff. To change Toko versions, edit the
exact root pin, update Cargo.lock, then run `make toko-tooling-update` followed
by `make toko-tooling-check`; builds never install snapshots implicitly.

Library code lives in `crates/`. See [qualification/](qualification/README.md)
for evidence scope and future fixture placement. No CI, installed canister
qualification, publication or deployment adapter is configured. The package
remains `publish = false`. The GitHub repository is
[`dragginzgame/ic-storage-new`](https://github.com/dragginzgame/ic-storage-new).

Read [AGENTS.md](AGENTS.md) and the
[research architecture](docs/DESIGN.md) before implementation.
