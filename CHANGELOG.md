# Changelog

## [0.78.1] - 2026-11-18

- Use the `ic-platform 0.34.16` stable-structures feature for collection engines and memory traits.
- Remove two unused Toko tooling helpers and their supporting policy files (#7).

## [0.78.0] - 2026-11-17

### ⚠️ Breaking changes

- The allocation directory now uses a Storage-owned binary layout and rejects
  0.77.0 backing. Start with fresh backing or export using the previous owner.
- Replace the Wire limits in `StorageConfig` with `directory_bytes`.

## [0.77.0] - 2026-11-17

### ⚠️ Breaking changes

- Replace the OP experiment with a single stable-memory `ic-storage` library.
- Restore merged named allocation governance from 0.64: 4,096 default allocations,
  128-byte canonical names, namespace grants and exclusive allocation leases.
- Use upstream ic-stable-structures 0.7.2 collection engines through checked leases.
- Remove active M0/derive/retained-heap packages and their qualification machinery.
- Use new directory identities; earlier OP and owned-collection storage requires
  explicit migration or fresh backing. No implicit reset or compatibility reader.

Earlier release notes remain in Git history and the local pre-replacement archive.

### ✅ Testing

- Qualify host collection reopen, allocation limits and ownership contracts.
- Qualify installed normal upgrades, failed-upgrade rollback, query discard,
  deletion and trapped multi-collection writes with Testkit 0.110.0.
