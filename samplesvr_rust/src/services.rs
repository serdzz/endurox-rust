use endurox_sys::server::tpreturn_fail;
use endurox_sys::{tplog_error, tplog_info, TpSvcInfoRaw};
use std::ffi::CStr;

#[derive(Debug)]
pub struct ServiceRequest {
    // Placeholder for actual request data
    pub service_name: String,
}

impl ServiceRequest {
    pub fn from_raw(rqst: *mut TpSvcInfoRaw) -> Result<Self, String> {
        // Parse the service name from the TpSvcInfoRaw structure
        let service_name = unsafe {
            let name_array = &(*rqst).name;
            CStr::from_ptr(name_array.as_ptr())
                .to_str()
                .map_err(|e| format!("Invalid UTF-8 in service name: {}", e))?
                .to_string()
        };

        Ok(ServiceRequest { service_name })
    }

    pub fn service_name(&self) -> String {
        self.service_name.clone()
    }
}

#[derive(Debug)]
pub struct ServiceResult {
    // Placeholder for actual result data
    pub success: bool,
    pub message: String,
}

impl ServiceResult {
    pub fn success(message: &str) -> Self {
        ServiceResult {
            success: true,
            message: message.to_string(),
        }
    }

    pub fn error(message: &str) -> Self {
        ServiceResult {
            success: false,
            message: message.to_string(),
        }
    }

    pub fn send_response(&self, rqst: *mut TpSvcInfoRaw) -> Result<(), String> {
        unsafe {
            if self.success {
                tplog_info(&format!("Service responded successfully: {}", self.message));

                use endurox_sys::ffi;
                use libc::c_long;
                use std::ffi::CString;

                let req = &*rqst;
                let msg_bytes = self.message.as_bytes();
                let needed_len = msg_bytes.len() + 1;

                // Reallocate buffer to fit response
                let ret_buf = if req.data.is_null() {
                    let string_type = CString::new("STRING").unwrap();
                    ffi::tpalloc(string_type.as_ptr(), std::ptr::null(), needed_len as c_long)
                } else {
                    ffi::tprealloc(req.data, needed_len as c_long)
                };

                if ret_buf.is_null() {
                    tplog_error("Failed to allocate return buffer");
                    tpreturn_fail(rqst);
                    return Ok(());
                }

                // Copy response message to buffer
                std::ptr::copy_nonoverlapping(
                    msg_bytes.as_ptr(),
                    ret_buf as *mut u8,
                    msg_bytes.len(),
                );
                *ret_buf.add(msg_bytes.len()) = 0; // null terminate

                // Return with TPSUCCESS (2) and length without null terminator
                ffi::tpreturn(ffi::TPSUCCESS, 0, ret_buf, msg_bytes.len() as c_long, 0);
            } else {
                tplog_error(&format!("Service responded with error: {}", self.message));
                tpreturn_fail(rqst);
            }
        }
        Ok(())
    }
}

pub fn echo_service(request: &ServiceRequest) -> ServiceResult {
    tplog_info(&format!("Echo service called with request: {:?}", request));
    ServiceResult::success(&format!("Echoed: {}", request.service_name()))
}

pub fn hello_service(request: &ServiceRequest) -> ServiceResult {
    tplog_info(&format!("Hello service called with request: {:?}", request));
    ServiceResult::success("Hello from Rust!")
}

pub fn status_service(request: &ServiceRequest) -> ServiceResult {
    tplog_info(&format!(
        "Status service called with request: {:?}",
        request
    ));
    ServiceResult::success("Status: OK")
}

pub fn dataproc_service(request: &ServiceRequest) -> ServiceResult {
    tplog_info(&format!(
        "Dataproc service called with request: {:?}",
        request
    ));
    ServiceResult::success("Data processed")
}
