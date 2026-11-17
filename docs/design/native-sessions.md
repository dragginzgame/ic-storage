# Native generation ownership

Status: public safe API in the pending 0.76.2 work, with host and closed installed
ordered-fixture evidence. This advances R2 and
[Storage #5 — Canic retained runtime, records and composed ownership](https://github.com/dragginzgame/ic-storage/issues/5).
It does not establish general retained pointer admission or a consumer startup contract,
physical page budgets or the full supplier composition requirement.

## Bootstrap and custody

`ordered::session::Session<K, V, A, P>` consumes valid live ordered maps and their
explicit `Root` declarations. Each declaration binds a stable root ID, participant
ID, schema ID, entry limit and key/value byte bounds. Root IDs must be strictly
increasing and nonzero; participant/schema IDs must be nonzero. At most 64 roots
fit the current grant mask. These are native logical bounds; the selected
allocators still own physical allocation limits, custody and validity.

`Session::from_maps(maps, expected)` compares the complete declaration roster,
requires healthy maps with matching entry limits and checks existing records if
the declaration narrows the map's K/V bounds. Equal K/V bounds require no redundant
table scan. The returned `AdmissionError` preserves every input map on expected
refusal. There is no automatic initialization, schema conversion or trusted flag.

Inputs are actual Rust-owned maps created, recovered or admitted under their
original allocator authority. This constructor cannot authenticate a caller's
schema label against raw retained memory: bootstrap must obtain declarations
from its admitted custody/schema context. It accepts no raw address, saved Vec
header or unverified allocator descriptor. Application logical codecs remain
responsible for validating field semantics and relationships between roots.

## Grants and scopes

Bootstrap issues one non-cloneable `Participant` per declared participant identity.
The grant owns a shared lifetime reference to the native family and may outlive
the bootstrap handle. Dropping or forgetting it does not permit reissuance in
that generation. A grant's read/update scopes can access only its declared roots.
The coordinator can combine distinct grants from the same session; foreign,
duplicate and empty grant lists refuse before invoking user code.

Read scopes expose lookup, key-only membership, cardinality, ranges/prefixes,
lazy key/value-length preflight, allocation usage and explicit checkpoint export.
`Read::export_pages` binds the declared root identity to the bounded page codec;
authority refuses before invoking the sink. The sink owns placement and manifest
publication. Its errors are distinct from authority and collection refusal, and
external page writes do not gain transaction semantics from the read scope.
They do not mutate or re-admit native maps. Write scopes additionally insert,
replace, delete/discard and apply a preflighted single-root batch, enforcing
each root's byte bounds and its map's entry limit. An ordinary lookup seeks within its tree and never
validates the complete table. Views, record references and capturing futures
cannot escape the callback. Forgetting a view leaves the scope's borrow/owner
with Storage; it creates no outstanding ownership ticket.

All roots share one poison boundary. A host unwind during any write scope leaves
every participant unavailable. Ordinary callback errors preserve earlier writes
and do not automatically poison or roll back. An application detecting a partial
logical failure can explicitly quarantine the family; this is irreversible.
The profile workload uses this policy for native allocation/storage errors.
Installed IC rollback remains a separate lifecycle qualification requirement.

`into_maps` returns all native map owners only after every grant ends and the
family remains healthy. Otherwise it returns the unchanged session. The caller
must then release and settle each map before moving/reissuing backing or ending
allocator loans. Returning maps is not itself durable root publication or a
completed retained close. No ordinary-message checkpoint flush is introduced.

## Metadata lifetime and installed boundary

The session's Rc, RefCell, root directory and grants are transient Rust metadata
with generation-long lifetimes. They use Global allocation; native node and
payload allocations keep their supplied A/P authorities without replacement.
Metadata allocation can abort/trap on allocator exhaustion. It is not included
in native map allocation measurements or a physical budget claim.

An installed adapter must supply metadata storage valid across ordinary messages
and end all owners before controlled startup discards private headers. The closed
ordered fixture now reserves a separate 64-KiB metadata window at 1,638,400, below
its per-entry scratch window at 2,097,152. Only public session construction selects
that metadata allocator. Ordinary entry resets cannot reclaim it. Close destroys
all participants, Rc and root directory before resetting the metadata reservation,
then settles every native node/payload allocation and publishes descriptors last.

The current four-root fixture uses 456 metadata bytes and returns to zero on close
(the earlier one-root cohort used 144). Controlled
startup reinitializes allocator selectors and private static headers; it never
reads the old metadata carriers. An omitted source close refuses before target
ownership is issued. This is one source-reviewed fixed-memory adapter, not a
consumer-callable allocator/bootstrap API. DB must not copy its pool/reset code.

In the earlier one-root cohort, two artifact checks and all three installed suites passed in
`target/evidence/ordered-native/attempt.NMbqQt/`, with the log at
`target/ordered-public-session-installed-retry.log` and server evidence under
`target/evidence/testkit/run.PkZdSG/evidence`. They cover metadata allocation
refusal, eight repeated close/reopen cycles, scratch-heavy queries, late reply
encoding failure, query rollback, source/target upgrade failures, two-domain
recovery and corrupt/missing state refusal. See the
[ordered evidence](ordered-storage.md#public-session-installed-evidence--pending-0762)
for that cohort's artifacts and measured costs.

The [four-root installed extension](ordered-storage.md#four-root-installed-evidence--pending-0762)
now exercises main/source/target/job declarations through independent grants and
composed scopes, with 512-byte keys, 4-KiB rows and seventeen 16-KiB jobs. Native
format 4 seals all four descriptors; `ORDFAM04` recovery binds the complete root
roster and nested portable maps. Tests cover trap rollback between roots and
after reply encoding, metadata refusal, close/reopen, compatible upgrades,
recovery in both heap domains, declaration/nested-checkpoint refusal and target/job
cleanup followed by reuse. This fixture does not implement the application's
maintenance state machine or a reusable retained runtime. The subsequent
[paged installed extension](ordered-recovery.md#installed-four-root-page-transactions)
now adds one-page imports through the same grants and shared migration state
machine. Reusable bootstrap, incremental export and consumer integration remain open.

## Evidence

`make native-profile-test` selects six product session tests, three compile-fail
examples and the seven profile/allocation tests. Product checks cover independent
grants, composed writes, declaration mismatch, bounds, last grant-mask bit,
already-poisoned inputs, foreign/duplicate/reentrant access, partial errors,
forgotten views/grants, owner lifetime, explicit quarantine and live readmission.

The [profile workload](../../qualification/ic-storage-native-feasibility/profile.md)
now consumes this public owner for both roots. Its previous private poison flag
and direct map access are gone. Application identity/revision rules, semantic
admission, portable family framing and explicit conversion stay in qualification.
Its existing refusal, allocation reclamation, retry and recovery checks pass
through the same public grants. The full retained runtime, installed profile
lifecycle and real Blob/DB participants remain open.
