# Qualification

This directory is reserved for repository-owned qualification documentation and
fixtures when an implemented persistence contract requires them. It is not a
Cargo package and contains no qualification claims.

Keep ordinary Rust tests beside their owning implementation under `crates/`.
When real cross-package qualification needs executable packages, create an
independent workspace here with its own lockfile, metadata and dependency
catalog. Keep those packages unpublished and outside product release groups;
qualification may depend on product crates, never the reverse. Do not create
an empty qualification workspace before that work exists.

Native models do not establish installed IC upgrade or rollback behavior.
Future qualification must distinguish native checks from installed canister
evidence and cover rejection of unknown or incompatible retained state. Keep raw
execution output under `target/evidence/`; retain compact conclusions here.
