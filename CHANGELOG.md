# Changelog

## [0.79.0] - 2026-11-25

### 🚀 Features

- Batch exact-key deletion within loaded B-tree leaves. The published crate includes the optimized map engine. [#10 Support bounded bulk removal of sorted keys in the next Storage engine](https://github.com/dragginzgame/ic-storage/issues/10)

### 💥 Breaking changes

- The concrete iterator behind `adapter::MapIter` now belongs to the packaged engine. Callers that name the upstream iterator type should use `adapter::MapIter`. The retained map layout and public record codecs are compatible with 0.78.9.

## [0.78.9] - 2026-11-24

### 🚀 Features

- Add `StableBTreeMap::remove_many` for prepared sets of exact keys. It reports actual removals, including zero on retry. [#10 Support bounded bulk removal of sorted keys in the next Storage engine](https://github.com/dragginzgame/ic-storage/issues/10)

### 🎯 Qualification

- Measure point and batch instruction counts, Wasm growth and peak working heap for 126 and 1,022 physical deletions. Verify byte-identical rollback and successful retry.

### 🧪 Testing

- Adopt Toko Tooling 0.46.3 and select the CI setup script directly, retiring three unused action and cache files.

## [0.78.8] - 2026-11-20

- Adopt Toko Tooling 0.45.23 and respect the caller’s Cargo network mode in product and qualification checks (#9).

## [0.78.7] - 2026-11-19

- Adopt IC Platform 0.34.18, Platform Runtime 0.23.22 and Toko Tooling 0.45.21.
- Align workspace lints, storage type documentation and qualification endpoint budgets with the new rules.

## [0.78.6] - 2026-11-19

- Select the default physical backing at one boundary and verify independent volatile custody alongside IC stable memory.

- Adopt Toko Tooling 0.45.18 and group the qualification canister exports with their access policies and argument limits.

## [0.78.5] - 2026-11-19

- Adopt Toko Tooling 0.45.16 and share backing custody across host and Wasm builds.
- Compile the qualification canister without a module guard and run portable collection fixtures with explicit vector backing.

## [0.78.4] - 2026-11-19

- Adopt Toko Tooling 0.45.15 and its bounded CI diagnostic redaction.

### 🎯 Qualification

- Check the refusal reasons for namespace, allocation-limit and foreign-format failures alongside backing preservation.

## [0.78.3] - 2026-11-18

- Adopt Toko Tooling 0.45.8 and group host and Wasm backing implementations by target.

### 🎯 Qualification

- Check row and index counts and revision before exercising installed rollback.

## [0.78.2] - 2026-11-18

- Adopt Toko Tooling 0.45.6 in both workspaces.
- Run full release qualification during upload and keep the optional registry dry run lightweight.

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
