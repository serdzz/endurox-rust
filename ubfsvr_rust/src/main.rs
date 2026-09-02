use endurox_rs::{
    tp_error, tp_info, AtmiCtx, AtmiResult, ServerHooks, TpReturnStatus, TpSvcInfo, UbfValue,
};

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::{
    T_CODE_FLD, T_COUNT_FLD, T_ID_FLD, T_MESSAGE_FLD, T_NAME_FLD, T_PRICE_FLD, T_STATUS_FLD,
    T_STRING_FLD,
};

/// UBFECHO - Echo UBF buffer back
fn service_ubfecho<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) {
    tp_info!(ctx, "UBFECHO service called");

    match svc.take_data_ubf() {
        Some(ubf) => {
            // Just echo the buffer back
            ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
        }
        None => {
            tp_error!(ctx, "UBFECHO: No data received");
            fail(ctx);
        }
    }
}

/// UBFTEST - Test UBF operations
fn service_ubftest<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) {
    tp_info!(ctx, "UBFTEST service called");

    // Use the request buffer, or allocate a fresh one if none was sent
    let mut ubf = match svc.take_data_ubf() {
        Some(buf) => buf,
        None => match ctx.tpalloc_ubf(1024) {
            Ok(buf) => buf,
            Err(e) => {
                tp_error!(ctx, "Failed to allocate UBF buffer: {}", e);
                fail(ctx);
                return;
            }
        },
    };

    // Read input fields if present
    if ctx.bpres(&ubf, T_NAME_FLD, 0) {
        match ubf.bget_string(T_NAME_FLD, 0) {
            Ok(name) => {
                tp_info!(ctx, "UBFTEST: Received name={}", name);

                // Add response message
                let msg = format!("Hello, {}!", name);
                if let Err(e) = ubf.bchg(T_MESSAGE_FLD, 0, UbfValue::String(msg), true) {
                    tp_error!(ctx, "Failed to add message: {}", e);
                }
            }
            Err(e) => {
                tp_error!(ctx, "Failed to get name: {}", e);
            }
        }
    }

    // Add status
    if let Err(e) = ubf.bchg(T_STATUS_FLD, 0, UbfValue::String("OK".to_string()), true) {
        tp_error!(ctx, "Failed to add status: {}", e);
    }

    // Add code
    if let Err(e) = ubf.bchg(T_CODE_FLD, 0, UbfValue::Long(0), true) {
        tp_error!(ctx, "Failed to add code: {}", e);
    }

    // Print buffer for debugging
    if let Err(e) = ubf.bprint() {
        tp_error!(ctx, "Failed to print UBF: {}", e);
    }

    tp_info!(ctx, "UBFTEST: Returning success");
    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

/// UBFADD - Add fields to UBF buffer
fn service_ubfadd<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) {
    tp_info!(ctx, "UBFADD service called");

    let mut ubf = match svc.take_data_ubf() {
        Some(buf) => buf,
        None => match ctx.tpalloc_ubf(2048) {
            Ok(buf) => buf,
            Err(e) => {
                tp_error!(ctx, "Failed to allocate UBF buffer: {}", e);
                fail(ctx);
                return;
            }
        },
    };

    // Add multiple fields
    let _ = ubf.bchg(
        T_STRING_FLD,
        0,
        UbfValue::String("Test String".to_string()),
        true,
    );
    let _ = ubf.bchg(
        T_NAME_FLD,
        0,
        UbfValue::String("John Doe".to_string()),
        true,
    );
    let _ = ubf.bchg(T_STATUS_FLD, 0, UbfValue::String("Added".to_string()), true);
    let _ = ubf.bchg(T_ID_FLD, 0, UbfValue::Long(12345), true);
    let _ = ubf.bchg(T_COUNT_FLD, 0, UbfValue::Long(100), true);
    let _ = ubf.bchg(T_PRICE_FLD, 0, UbfValue::Double(99.99), true);

    let used = ctx.bused(&ubf).unwrap_or(0);
    tp_info!(ctx, "UBFADD: Added fields, used={} bytes", used);

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

/// UBFGET - Get fields from UBF buffer
fn service_ubfget<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) {
    tp_info!(ctx, "UBFGET service called");

    let ubf = match svc.take_data_ubf() {
        Some(buf) => buf,
        None => {
            tp_error!(ctx, "UBFGET: No data received");
            fail(ctx);
            return;
        }
    };

    // Try to read various fields
    if let Ok(name) = ubf.bget_string(T_NAME_FLD, 0) {
        tp_info!(ctx, "UBFGET: T_NAME_FLD={}", name);
    }

    if let Ok(id) = ubf.bget_long(T_ID_FLD, 0) {
        tp_info!(ctx, "UBFGET: T_ID_FLD={}", id);
    }

    if let Ok(price) = ubf.bget_double(T_PRICE_FLD, 0) {
        tp_info!(ctx, "UBFGET: T_PRICE_FLD={}", price);
    }

    // Echo back
    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

/// Return TPFAIL with a minimal fresh buffer.
fn fail(ctx: &AtmiCtx) {
    if let Ok(ubf) = ctx.tpalloc_ubf(256) {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 0, ubf, 0);
    }
}

// Server initialization
fn server_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    tp_info!(ctx, "ubfsvr_rust starting...");

    let services: [(&str, endurox_rs::RustServiceCallback); 4] = [
        ("UBFECHO", service_ubfecho),
        ("UBFTEST", service_ubftest),
        ("UBFADD", service_ubfadd),
        ("UBFGET", service_ubfget),
    ];

    for (service_name, handler) in services {
        match ctx.tpadvertise(service_name, handler) {
            Ok(_) => {
                tp_info!(ctx, "Successfully advertised {}", service_name);
            }
            Err(e) => {
                tp_error!(ctx, "Failed to advertise {}: {}", service_name, e);
                return Err(e);
            }
        }
    }

    tp_info!(ctx, "ubfsvr_rust initialized successfully");
    Ok(())
}

// Server shutdown
fn server_done(ctx: &AtmiCtx) {
    tp_info!(ctx, "ubfsvr_rust shutting down...");
}

// Main function
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
