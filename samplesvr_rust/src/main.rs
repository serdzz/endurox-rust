#![allow(static_mut_refs)]
use endurox_sys::{self, TpSvcInfoRaw, tplog_info, tplog_error};
use endurox_sys::server::*;

mod services;
use services::*;
use std::collections::HashMap;

// Type alias for service handler to reduce complexity
type ServiceHandler = fn(&ServiceRequest) -> ServiceResult;

// Service registry
static mut SERVICE_REGISTRY: Option<HashMap<String, ServiceHandler>> = None;

// Initialize service registry
fn init_services() {
    let mut registry = HashMap::new();
    registry.insert(
        "ECHO".to_string(),
        echo_service as fn(&ServiceRequest) -> ServiceResult,
    );
    registry.insert(
        "HELLO".to_string(),
        hello_service as fn(&ServiceRequest) -> ServiceResult,
    );
    registry.insert(
        "STATUS".to_string(),
        status_service as fn(&ServiceRequest) -> ServiceResult,
    );
    registry.insert(
        "DATAPROC".to_string(),
        dataproc_service as fn(&ServiceRequest) -> ServiceResult,
    );
    registry.insert(
        "TRANSACTION".to_string(),
        transaction_service as fn(&ServiceRequest) -> ServiceResult,
    );

    // Safe assignment with proper synchronization would be better in production
    unsafe {
        SERVICE_REGISTRY = Some(registry);
    }
}

// Generic service dispatcher
extern "C" fn service_dispatcher(rqst: *mut TpSvcInfoRaw) {
    let request = match ServiceRequest::from_raw(rqst) {
        Ok(req) => req,
        Err(e) => {
            tplog_error(&format!("Failed to parse service request: {}", e));
            unsafe {
                tpreturn_fail(rqst);
            }
            return;
        }
    };

    let service_name = request.service_name();
    let result = unsafe {
        let registry_ptr = &raw const SERVICE_REGISTRY;
        match (*registry_ptr).as_ref() {
            Some(registry) => match registry.get(&service_name) {
                Some(handler) => handler(&request),
                None => {
                    tplog_error(&format!("Unknown service: {}", service_name));
                    ServiceResult::error("Service not found")
                }
            },
            None => {
                tplog_error("Service registry not initialized");
                ServiceResult::error("Registry error")
            }
        }
    };

    match result.send_response(rqst) {
        Ok(_) => {}
        Err(e) => tplog_error(&format!("Failed to send response: {}", e)),
    }
}

// Server initialization
#[no_mangle]
pub extern "C" fn tpsvrinit(_argc: libc::c_int, _argv: *mut *mut libc::c_char) -> libc::c_int {
    tplog_info("samplesvr_rust starting...");

    init_services();

    let services = ["ECHO", "HELLO", "STATUS", "DATAPROC", "TRANSACTION"];

    for service in &services {
        match advertise_service(service, service_dispatcher) {
            Ok(_) => tplog_info(&format!("Successfully advertised {}", service)),
            Err(e) => {
                tplog_error(&format!("Failed to advertise {}: {}", service, e));
                return -1;
            }
        }
    }

    tplog_info("samplesvr_rust initialized successfully");
    0
}

// Server shutdown
#[no_mangle]
pub extern "C" fn tpsvrdone() {
    tplog_info("samplesvr_rust shutting down...");
}

// Main function - использует endurox_sys::server::run_server
fn main() -> ! {
    run_server(tpsvrinit, tpsvrdone)
}
