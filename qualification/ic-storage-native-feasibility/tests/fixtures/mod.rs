//! Shared compiler and host fixtures; no application-facing API.

mod region;

impl region::sealed::Sealed for ic_storage_native_feasibility::record::narrow::V3 {}
impl region::Element for ic_storage_native_feasibility::record::narrow::V3 {}
