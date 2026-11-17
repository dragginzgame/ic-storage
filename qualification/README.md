# Installed stable storage qualification

Two private packages exercise the current public library:

- `storage-canister`: 4,096 named allocations; upstream map, set and cell over
  checked Storage leases. Ordinary init/post-upgrade rebuild transient owners.
- `storage-tests`: Testkit 0.110 installed tests for heap-replacing upgrades,
  failed post-upgrade rollback, query-write isolation, multi-root mutation and
  growth rollback, deletion and subsequent reuse.

Run `make storage-setup` explicitly when the selected server is not prepared,
then `make storage-installed-test`. Inputs build offline with the pinned compiler
and lockfile. Server evidence remains under `target/evidence/testkit/`.
This does not migrate or qualify a deployed consumer's application schema.
