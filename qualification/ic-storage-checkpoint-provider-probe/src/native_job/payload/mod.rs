//! Select one concrete payload while retaining one durable transition adapter.
#[cfg(feature = "native-record-vector-job")]
mod records;
#[cfg(feature = "native-record-vector-job")]
pub(super) use records::*;
#[cfg(not(feature = "native-record-vector-job"))]
mod scalar;
#[cfg(not(feature = "native-record-vector-job"))]
pub(super) use scalar::*;
