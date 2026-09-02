use endurox_rs::{AtmiCtx, UbfValue};

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::{T_COUNT_FLD, T_ID_FLD, T_MESSAGE_FLD, T_NAME_FLD, T_PRICE_FLD, T_STATUS_FLD};

#[test]
#[ignore] // Run only with Enduro/X environment
fn test_ubfecho() {
    let ctx = AtmiCtx::new().expect("Failed to create context");
    ctx.tpinit().expect("Failed to init client");

    // Create UBF buffer
    let mut ubf = ctx.tpalloc_ubf(1024).expect("Failed to create UBF buffer");
    ubf.bchg(T_NAME_FLD, 0, UbfValue::String("Test".to_string()), true)
        .expect("Failed to add name");

    let mut rsp = ctx
        .tpalloc_ubf(1024)
        .expect("Failed to create reply buffer");
    let result = ctx.tpcall("UBFECHO", &ubf, &mut rsp, 0);

    assert!(result.is_ok());
}

#[test]
#[ignore]
fn test_ubftest() {
    let ctx = AtmiCtx::new().expect("Failed to create context");
    ctx.tpinit().expect("Failed to init client");

    // Create request buffer
    let mut ubf = ctx.tpalloc_ubf(1024).expect("Failed to create UBF buffer");
    ubf.bchg(T_NAME_FLD, 0, UbfValue::String("Rust".to_string()), true)
        .expect("Failed to add name");

    let mut response = ctx
        .tpalloc_ubf(1024)
        .expect("Failed to create reply buffer");
    ctx.tpcall("UBFTEST", &ubf, &mut response, 0)
        .expect("UBFTEST call failed");

    // Check response fields
    assert!(ctx.bpres(&response, T_MESSAGE_FLD, 0));
    assert!(ctx.bpres(&response, T_STATUS_FLD, 0));

    let message = response
        .bget_string(T_MESSAGE_FLD, 0)
        .expect("Failed to get message");
    assert_eq!(message, "Hello, Rust!");

    let status = response
        .bget_string(T_STATUS_FLD, 0)
        .expect("Failed to get status");
    assert_eq!(status, "OK");
}

#[test]
#[ignore]
fn test_ubfadd() {
    let ctx = AtmiCtx::new().expect("Failed to create context");
    ctx.tpinit().expect("Failed to init client");

    // Call UBFADD with empty buffer
    let ubf = ctx.tpalloc_ubf(2048).expect("Failed to create UBF buffer");
    let mut response = ctx
        .tpalloc_ubf(2048)
        .expect("Failed to create reply buffer");
    ctx.tpcall("UBFADD", &ubf, &mut response, 0)
        .expect("UBFADD call failed");

    // Verify fields were added
    assert!(ctx.bpres(&response, T_NAME_FLD, 0));
    assert!(ctx.bpres(&response, T_ID_FLD, 0));
    assert!(ctx.bpres(&response, T_COUNT_FLD, 0));
    assert!(ctx.bpres(&response, T_PRICE_FLD, 0));

    let name = response
        .bget_string(T_NAME_FLD, 0)
        .expect("Failed to get name");
    assert_eq!(name, "John Doe");

    let id = response.bget_long(T_ID_FLD, 0).expect("Failed to get ID");
    assert_eq!(id, 12345);

    let count = response
        .bget_long(T_COUNT_FLD, 0)
        .expect("Failed to get count");
    assert_eq!(count, 100);

    let price = response
        .bget_double(T_PRICE_FLD, 0)
        .expect("Failed to get price");
    assert!((price - 99.99).abs() < 0.01);
}

#[test]
#[ignore]
fn test_ubfget() {
    let ctx = AtmiCtx::new().expect("Failed to create context");
    ctx.tpinit().expect("Failed to init client");

    // Create buffer with data
    let mut ubf = ctx.tpalloc_ubf(2048).expect("Failed to create UBF buffer");
    ubf.bchg(
        T_NAME_FLD,
        0,
        UbfValue::String("Test User".to_string()),
        true,
    )
    .expect("Failed to add name");
    ubf.bchg(T_ID_FLD, 0, UbfValue::Long(9999), true)
        .expect("Failed to add ID");
    ubf.bchg(T_PRICE_FLD, 0, UbfValue::Double(123.45), true)
        .expect("Failed to add price");

    let mut response = ctx
        .tpalloc_ubf(2048)
        .expect("Failed to create reply buffer");
    let result = ctx.tpcall("UBFGET", &ubf, &mut response, 0);

    assert!(result.is_ok());
}
