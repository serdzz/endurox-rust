use endurox_sys::client::EnduroxClient;
use endurox_sys::ubf::UbfBuffer;

// UBF Field IDs (from test.fd - base 1000)
const T_NAME_FLD: i32 = 1002;
const T_MESSAGE_FLD: i32 = 1003;
const T_STATUS_FLD: i32 = 1004;
const T_ID_FLD: i32 = 1012;
const T_COUNT_FLD: i32 = 1011;
const T_PRICE_FLD: i32 = 1021;

#[test]
#[ignore] // Run only with Enduro/X environment
fn test_ubfecho() {
    let client = EnduroxClient::new().expect("Failed to init client");

    // Create UBF buffer
    let mut ubf = UbfBuffer::new(1024).expect("Failed to create UBF buffer");
    ubf.add_string(T_NAME_FLD, "Test")
        .expect("Failed to add name");

    let ptr = ubf.into_raw();
    let result = unsafe { client.call_service_raw("UBFECHO", ptr) };

    assert!(result.is_ok());
}

#[test]
#[ignore]
fn test_ubftest() {
    let client = EnduroxClient::new().expect("Failed to init client");

    // Create request buffer
    let mut ubf = UbfBuffer::new(1024).expect("Failed to create UBF buffer");
    ubf.add_string(T_NAME_FLD, "Rust")
        .expect("Failed to add name");

    let ptr = ubf.into_raw();
    let result = unsafe { client.call_service_raw("UBFTEST", ptr) };

    assert!(result.is_ok());

    // Parse response
    let response_ptr = result.unwrap();
    let response = unsafe { UbfBuffer::from_raw(response_ptr) };

    // Check response fields
    assert!(response.is_present(T_MESSAGE_FLD, 0));
    assert!(response.is_present(T_STATUS_FLD, 0));

    let message = response
        .get_string(T_MESSAGE_FLD, 0)
        .expect("Failed to get message");
    assert_eq!(message, "Hello, Rust!");

    let status = response
        .get_string(T_STATUS_FLD, 0)
        .expect("Failed to get status");
    assert_eq!(status, "OK");
}

#[test]
#[ignore]
fn test_ubfadd() {
    let client = EnduroxClient::new().expect("Failed to init client");

    // Call UBFADD with empty buffer
    let ubf = UbfBuffer::new(2048).expect("Failed to create UBF buffer");
    let ptr = ubf.into_raw();

    let result = unsafe { client.call_service_raw("UBFADD", ptr) };
    assert!(result.is_ok());

    // Parse response
    let response_ptr = result.unwrap();
    let response = unsafe { UbfBuffer::from_raw(response_ptr) };

    // Verify fields were added
    assert!(response.is_present(T_NAME_FLD, 0));
    assert!(response.is_present(T_ID_FLD, 0));
    assert!(response.is_present(T_COUNT_FLD, 0));
    assert!(response.is_present(T_PRICE_FLD, 0));

    let name = response
        .get_string(T_NAME_FLD, 0)
        .expect("Failed to get name");
    assert_eq!(name, "John Doe");

    let id = response.get_long(T_ID_FLD, 0).expect("Failed to get ID");
    assert_eq!(id, 12345);

    let count = response
        .get_long(T_COUNT_FLD, 0)
        .expect("Failed to get count");
    assert_eq!(count, 100);

    let price = response
        .get_double(T_PRICE_FLD, 0)
        .expect("Failed to get price");
    assert!((price - 99.99).abs() < 0.01);
}

#[test]
#[ignore]
fn test_ubfget() {
    let client = EnduroxClient::new().expect("Failed to init client");

    // Create buffer with data
    let mut ubf = UbfBuffer::new(2048).expect("Failed to create UBF buffer");
    ubf.add_string(T_NAME_FLD, "Test User")
        .expect("Failed to add name");
    ubf.add_long(T_ID_FLD, 9999).expect("Failed to add ID");
    ubf.add_double(T_PRICE_FLD, 123.45)
        .expect("Failed to add price");

    let ptr = ubf.into_raw();
    let result = unsafe { client.call_service_raw("UBFGET", ptr) };

    assert!(result.is_ok());
}
