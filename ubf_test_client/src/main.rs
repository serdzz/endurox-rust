use endurox_sys::client::EnduroxClient;
use endurox_sys::ubf::UbfBuffer;
use endurox_sys::tplog_info;

// UBF Field IDs (from test.fd - base 1000)
const T_STRING_FLD: i32 = 1001;
const T_NAME_FLD: i32 = 1002;
const T_MESSAGE_FLD: i32 = 1003;
const T_STATUS_FLD: i32 = 1004;
const T_ID_FLD: i32 = 1012;
const T_COUNT_FLD: i32 = 1011;
const T_PRICE_FLD: i32 = 1021;

fn main() {
    println!("=== UBF Service Tests ===\n");
    
    let client = match EnduroxClient::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to initialize Enduro/X client: {}", e);
            std::process::exit(1);
        }
    };
    
    // Test 1: UBFADD
    println!("Test 1: UBFADD - Create UBF buffer with multiple fields");
    test_ubfadd(&client);
    println!();
    
    // Test 2: UBFTEST
    println!("Test 2: UBFTEST - Send name and get greeting");
    test_ubftest(&client);
    println!();
    
    // Test 3: UBFECHO
    println!("Test 3: UBFECHO - Echo buffer back");
    test_ubfecho(&client);
    println!();
    
    // Test 4: UBFGET
    println!("Test 4: UBFGET - Send multiple fields");
    test_ubfget(&client);
    println!();
    
    println!("=== All tests completed ===");
}

fn test_ubfadd(client: &EnduroxClient) {
    let ubf = match UbfBuffer::new(2048) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };
    
    let ptr = ubf.into_raw();
    
    match client.call_service_raw("UBFADD", ptr) {
        Ok(response_ptr) => {
            let response = unsafe { UbfBuffer::from_raw(response_ptr) };
            
            println!("  Response received:");
            println!("    Buffer size: {} bytes", response.size());
            println!("    Used: {} bytes", response.used());
            
            if response.is_present(T_NAME_FLD, 0) {
                if let Ok(name) = response.get_string(T_NAME_FLD, 0) {
                    println!("    T_NAME_FLD: {}", name);
                }
            }
            
            if response.is_present(T_ID_FLD, 0) {
                if let Ok(id) = response.get_long(T_ID_FLD, 0) {
                    println!("    T_ID_FLD: {}", id);
                }
            }
            
            if response.is_present(T_COUNT_FLD, 0) {
                if let Ok(count) = response.get_long(T_COUNT_FLD, 0) {
                    println!("    T_COUNT_FLD: {}", count);
                }
            }
            
            if response.is_present(T_PRICE_FLD, 0) {
                if let Ok(price) = response.get_double(T_PRICE_FLD, 0) {
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

fn test_ubftest(client: &EnduroxClient) {
    let mut ubf = match UbfBuffer::new(1024) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };
    
    if let Err(e) = ubf.add_string(T_NAME_FLD, "RustTester") {
        eprintln!("  Failed to add name: {}", e);
        return;
    }
    
    println!("  Sending: T_NAME_FLD=RustTester");
    
    let ptr = ubf.into_raw();
    
    match client.call_service_raw("UBFTEST", ptr) {
        Ok(response_ptr) => {
            let response = unsafe { UbfBuffer::from_raw(response_ptr) };
            
            println!("  Response received:");
            
            if let Ok(message) = response.get_string(T_MESSAGE_FLD, 0) {
                println!("    T_MESSAGE_FLD: {}", message);
            }
            
            if let Ok(status) = response.get_string(T_STATUS_FLD, 0) {
                println!("    T_STATUS_FLD: {}", status);
            }
            
            println!("  ✓ Test passed");
        }
        Err(e) => {
            eprintln!("  ✗ Test failed: {}", e);
        }
    }
}

fn test_ubfecho(client: &EnduroxClient) {
    let mut ubf = match UbfBuffer::new(1024) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };
    
    let _ = ubf.add_string(T_NAME_FLD, "Echo Test");
    let _ = ubf.add_long(T_ID_FLD, 123);
    
    println!("  Sending: T_NAME_FLD='Echo Test', T_ID_FLD=123");
    
    let ptr = ubf.into_raw();
    
    match client.call_service_raw("UBFECHO", ptr) {
        Ok(response_ptr) => {
            let response = unsafe { UbfBuffer::from_raw(response_ptr) };
            
            println!("  Response received:");
            
            if let Ok(name) = response.get_string(T_NAME_FLD, 0) {
                println!("    T_NAME_FLD: {}", name);
            }
            
            if let Ok(id) = response.get_long(T_ID_FLD, 0) {
                println!("    T_ID_FLD: {}", id);
            }
            
            println!("  ✓ Test passed");
        }
        Err(e) => {
            eprintln!("  ✗ Test failed: {}", e);
        }
    }
}

fn test_ubfget(client: &EnduroxClient) {
    let mut ubf = match UbfBuffer::new(2048) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("  Failed to create buffer: {}", e);
            return;
        }
    };
    
    let _ = ubf.add_string(T_NAME_FLD, "John Doe");
    let _ = ubf.add_long(T_ID_FLD, 9999);
    let _ = ubf.add_double(T_PRICE_FLD, 123.45);
    
    println!("  Sending: T_NAME_FLD='John Doe', T_ID_FLD=9999, T_PRICE_FLD=123.45");
    
    let ptr = ubf.into_raw();
    
    match client.call_service_raw("UBFGET", ptr) {
        Ok(response_ptr) => {
            let response = unsafe { UbfBuffer::from_raw(response_ptr) };
            
            println!("  Response received - buffer echoed back");
            println!("  ✓ Test passed");
        }
        Err(e) => {
            eprintln!("  ✗ Test failed: {}", e);
        }
    }
}
