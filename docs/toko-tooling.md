# toko-tooling

The actual source distribution of Dragginzgame development tooling: scripts,
Make interfaces, CI actions, catalogs, rules, licenses and guides. Its empty
Rust library is dependency-free and `no_std`, with no build script.

Declare the tooling release in repository-root Cargo metadata:

```toml
[workspace.metadata.toko-tooling]
version = "0.34.0"
```

Standalone root packages may use `[package.metadata.toko-tooling]` instead.
No application dependency or tooling-only workspace package is required.
`make shared-tooling-update` resolves this exact release from crates.io, writes
`toko-tooling.lock` with its archive checksum and installs the authenticated payload.
Commit the declaration, lock and snapshot receipt together. Application Cargo.lock
and its dependency graph are independent.

`make shared-tooling-install` reinstalls the locked payload. Offline
`make shared-tooling-check` validates declaration, lock and installed files;
common CI/release entrypoints require it. Installation preserves consumer edits,
retains recorded entrypoints and prepares/checks the selected tools.

Follow the [migration guide](https://github.com/dragginzgame/toko-tooling/blob/main/docs/consuming-snapshots.md#migration-from-cargo-dependencies)
to bootstrap from 0.33 and explicitly remove the former dependency declarations
and selection-only packages. Releases before 0.33 contain only a marker;
root metadata selection requires 0.34 or later.
