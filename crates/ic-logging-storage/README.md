# IC Logging Storage

See [Storage naming adoption](../../docs/migration/storage-naming.md) for the repository and adapter rename.

Optional bounded checkpoints for IC Logging, following Metrics Storage's explicit
load/save model. The package consumes Logging
0.25's validated `LogSnapshot` restoration contract.

IC Observability owns the engine, snapshot admission, append, retention, UTF-8
truncation and cursor scans. IC Storage owns durable encoding and governed
Storage integration. Neither adapter depends on the other. Hosts retain clocks,
allocation declarations, stable identity, authorization, lifecycle and cadence.
No endpoint, recorder, timer or automatic checkpoint is installed.

Upgrading from the retired allocator and collection formats requires hosts to
use fresh/reset whole backing and rebuild against Storage-owned memory and codec
types. Old manager/map images have no decoder or automatic reset. Current Storage-owned images and discriminator-3 logging rows remain compatible
with the Logging 0.25 adoption; it requires no reset or migration.

## Host-owned storage

Use a map opened with an admitted `ic-storage::RuntimeMemory` handle. Choose
`StableBTreeMap::create(handle)` for known fresh storage, or `load(handle)` for known
retained storage. Never initialize an unknown/corrupt map as a recovery fallback.
`LoggingStore::new(map)` and `into_map()` transfer that same allocation lease;
drop every owner before reopening. Collection geometry, corruption/backing traps,
allocation and growth retain Storage's contracts.

```rust
use std::num::NonZeroUsize;
use ic_logging::{EntryInput, Level, Limits, Log};
use ic_logging_storage::{LogIdentity, LogKey, LoggingStore};
use ic_storage::{GenericAllocationPolicy, AllocationGrants, MemoryAuthority,
    MemoryRequest, MemoryRuntime, SealedDeclarationSnapshot, VectorMemory};
use ic_storage::adapter::StableBTreeMap;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let pool = AllocationGrants::new(vec![MemoryAuthority::new("app", "app.")?])?;
let declarations = SealedDeclarationSnapshot::new(vec![MemoryRequest::new("app", "app.logs.v1")?])?;
let mut runtime = MemoryRuntime::new(VectorMemory::default())?;
runtime.bootstrap(&declarations, &pool, &GenericAllocationPolicy)?;
let store_key = ic_storage::StableKey::parse("app.logs.v1")?;
let mut store = LoggingStore::new(StableBTreeMap::create(runtime.open_memory(&store_key)?)?);
let key = LogKey::new("app", "worker")?;
let identity = LogIdentity { epoch: 0 };
let limits = Limits { max_entries: NonZeroUsize::new(128).unwrap(),
    max_message_bytes: 192, max_label_bytes: 32, max_text_bytes: 32_768 };
let mut log = store.load(&key, identity, limits)?.unwrap_or_else(|| Log::new(limits));
log.append(EntryInput { created_at_secs: 10, level: Level::Info,
    source: "worker", topic: Some("delivery"), message: "intent recorded" })
    .map_err(|error| format!("append refused: {error:?}"))?;
store.save(&key, identity, &log)?;
drop(store);
let reopened = LoggingStore::new(StableBTreeMap::load(runtime.open_memory(&store_key)?)?);
let restored = reopened.load(&key, identity, limits)?.unwrap();
assert_eq!(restored.snapshot(), log.snapshot());
# Ok(())
# }
```

Only `None` is absence. Malformed, wrong-generation or wrong-configuration rows
are typed refusals. A stored empty history still retains both append watermarks;
it never becomes absence. Sequence exhaustion remains exhausted after reopen.
The host supplies the same epoch across ordinary upgrades and owns generation
identity; schema validity cannot establish authenticity or instance continuity.

## Bounds and IC Wire checkpoint contract

