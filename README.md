# IC Storage

One crate for named stable-memory allocations and upstream
`ic-stable-structures` collections on the Internet Computer.

- Use names such as `app.users.v1` instead of assigning numeric memory IDs.
- Default limits: **4,096 named allocations**, **128 bytes per name**.
- Namespace grants and exclusive leases keep independently owned stores apart.
- BTreeMap, BTreeSet, Cell, Vec, MinHeap and Log use upstream engines and codecs.
- Ordinary stable-memory persistence on stable Rust 1.100.0.

See the [crate API and example](crates/ic-storage/README.md),
[design](docs/DESIGN.md) and [allocation contract](docs/ALLOCATION.md).
Version 0.77.0 replaces the experimental OP API and stored
formats. No consumer migration or reset is performed by this repository change.

## Development

Explicit setup: `make install-tools`, then `cargo fetch --locked` and
`cargo fetch --manifest-path qualification/Cargo.toml --locked`.
Common tools live under `.tools/host/bin` and `.tools/rust/common/bin`;
`make tools-check` checks their prepared identities offline.

| Command | Scope |
| --- | --- |
| `make test` | Allocation and upstream collection integration, ownership compile failures and documentation examples |
| `make lint` | Product host targets and Wasm library |
| `make fmt-check` | Product and qualification formatting |
| `make storage-setup` | Explicit installed-test server preparation |
| `make storage-installed-test` | Actual IC upgrade, query isolation and trapped-write rollback |
| `make ci` | Aggregate formatting, dependencies, host tests, docs and host/Wasm lint |

Shared setup/formatting/tool identity mechanics belong to Toko Tooling. Product
commands select the library and qualification workspace; CI calls the same commands.
Checks do not install compilers or fetch dependencies.

The OP source was removed from the active workspace. Its Git history is intact;
the pre-replacement worktree and history backup remain locally under
`target/op-archive/2026-11-17/`, and the original source trees under `.op-backup/`.
These local backups include uncommitted work and are not required to build.
