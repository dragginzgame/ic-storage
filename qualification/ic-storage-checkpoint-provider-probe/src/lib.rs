//! Governed checkpoint provider fixtures; ordinary and native bridge profiles.

#[cfg(feature = "native-bridge")]
mod native_bridge;
#[cfg(not(feature = "native-bridge"))]
mod ordinary;
mod provider;
