# Troubleshooting Storage

Preserve backing bytes when investigating failures. Never replace governance
alone to make bootstrap succeed. The current format is an intentional hard cut;
old manager/collection images require an explicitly chosen fresh/reset installation.

| Failure | Action |
| --- | --- |
| `NotBootstrapped` | Bootstrap the host-owned runtime before opening stores. |
| `NotAdmitted` | Check the stable key, current namespace grant and explicit historical selection. |
| `AlreadyOpen` | Drop the existing collection/lease before reopening that allocation. |
| `BackingAlreadyClaimed` | Retain one runtime; detached leases also retain custody. |
| `BindingMismatch` | Reuse the exact host declarations, grants and policy identity for warm verification. |
| Admission rejection | Correct duplicate keys, overlapping namespaces or the host policy. No new authority was published. |
| `CapacityExceeded` | Check allocation, extent, physical-page and Wire budgets, including accumulated directory metadata. |
| `ArithmeticOverflow` | Correct the requested page count. |
| `BackingRefused` | Resolve actual backing capacity; the committed allocation did not grow. |
| `Poisoned` / invalid directory | Drop every owner and preserve bytes before reopening under the current format. Never bypass validation. |

Native partial writes are not transactions and can prevent reopening. Checksums
provide corruption detection, not repair. There is no fallback snapshot or automatic
reset. Installed update/upgrade traps rely on IC message rollback.

For diagnosis, retain the exact typed error, package versions, declarations,
namespace grants, `StorageConfig` and a protected backing copy. When a valid
runtime is available, `memory_allocations` returns checked name/ID/page facts.
There is no old fixed-slot doctor or bucket report. Hosts own application lifecycle
and schemas; allocation success does not prove application-data compatibility.
See [operations](operations.md), [allocation](../../crates/ic-storage/ALLOCATION.md)
and [safety](../../SAFETY.md).
