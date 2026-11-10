# Operations and diagnostics

Use [Storage's allocation guide](../../crates/ic-storage/ALLOCATION.md) to construct
and bootstrap a runtime. `StorageConfig` owns allocation, extent, physical-page
and Wire resource ceilings. There is no bucket-size configuration or numeric
memory-ID pool.

## Allocation observations

`runtime.memory_allocations()` validates the directory and returns retained
`MemoryAllocation` rows: stable key, opaque ID and logical pages. This includes
omitted names, which require fresh current admission before opening. Rows are
passive observations and do not carry a lease. `allocation_id` resolves a retained
name without granting access. `committed_generation` observes the cached directory
generation after bootstrap.

Logical pages measure addressable capacity, not live payload bytes. Physical
backing also contains the root, append-only directory snapshots and possible
abandoned native tails. There is no bucket slack or 255-row fixed report.
`memory_allocation_summary`, old ledger exports and fixture doctor reports are
removed; hosts must update diagnostic projections to the new named rows.

## Capacity and generation

Defaults permit 4,096 names, 16,384 data extents and 1,048,576 physical Wasm pages.
IDs use `u64`; configured budgets and physical resources limit actual capacity.
Directory metadata is not reclaimed, so repeated growth consumes additional
physical capacity. Reopening the same names does not write metadata or advance
generation. New names and nonzero growth each commit a new generation.

`RuntimeMemory::grow` returns typed errors for arithmetic overflow, resource
limits, backing refusal, poisoning or directory invalidity. Through the owned
`Memory` trait, failures become `-1`. Zero-page growth reads the cached logical
extent and still refuses a poisoned owner.

## Failure handling

Do not retry through a poisoned owner. Drop the runtime and all handles before
reopening and validating the backing. Native partial writes are not rolled back;
a malformed root refuses without repair or fallback. Installed Wasm update and
upgrade traps rely on IC rollback. Foreign retained formats require the host's
explicit reset or fresh backing, never a partial governance reset.

Data IO checks the complete span before translating extents. Directory integrity
is checked at construction, bootstrap, lease opening and growth, not before each
read/write. Hosts must prevent raw backing edits throughout custody.
See [safety](../../SAFETY.md) and [advanced integration](../../ADVANCED.md).
