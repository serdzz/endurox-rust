use endurox_sys::server::tpreturn_fail;
use endurox_sys::ubf::UbfBuffer;
use endurox_sys::ubf_fields::*;
use endurox_sys::ubf_struct::UbfStruct;
use endurox_sys::UbfStruct as UbfStructDerive;
use endurox_sys::{tplog_error, tplog_info, TpSvcInfoRaw};
use serde::{Deserialize, Serialize};
use std::ffi::CStr;

use crate::db::DbPool;
use crate::models::Transaction;
use crate::schema;

#[derive(Debug)]
pub struct ServiceRequest {
    pub service_name: String,
    pub ubf_buffer: Option<UbfBuffer>,
}

impl ServiceRequest {
    pub fn from_raw(rqst: *mut TpSvcInfoRaw) -> Result<Self, String> {
        let service_name = unsafe {
            let name_array = &(*rqst).name;
            CStr::from_ptr(name_array.as_ptr())
                .to_str()
                .map_err(|e| format!("Invalid UTF-8 in service name: {}", e))?
                .to_string()
        };

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
    #[allow(dead_code)]
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

                if let Some(ref ubf_buf) = self.ubf_buffer {
                    tplog_info("Service responded successfully with UBF buffer");

                    let buffer_data = ubf_buf.as_bytes();
                    let needed_len = buffer_data.len();

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

                    std::ptr::copy_nonoverlapping(
                        buffer_data.as_ptr(),
                        ret_buf as *mut u8,
                        buffer_data.len(),
                    );

                    ffi::tpreturn(ffi::TPSUCCESS, 0, ret_buf, buffer_data.len() as c_long, 0);
                } else {
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
            } else if let Some(ref ubf_buf) = self.ubf_buffer {
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
        Ok(())
    }
}

// UBF Request/Response structures
#[derive(Debug, Deserialize, Serialize, UbfStructDerive)]
struct CreateTransactionRequest {
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

#[derive(Debug, Deserialize, Serialize, UbfStructDerive)]
struct GetTransactionRequest {
    #[ubf(field = T_TRANS_ID_FLD)]
    transaction_id: String,
}

/// CREATE_TXN - Create new transaction in Oracle DB
pub fn create_transaction_service(request: &ServiceRequest, pool: &DbPool) -> ServiceResult {
    tplog_info("CREATE_TXN service called");

    let ubf_buf = match &request.ubf_buffer {
        Some(buf) => buf,
        None => {
            tplog_error("CREATE_TXN requires UBF buffer");
            return create_error_response("unknown", "MISSING_BUFFER", "UBF buffer required");
        }
    };

    let req = match CreateTransactionRequest::from_ubf(ubf_buf) {
        Ok(req) => req,
        Err(e) => {
            tplog_error(&format!("Failed to decode request: {}", e));
            return create_error_response("unknown", "DECODE_ERROR", &e.to_string());
        }
    };

    tplog_info(&format!(
        "Creating transaction: id={}, type={}, account={}, amount={}",
        req.transaction_id, req.transaction_type, req.account, req.amount
    ));

    // Validate transaction type
    if req.transaction_type.to_lowercase() != "sale" {
        tplog_error(&format!(
            "Invalid transaction type: {}",
            req.transaction_type
        ));
        return create_error_response(
            &req.transaction_id,
            "INVALID_TYPE",
            &format!(
                "Only 'sale' transactions are supported, got '{}'",
                req.transaction_type
            ),
        );
    }

    // Get database connection
    let conn = match crate::db::get_connection(pool) {
        Ok(conn) => conn,
        Err(e) => {
            tplog_error(&format!("Failed to get DB connection: {}", e));
            return create_error_response(&req.transaction_id, "DB_ERROR", &e);
        }
    };

    // Create new transaction
    let message = format!("Transaction {} created successfully", req.transaction_id);

    // Insert into database using prepared statement
    match conn.execute(
        schema::CREATE_TRANSACTION,
        &[
            &req.transaction_id,
            &req.transaction_type,
            &req.account,
            &(req.amount as f64),
            &req.currency,
            &req.description,
            &"SUCCESS",
            &message,
            &None::<String>,
            &None::<String>,
        ],
    ) {
        Ok(_) => {
            // Commit the transaction
            if let Err(e) = conn.commit() {
                tplog_error(&format!("Failed to commit transaction: {}", e));
                return create_error_response(
                    &req.transaction_id,
                    "DB_COMMIT_ERROR",
                    &e.to_string(),
                );
            }

            tplog_info(&format!(
                "Transaction {} created successfully",
                req.transaction_id
            ));
            create_success_response(&req.transaction_id, &message)
        }
        Err(e) => {
            tplog_error(&format!("Failed to insert transaction: {}", e));
            create_error_response(&req.transaction_id, "DB_INSERT_ERROR", &e.to_string())
        }
    }
}

/// GET_TXN - Get transaction from Oracle DB
pub fn get_transaction_service(request: &ServiceRequest, pool: &DbPool) -> ServiceResult {
    tplog_info("GET_TXN service called");

    let ubf_buf = match &request.ubf_buffer {
        Some(buf) => buf,
        None => {
            tplog_error("GET_TXN requires UBF buffer");
            return create_error_response("unknown", "MISSING_BUFFER", "UBF buffer required");
        }
    };

    let req = match GetTransactionRequest::from_ubf(ubf_buf) {
        Ok(req) => req,
        Err(e) => {
            tplog_error(&format!("Failed to decode request: {}", e));
            return create_error_response("unknown", "DECODE_ERROR", &e.to_string());
        }
    };

    tplog_info(&format!("Getting transaction: id={}", req.transaction_id));

    let conn = match crate::db::get_connection(pool) {
        Ok(conn) => conn,
        Err(e) => {
            tplog_error(&format!("Failed to get DB connection: {}", e));
            return create_error_response(&req.transaction_id, "DB_ERROR", &e);
        }
    };

    // Query transaction
    let result = conn.query_row(schema::GET_TRANSACTION, &[&req.transaction_id]);

    match result {
        Ok(row) => match Transaction::from_row(&row) {
            Ok(txn) => {
                tplog_info(&format!(
                    "Transaction {} found: status={}",
                    txn.id, txn.status
                ));
                create_success_response(&txn.id, &txn.message.unwrap_or_else(|| "OK".to_string()))
            }
            Err(e) => {
                tplog_error(&format!("Failed to parse row: {}", e));
                create_error_response(&req.transaction_id, "PARSE_ERROR", &e.to_string())
            }
        },
        Err(e) if e.kind() == oracle::ErrorKind::NoDataFound => {
            tplog_error(&format!("Transaction {} not found", req.transaction_id));
            create_error_response(&req.transaction_id, "NOT_FOUND", "Transaction not found")
        }
        Err(e) => {
            tplog_error(&format!("Failed to query transaction: {}", e));
            create_error_response(&req.transaction_id, "DB_QUERY_ERROR", &e.to_string())
        }
    }
}

/// LIST_TXN - List all transactions
pub fn list_transactions_service(_request: &ServiceRequest, pool: &DbPool) -> ServiceResult {
    tplog_info("LIST_TXN service called");

    let conn = match crate::db::get_connection(pool) {
        Ok(conn) => conn,
        Err(e) => {
            tplog_error(&format!("Failed to get DB connection: {}", e));
            return create_error_response("", "DB_ERROR", &e);
        }
    };

    // Query all transactions
    match conn.query(schema::LIST_TRANSACTIONS, &[]) {
        Ok(rows) => {
            let mut count = 0;
            for row_result in rows {
                match row_result {
                    Ok(_row) => count += 1,
                    Err(e) => {
                        tplog_error(&format!("Error reading row: {}", e));
                        return create_error_response("", "ROW_ERROR", &e.to_string());
                    }
                }
            }
            tplog_info(&format!("Found {} transactions", count));
            let message = format!("Found {} transactions", count);
            create_success_response("", &message)
        }
        Err(e) => {
            tplog_error(&format!("Failed to list transactions: {}", e));
            create_error_response("", "DB_QUERY_ERROR", &e.to_string())
        }
    }
}

// Helper functions to create responses
fn create_success_response(transaction_id: &str, message: &str) -> ServiceResult {
    let response = TransactionResponse {
        transaction_id: transaction_id.to_string(),
        status: "SUCCESS".to_string(),
        message: message.to_string(),
        error_code: None,
        error_message: None,
    };

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

    ServiceResult::success_ubf(response_buf)
}

fn create_error_response(
    transaction_id: &str,
    error_code: &str,
    error_message: &str,
) -> ServiceResult {
    let response = TransactionResponse {
        transaction_id: transaction_id.to_string(),
        status: "ERROR".to_string(),
        message: "Operation failed".to_string(),
        error_code: Some(error_code.to_string()),
        error_message: Some(error_message.to_string()),
    };

    let mut response_buf = match UbfBuffer::new(1024) {
        Ok(buf) => buf,
        Err(e) => {
            tplog_error(&format!("Failed to create error buffer: {}", e));
            return ServiceResult::error("Failed to create error buffer");
        }
    };

    if let Err(e) = response.update_ubf(&mut response_buf) {
        tplog_error(&format!("Failed to encode error response: {}", e));
        return ServiceResult::error(&format!("Encode error: {}", e));
    }

    ServiceResult::error_ubf(response_buf)
}
