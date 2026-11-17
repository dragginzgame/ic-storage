# Changelog

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
