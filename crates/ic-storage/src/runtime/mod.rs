//! Named allocation admission and exclusive leases over the owned directory.
mod admission;
mod custody;
mod directory_model;

use crate::{Memory, StableKey};
pub use admission::{
    AdmissionError, AllocationGrants, BootstrapAdmission, GenericAllocationPolicy, MemoryAuthority,
    MemoryRequest, PolicyIdentity, RuntimeBootstrapPolicy, SealedDeclarationSnapshot,
};
pub use custody::{BackingIdentity, MemoryBacking};
pub use directory_model::Error as DirectoryError;
use directory_model::leases::{Lease, Owner};
pub use directory_model::{AllocationId, StorageConfig};
use std::collections::BTreeSet;

/// Construction refuses aliased or malformed backing without resetting it.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeConstructionError {
    /// Another runtime or detached lease retains custody.
    #[error("backing already claimed")]
    BackingAlreadyClaimed,
    /// The current owned directory could not be loaded or initialized.
    #[error(transparent)]
    Directory(#[from] DirectoryError),
}

/// Allocation growth failures before authority publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeGrowError {
    /// Requested page arithmetic overflowed.
    #[error("allocation page count overflow")]
    ArithmeticOverflow,
    /// The configured allocation or directory resource budget was exceeded.
    #[error("allocation capacity exceeded")]
    CapacityExceeded,
    /// Physical backing capacity was refused.
    #[error("backing refused growth by {additional_pages} pages")]
    BackingRefused { additional_pages: u64 },
    /// A previous backing failure poisoned this owner.
    #[error("allocation owner is poisoned")]
    Poisoned,
    /// Directory validation failed at a checked boundary.
    #[error("allocation directory validation failed")]
    InvalidDirectory,
}

/// A key cannot be opened until current host admission succeeds.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeOpenError {
    /// Bootstrap has not committed current authority.
    #[error("runtime is not bootstrapped")]
    NotBootstrapped,
    /// The key is absent from the current admitted declarations.
    #[error("key is not admitted: {0}")]
    NotAdmitted(StableKey),
    /// An exclusive lease for this allocation is already alive.
    #[error("allocation {id} already open")]
    AlreadyOpen { id: u64 },
    /// The directory refused opening.
    #[error(transparent)]
    Directory(#[from] DirectoryError),
}

