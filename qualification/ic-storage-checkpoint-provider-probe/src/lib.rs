//! Governed checkpoint provider fixtures; ordinary and native bridge profiles.

#[cfg(all(feature = "native-bridge", not(feature = "native-job")))]
mod native_bridge;
#[cfg(feature = "native-job")]
mod native_job;
#[cfg(not(feature = "native-bridge"))]
mod ordinary;
mod provider;
