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
no runtime dependencies. The root selects Toko Tooling 0.44.3 exactly as a
development dependency. Its authenticated snapshot is installed through the
canonical exporter under `vendor/toko-tooling/`; [TOKO-TOOLING.md](TOKO-TOOLING.md)
owns the common rules and links to immutable upstream references.

Explicit setup and offline admission are separate:

```sh
make install-tools
make tools-check
cargo fetch --locked
make install-hooks
```

Common tools are selected through `.tools/host/bin` and
`.tools/rust/common/bin`; Make sets PATH automatically. Add those directories
to PATH for interactive use. Compiler setup remains explicit; checks never
download a compiler or dependencies.

Focused development commands:

```sh
make check
make fmt-check
make lint
make doc-check
make deps-list
make deps-tree
make deps-boundaries
make toko-tooling-check
```

`make deps-check` aggregates snapshot admission, declaration/pinning checks and
dependency boundaries. `make deps-pins` requires Cargo.lock to be tracked by Git.
`deps-boundaries` checks
direct IC dependency policy and excludes tooling from the normal/build graph
across all targets. Neither check proves installed IC behavior.

`make deps-list` uses Toko's package inventory (local versions, workspaces and
release groups); `make deps-tree` shows external dependencies too.
`make deps-update` uses Toko's updater for the root and any future qualification
workspace. It explicitly permits network access and updates Cargo.lock
within root requirements. Use `DEPENDENCY_PACKAGE=name` to select one package,
with optional `DEPENDENCY_PRECISE=version`. Review the diff. To change Toko versions, edit the
exact root pin, update Cargo.lock, then run `make toko-tooling-update` followed
by `make toko-tooling-check`; builds never install snapshots implicitly.

`make ci` is the aggregate hosted gate: formatting, dependency policy, strict
Clippy and Rustdoc checks. GitHub Actions runs it on Linux and both macOS
architectures after explicit setup, with one separate Gitleaks scan per run.
No runtime tests exist yet; add focused tests with the implementation. Local
development uses the individual commands above. The canonical pre-commit hook
formats the selected commit payload without modifying unrelated work.

Library code lives in `crates/`. See [qualification/](qualification/README.md)
for evidence scope and future fixture placement. Installed canister
qualification, publication and deployment are not configured. The package
remains `publish = false`. The GitHub repository is
[`dragginzgame/ic-storage-new`](https://github.com/dragginzgame/ic-storage-new).

Read [AGENTS.md](AGENTS.md) and the
[research architecture](docs/DESIGN.md) before implementation.
