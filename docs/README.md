# IC Storage documentation

The documents in this directory serve different audiences. Use this index to
distinguish current integration guidance from historical engineering evidence.

## Start here

- [IC Storage overview](../README.md) and [migration map](migration/ic-persistence.md) describe the repository and its consolidation history.
- [Governed Storage collections](../crates/ic-storage/README.md) cover collection leases, regions and owned formats.
- [Logging Storage](../crates/ic-logging-storage/README.md) covers explicit bounded logging checkpoints and installed qualification.

- [Project overview and quick start](../crates/ic-storage/README.md) explains in plain language
  what Storage protects, when it is useful, and how to integrate it.
- [Advanced integration](../ADVANCED.md) covers runtime ownership, custom
  policies, host namespace grants, recovery, and manual bootstrap.
- [Safety invariants](../SAFETY.md) defines the properties that implementation
  changes must preserve.
- [Implementation ownership](design/ownership.md) identifies the runtime, collection,
  host-tooling and consumer integration boundaries.
- [Host support and qualification](operations/host-support.md) declares native prerequisites,
  supported macOS hosts, and qualification gaps.

## Current focused guides

- [Current ownership ledger](design/current-ledger.md) explains the current format,
  retained ownership, bounded metadata and deployment requirements.

- [Operations and diagnostics](operations/operations.md) explains allocation reports,
  resource budgets and growth failures.
- [Troubleshooting](operations/troubleshooting.md) maps common symptoms and typed errors
  to safe next actions without resetting durable state.
- [Named allocation and admission](../crates/ic-storage/ALLOCATION.md) defines
  placement, namespace grants, retained-name admission and resource limits.

When current guides disagree, the crate's public API and safety invariants are
authoritative. This project is pre-1.0 and maintains only its current API and
durable format.

The package version is declared in [Cargo.toml](../Cargo.toml); current guides
describe the maintained implementation without separate release stamps.
Historical qualification records retain the exact versions and source snapshots
they checked.

## Integration and qualification records

These documents record specific consumer source snapshots, environments, and
test results. They are evidence for the versions named in each document, not a
promise that a later consumer graph or deployment was requalified.

- [Issue reconciliation](issue-reconciliation.md)
- [Runtime IO and maintainer improvements qualification](runtime-io-qualification.md)
- [Bounded ledger codec qualification](ledger-codec-qualification.md)
- [Current ownership-ledger qualification](current-ledger-qualification.md)
- [Allocation diagnostics buffer qualification](allocation-diagnostics-qualification.md)
- [Shared release workflow qualification](release-workflow-qualification.md)
- [Canic checked-slot qualification for 0.25.0](consumer-qualification-0.25.0-canic.md)
- [Canic memory qualification for 0.24.6](consumer-qualification-0.24.6-canic.md)
- [IcyDB qualification for 0.24.4](consumer-qualification-0.24.4.md)
- [IcyDB qualification for 0.24.3](consumer-qualification-0.24.3.md)
- [Consumer qualification for 0.20.0](consumer-qualification-0.20.0.md)

## Historical design and measurement records

- [Key-only allocation qualification](key-only-recovery.md)
- [Recovered-metadata admission qualification](recovered-admission.md)

These reports explain why earlier changes were made and retain their original
measurements. Use the current guides above for present-day API instructions.

- [CANIC-162 physical allocation study](canic162-memory-attribution.md)
- [Opaque ledger payload encoding](opaque-ledger-payloads.md)
- [Logical bootstrap cleanup](logical-bootstrap-cleanup.md)
- [Wasm codec size audit](audits/wasm-codec-size-2026-07-14.md)
- [Recurring audit records](audits/recurring)

Current maintenance reviews use the [shared methods and Persistence overlay](audits/README.md).
Historical reports identify the methods and selected results they retain.

The [current Storage/ICSS comparison](audits/recurring/benchmark-full-catalogue-2026-11-05.md)
retains a compact selection covering all 565 cases. Superseded experiment exports
and prototype patches were retired in the authorized evidence cleanup; they are
not a permanent archive. New raw output belongs under `target/evidence/` and is
removable by explicit `cargo clean`. Small historical qualification tables and
consumer inventories remain with their existing owners; see the
[measurement index](measurements/README.md).

## Maintainers

- [Storage tooling adoption](adoption/storage-tooling.md) records the selected
  tooling, local integration boundaries and remaining blockers. Shared reference
  guides stay upstream at pinned revisions.
- [CI change selection](validation/ci-selection.md) and
  [release validation](validation/storage-release-validation.md) describe the
  consumer-owned qualification plans.
- [Detailed 0.29 release notes](changelog/0.29.md)
- [Detailed 0.28 release notes](changelog/0.28.md)
- [Local developer setup](https://github.com/dragginzgame/toko-tooling/blob/12459b6029115b830b8477879d3bea2f030b6e63/docs/adoption/local-setup.md) and [product host requirements](operations/host-support.md)
- [Detailed 0.27 release notes](changelog/0.27.md)
- [Detailed 0.26 release notes](changelog/0.26.md)
- [Detailed 0.25 release notes](changelog/0.25.md)
- [Release guide](../RELEASING.md)
- [Owned directory design](design/dynamic-allocation.md)

### Documentation release checklist

Before publishing a release that changes the API, durable format, terminology,
or workflow:

- update the crate version and README dependency example together;
- update examples, diagrams, captions, and alt text when terminology changes;
- run doctests with warnings denied;
- verify every local Markdown link and heading anchor;
- confirm current guidance does not retain a superseded pre-1.0 API or wire
  shape; and
- keep historical measurements and qualification claims labeled with their
  original version and scope.
