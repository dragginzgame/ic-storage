//! Canonical declarations, namespace grants and composable host admission.
use crate::{AllocationId, StableKey};
use std::collections::BTreeSet;

/// Invalid host declarations or grants; no allocation authority has been granted.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AdmissionError {
    /// Invalid owner label or namespace.
    #[error("invalid authority or namespace")]
    Invalid,
    /// Stable-key grammar failure.
    #[error(transparent)]
    Key(#[from] crate::StableKeyError),
    /// Duplicate or overlapping declaration/grant.
    #[error("duplicate key or overlapping namespace")]
    Duplicate,
    /// The host has not granted this owner the key's namespace.
    #[error("namespace is not granted to this authority")]
    Unauthorized,
    /// Historical selection is unknown or exceeds the configured count.
    #[error("unknown historical allocation or declaration budget exceeded")]
    Limit,
}
fn owner_valid(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= 128
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Host grant for one owner and a dot-terminated namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryAuthority {
    authority: String,
    prefix: String,
}
impl MemoryAuthority {
    /// Check owner and namespace syntax. Names do not select numeric IDs.
    ///
    /// # Errors
    /// Rejects invalid authority labels or namespace prefixes.
    pub fn new(
        authority: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Result<Self, AdmissionError> {
        let authority = authority.into();
        let prefix = prefix.into();
        if !owner_valid(&authority) || !crate::key::is_key_prefix(&prefix) {
            return Err(AdmissionError::Invalid);
        }
        Ok(Self { authority, prefix })
    }
}

/// Canonical, non-overlapping namespace grants for a host composition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationGrants(Vec<MemoryAuthority>);
impl AllocationGrants {
    /// Validate namespace overlap before runtime bootstrap.
    ///
    /// # Errors
    /// Rejects duplicate or overlapping namespace grants.
    pub fn new(mut grants: Vec<MemoryAuthority>) -> Result<Self, AdmissionError> {
        grants.sort_unstable_by(|a, b| a.prefix.cmp(&b.prefix));
        if grants
            .windows(2)
            .any(|pair| pair[1].prefix.starts_with(&pair[0].prefix))
        {
            return Err(AdmissionError::Duplicate);
        }
        Ok(Self(grants))
    }
    pub(super) fn check(&self, request: &MemoryRequest) -> Result<(), AdmissionError> {
        if self.0.iter().any(|grant| {
            grant.authority == request.authority && request.key.as_str().starts_with(&grant.prefix)
        }) {
            Ok(())
        } else {
            Err(AdmissionError::Unauthorized)
        }
    }
}

/// Checked request naming a store under one host-granted authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRequest {
    authority: String,
    key: StableKey,
}
impl MemoryRequest {
    /// Construct a key-only allocation request.
    ///
    /// # Errors
    /// Rejects invalid authority labels or stable-key syntax.
    pub fn new(authority: impl Into<String>, key: &str) -> Result<Self, AdmissionError> {
        let authority = authority.into();
        if !owner_valid(&authority) {
            return Err(AdmissionError::Invalid);
        }
        Ok(Self {
            authority,
            key: StableKey::parse(key)?,
        })
    }
    /// Declaring authority label.
    #[must_use]
    pub fn authority(&self) -> &str {
        &self.authority
    }
    /// Requested durable name.
    #[must_use]
    pub const fn stable_key(&self) -> &StableKey {
        &self.key
    }
}

/// Immutable canonical request set, with duplicate names rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedDeclarationSnapshot(Vec<MemoryRequest>);
impl SealedDeclarationSnapshot {
    /// Seal the complete host-composed requests.
    ///
    /// # Errors
    /// Rejects duplicate stable keys in the declaration set.
    pub fn new(mut requests: Vec<MemoryRequest>) -> Result<Self, AdmissionError> {
        requests.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        if requests.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(AdmissionError::Duplicate);
        }
        Ok(Self(requests))
    }
    /// Borrow checked requests.
    #[must_use]
    pub fn requests(&self) -> &[MemoryRequest] {
        &self.0
    }
}

