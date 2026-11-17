//! Prepared-artifact gate and the single supplier-owned canister engine.
use super::{Backing, Error, Geometry, Grant, Read, Root, Runtime, Write};
use core::cell::RefCell;
std::thread_local! { static ENGINE: RefCell<Option<Runtime>> = const {RefCell::new(None)}; }
#[expect(
    unsafe_code,
    reason = "mandatory import resolved only by the supplier's artifact preparation"
)]
#[link(wasm_import_module = "ic_storage_retained_v1")]
unsafe extern "C" {
    fn admitted() -> i32;
}
#[expect(
    unsafe_code,
    reason = "invoke the artifact gate before touching generation-local Rust state"
)]
fn gate() -> Result<(), Error> {
    // SAFETY: ordinary raw Wasm has an unresolved non-IC import and cannot be
    // installed. retained_build resolves it only alongside mandatory transient
    // startup reset and admitted linker geometry; no caller boolean bypass exists.
    if unsafe { admitted() } != 0x535401 {
        return Err(Error::Startup);
    }
    Ok(())
}
/// Single retained canister runtime. Build with the documented linker geometry,
/// prepare using `retained_build::prepare`, create only in init, close in pre-upgrade
/// and reopen in post-upgrade. Missing source close refuses before target issuance.
/// Application service/callback quiescence and logical recovery policy stay with
/// the application; this API never admits arbitrary Global-owned Rust values.
pub struct Canister;
impl Canister {
    /// Initialize a genuinely fresh native directory and issue its session.
    ///
    /// # Errors
    /// Refuses an existing engine/directory, invalid declarations or unsupported geometry.
    pub fn create(geometry: Geometry, roots: &[Root]) -> Result<(), Error> {
        gate()?;
        ENGINE.with(|slot| {
            let mut slot = slot.try_borrow_mut().map_err(|_| Error::Busy)?;
            if slot.is_some() {
                return Err(Error::State);
            }
            *slot = Some(Runtime::create(Backing::wasm(geometry)?, roots)?);
            Ok(())
        })
    }
    /// Reopen retained roots after complete source close, under exact declarations.
    /// No failed admission creates, repairs or resets existing native bytes.
    ///
    /// # Errors
    /// Refuses active owners, missing/corrupt/incompatible data or changed geometry.
    pub fn reopen(geometry: Geometry, roots: &[Root]) -> Result<(), Error> {
        gate()?;
        super::declarations(roots)?;
        ENGINE.with(|slot| {
            let mut slot = slot.try_borrow_mut().map_err(|_| Error::Busy)?;
            if let Some(engine) = slot.as_ref() {
                if engine.backing.geometry != geometry {
                    return Err(Error::Geometry);
                }
                return engine.reopen(roots);
            }
            let runtime = Runtime {
                backing: Backing::wasm(geometry)?,
                roots: roots.to_vec(),
                state: RefCell::new(None),
            };
            runtime.reopen(roots)?;
            *slot = Some(runtime);
            Ok(())
        })
    }
    fn with<R>(operation: impl FnOnce(&Runtime) -> Result<R, Error>) -> Result<R, Error> {
        gate()?;
        ENGINE.with(|slot| {
            let slot = slot.try_borrow().map_err(|_| Error::Busy)?;
            operation(slot.as_ref().ok_or(Error::State)?)
        })
    }
    /// Issue one generation-scoped participant token.
    ///
    /// # Errors
    /// Refuses unavailable, unknown or already claimed identities.
    pub fn grant(participant: u64) -> Result<Grant, Error> {
        Self::with(|r| r.grant(participant))
    }
    /// Read granted roots synchronously; references/iterators cannot escape.
    ///
    /// # Errors
    /// Refuses unavailable/busy state and stale, foreign or duplicate grants.
    pub fn read<R>(
        grants: &[&Grant],
        operation: impl for<'a> FnOnce(Read<'a>) -> R,
    ) -> Result<R, Error> {
        Self::with(|r| r.read(grants, operation))
    }
    /// Write granted roots in the enclosing IC message. Normal Err retains prior
    /// writes; a caller needing rollback must trap the enclosing message.
    ///
    /// # Errors
    /// Refuses unavailable/busy state and invalid participant grants.
    pub fn update<R>(
        grants: &[&Grant],
        operation: impl for<'a> FnOnce(Write<'a>) -> R,
    ) -> Result<R, Error> {
        Self::with(|r| r.update(grants, operation))
    }
    /// Revoke tokens and settle all native owners before durable close publication.
    /// Must run in pre-upgrade. A trap rolls back code, records and custody together.
    ///
    /// # Errors
    /// Refuses active callbacks, closed state or an unsettled/poisoned owner.
    pub fn close() -> Result<(), Error> {
        Self::with(Runtime::close)
    }
}