Namespaces/names contain 1..=64 / 1..=128 ASCII bytes from letters, digits,
`._/-:`. Each key encodes namespace, NUL, name. The host bounds key cardinality
and keeps authority outside these labels. Each log admits at most 1,024 configured
entries, 4,096 message bytes, 128 bytes per source/topic label and a configured
combined retained-text ceiling of at most 65,536 UTF-8 bytes. This text ceiling
counts source, optional topic and message bytes, excluding metadata and heap
overhead; it is not a complete heap cap. The complete
encoded record is at most 65,536 bytes; not every combination of maximum settings
fits that byte ceiling. Oversized saves fail before map mutation. Saving uses the caller's borrowed checkpoint without copying retained history
or collecting encoded row views. Comparison borrows the decoded prior log.
Checkpoint work still scales with retained history, unlike cursor scans.

The format uses `ICL` plus discriminator `3`, followed by IC Wire's unframed
Serde/Postcard payload. The private schema carries epoch, four fixed limits (including combined retained text),
optional next sequence and timestamp, then oldest-first entries. Each entry
carries sequence, timestamp, severity, truncation flag, source, optional topic and
message. The schema's severity order is Debug/Info/Ok/Warn/Error. IC Wire owns
primitive representation, UTF-8 and trailing-byte refusal; Storage does not
maintain another primitive encoder or parser.

IC Wire admits at most 1,024 decoded rows and applies explicit byte, node,
depth and allocation-traversal allowances. The payload allowance excludes the
four-byte format marker. The 4 MiB traversal allowance is conservative accounting,
not measured heap use; node/depth allowances are 65,600 and 16. Decoded text
borrows the input until configured label/message and combined-text limits pass, before constructing
owned entries for canonical `Log::restore`. Codec resource failures are typed.
Unknown formats never select another reader or a reset fallback.

## Upgrade disposition

The maintainer selected a hard cut for this adoption. Published discriminator-2
Logging checkpoints and earlier experimental formats are retired. Discriminator 3
never infers a missing text budget or selects an old reader. Load, save and normal
remove refuse old rows; none silently resets storage. Hosts must explicitly retire
old Logging rows through their governed map administration (`into_map`) or supply
a fresh allocation before saving current checkpoints. No deployed data is removed
by adopting the source. Keep allocation governance intact during a component reset.

Metrics Storage has its own IC Wire format and explicit
[old-data reset requirement](../ic-metrics-storage/README.md#upgrade-disposition).

Save verifies the old row and rejects decreasing/exhaustion-clearing watermarks,
rewritten issued rows and resurrection of pruned rows. An unchanged next sequence
cannot invent a new timestamp watermark. Identical checkpoints skip stable writes.
Replacing the same row reuses Storage's collection allocation; pruning never
accumulates a second archival row and never removes the watermark checkpoint.

The host must load current state before changes and establish its own identity.
These consistency checks do not authenticate a checkpoint or detect every reset
followed by enough new appends. Explicit `remove` admits the old identity/limits;
for a deliberate reset, save with a new epoch in the same update without an await.
Corrupt records require explicit host map administration; there is no silent reset.

Checkpoint in successful update execution. Await boundaries, IC trap rollback,
query non-persistence and whole-application upgrade acceptance require installed
consumer qualification. Native `VectorMemory` tests prove codec/storage behaviour,
not IC effects or instruction/cycle costs. Diagnostic history never authorizes
retrying uncertain paid effects or proves that a remote effect completed.

The [installed qualification fixture](../../qualification/ic-storage-test-fixtures/tests/fixtures/logging_qualification/main.rs)
and its [host runner](../../qualification/README.md) exercise
query non-persistence, update/upgrade rollback, successful reopen, watermarks after pruning all rows, combined-text eviction
and refusal, and forged retained-text admission. Run `make test-runtime-logging` with prepared dependencies
and server. This checks the supplied host's original Wasm, not application
acceptance, optimized variants or cost measurements.

Logging Storage owns the independent `logging-storage` release group. Its version
and [release ledger](../../releases/logging-storage/CHANGELOG.md) are separate from Memory/Storage and Metrics
Stable. Use the compatible core requirements in its package metadata; see
[group commands](../../RELEASING.md#release-groups). Grouping changes no checkpoint
format or host retirement obligation.

Collection failures propagate as `StoreError::Storage`. Hosts must handle fallible
map construction and iteration under the current Storage API.
