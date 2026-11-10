# Current ownership ledger qualification

## 0.56.0 Wire hard cut

The current implementation replaces both CBOR layers with bounded IC Wire native
records. The native and Wasm development checks are recorded in the handoff.
Linux installed IO, upgrade/rollback, Storage, custody and Logging checks now
pass for original Wasm; [the installed qualification record](runtime-io-qualification.md#complexity-cleanup-installed-qualification--2026-10-30)
binds the exact working-tree source, incoming lock, tools and retained logs.
Earlier measurements and installed checks below retain their original sources
and byte formats. They do not qualify the Wire cut. No CBOR reader or automatic
reset is added; the [current contract](design/current-ledger.md) owns retained-data
obligations. Optimized variants, macOS and full release checks were not rerun.

## Earlier qualification history

Preserved source-bound reports, benchmarks and retired-format qualification are
in [the historical ledger qualification record](history/ledger-qualification.md).
They retain their original inputs and limitations; current proof belongs to the
Wire record above. No logs, archived fixtures or recovery artifacts were removed.
