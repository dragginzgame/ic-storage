# Platform runtime composition qualification

This unpublished consumer reproduces the dependency boundary in Storage #152:
local candidate Storage plus published `ic-platform` with its `runtime` feature.
It belongs to the existing independent qualification workspace and is excluded
from its default member. It neither replaces the native PocketIC driver nor adds
the facade to production Storage.

`make check-dependency-adoption` admits its native and Wasm graphs, requiring one
Platform, Runtime, Types and executor guard, then compiles both with Clippy. Cargo
also enforces the guard's exact internal requirement and singleton `links` identity.
The caller fixtures check that a duplicate guard or compilation failure refuses.

Run the composed native create/reopen check explicitly:

```sh
cargo test --locked --offline --manifest-path qualification/Cargo.toml \
  -p ic-storage-platform-qualification
```

The same public helper compiles the deployed `DefaultMemoryImpl` path on Wasm.
Installed IO, bounds/growth refusal, upgrades and rollback remain the separate
Storage runtime suite; compilation alone does not qualify those behaviors.
The sibling `ic-storage-default-qualification` consumer selects only default
Storage. The same gate observes and compiles it separately on native and Wasm,
forbidding Runtime on native and execution/Types/Candid on Wasm. This prevents
runtime-enabled composition from masking missing features in the IC0-only backing.
These fixtures test the working candidate; registry-only verification remains a
separate post-publication proof.
