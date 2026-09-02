use crate::ubf_fields::*;
use diesel::prelude::*;
use endurox_rs::{
    tp_error, tp_info, AtmiCtx, TpReturnStatus, TpSvcInfo, TypedUbf, UbfDeserialize, UbfSerialize,
};
use serde::{Deserialize, Serialize};

use crate::db::{DbConnection, DbPool};
use crate::models::{NewTransaction, Transaction};
use crate::schema::transactions;

// Macro to execute database operations for both PostgreSQL and Oracle
macro_rules! execute_db {
    ($conn:expr, $operation:expr) => {
        match $conn {
            DbConnection::Postgres(ref mut pg_conn) => $operation(pg_conn),
            #[cfg(feature = "oracle")]
            DbConnection::Oracle(ref mut oci_conn) => $operation(oci_conn),
        }
    };
}

/// Outcome of a service handler, consumed by the dispatcher in `main.rs`.
#[derive(Debug)]
pub enum ServiceResult<'ctx> {
    /// Success with a plain STRING response.
    #[allow(dead_code)]
    Message(String),
    /// Success with a UBF response buffer.
    Ubf(TypedUbf<'ctx>),
    /// Failure with a log message; TPFAIL is returned to the caller.
    Error(String),
    /// Failure with error details carried in a UBF buffer.
    #[allow(dead_code)]
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

// UBF Request/Response structures
#[derive(Debug, Deserialize, Serialize, UbfSerialize, UbfDeserialize)]
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

#[derive(Debug, Deserialize, Serialize, UbfSerialize, UbfDeserialize)]
struct GetTransactionRequest {
    #[ubf(field = T_TRANS_ID_FLD)]
    transaction_id: String,
}

/// CREATE_TXN - Create new transaction in the database
pub fn create_transaction_service<'ctx>(
    ctx: &'ctx AtmiCtx,
    svc: &mut TpSvcInfo<'ctx>,
    pool: &DbPool,
) -> ServiceResult<'ctx> {
    tp_info!(ctx, "CREATE_TXN service called");

    let ubf = match svc.take_data_ubf() {
        Some(buf) => buf,
        None => {
            tp_error!(ctx, "CREATE_TXN requires UBF buffer");
            return create_error_response(ctx, "unknown", "MISSING_BUFFER", "UBF buffer required");
        }
    };

    let req: CreateTransactionRequest = match ubf.ubf_read() {
        Ok(req) => req,
        Err(e) => {
            tp_error!(ctx, "Failed to decode request: {}", e);
            return create_error_response(ctx, "unknown", "DECODE_ERROR", &e.to_string());
        }
    };

    tp_info!(
        ctx,
        "Creating transaction: id={}, type={}, account={}, amount={}",
        req.transaction_id,
        req.transaction_type,
        req.account,
        req.amount
    );

    // Validate transaction type
    if req.transaction_type.to_lowercase() != "sale" {
        tp_error!(ctx, "Invalid transaction type: {}", req.transaction_type);
        return create_error_response(
            ctx,
            &req.transaction_id,
            "INVALID_TYPE",
            &format!(
                "Only 'sale' transactions are supported, got '{}'",
                req.transaction_type
            ),
        );
    }

    // Get database connection
    let mut conn = match crate::db::get_connection(pool) {
        Ok(conn) => conn,
        Err(e) => {
            tp_error!(ctx, "Failed to get DB connection: {}", e);
            return create_error_response(ctx, &req.transaction_id, "DB_ERROR", &e);
        }
    };

    // Create new transaction
    let message = format!("Transaction {} created successfully", req.transaction_id);

    let new_txn = NewTransaction {
        id: req.transaction_id.clone(),
        transaction_type: req.transaction_type,
        account: req.account,
        amount: req.amount,
        currency: req.currency,
        description: req.description,
        status: "SUCCESS".to_string(),
        message: Some(message.clone()),
        error_code: None,
        error_message: None,
    };

    // Insert into database using Diesel
    let result = execute_db!(&mut conn, |conn| {
        diesel::insert_into(transactions::table)
            .values(&new_txn)
            .execute(conn)
    });

    match result {
        Ok(_) => {
            tp_info!(
                ctx,
                "Transaction {} created successfully",
                req.transaction_id
            );
            create_success_response(ctx, &req.transaction_id, &message)
        }
        Err(e) => {
            tp_error!(ctx, "Failed to insert transaction: {}", e);
            create_error_response(ctx, &req.transaction_id, "DB_INSERT_ERROR", &e.to_string())
        }
    }
}

