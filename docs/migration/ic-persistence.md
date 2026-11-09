# IC Persistence consolidation

This is a repository/workspace migration, not an API or persistence redesign.
The integration checkout is `/tmp/ic-persistence-integration`, branch
`integration/ic-persistence`. Neither original checkout is renamed or replaced.

## Preserved inputs

Inventory observed on 2026-10-20:

| Input | Exact committed revision | Published version | Working tree |
| --- | --- | --- | --- |
| Memory | `c345aad56d49ee70d81519bf25b90079102b2886` | 0.49.4 | clean, main equals origin/main |
| Storage | `8e57c5a322bef3e4a6ea5ef6ca7f38e18dbdf580` | 0.5.9 | clean, main equals origin/main |

Crates.io reported both versions non-yanked. Checksums were
`bcd29857791e3bc4a722ca5a0854238853af8e9b6a35eccfff1c9f13d84e69ae`
(Memory) and `cf2025d091e52dd9f66af450a16d9f985b2c8dd892bbb196eda2bc97a9332241`
(Storage). These are registry observations, not new publication receipts.

Both original repositories' refs, including local branches and tags, are retained
under `refs/preservation/memory/` and `refs/preservation/storage/`. Backup bundles
are in `/tmp/ic-persistence-preservation/`; `combined-with-pr-heads.bundle` also
preserves the fetched open PR heads. Copy these to durable maintainer storage
before removing temporary directories. Ignored build artifacts, old release
receipts and tool installations remain in the original checkouts. The saved
release-state inventory found 78 Memory plans and 20 Storage plans, all marked
complete, with no active release lock in either checkout. They are retained in
place and are not reinterpreted as combined-workspace receipts.

Storage's seven additional worktrees were clean. Their retained branch heads are:

| Branch | Head |
| --- | --- |
| `cleanup/benchmark-allocations` | `bbd07f5fa0da9c9dc64d5de3b86266f0a930a783` |
| `canic/bounded-cell` | `32b584de7af0b0159ba4aeb43245a80557536ea5` |
| `canic/stable-map-clear` | `0dfed834600d0a83f92270f2182665e81fc06b50` |
| `api/narrow-codec-traits` | `77d77c11926160eefcef4f29fd6514e93a78639f` |
| `perf/reuse-decoded-deletion-nodes` | `af85bfeeba1ceda9c719f1bdc43eaa6af49fa2b1` |
| `cleanup/reclaim-validated-nodes` | `f3c8fd4be385238a2396394bbe23dfda9a2fdb16` |
| `maintenance/rust-1.99.1` | `c46f4617160b6376cd4902806a014b36492d39e0` |

Some are older private-tree experiments or superseded maintenance; preservation
is not approval to merge them into today's governed adapters. No unpublished
branch was deleted, reset or silently selected as the new product baseline.

Open PR heads are separately retained under `refs/preservation/{memory,storage}/pr/N`:

