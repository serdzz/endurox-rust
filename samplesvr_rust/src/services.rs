use crate::ubf_fields::*;
use endurox_rs::{
    tp_error, tp_info, AtmiCtx, TpReturnStatus, TpSvcInfo, TypedUbf, UbfDeserialize, UbfSerialize,
};
use serde::{Deserialize, Serialize};

/// Outcome of a service handler, consumed by the dispatcher in `main.rs`.
#[derive(Debug)]
pub enum ServiceResult<'ctx> {
    /// Success with a plain STRING response.
    Message(String),
    /// Success with a UBF response buffer.
    Ubf(TypedUbf<'ctx>),
    /// Failure with a log message; TPFAIL is returned to the caller.
    Error(String),
    /// Failure with error details carried in a UBF buffer.
    ErrorUbf(TypedUbf<'ctx>),
}

/// Send the handler outcome back to the caller via tpreturn.
pub fn send_response<'ctx>(
    ctx: &'ctx AtmiCtx,
    svc: &mut TpSvcInfo<'ctx>,
    result: ServiceResult<'ctx>,
) {
    match result {
        ServiceResult::Message(msg) => {
            tp_info!(ctx, "Service responded successfully: {}", msg);
            match ctx.tpalloc("STRING", "", msg.len() + 1) {
                Ok(mut buf) => {
                    let mut bytes = msg.into_bytes();
                    bytes.push(0);
                    if let Err(e) = buf.set_bytes(&bytes) {
                        tp_error!(ctx, "Failed to fill return buffer: {}", e);
                        ctx.tpreturn(TpReturnStatus::Fail, 0, buf, 0);
                        return;
                    }
                    ctx.tpreturn(TpReturnStatus::Success, 0, buf, 0);
                }
                Err(e) => {
                    tp_error!(ctx, "Failed to allocate return buffer: {}", e);
                    fail_with_request(ctx, svc);
                }
            }
        }
        ServiceResult::Ubf(ubf) => {
            tp_info!(ctx, "Service responded successfully with UBF buffer");
            ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
        }
        ServiceResult::Error(msg) => {
            tp_error!(ctx, "Service responded with error: {}", msg);
            fail_with_request(ctx, svc);
        }
        ServiceResult::ErrorUbf(ubf) => {
            tp_error!(ctx, "Service responded with UBF error");
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 0, ubf, 0);
        }
    }
}

/// Return TPFAIL reusing the request buffer (or a fresh one if already taken).
fn fail_with_request<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) {
    let buf = match svc.take_data() {
        Some(buf) => buf,
        None => match ctx.tpalloc_ubf(256) {
            Ok(ubf) => ubf.into_inner(),
            Err(e) => {
                tp_error!(ctx, "Failed to allocate failure buffer: {}", e);
                return;
            }
        },
    };
    ctx.tpreturn(TpReturnStatus::Fail, 0, buf, 0);
}

pub fn echo_service<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) -> ServiceResult<'ctx> {
    tp_info!(ctx, "Echo service called for service: {}", svc.name());
    ServiceResult::Message(format!("Echoed: {}", svc.name()))
}

pub fn hello_service<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) -> ServiceResult<'ctx> {
    tp_info!(ctx, "Hello service called for service: {}", svc.name());
    ServiceResult::Message("Hello from Rust!".to_string())
}

pub fn status_service<'ctx>(ctx: &'ctx AtmiCtx, svc: &mut TpSvcInfo<'ctx>) -> ServiceResult<'ctx> {
    tp_info!(ctx, "Status service called for service: {}", svc.name());
    ServiceResult::Message("Status: OK".to_string())
}

pub fn dataproc_service<'ctx>(
    ctx: &'ctx AtmiCtx,
    svc: &mut TpSvcInfo<'ctx>,
) -> ServiceResult<'ctx> {
    tp_info!(ctx, "Dataproc service called for service: {}", svc.name());
    ServiceResult::Message("Data processed".to_string())
}

// Transaction structures with UBF derive
#[derive(Debug, Deserialize, Serialize, UbfSerialize, UbfDeserialize)]
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

#[derive(Debug, Serialize, Deserialize, UbfSerialize, UbfDeserialize)]
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

/// Serialize a transaction response into a fresh UBF buffer.
fn response_to_ubf<'ctx>(
    ctx: &'ctx AtmiCtx,
    response: &TransactionResponse,
) -> Result<TypedUbf<'ctx>, String> {
    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("Failed to create response buffer: {}", e))?;
    buf.ubf_write(response, true)
        .map_err(|e| format!("Failed to encode response: {}", e))?;
    Ok(buf)
}

pub fn transaction_service<'ctx>(
    ctx: &'ctx AtmiCtx,
    svc: &mut TpSvcInfo<'ctx>,
) -> ServiceResult<'ctx> {
    tp_info!(ctx, "Transaction service called");

    // Get UBF buffer from request
    let ubf = match svc.take_data_ubf() {
        Some(buf) => buf,
        None => {
            tp_error!(ctx, "Transaction service requires UBF buffer");

            let error_response = TransactionResponse {
                transaction_id: "unknown".to_string(),
                status: "ERROR".to_string(),
                message: "UBF buffer required".to_string(),
                error_code: Some("MISSING_BUFFER".to_string()),
                error_message: Some("Request must contain UBF buffer".to_string()),
            };

            return match response_to_ubf(ctx, &error_response) {
                Ok(buf) => ServiceResult::ErrorUbf(buf),
                Err(_) => ServiceResult::Error("UBF buffer required".to_string()),
            };
        }
    };

    // Decode transaction request
    let trans_req: TransactionRequest = match ubf.ubf_read() {
        Ok(req) => req,
        Err(e) => {
            tp_error!(ctx, "Failed to decode transaction request: {}", e);

            let error_response = TransactionResponse {
                transaction_id: "unknown".to_string(),
                status: "ERROR".to_string(),
                message: "Failed to decode request".to_string(),
                error_code: Some("DECODE_ERROR".to_string()),
                error_message: Some(e.to_string()),
            };

            return match response_to_ubf(ctx, &error_response) {
                Ok(buf) => ServiceResult::ErrorUbf(buf),
                Err(_) => ServiceResult::Error(format!("Decode error: {}", e)),
            };
        }
    };

    tp_info!(
        ctx,
        "Processing transaction: id={}, type={}, account={}, amount={}, currency={}",
        trans_req.transaction_id,
        trans_req.transaction_type,
        trans_req.account,
        trans_req.amount,
        trans_req.currency
    );

    // Check if transaction type is "sale"
    let (status, message, error_code, error_message) =
        if trans_req.transaction_type.to_lowercase() != "sale" {
            tp_error!(
                ctx,
                "Transaction validation failed: expected 'sale', got '{}'",
                trans_req.transaction_type
            );
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
            tp_info!(
                ctx,
                "Transaction {} validated successfully",
                trans_req.transaction_id
            );
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

    // Always return SUCCESS - error details are in the UBF buffer
    match response_to_ubf(ctx, &response) {
        Ok(buf) => ServiceResult::Ubf(buf),
        Err(e) => {
            tp_error!(ctx, "{}", e);
            ServiceResult::Error(e)
        }
    }
}
