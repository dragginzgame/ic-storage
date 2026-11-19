//! Named allocation admission and exclusive leases over the owned directory.
mod admission;
mod custody;
mod directory_model;

use crate::{Memory, StableKey};
pub use admission::{
    AdmissionError, AllocationGrants, BootstrapAdmission, GenericAllocationPolicy, MemoryAuthority,
    MemoryRequest, PolicyIdentity, RuntimeBootstrapPolicy, SealedDeclarationSnapshot,
};
pub use custody::MemoryBacking;
use directory_model::leases::{Lease, Owner};
pub use directory_model::{AllocationId, Error as DirectoryError, StorageConfig};
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
    BackingRefused {
        /// Requested additional pages.
        additional_pages: u64,
    },
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
    AlreadyOpen {
        /// Allocation with an existing lease.
        id: u64,
    },
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
    pub fn new(memory: M) -> Result<Self, RuntimeConstructionError>
    where
        M: MemoryBacking,
    {
        Self::new_with_config(memory, StorageConfig::default())
    }

    /// Use explicit directory resource limits; reopening revalidates under those limits.
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
        let requests = admission.complete()?;
        result.map_err(RuntimeBootstrapError::Policy)?;
        for request in &requests {
            policy
                .validate_allocation(request.stable_key())
                .map_err(RuntimeBootstrapError::Policy)?;
        }
        let admitted: BTreeSet<_> = requests
            .into_iter()
            .map(|request| request.stable_key().clone())
            .collect();
        self.owner.bind(admitted.iter().cloned().collect())?;
        self.binding = Some(Binding {
            source: source.clone(),
            grants: grants.clone(),
            policy: policy_identity,
            admitted,
        });
        Ok(self.owner.generation())
    }

    /// Compare the complete established binding without IO or policy callbacks.
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
    ///
    /// # Panics
    /// Propagates backing failures or a broken internal binding invariant.
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
}

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

/// Exclusive non-cloneable allocation lease. IO trusts custody between checked
/// boundaries; growth commits the directory before publishing its new extent.
/// A detached lease retains the backing claim until it is dropped.
///
pub struct RuntimeMemory<M: Memory> {
    lease: Lease<M>,
}

impl<M: Memory> RuntimeMemory<M> {
    /// Logical page extent.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.lease.size()
    }

    /// Reserve additional zero-filled logical pages without exposing backing.
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

    fn write(&self, offset: u64, bytes: &[u8]) {
        self.write(offset, bytes);
    }
}