/// Bootstrap failed before current authority could be published.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeBootstrapError<E> {
    /// Host declarations or namespace grants were invalid.
    #[error(transparent)]
    Admission(#[from] AdmissionError),
    /// The host's policy rejected the preparation or an allocation.
    #[error("host policy rejected allocation")]
    Policy(E),
    /// Current bootstrap inputs differ from the established binding.
    #[error("bootstrap binding mismatch")]
    BindingMismatch,
    /// A shared verification cannot bootstrap an uninitialized owner.
    #[error("runtime is not bootstrapped")]
    NotBootstrapped,
    /// Durable directory commitment failed.
    #[error(transparent)]
    Directory(#[from] DirectoryError),
}

struct Binding {
    source: SealedDeclarationSnapshot,
    grants: AllocationGrants,
    policy: PolicyIdentity,
    admitted: BTreeSet<StableKey>,
}

///
/// `MemoryRuntime`
///
/// One custodian of named allocations on a backing. Bootstrap validates namespace
/// grants and host policy, commits bindings, then permits exclusive leases.
/// Omitted retained names keep their allocation, but require current admission
/// before opening. IDs are opaque u64 values, never selected by applications.
///
pub struct MemoryRuntime<M: Memory> {
    owner: Owner<M>,
    config: StorageConfig,
    binding: Option<Binding>,
}

impl<M: Memory> MemoryRuntime<M> {
    /// Claim backing and create or read its owned directory. Never reset nonempty backing.
    ///
    /// # Errors
    /// Rejects duplicate backing custody or invalid/non-admissible retained directory data.
    pub fn new(memory: M) -> Result<Self, RuntimeConstructionError>
    where
        M: MemoryBacking,
    {
        Self::new_with_config(memory, StorageConfig::default())
    }
    /// Use explicit directory resource limits; reopening revalidates under those limits.
    ///
    /// # Errors
    /// Rejects duplicate backing custody, invalid configuration, or invalid/non-admissible retained directory data.
    pub fn new_with_config(
        memory: M,
        config: StorageConfig,
    ) -> Result<Self, RuntimeConstructionError>
    where
        M: MemoryBacking,
    {
        let owner = Owner::init(memory, config).map_err(|error| match error {
            DirectoryError::AlreadyClaimed => RuntimeConstructionError::BackingAlreadyClaimed,
            error => RuntimeConstructionError::Directory(error),
        })?;
        Ok(Self {
            owner,
            config,
            binding: None,
        })
    }
    /// Whether current host authority has been committed.
    #[must_use]
    pub const fn is_bootstrapped(&self) -> bool {
        self.binding.is_some()
    }
    /// Current durable directory generation, after bootstrap admission.
    ///
    /// # Errors
    /// Returns `NotBootstrapped` until bootstrap establishes a binding.
    pub fn committed_generation(&self) -> Result<u64, RuntimeOpenError> {
        if self.binding.is_none() {
            return Err(RuntimeOpenError::NotBootstrapped);
        }
        Ok(self.owner.generation())
    }
    /// Effective resource admission limits.
    #[must_use]
    pub const fn config(&self) -> StorageConfig {
        self.config
    }
    /// Admit a complete declaration set, retaining omitted historical bindings.
    ///
    /// # Errors
    /// Rejects invalid grants or declarations, host-policy refusal, conflicting bindings, and directory validation or commitment failures.
    pub fn bootstrap<P: RuntimeBootstrapPolicy>(
        &mut self,
        source: &SealedDeclarationSnapshot,
        grants: &AllocationGrants,
        policy: &P,
    ) -> Result<u64, RuntimeBootstrapError<P::Error>> {
        if self.binding.is_some() {
            return self.verify_bootstrap(source, grants, policy);
        }
        self.owner.verify()?;
        let policy_identity = policy.runtime_bootstrap_identity();
        let bindings = self.owner.bindings();
        let mut admission =
            BootstrapAdmission::new(source, grants, &bindings, self.config.allocations)?;
        let result = policy.prepare_bootstrap(&mut admission);
        let (requests, admitted) = admission.complete()?;
        result.map_err(RuntimeBootstrapError::Policy)?;
        for request in requests {
            policy
                .validate_allocation(request.stable_key())
                .map_err(RuntimeBootstrapError::Policy)?;
        }
        self.owner.bind(&admitted)?;
        self.binding = Some(Binding {
            source: source.clone(),
            grants: grants.clone(),
            policy: policy_identity,
            admitted,
        });
        Ok(self.owner.generation())
    }
    /// Compare the complete established binding without IO or policy callbacks.
    ///
    /// # Errors
    /// Returns `NotBootstrapped` or `BindingMismatch` when the supplied composition differs from the established binding.
    pub fn verify_bootstrap<P: RuntimeBootstrapPolicy>(
        &self,
        source: &SealedDeclarationSnapshot,
        grants: &AllocationGrants,
        policy: &P,
    ) -> Result<u64, RuntimeBootstrapError<P::Error>> {
        let binding = self
            .binding
            .as_ref()
            .ok_or(RuntimeBootstrapError::NotBootstrapped)?;
        if binding.source != *source
            || binding.grants != *grants
            || binding.policy != policy.runtime_bootstrap_identity()
        {
            return Err(RuntimeBootstrapError::BindingMismatch);
        }
        Ok(self.owner.generation())
    }
    /// Open one currently admitted name, preserving custody through detached handles.
    /// Uses the live admitted directory without backing IO. To check backing
    /// integrity explicitly, use [`Self::diagnostics`] or [`Self::memory_allocations`].
    ///
    /// # Panics
    /// Propagates a broken internal binding invariant.
    ///
    /// # Errors
    /// Rejects unbootstrapped or unadmitted access, an existing lease, and a poisoned owner.
    pub fn open_memory(&self, key: &StableKey) -> Result<RuntimeMemory<M>, RuntimeOpenError> {
        let binding = self
            .binding
            .as_ref()
            .ok_or(RuntimeOpenError::NotBootstrapped)?;
        if !binding.admitted.contains(key) {
            return Err(RuntimeOpenError::NotAdmitted(key.clone()));
        }
        let lease = self.owner.lease(key).map_err(|error| match error {
            DirectoryError::AlreadyOpen => RuntimeOpenError::AlreadyOpen {
                id: self.allocation_id(key).unwrap().get(),
            },
            error => RuntimeOpenError::Directory(error),
        })?;
        Ok(RuntimeMemory { lease })
    }
    /// Verify component requirements against current host admission without IO.
    ///
    /// # Errors
    /// Rejects unbootstrapped access or requirements not admitted under the requested authority.
    pub fn verify_authority(
        &self,
        requirements: &SealedDeclarationSnapshot,
        authority: &str,
    ) -> Result<(), RuntimeOpenError> {
        let binding = self
            .binding
            .as_ref()
            .ok_or(RuntimeOpenError::NotBootstrapped)?;
        for request in requirements.requests() {
            if request.authority() != authority
                || !binding.admitted.contains(request.stable_key())
                || binding.grants.check(request).is_err()
            {
                return Err(RuntimeOpenError::NotAdmitted(request.stable_key().clone()));
            }
        }
        Ok(())
    }
    /// Resolve a retained identity for diagnostics; this grants no access.
    #[must_use]
    pub fn allocation_id(&self, key: &StableKey) -> Option<AllocationId> {
        self.owner.allocation_id(key)
    }
    /// Observe checked directory entries and actual logical extents.
    ///
    /// # Errors
    /// Returns a directory error when ownership is poisoned or the retained directory fails validation.
    pub fn memory_allocations(&self) -> Result<Vec<MemoryAllocation>, DirectoryError> {
        self.owner.verify()?;
        Ok(self
            .owner
            .bindings()
            .into_iter()
            .map(|(stable_key, id, pages)| MemoryAllocation {
                stable_key,
                id,
                pages,
            })
            .collect())
    }

    /// Validate the current directory and report physical capacity and custody.
    /// Available before bootstrap; collecting diagnostics never admits or opens a key.
    /// Page totals describe reserved capacity, not application payload occupancy.
    ///
    /// # Errors
    /// Rejects a poisoned owner or backing that no longer matches its directory.
    pub fn diagnostics(&self) -> Result<StorageDiagnostics, DirectoryError> {
        let mut report = self.owner.diagnostics()?;
        report.admitted_allocations = self.binding.as_ref().map_or(0, |b| b.admitted.len());
        Ok(report)
    }
}

///
/// `StorageDiagnostics`
///
/// Read-only totals from the validated current directory and physical backing.
/// The protected root occupies one additional page: data, metadata, unassigned
/// and root pages sum to physical pages. Unassigned pages are not reclaimable
/// through this API. Diagnostics grant no allocation authority.
///

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct StorageDiagnostics {
    /// Committed directory generation.
    pub generation: u64,
    /// Retained named allocations, including currently omitted names.
    pub allocations: usize,
    /// Names admitted by this runtime's bootstrap, or zero before bootstrap.
    pub admitted_allocations: usize,
    /// Currently live exclusive allocation leases.
    pub live_leases: usize,
    /// Total allocated application buckets.
    pub extents: usize,
    /// Logical data capacity across retained allocations, in 64 KiB pages.
    pub data_pages: u64,
    /// Reserved allocation controls excluding the protected root page, in 64 KiB pages.
    pub metadata_pages: u64,
    /// Used root, name/size rows and bucket-owner bytes.
    pub directory_bytes: u64,
    /// Actual physical backing extent, in 64 KiB pages.
    pub physical_pages: u64,
    /// Reserved bucket slack outside logical data, controls and the root page.
    pub unassigned_pages: u64,
}

///
/// `MemoryAllocation`
///
/// Passive diagnostic entry; no lease or allocation authority is carried.
///
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MemoryAllocation {
    /// Durable name.
    pub stable_key: StableKey,
    /// Opaque identity.
    pub id: AllocationId,
    /// Logical extent in Wasm pages.
    pub pages: u64,
}

///
/// `RuntimeMemory`
///
/// Exclusive non-cloneable allocation lease. IO trusts custody between checked
/// boundaries; growth commits the directory before publishing its new extent.
/// A detached lease retains the backing claim until it is dropped.
///
pub struct RuntimeMemory<M: Memory> {
    lease: Lease<M>,
}
impl<M: Memory> RuntimeMemory<M> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn poison_on_unwind(&self) -> directory_model::leases::PanicGuard<M> {
        self.lease.poison_on_unwind()
    }
    /// Logical page extent.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.lease.size()
    }
    /// Reserve additional zero-filled logical pages without exposing backing.
    ///
    /// # Errors
    /// Rejects arithmetic overflow, resource limits, backing refusal, poisoned ownership, or invalid directory state.
    pub fn grow(&self, pages: u64) -> Result<u64, RuntimeGrowError> {
        self.lease.try_grow(pages).map_err(|error| match error {
            DirectoryError::ArithmeticOverflow => RuntimeGrowError::ArithmeticOverflow,
            DirectoryError::Growth => RuntimeGrowError::BackingRefused {
                additional_pages: pages,
            },
            DirectoryError::Budget => RuntimeGrowError::CapacityExceeded,
            DirectoryError::Poisoned => RuntimeGrowError::Poisoned,
            _ => RuntimeGrowError::InvalidDirectory,
        })
    }
    /// Read a completely bounds-checked logical range.
    pub fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.lease.read(offset, bytes);
    }
    /// Write a completely bounds-checked logical range.
    pub fn write(&self, offset: u64, bytes: &[u8]) {
        self.lease.write(offset, bytes);
    }
}
impl<M: Memory> Memory for RuntimeMemory<M> {
    fn size(&self) -> u64 {
        self.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        self.grow(pages)
            .map_or(-1, |previous| i64::try_from(previous).expect("page count"))
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.read(offset, bytes);
    }
    #[expect(
        unsafe_code,
        reason = "Forward the raw-read contract through the governed lease"
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        // SAFETY: forwarding preserves the caller's exclusive destination.
        unsafe { self.lease.read_unsafe(offset, dst, count) }
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        self.write(offset, bytes);
    }
}