/// Host-selected token binding the exact configured policy to a runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyIdentity([u8; 32]);
impl PolicyIdentity {
    /// Construct a complete policy identity.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Read-only recovered entries and explicit current historical admission.
pub struct BootstrapAdmission<'a> {
    grants: &'a AllocationGrants,
    recovered: &'a [(StableKey, AllocationId, u64)],
    selected: Vec<MemoryRequest>,
    keys: BTreeSet<StableKey>,
    maximum: usize,
    failure: Option<AdmissionError>,
}
impl<'a> BootstrapAdmission<'a> {
    pub(super) fn new(
        source: &SealedDeclarationSnapshot,
        grants: &'a AllocationGrants,
        recovered: &'a [(StableKey, AllocationId, u64)],
        maximum: usize,
    ) -> Result<Self, AdmissionError> {
        if source.0.len() > maximum {
            return Err(AdmissionError::Limit);
        }
        for request in &source.0 {
            grants.check(request)?;
        }
        Ok(Self {
            grants,
            recovered,
            selected: source.0.clone(),
            keys: source.0.iter().map(|request| request.key.clone()).collect(),
            maximum,
            failure: None,
        })
    }
    /// Retained names, identities and extents. This grants no memory access.
    #[must_use]
    pub const fn recovered_allocations(&self) -> &[(StableKey, AllocationId, u64)] {
        self.recovered
    }
    /// Whether this bootstrap has selected a key.
    #[must_use]
    pub fn is_declared(&self, key: &StableKey) -> bool {
        self.keys.contains(key)
    }
    /// Select a retained name under current grants. An ignored failure still rejects bootstrap.
    ///
    /// # Errors
    /// Rejects invalid or unauthorized names, missing retained entries, duplicates, capacity limits, or a previous admission failure.
    pub fn include_historical(&mut self, authority: &str, key: &str) -> Result<(), AdmissionError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.include(authority, key);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    fn include(&mut self, authority: &str, key: &str) -> Result<(), AdmissionError> {
        let request = MemoryRequest::new(authority, key)?;
        self.grants.check(&request)?;
        if self.selected.len() >= self.maximum
            || !self.recovered.iter().any(|(key, _, _)| key == &request.key)
        {
            return Err(AdmissionError::Limit);
        }
        if !self.keys.insert(request.key.clone()) {
            return Err(AdmissionError::Duplicate);
        }
        self.selected.push(request);
        Ok(())
    }
    pub(super) fn complete(
        self,
    ) -> Result<(Vec<MemoryRequest>, BTreeSet<StableKey>), AdmissionError> {
        if let Some(error) = self.failure {
            Err(error)
        } else {
            Ok((self.selected, self.keys))
        }
    }
}

/// Host-specific recovered-state and key admission, before durable commitment.
pub trait RuntimeBootstrapPolicy {
    /// Policy-specific rejection.
    type Error;
    /// Validate one admitted key; numeric placement is owned by Storage.
    ///
    /// # Errors
    /// Returns the host policy error when this key is rejected.
    fn validate_allocation(&self, key: &StableKey) -> Result<(), Self::Error>;
    /// Inspect retained names and optionally select historical keys.
    ///
    /// # Errors
    /// Returns the host policy error when recovered-state inspection rejects admission.
    fn prepare_bootstrap(
        &self,
        _admission: &mut BootstrapAdmission<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Bind all effective host policy configuration.
    fn runtime_bootstrap_identity(&self) -> PolicyIdentity;
}
/// Standard policy relying on Storage's namespace admission and resource limits.
pub struct GenericAllocationPolicy;
impl RuntimeBootstrapPolicy for GenericAllocationPolicy {
    type Error = std::convert::Infallible;
    fn validate_allocation(&self, _key: &StableKey) -> Result<(), Self::Error> {
        Ok(())
    }
    fn runtime_bootstrap_identity(&self) -> PolicyIdentity {
        PolicyIdentity::new(*b"ic-storage.noop-policy..........")
    }
}
