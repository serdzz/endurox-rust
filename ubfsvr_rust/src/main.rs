#![allow(static_mut_refs)]
use endurox_sys::{self, TpSvcInfoRaw, tplog_info, tplog_error};
use endurox_sys::server::*;
use endurox_sys::ubf::*;

// UBF Field IDs (from test.fd - base 1000)
const T_STRING_FLD: i32 = 1001;
const T_NAME_FLD: i32 = 1002;
const T_MESSAGE_FLD: i32 = 1003;
const T_STATUS_FLD: i32 = 1004;

const T_COUNT_FLD: i32 = 1011;
const T_ID_FLD: i32 = 1012;
const T_CODE_FLD: i32 = 1013;

const T_PRICE_FLD: i32 = 1021;

/// UBFECHO - Echo UBF buffer back
extern "C" fn service_ubfecho(rqst: *mut TpSvcInfoRaw) {
    tplog_info("UBFECHO service called");
    
    unsafe {
        let req = &*rqst;
        
        if req.data.is_null() {
            tplog_error("UBFECHO: No data received");
            tpreturn_fail(rqst);
            return;
        }
        
        // Just echo the buffer back
        tpreturn_echo(rqst);
    }
}

/// UBFTEST - Test UBF operations
extern "C" fn service_ubftest(rqst: *mut TpSvcInfoRaw) {
    tplog_info("UBFTEST service called");
    
    unsafe {
        let req = &*rqst;
        
        // Create UBF buffer from request
        let mut ubf = if !req.data.is_null() {
            UbfBuffer::from_raw(req.data)
        } else {
            match UbfBuffer::new(1024) {
                Ok(buf) => buf,
                Err(e) => {
                    tplog_error(&format!("Failed to allocate UBF buffer: {}", e));
                    tpreturn_fail(rqst);
                    return;
                }
            }
        };
        
        // Read input fields if present
        if ubf.is_present(T_NAME_FLD, 0) {
            match ubf.get_string(T_NAME_FLD, 0) {
                Ok(name) => {
                    tplog_info(&format!("UBFTEST: Received name={}", name));
                    
                    // Add response message
                    let msg = format!("Hello, {}!", name);
                    if let Err(e) = ubf.add_string(T_MESSAGE_FLD, &msg) {
                        tplog_error(&format!("Failed to add message: {}", e));
                    }
                }
                Err(e) => tplog_error(&format!("Failed to get name: {}", e)),
            }
        }
        
        // Add status
        if let Err(e) = ubf.add_string(T_STATUS_FLD, "OK") {
            tplog_error(&format!("Failed to add status: {}", e));
        }
        
        // Add code
        if let Err(e) = ubf.add_long(T_CODE_FLD, 0) {
            tplog_error(&format!("Failed to add code: {}", e));
        }
        
        // Print buffer for debugging
        if let Err(e) = ubf.print() {
            tplog_error(&format!("Failed to print UBF: {}", e));
        }
        
        let ptr = ubf.into_raw();
        let len = endurox_sys::ffi::Bused(ptr);
        
        tplog_info("UBFTEST: Returning success");
        endurox_sys::ffi::tpreturn(endurox_sys::TPSUCCESS, 0, ptr, len, 0);
    }
}

/// UBFADD - Add fields to UBF buffer
extern "C" fn service_ubfadd(rqst: *mut TpSvcInfoRaw) {
    tplog_info("UBFADD service called");
    
    unsafe {
        let req = &*rqst;
        
        let mut ubf = if !req.data.is_null() {
            UbfBuffer::from_raw(req.data)
        } else {
            match UbfBuffer::new(2048) {
                Ok(buf) => buf,
                Err(e) => {
                    tplog_error(&format!("Failed to allocate UBF buffer: {}", e));
                    tpreturn_fail(rqst);
                    return;
                }
            }
        };
        
        // Add multiple fields
        let _ = ubf.add_string(T_STRING_FLD, "Test String");
        let _ = ubf.add_string(T_NAME_FLD, "John Doe");
        let _ = ubf.add_string(T_STATUS_FLD, "Added");
        let _ = ubf.add_long(T_ID_FLD, 12345);
        let _ = ubf.add_long(T_COUNT_FLD, 100);
        let _ = ubf.add_double(T_PRICE_FLD, 99.99);
        
        tplog_info(&format!("UBFADD: Added fields, used={} bytes", ubf.used()));
        
        let ptr = ubf.into_raw();
        let len = endurox_sys::ffi::Bused(ptr);
        
        endurox_sys::ffi::tpreturn(endurox_sys::TPSUCCESS, 0, ptr, len, 0);
    }
}

/// UBFGET - Get fields from UBF buffer
extern "C" fn service_ubfget(rqst: *mut TpSvcInfoRaw) {
    tplog_info("UBFGET service called");
    
    unsafe {
        let req = &*rqst;
        
        if req.data.is_null() {
            tplog_error("UBFGET: No data received");
            tpreturn_fail(rqst);
            return;
        }
        
        let ubf = UbfBuffer::from_raw(req.data);
        
        // Try to read various fields
        if let Ok(name) = ubf.get_string(T_NAME_FLD, 0) {
            tplog_info(&format!("UBFGET: T_NAME_FLD={}", name));
        }
        
        if let Ok(id) = ubf.get_long(T_ID_FLD, 0) {
            tplog_info(&format!("UBFGET: T_ID_FLD={}", id));
        }
        
        if let Ok(price) = ubf.get_double(T_PRICE_FLD, 0) {
            tplog_info(&format!("UBFGET: T_PRICE_FLD={}", price));
        }
        
        // Echo back
        let ptr = ubf.into_raw();
        let len = endurox_sys::ffi::Bused(ptr);
        
        endurox_sys::ffi::tpreturn(endurox_sys::TPSUCCESS, 0, ptr, len, 0);
    }
}

// Server initialization
#[no_mangle]
pub extern "C" fn tpsvrinit(_argc: libc::c_int, _argv: *mut *mut libc::c_char) -> libc::c_int {
    tplog_info("ubfsvr_rust starting...");
    
    let services = [
        ("UBFECHO", service_ubfecho as extern "C" fn(*mut TpSvcInfoRaw)),
        ("UBFTEST", service_ubftest as extern "C" fn(*mut TpSvcInfoRaw)),
        ("UBFADD", service_ubfadd as extern "C" fn(*mut TpSvcInfoRaw)),
        ("UBFGET", service_ubfget as extern "C" fn(*mut TpSvcInfoRaw)),
    ];
    
    for (service_name, handler) in &services {
        match advertise_service(service_name, *handler) {
            Ok(_) => tplog_info(&format!("Successfully advertised {}", service_name)),
            Err(e) => {
                tplog_error(&format!("Failed to advertise {}: {}", service_name, e));
                return -1;
            }
        }
    }
    
    tplog_info("ubfsvr_rust initialized successfully");
    0
}

// Server shutdown
#[no_mangle]
pub extern "C" fn tpsvrdone() {
    tplog_info("ubfsvr_rust shutting down...");
}

// Main function
fn main() -> ! {
    run_server(tpsvrinit, tpsvrdone)
}
