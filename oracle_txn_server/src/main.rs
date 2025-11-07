#![allow(static_mut_refs)]
use endurox_sys::server::*;
use endurox_sys::{self, tplog_error, tplog_info, TpSvcInfoRaw};

mod db;
mod models;
mod schema;
mod services;

use db::DbPool;
use services::*;
use std::collections::HashMap;

// Type alias for service handler
type ServiceHandler = fn(&ServiceRequest, &DbPool) -> ServiceResult;

// Global state
static mut SERVICE_REGISTRY: Option<HashMap<String, ServiceHandler>> = None;
static mut DB_POOL: Option<DbPool> = None;

// Initialize service registry
fn init_services() {
    let mut registry = HashMap::new();

    registry.insert(
        "CREATE_TXN".to_string(),
        create_transaction_service as ServiceHandler,
    );

    registry.insert(
        "GET_TXN".to_string(),
        get_transaction_service as ServiceHandler,
    );

    registry.insert(
        "LIST_TXN".to_string(),
        list_transactions_service as ServiceHandler,
    );

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
        let pool = match &DB_POOL {
            Some(pool) => pool,
            None => {
                tplog_error("Database pool not initialized");
                return ServiceResult::error("Database pool not initialized")
                    .send_response(rqst)
                    .unwrap_or(());
            }
        };

        let registry_ptr = &raw const SERVICE_REGISTRY;
        match (*registry_ptr).as_ref() {
            Some(registry) => match registry.get(&service_name) {
                Some(handler) => handler(&request, pool),
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
    tplog_info("oracle_txn_server starting...");

    // Initialize database pool
    match db::init_pool() {
        Ok(pool) => {
            tplog_info("Database connection pool initialized");
            unsafe {
                DB_POOL = Some(pool);
            }
        }
        Err(e) => {
            tplog_error(&format!("Failed to initialize database pool: {}", e));
            tplog_error("Make sure DATABASE_URL environment variable is set");
            tplog_error("Example: export DATABASE_URL='oracle://user:pass@host:1521/service'");
            return -1;
        }
    }

    // Initialize service registry
    init_services();

    // Advertise services
    let services = ["CREATE_TXN", "GET_TXN", "LIST_TXN"];

    for service in &services {
        match advertise_service(service, service_dispatcher) {
            Ok(_) => tplog_info(&format!("Successfully advertised {}", service)),
            Err(e) => {
                tplog_error(&format!("Failed to advertise {}: {}", service, e));
                return -1;
            }
        }
    }

    tplog_info("oracle_txn_server initialized successfully");
    tplog_info("Available services: CREATE_TXN, GET_TXN, LIST_TXN");
    0
}

// Server shutdown
#[no_mangle]
pub extern "C" fn tpsvrdone() {
    tplog_info("oracle_txn_server shutting down...");

    unsafe {
        if let Some(pool) = DB_POOL.take() {
            drop(pool);
            tplog_info("Database connection pool closed");
        }
    }
}

// Main function
fn main() -> ! {
    run_server(tpsvrinit, tpsvrdone)
}
