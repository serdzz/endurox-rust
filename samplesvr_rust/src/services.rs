use endurox_sys::server::tpreturn_fail;
use endurox_sys::ubf::UbfBuffer;
use endurox_sys::ubf_fields::*;
use endurox_sys::ubf_struct::UbfStruct;
use endurox_sys::UbfStruct as UbfStructDerive;
use endurox_sys::{tplog_error, tplog_info, TpSvcInfoRaw};
use serde::{Deserialize, Serialize};
use std::ffi::CStr;

#[derive(Debug)]
pub struct ServiceRequest {
    pub service_name: String,
    pub ubf_buffer: Option<UbfBuffer>,
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

        // Try to get UBF buffer if data is present
        let ubf_buffer = unsafe {
            let req = &*rqst;
            if !req.data.is_null() && req.len > 0 {
                let buffer_data =
                    std::slice::from_raw_parts(req.data as *const u8, req.len as usize);
                UbfBuffer::from_bytes(buffer_data).ok()
            } else {
                None
            }
        };

        Ok(ServiceRequest {
            service_name,
            ubf_buffer,
        })
    }

    pub fn service_name(&self) -> String {
        self.service_name.clone()
    }
}

#[derive(Debug)]
pub struct ServiceResult {
    pub success: bool,
    pub message: String,
    pub ubf_buffer: Option<UbfBuffer>,
}

impl ServiceResult {
    pub fn success(message: &str) -> Self {
        ServiceResult {
            success: true,
            message: message.to_string(),
            ubf_buffer: None,
        }
    }

    pub fn success_ubf(ubf_buffer: UbfBuffer) -> Self {
        ServiceResult {
            success: true,
            message: String::new(),
            ubf_buffer: Some(ubf_buffer),
        }
    }

    pub fn error(message: &str) -> Self {
        ServiceResult {
            success: false,
            message: message.to_string(),
            ubf_buffer: None,
        }
    }

    pub fn error_ubf(ubf_buffer: UbfBuffer) -> Self {
        ServiceResult {
            success: false,
            message: String::new(),
            ubf_buffer: Some(ubf_buffer),
        }
    }

