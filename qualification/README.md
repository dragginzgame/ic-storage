# Qualification

This directory is reserved for repository-owned qualification documentation and
fixtures when an implemented persistence contract requires them. It is not a
Cargo package and contains no qualification claims.

Keep Rust tests beside their owning implementation under `crates/`. Add real
qualification packages under `crates/` only when they have executable work, with
an explicit private fixture role and root-owned dependencies.

Native models do not establish installed IC upgrade or rollback behavior.
Future qualification must distinguish native checks from installed canister
evidence and cover rejection of unknown or incompatible retained state. Keep raw
execution output under `target/evidence/`; retain compact conclusions here.