- [Memory #125](https://github.com/dragginzgame/ic-memory/pull/125),
  `8e36c805b7af6f9afe83a2714dbecc468dd522f1`: Shared evidence delegation.
  Released main already calls the canonical collector; retain the head for review.
- [Memory #126](https://github.com/dragginzgame/ic-memory/pull/126),
  `764b1276eebcdb66b8ea1beb30f87061ec6a3a0a`: Build observation command boundary.
  Released main has since adopted Build 0.24/Host 0.32 and the command-spec API.
  Do not merge its older dependency selections over current main.
- [Storage #18](https://github.com/dragginzgame/ic-storage/pull/18), draft,
  `65e769e9b1d148fec0fbf2b55b97ad5bff24572a`: Shared 0.25.9 release retry protection.
  Its intent is incorporated by canonical snapshot adoption, not by copying a
  modified vendored runner. No PR was closed or otherwise changed remotely.

## Migration map and boundaries

| Previous owner/path | Combined destination | Disposition |
| --- | --- | --- |
| Memory Git history and root tooling | repository root | preserved as first-parent history |
| Storage Git history | import merge's second parent | original commit identities retained; initial tree under `imports/ic-storage` |
| Storage `crates/ic-storage` | `crates/ic-storage` | same library source, tests, README and license |
| Memory `crates/ic-memory` | unchanged | same runtime, wire fixtures, examples, tests and budgets |
| Two workspace roots/catalogs/locks | root `Cargo.toml` / `Cargo.lock` | Storage's Memory registry edge becomes path-plus-version; other root selections retained |
| Storage root release/CI adapters | Memory root adapters and CI | two-package receipts/publication and combined native/Wasm/MSRV gates |
| Two Shared snapshots | root `.shared-tooling.snapshot` | committed Shared 0.25.9, canonical takeover/export; dirty sibling source excluded |
| Storage private-tree research | `docs/archive/private-btree` | preserved historical research, no retired implementation revived |
| Storage changelog and handoff | `docs/history/ic-storage` | original bytes retained as evidence; not current instructions |
| Storage ownership guide | `docs/storage/upstreams.md` | current collection ownership within this repository |
| Memory installed runtime runner | `qualification` | independent host graph and lock retained |
| Archive consumer | `testing/package-consumer.rs` plus generated temporary workspace | verifies published archive contents, single Memory identity, lease and reopen behavior |

The Storage root Cargo files and Makefile are archived as evidence; they are not
active workspaces or commands. No live symlinks or sibling-source build dependencies
are introduced. Storage keeps its prior Rust/Clippy lint policy. Both package
licenses retain their original copyright notices and MIT terms.

## Version and public identity

The initial import preserved Memory **0.49.4** and Storage **0.5.9**. The completed
**0.50.0** release converted Storage to workspace inheritance and aligned both
local lock records and the internal Memory requirement. Subsequent releases
advance the common workspace version. The temporary inheritance exception and
initial conversion code have been retired; the strict shared inheritance gate
now applies directly. Original plans, source history and receipts remain retained.

The repository move does not rename crates/imports, alter public signatures or
change durable bytes, allocation keys or lease custody. The later 0.50 release
changes Cargo's pre-1.0 dependency identity: consumers using both crates must
select Memory 0.50 and Storage 0.50 together. A Memory 0.49 lease is a different
Rust type from a Memory 0.50 lease even though the API implementation is unchanged.
This is dependency alignment, not a data migration. No downstream files are edited.

Path/Git consumers must also select both packages from the same checkout/source.
Mixing a path Storage with registry Memory can produce two Memory identities even
at an equal version. The published manifests strip internal paths, so ordinary
registry consumers use the coordinated registry versions. Independent archive
qualification deliberately patches both registry names to the extracted archives
and checks the shared lease type; it does not claim a live registry publication.

## Cutover checklist

1. Review the history import separately from integration/configuration commits.
   Preserve its merge ancestry: fast-forward or merge the integration branch;
   do not squash the import or rebase away Storage's second parent.
   Preserve the bundles and outstanding branch/PR disposition before any cleanup.
2. Review hosted Linux/macOS ARM/macOS Intel CI after separately authorizing a
   branch push. Local Linux evidence does not qualify native macOS or hosted CI.
3. The maintainer selected a new public `dragginzgame/ic-persistence` repository
   on 2026-10-20 instead of renaming Memory. The destination has been created
   with collaborator-only PR creation and the description “Allocation governance
   and governed stable collections for Internet Computer canisters; publishes
   ic-memory and ic-storage.” Publish the consolidated history to its `main`;
   retain historical evidence through the retirement records below.
4. The consolidated checkout is now `/home/adam/projects/ic-persistence`.
   The maintainer subsequently authorized retirement of both original checkouts
   and GitHub repositories. Both local checkouts were removed after verified
   backup. All 121 issues now belong to IC Persistence; the
   [issue map](../history/issue-transfers.json) and
   [comment map](../history/comment-transfers.json) preserve their references.
   All 23 PRs have [complete exports](../history/pull-requests/README.md).
   Backups are retained at
   `/home/adam/projects/ic-persistence-retirement-2026-10-20/`.
   GitHub deletion remains pending because the CLI token lacks `delete_repo`.
5. The initial 0.50.0 coordinated release is complete. Subsequent explicitly
   authorized releases verify both receipts and registry checksums, upload
   Memory before Storage, and resume only checksum-matching versions. Keep
   saved plans, logs and qualified archives when publication fails.
6. Coordinate downstream dependency alignment and branch protection separately.
   The three formerly open PRs are superseded by the consolidated implementation;
   their original review histories and states remain in the PR exports.
