//! Governed checkpoint provider fixtures; ordinary and native bridge profiles.

#[cfg(all(feature = "allocation-metrics", target_arch = "wasm32"))]
mod allocation_metrics;

#[cfg(all(feature = "paged-store", feature = "native-bridge"))]
compile_error!("paged-store is an ordinary storage probe, not a native bridge profile");

#[cfg(any(
    feature = "native-job",
    feature = "native-record-vector-job",
    feature = "native-map-job",
    feature = "paged-store"
))]
#[cfg(target_arch = "wasm32")]
mod job_control;

#[cfg(feature = "native-map")]
#[cfg(target_arch = "wasm32")]
mod native_map;

#[cfg(all(
    feature = "native-bridge",
    not(any(
        feature = "native-job",
        feature = "native-record-vector",
        feature = "native-map"
    ))
))]
#[cfg(target_arch = "wasm32")]
mod native_bridge;
#[cfg(any(
    all(feature = "native-job", not(feature = "native-record-vector")),
    all(
        feature = "native-record-vector-job",
        not(any(
            feature = "native-record-vector-source",
            feature = "native-record-vector-v2",
            feature = "native-record-vector-convert"
        ))
    )
))]
#[cfg(not(feature = "native-map"))]
#[cfg(target_arch = "wasm32")]
mod native_job;
#[cfg(not(any(feature = "native-bridge", feature = "paged-store")))]
#[cfg(target_arch = "wasm32")]
mod ordinary;
#[cfg(all(feature = "paged-store", target_arch = "wasm32"))]
mod paged_store;
#[cfg(any(target_arch = "wasm32", all(test, feature = "native-map-job")))]
mod provider;
#[cfg(all(
    feature = "native-record-vector",
    not(feature = "native-record-vector-source"),
    any(
        not(feature = "native-record-vector-job"),
        feature = "native-record-vector-v2",
        feature = "native-record-vector-convert"
    )
))]
#[cfg(not(feature = "native-map"))]
#[cfg(target_arch = "wasm32")]
mod record_vector;
#[cfg(feature = "native-record-vector-source")]
#[cfg(not(feature = "native-map"))]
#[cfg(target_arch = "wasm32")]
mod record_vector_source;

#[cfg(all(
    feature = "native-record-vector-convert",
    not(any(
        feature = "native-record-vector-v2",
        feature = "native-record-vector-source"
    ))
))]
#[cfg(not(feature = "native-map"))]
#[cfg(target_arch = "wasm32")]
mod record_vector_convert;
