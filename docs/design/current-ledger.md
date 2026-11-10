# Current allocation contract

Storage's named allocation interface uses the independently implemented STORNAM1
version-1 backing. The maintainer authorized fresh/reset backing for this
replacement on 2026-11-10. The former
snapshot directory, fixed-slot manager/ledger and prior collection images have
no compatibility readers. A nonempty incompatible backing is rejected without
writes; hosts must explicitly retire/reset the complete old backing or supply
fresh backing. No automatic migration, reset or fallback is provided.

See the maintained [allocation guide](../../crates/ic-storage/ALLOCATION.md) for
namespace admission, durable controls, resource limits, custody and failure
behavior. The [backing design](dynamic-allocation.md) records geometry and
qualification requirements. Historical allocation documents describe retired
formats and are not the current upgrade contract.
