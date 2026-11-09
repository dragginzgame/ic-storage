# IC Storage

See [Storage naming adoption](docs/migration/storage-naming.md) for the repository and adapter rename.

Allocation governance and governed stable collections for Internet Computer
canisters, maintained in one repository with three packages in three release groups.

| Crate | Responsibility | Guide |
| --- | --- | --- |
| `ic-storage` | Allocation governance, exclusive leases and collections | [Storage](crates/ic-storage/README.md) |
| `ic-metrics-storage` | Validated metric snapshots in governed Storage maps | [Metrics Storage](crates/ic-metrics-storage/README.md) |
| `ic-logging-storage` | Bounded logging checkpoints in governed Storage maps | [Logging Storage](crates/ic-logging-storage/README.md) |

Storage owns named allocation, exclusive leases and Cell, Map, Set, Vector and
map regions. There is no production `ic-stable-structures` dependency. Applications retain
lifecycle and recovery policy. This replacement is a complete format hard cut;
hosts must explicitly reset backing or install fresh storage. The stable metric adapter
owns its V1 records here; allocation-free arithmetic remains in IC Metrics.
See [ownership](docs/ownership.md) and [safety](SAFETY.md).

The root Cargo workspace owns all three packages, one dependency catalog and one
lockfile, with Rust 1.99.1 as the minimum. Run Make and Cargo commands here.
The independent [installed runtime runner](qualification/README.md)
and [map comparison benchmark](qualification/ic-storage-benchmarks/README.md)
retain a separate lockfile. Archive consumer qualification uses isolated,
generated workspaces so a workspace path dependency cannot mask missing package
contents or a second Storage identity.

[Setup](docs/host-support.md) · [Releasing](RELEASING.md) ·
[Documentation](docs/README.md) · [Migration map](docs/migration/ic-persistence.md) ·
[Current handoff](docs/status/current.md) · [Contributing](rules/contributions.md)

Release ownership: Storage owns `storage`; Metrics
Stable and Logging Storage release independently. See [group commands](RELEASING.md#release-groups),
[Metrics Storage notes](releases/metrics-storage/CHANGELOG.md) and
[Logging Storage notes](releases/logging-storage/CHANGELOG.md).
