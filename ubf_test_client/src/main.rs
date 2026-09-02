use endurox_rs::{AtmiCtx, TypedUbf, UbfValue};

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::{T_COUNT_FLD, T_ID_FLD, T_MESSAGE_FLD, T_NAME_FLD, T_PRICE_FLD, T_STATUS_FLD};

fn main() {
    println!("=== UBF Service Tests ===\n");

    let ctx = match AtmiCtx::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create ATMI context: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = ctx.tpinit() {
        eprintln!("Failed to initialize Enduro/X client: {}", e);
        std::process::exit(1);
    }

    // Test 1: UBFADD
    println!("Test 1: UBFADD - Create UBF buffer with multiple fields");
    test_ubfadd(&ctx);
    println!();

    // Test 2: UBFTEST
    println!("Test 2: UBFTEST - Send name and get greeting");
    test_ubftest(&ctx);
    println!();

    // Test 3: UBFECHO
    println!("Test 3: UBFECHO - Echo buffer back");
    test_ubfecho(&ctx);
    println!();

    // Test 4: UBFGET
    println!("Test 4: UBFGET - Send multiple fields");
    test_ubfget(&ctx);
    println!();

    println!("=== All tests completed ===");
}

fn call<'ctx>(
    ctx: &'ctx AtmiCtx,
    svc: &str,
    req: &TypedUbf<'ctx>,
) -> Result<TypedUbf<'ctx>, String> {
    let mut rsp = ctx
        .tpalloc_ubf(2048)
        .map_err(|e| format!("Failed to create reply buffer: {}", e))?;
    ctx.tpcall(svc, req, &mut rsp, 0)
        .map_err(|e| e.to_string())?;
    Ok(rsp)
}

fn test_ubfadd(ctx: &AtmiCtx) {
    let ubf = match ctx.tpalloc_ubf(2048) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };

    match call(ctx, "UBFADD", &ubf) {
        Ok(mut response) => {
            println!("  Response received:");
            println!("    Buffer size: {} bytes", response.bsizeof().unwrap_or(0));
            println!("    Used: {} bytes", ctx.bused(&response).unwrap_or(0));

            if ctx.bpres(&response, T_NAME_FLD, 0) {
                if let Ok(name) = response.bget_string(T_NAME_FLD, 0) {
                    println!("    T_NAME_FLD: {}", name);
                }
            }

            if ctx.bpres(&response, T_ID_FLD, 0) {
                if let Ok(id) = response.bget_long(T_ID_FLD, 0) {
                    println!("    T_ID_FLD: {}", id);
                }
            }

            if ctx.bpres(&response, T_COUNT_FLD, 0) {
                if let Ok(count) = response.bget_long(T_COUNT_FLD, 0) {
                    println!("    T_COUNT_FLD: {}", count);
                }
            }

            if ctx.bpres(&response, T_PRICE_FLD, 0) {
                if let Ok(price) = response.bget_double(T_PRICE_FLD, 0) {
                    println!("    T_PRICE_FLD: {:.2}", price);
                }
            }

            println!("  ✓ Test passed");
        }
        Err(e) => {
            eprintln!("  ✗ Test failed: {}", e);
        }
    }
}

fn test_ubftest(ctx: &AtmiCtx) {
    let mut ubf = match ctx.tpalloc_ubf(1024) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };

    if let Err(e) = ubf.bchg(
        T_NAME_FLD,
        0,
        UbfValue::String("RustTester".to_string()),
        true,
    ) {
        eprintln!("  Failed to add name: {}", e);
        return;
    }

    println!("  Sending: T_NAME_FLD=RustTester");

    match call(ctx, "UBFTEST", &ubf) {
        Ok(response) => {
            println!("  Response received:");

            if let Ok(message) = response.bget_string(T_MESSAGE_FLD, 0) {
                println!("    T_MESSAGE_FLD: {}", message);
            }

            if let Ok(status) = response.bget_string(T_STATUS_FLD, 0) {
                println!("    T_STATUS_FLD: {}", status);
            }

            println!("  ✓ Test passed");
        }
        Err(e) => {
            eprintln!("  ✗ Test failed: {}", e);
        }
    }
}

fn test_ubfecho(ctx: &AtmiCtx) {
    let mut ubf = match ctx.tpalloc_ubf(1024) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };

    let _ = ubf.bchg(
        T_NAME_FLD,
        0,
        UbfValue::String("Echo Test".to_string()),
        true,
    );
    let _ = ubf.bchg(T_ID_FLD, 0, UbfValue::Long(123), true);

    println!("  Sending: T_NAME_FLD='Echo Test', T_ID_FLD=123");

    match call(ctx, "UBFECHO", &ubf) {
        Ok(response) => {
            println!("  Response received:");

            if let Ok(name) = response.bget_string(T_NAME_FLD, 0) {
                println!("    T_NAME_FLD: {}", name);
            }

            if let Ok(id) = response.bget_long(T_ID_FLD, 0) {
                println!("    T_ID_FLD: {}", id);
            }

            println!("  ✓ Test passed");
        }
        Err(e) => {
            eprintln!("  ✗ Test failed: {}", e);
        }
    }
}

fn test_ubfget(ctx: &AtmiCtx) {
    let mut ubf = match ctx.tpalloc_ubf(2048) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };

    let _ = ubf.bchg(
        T_NAME_FLD,
        0,
        UbfValue::String("John Doe".to_string()),
        true,
    );
    let _ = ubf.bchg(T_ID_FLD, 0, UbfValue::Long(9999), true);
    let _ = ubf.bchg(T_PRICE_FLD, 0, UbfValue::Double(123.45), true);

    println!("  Sending: T_NAME_FLD='John Doe', T_ID_FLD=9999, T_PRICE_FLD=123.45");

    match call(ctx, "UBFGET", &ubf) {
        Ok(_response) => {
            println!("  Response received - buffer echoed back");
            println!("  ✓ Test passed");
        }
        Err(e) => {
            eprintln!("  ✗ Test failed: {}", e);
        }
    }
}
