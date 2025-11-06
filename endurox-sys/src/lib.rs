//! Enduro/X FFI биндинги
//!
//! Этот crate предоставляет безопасные обертки вокруг Enduro/X C API.
//!
//! ## Features
//! - `server` - Server API (tpsvrinit, tpsvrdone, ndrx_main)
//! - `client` - Client API (tpinit, tpterm, tpacall, tpgetrply)
//! - `ubf` - UBF (Unified Buffer Format) API
//!
//! ## Modules
//! - `ffi` - Raw FFI биндинги
//! - `server` - Server API
//! - `client` - Client API
//! - `ubf` - UBF API
//! - `log` - Logging функции

#![allow(dead_code)]
#![allow(static_mut_refs)]

pub mod ffi;
pub mod log;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "ubf")]
pub mod ubf;

#[cfg(feature = "ubf")]
pub mod ubf_struct;

#[cfg(feature = "ubf")]
pub mod ubf_fields;

// Re-export derive macro
#[cfg(feature = "derive")]
pub use endurox_derive::UbfStruct;

// Re-export common types
pub use ffi::{TpSvcInfoRaw, TPFAIL, TPSUCCESS};
pub use log::{tplog_debug, tplog_error, tplog_info, tplog_warn};

#[cfg(feature = "server")]
pub use server::*;

#[cfg(feature = "client")]
pub use client::*;

// Stub implementations for client-only builds to satisfy libatmisrvnomain linkage
#[cfg(all(feature = "client", not(feature = "server")))]
mod client_stubs {
    use libc::{c_char, c_int};

    #[no_mangle]
    pub extern "C" fn tpsvrinit(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
        -1
    }

    #[no_mangle]
    pub extern "C" fn tpsvrdone() {}
}