    pub fn send_response(&self, rqst: *mut TpSvcInfoRaw) -> Result<(), String> {
        unsafe {
            if self.success {
                use endurox_sys::ffi;
                use libc::c_long;
                use std::ffi::CString;

                let req = &*rqst;

                // Check if we have UBF buffer to send
                if let Some(ref ubf_buf) = self.ubf_buffer {
                    tplog_info("Service responded successfully with UBF buffer");

                    let buffer_data = ubf_buf.as_bytes();
                    let needed_len = buffer_data.len();

                    // Allocate or reallocate UBF buffer
                    let ret_buf = if req.data.is_null() {
                        let ubf_type = CString::new("UBF").unwrap();
                        ffi::tpalloc(ubf_type.as_ptr(), std::ptr::null(), needed_len as c_long)
                    } else {
                        ffi::tprealloc(req.data, needed_len as c_long)
                    };

                    if ret_buf.is_null() {
                        tplog_error("Failed to allocate UBF return buffer");
                        tpreturn_fail(rqst);
                        return Ok(());
                    }

                    // Copy UBF data to buffer
                    std::ptr::copy_nonoverlapping(
                        buffer_data.as_ptr(),
                        ret_buf as *mut u8,
                        buffer_data.len(),
                    );

                    ffi::tpreturn(ffi::TPSUCCESS, 0, ret_buf, buffer_data.len() as c_long, 0);
                } else {
                    // String response
                    tplog_info(&format!("Service responded successfully: {}", self.message));

                    let msg_bytes = self.message.as_bytes();
                    let needed_len = msg_bytes.len() + 1;

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

                    std::ptr::copy_nonoverlapping(
                        msg_bytes.as_ptr(),
                        ret_buf as *mut u8,
                        msg_bytes.len(),
                    );
                    *ret_buf.add(msg_bytes.len()) = 0;

                    ffi::tpreturn(ffi::TPSUCCESS, 0, ret_buf, msg_bytes.len() as c_long, 0);
                }
            } else {
                // Error case
                if let Some(ref ubf_buf) = self.ubf_buffer {
                    tplog_error("Service responded with UBF error");

                    use endurox_sys::ffi;
                    use libc::c_long;
                    use std::ffi::CString;

                    let req = &*rqst;
                    let buffer_data = ubf_buf.as_bytes();
                    let needed_len = buffer_data.len();

                    let ret_buf = if req.data.is_null() {
                        let ubf_type = CString::new("UBF").unwrap();
                        ffi::tpalloc(ubf_type.as_ptr(), std::ptr::null(), needed_len as c_long)
                    } else {
                        ffi::tprealloc(req.data, needed_len as c_long)
                    };

                    if !ret_buf.is_null() {
                        std::ptr::copy_nonoverlapping(
                            buffer_data.as_ptr(),
                            ret_buf as *mut u8,
                            buffer_data.len(),
                        );
                        ffi::tpreturn(ffi::TPFAIL, 0, ret_buf, buffer_data.len() as c_long, 0);
                    } else {
                        tpreturn_fail(rqst);
                    }
                } else {
                    tplog_error(&format!("Service responded with error: {}", self.message));
                    tpreturn_fail(rqst);
                }
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

// Transaction structures with UBF derive
#[derive(Debug, Deserialize, Serialize, UbfStructDerive)]
struct TransactionRequest {
    #[ubf(field = T_TRANS_TYPE_FLD)]
    transaction_type: String,

    #[ubf(field = T_TRANS_ID_FLD)]
    transaction_id: String,

    #[ubf(field = T_ACCOUNT_FLD)]
    account: String,

    #[ubf(field = T_AMOUNT_FLD)]
    amount: i64,

    #[ubf(field = T_CURRENCY_FLD)]
    currency: String,

    #[ubf(field = T_DESC_FLD)]
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, UbfStructDerive)]
struct TransactionResponse {
    #[ubf(field = T_TRANS_ID_FLD)]
    transaction_id: String,

    #[ubf(field = T_STATUS_FLD)]
    status: String,

    #[ubf(field = T_MESSAGE_FLD)]
    message: String,

    #[ubf(field = T_ERROR_CODE_FLD)]
    error_code: Option<String>,

    #[ubf(field = T_ERROR_MSG_FLD)]
    error_message: Option<String>,
}

pub fn transaction_service(request: &ServiceRequest) -> ServiceResult {
    tplog_info("Transaction service called");

    // Get UBF buffer from request
    let ubf_buf = match &request.ubf_buffer {
        Some(buf) => buf,
        None => {
            tplog_error("Transaction service requires UBF buffer");

            // Return error in UBF format
            let mut error_buf = match UbfBuffer::new(512) {
                Ok(buf) => buf,
                Err(_) => return ServiceResult::error("Failed to create error buffer"),
            };

            let error_response = TransactionResponse {
                transaction_id: "unknown".to_string(),
                status: "ERROR".to_string(),
                message: "UBF buffer required".to_string(),
                error_code: Some("MISSING_BUFFER".to_string()),
                error_message: Some("Request must contain UBF buffer".to_string()),
            };

            if error_response.update_ubf(&mut error_buf).is_ok() {
                return ServiceResult::error_ubf(error_buf);
            }
            return ServiceResult::error("UBF buffer required");
        }
    };

    // Decode transaction request
    let trans_req = match TransactionRequest::from_ubf(ubf_buf) {
        Ok(req) => req,
        Err(e) => {
            tplog_error(&format!("Failed to decode transaction request: {}", e));

            let mut error_buf = match UbfBuffer::new(512) {
                Ok(buf) => buf,
                Err(_) => return ServiceResult::error("Failed to create error buffer"),
            };

            let error_response = TransactionResponse {
                transaction_id: "unknown".to_string(),
                status: "ERROR".to_string(),
                message: "Failed to decode request".to_string(),
                error_code: Some("DECODE_ERROR".to_string()),
                error_message: Some(e.to_string()),
            };

            if error_response.update_ubf(&mut error_buf).is_ok() {
                return ServiceResult::error_ubf(error_buf);
            }
            return ServiceResult::error(&format!("Decode error: {}", e));
        }
    };

    tplog_info(&format!(
        "Processing transaction: id={}, type={}, account={}, amount={}, currency={}",
        trans_req.transaction_id,
        trans_req.transaction_type,
        trans_req.account,
        trans_req.amount,
        trans_req.currency
    ));

    // Check if transaction type is "sale"
    let (status, message, error_code, error_message) =
        if trans_req.transaction_type.to_lowercase() != "sale" {
            tplog_error(&format!(
                "Transaction validation failed: expected 'sale', got '{}'",
                trans_req.transaction_type
            ));
            (
                "ERROR".to_string(),
                "Transaction validation failed".to_string(),
                Some("INVALID_TYPE".to_string()),
                Some(format!(
                    "Expected 'sale' transaction type, got '{}'",
                    trans_req.transaction_type
                )),
            )
        } else {
            tplog_info(&format!(
                "Transaction {} validated successfully",
                trans_req.transaction_id
            ));
            (
                "SUCCESS".to_string(),
                format!(
                    "Transaction {} processed successfully",
                    trans_req.transaction_id
                ),
                None,
                None,
            )
        };

    // Create response
    let response = TransactionResponse {
        transaction_id: trans_req.transaction_id,
        status,
        message,
        error_code,
        error_message,
    };

    // Encode response to UBF
    let mut response_buf = match UbfBuffer::new(1024) {
        Ok(buf) => buf,
        Err(e) => {
            tplog_error(&format!("Failed to create response buffer: {}", e));
            return ServiceResult::error("Failed to create response buffer");
        }
    };

    if let Err(e) = response.update_ubf(&mut response_buf) {
        tplog_error(&format!("Failed to encode response: {}", e));
        return ServiceResult::error(&format!("Failed to encode response: {}", e));
    }

    // Always return SUCCESS - error details are in the UBF buffer
    ServiceResult::success_ubf(response_buf)
}
