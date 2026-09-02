#![allow(dead_code)]
use endurox_rs::{tp_error, tp_info, AtmiCtx, AtmiResult, ServerHooks, TpSvcInfo};

mod services;
use services::*;
use std::collections::HashMap;
use std::sync::OnceLock;

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
pub mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}

// Type alias for service handler to reduce complexity
type ServiceHandler = for<'ctx> fn(&'ctx AtmiCtx, &mut TpSvcInfo<'ctx>) -> ServiceResult<'ctx>;

// Service registry
static SERVICE_REGISTRY: OnceLock<HashMap<String, ServiceHandler>> = OnceLock::new();

// Initialize service registry
fn init_services() {
    let mut registry: HashMap<String, ServiceHandler> = HashMap::new();
    registry.insert("ECHO".to_string(), echo_service as ServiceHandler);
    registry.insert("HELLO".to_string(), hello_service as ServiceHandler);
    registry.insert("STATUS".to_string(), status_service as ServiceHandler);
    registry.insert("DATAPROC".to_string(), dataproc_service as ServiceHandler);
    registry.insert(
        "TRANSACTION".to_string(),
        transaction_service as ServiceHandler,
    );

    let _ = SERVICE_REGISTRY.set(registry);
}

// Generic service dispatcher
fn service_dispatcher<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) {
    let service_name = svc.name().to_string();

    let result = match SERVICE_REGISTRY.get() {
        Some(registry) => match registry.get(&service_name) {
            Some(handler) => handler(ctx, svc),
            None => {
                tp_error!(ctx, "Unknown service: {}", service_name);
                ServiceResult::Error("Service not found".to_string())
            }
        },
        None => {
            tp_error!(ctx, "Service registry not initialized");
            ServiceResult::Error("Registry error".to_string())
        }
    };

    send_response(ctx, svc, result);
}

// Server initialization
fn server_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    tp_info!(ctx, "samplesvr_rust starting...");

    init_services();

    let services = ["ECHO", "HELLO", "STATUS", "DATAPROC", "TRANSACTION"];

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

    tp_info!(ctx, "samplesvr_rust initialized successfully");
    Ok(())
}

// Server shutdown
fn server_done(ctx: &AtmiCtx) {
    tp_info!(ctx, "samplesvr_rust shutting down...");
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