/// GET_TXN - Get transaction from the database
pub fn get_transaction_service<'ctx>(
    ctx: &'ctx AtmiCtx,
    svc: &mut TpSvcInfo<'ctx>,
    pool: &DbPool,
) -> ServiceResult<'ctx> {
    tp_info!(ctx, "GET_TXN service called");

    let ubf = match svc.take_data_ubf() {
        Some(buf) => buf,
        None => {
            tp_error!(ctx, "GET_TXN requires UBF buffer");
            return create_error_response(ctx, "unknown", "MISSING_BUFFER", "UBF buffer required");
        }
    };

    let req: GetTransactionRequest = match ubf.ubf_read() {
        Ok(req) => req,
        Err(e) => {
            tp_error!(ctx, "Failed to decode request: {}", e);
            return create_error_response(ctx, "unknown", "DECODE_ERROR", &e.to_string());
        }
    };

    tp_info!(ctx, "Getting transaction: id={}", req.transaction_id);

    let mut conn = match crate::db::get_connection(pool) {
        Ok(conn) => conn,
        Err(e) => {
            tp_error!(ctx, "Failed to get DB connection: {}", e);
            return create_error_response(ctx, &req.transaction_id, "DB_ERROR", &e);
        }
    };

    // Query transaction using Diesel
    use crate::schema::transactions::dsl::*;

    let result = execute_db!(&mut conn, |conn| {
        transactions
            .filter(id.eq(&req.transaction_id))
            .first::<Transaction>(conn)
    });

    match result {
        Ok(txn) => {
            tp_info!(ctx, "Transaction {} found: status={}", txn.id, txn.status);
            create_success_response(
                ctx,
                &txn.id,
                &txn.message.unwrap_or_else(|| "OK".to_string()),
            )
        }
        Err(diesel::result::Error::NotFound) => {
            tp_error!(ctx, "Transaction {} not found", req.transaction_id);
            create_error_response(
                ctx,
                &req.transaction_id,
                "NOT_FOUND",
                "Transaction not found",
            )
        }
        Err(e) => {
            tp_error!(ctx, "Failed to query transaction: {}", e);
            create_error_response(ctx, &req.transaction_id, "DB_QUERY_ERROR", &e.to_string())
        }
    }
}

/// LIST_TXN - List all transactions
pub fn list_transactions_service<'ctx>(
    ctx: &'ctx AtmiCtx,
    _svc: &mut TpSvcInfo<'ctx>,
    pool: &DbPool,
) -> ServiceResult<'ctx> {
    tp_info!(ctx, "LIST_TXN service called");

    let mut conn = match crate::db::get_connection(pool) {
        Ok(conn) => conn,
        Err(e) => {
            tp_error!(ctx, "Failed to get DB connection: {}", e);
            return create_error_response(ctx, "", "DB_ERROR", &e);
        }
    };

    // Query all transactions using Diesel (limit 100)
    use crate::schema::transactions::dsl::*;

    let result = execute_db!(&mut conn, |conn| {
        transactions
            .order(created_at.desc())
            .limit(100)
            .load::<Transaction>(conn)
    });

    match result {
        Ok(results) => {
            let count = results.len();
            tp_info!(ctx, "Found {} transactions", count);
            let msg = format!("Found {} transactions", count);
            create_success_response(ctx, "", &msg)
        }
        Err(e) => {
            tp_error!(ctx, "Failed to list transactions: {}", e);
            create_error_response(ctx, "", "DB_QUERY_ERROR", &e.to_string())
        }
    }
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

// Helper functions to create responses
fn create_success_response<'ctx>(
    ctx: &'ctx AtmiCtx,
    transaction_id: &str,
    message: &str,
) -> ServiceResult<'ctx> {
    let response = TransactionResponse {
        transaction_id: transaction_id.to_string(),
        status: "SUCCESS".to_string(),
        message: message.to_string(),
        error_code: None,
        error_message: None,
    };

    match response_to_ubf(ctx, &response) {
        Ok(buf) => ServiceResult::Ubf(buf),
        Err(e) => {
            tp_error!(ctx, "{}", e);
            ServiceResult::Error(e)
        }
    }
}

fn create_error_response<'ctx>(
    ctx: &'ctx AtmiCtx,
    transaction_id: &str,
    error_code: &str,
    error_message: &str,
) -> ServiceResult<'ctx> {
    let response = TransactionResponse {
        transaction_id: transaction_id.to_string(),
        status: "ERROR".to_string(),
        message: "Operation failed".to_string(),
        error_code: Some(error_code.to_string()),
        error_message: Some(error_message.to_string()),
    };

    match response_to_ubf(ctx, &response) {
        // Return success with error details inside UBF, like TRANSACTION service does
        Ok(buf) => ServiceResult::Ubf(buf),
        Err(e) => {
            tp_error!(ctx, "{}", e);
            ServiceResult::Error(e)
        }
    }
}
