#![allow(dead_code)]
use endurox_rs::{tp_error, tp_info, AtmiCtx, AtmiResult, ServerHooks, TpSvcInfo};

mod db;
mod models;
mod schema;
mod services;
mod xa;

use db::DbPool;
use services::*;
use std::collections::HashMap;
use std::sync::OnceLock;

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
pub mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}

// Type alias for service handler to reduce complexity
type ServiceHandler =
    for<'ctx> fn(&'ctx AtmiCtx, &mut TpSvcInfo<'ctx>, &DbPool) -> ServiceResult<'ctx>;

// Global state
static SERVICE_REGISTRY: OnceLock<HashMap<String, ServiceHandler>> = OnceLock::new();
static DB_POOL: OnceLock<DbPool> = OnceLock::new();

// Initialize service registry
fn init_services() {
    let mut registry: HashMap<String, ServiceHandler> = HashMap::new();

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

    let _ = SERVICE_REGISTRY.set(registry);
}

// Generic service dispatcher
fn service_dispatcher<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) {
    let service_name = svc.name().to_string();

    let result = match DB_POOL.get() {
        Some(pool) => match SERVICE_REGISTRY.get() {
            Some(registry) => match registry.get(&service_name) {
                Some(handler) => handler(ctx, svc, pool),
                None => {
                    tp_error!(ctx, "Unknown service: {}", service_name);
                    ServiceResult::Error("Service not found".to_string())
                }
            },
            None => {
                tp_error!(ctx, "Service registry not initialized");
                ServiceResult::Error("Registry error".to_string())
            }
        },
        None => {
            tp_error!(ctx, "Database pool not initialized");
            ServiceResult::Error("Database pool not initialized".to_string())
        }
    };

    send_response(ctx, svc, result);
}

// Server initialization
fn server_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    tp_info!(ctx, "oracle_txn_server starting...");

    // Initialize database pool
    match db::init_pool() {
        Ok(pool) => {
            tp_info!(ctx, "Database connection pool initialized");
            let _ = DB_POOL.set(pool);
        }
        Err(e) => {
            tp_error!(ctx, "Failed to initialize database pool: {}", e);
            tp_error!(ctx, "Make sure DATABASE_URL environment variable is set");
            tp_error!(
                ctx,
                "Example: export DATABASE_URL='oracle://user:pass@host:1521/service'"
            );
            return Err(endurox_rs::AtmiError::new(
                endurox_rs::AtmiError::TPESYSTEM,
                format!("database pool initialization failed: {}", e),
            ));
        }
    }

    // Initialize service registry
    init_services();

    // Advertise services
    let services = ["CREATE_TXN", "GET_TXN", "LIST_TXN"];

    for service in &services {
        match ctx.tpadvertise(service, service_dispatcher) {
            Ok(_) => {
                tp_info!(ctx, "Successfully advertised {}", service);
            }
            Err(e) => {
                tp_error!(ctx, "Failed to advertise {}: {}", service, e);
                return Err(e);
            }
        }
    }

    tp_info!(ctx, "oracle_txn_server initialized successfully");
    tp_info!(ctx, "Available services: CREATE_TXN, GET_TXN, LIST_TXN");
    Ok(())
}

// Server shutdown
fn server_done(ctx: &AtmiCtx) {
    tp_info!(ctx, "oracle_txn_server shutting down...");
}

// Main function - uses endurox_rs::AtmiCtx::tp_run
fn main() {
    let ctx = match AtmiCtx::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to create ATMI context: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = ctx.tp_run(ServerHooks::new(server_init).done(server_done)) {
        eprintln!("server failed: {}", e);
        std::process::exit(1);
    }
}
