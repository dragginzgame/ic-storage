//! Experimental Rust orthogonal persistence for the Internet Computer.
//!
//! The allocation-free [`m0`] library validates byte-defined memory images and
//! offers append-only object allocation and scoped mutation in caller-owned
//! retained-region buffers. It does not install upgrade hooks or create raw
//! slices; the separate qualification fixture owns live IC memory custody.
#![no_std]
#![forbid(unsafe_code)]

pub mod m0;
