# IC Storage

One crate for named stable-memory allocations with an owned B-tree and Cell on the Internet Computer.

- Use names such as `app.users.v1` instead of assigning numeric memory IDs.
- Default limits: **4,096 named allocations**, **128 bytes per name**.
- Namespace grants and exclusive leases keep independently owned stores apart.
- The B-tree and Cell use Storage-owned memory and record interfaces.
- Ordinary stable-memory persistence on stable Rust 1.100.0.

See the [crate API and example](crates/ic-storage/README.md),
[design](docs/DESIGN.md) and [allocation contract](docs/ALLOCATION.md).
Version 0.78 uses a Storage-owned directory format. Use fresh backing or
explicitly export older data with its previous owner before migration.

## Development

Explicit setup: `make install-tools`, then `cargo fetch --locked` and
`cargo fetch --manifest-path qualification/Cargo.toml --locked`.
Common tools live under `.tools/host/bin` and `.tools/rust/common/bin`;
`make tools-check` checks their prepared identities offline.

| Command | Scope |
| --- | --- |
| `make test` | Allocation and owned collection integration, ownership compile failures and documentation examples |
| `make lint` | Product host targets and Wasm library |
| `make fmt-check` | Product and qualification formatting |
| `make storage-setup` | Explicit installed-test server preparation |
| `make storage-installed-test` | Actual IC upgrade, query isolation and trapped-write rollback |
| `make ci` | Aggregate formatting, dependencies, host tests, docs and host/Wasm lint |

Shared setup/formatting/tool identity mechanics belong to Toko Tooling. Product
commands select the library and qualification workspace; CI calls the same commands.
Cargo checks use the lockfile and inherit the caller’s network settings. With dependencies cached,
run `CARGO_NET_OFFLINE=true make ci` to opt into offline verification.

## Publication

For an authorized release from a clean release commit, run `make publish` once.
It runs product lint, dependency and formatting checks, host tests, docs,
qualification lint, and the installed upgrade and rollback test before Cargo
verifies the archive and uploads `ic-storage` to crates.io. Prepare the installed
test server with `make storage-setup` if needed. Use `make ci` for development
and CI; running it immediately before publication repeats those checks.

For an optional preflight, `make publish-check` runs product host and Wasm lint
and Cargo's registry dry run. A later upload always runs its full qualification.
Both commands record attempt evidence under `target/publication/`.
