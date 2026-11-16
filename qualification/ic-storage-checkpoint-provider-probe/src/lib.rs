//! Governed checkpoint provider fixtures; ordinary and native bridge profiles.

#[cfg(all(
    feature = "native-bridge",
    not(any(feature = "native-job", feature = "native-record-vector"))
))]
mod native_bridge;
#[cfg(all(feature = "native-job", not(feature = "native-record-vector")))]
mod native_job;
#[cfg(not(feature = "native-bridge"))]
mod ordinary;
mod provider;
#[cfg(all(
    feature = "native-record-vector",
    not(feature = "native-record-vector-source")
))]
mod record_vector;
#[cfg(feature = "native-record-vector-source")]
mod record_vector_source;
