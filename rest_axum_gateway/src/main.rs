//! REST gateway for Enduro/X services, built on axum + tokio.
//!
//! Functionally identical to the Actix-based `rest_gateway` crate (same
//! endpoints, same JSON shapes, same UBF mapping), but on a different HTTP
//! stack.
//!
//! # Concurrency design: bounded blocking layer for ATMI
//!
//! `AtmiCtx` from endurox-rs is `!Send`/`!Sync`: an ATMI context (and its
//! reply queue) belongs to the OS thread that created it and must never move.
//! axum handlers run on the multi-threaded tokio runtime, so Enduro/X calls
//! are routed through `tokio::task::spawn_blocking`. Each blocking-pool
//! thread lazily creates its own `thread_local!` `AtmiCtx` on first use —
//! the same per-thread-context pattern the Actix gateway uses for its worker
//! threads.
//!
//! The blocking pool is capped via `max_blocking_threads` on the runtime
//! builder (env `REST_WORKERS`, default 2, clamped to 1..=4). Every blocking
//! thread that touches ATMI owns a client reply queue, and hosts with a small
//! RLIMIT_MSGQUEUE cannot afford many of them (tpinit fails with TPEOS
//! "Too many open files" otherwise). tokio keeps blocking threads alive for
//! a while between uses, so in steady state the same few threads (and hence
//! the same few AtmiCtx instances) are reused.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use endurox_rs::{tp_error, tp_info, AtmiCtx, TypedBuffer, UbfDeserialize, UbfSerialize};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::*;

thread_local! {
    // One ATMI context per blocking-pool thread (see module docs). AtmiCtx is
    // !Send/!Sync, so it must never leave the thread that created it; the
    // blocking tpcall API is used directly from spawn_blocking closures.
    static CLIENT: RefCell<Option<AtmiCtx>> = const { RefCell::new(None) };
}

fn with_ctx<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&AtmiCtx) -> Result<R, String>,
{
    CLIENT.with(|c| {
        let mut slot = c.borrow_mut();
        if slot.is_none() {
            let ctx = AtmiCtx::new().map_err(|e| format!("failed to create context: {}", e))?;
            ctx.tpinit().map_err(|e| format!("tpinit failed: {}", e))?;
            *slot = Some(ctx);
        }
        f(slot.as_ref().unwrap())
    })
}

/// Run `f` on the bounded blocking pool where per-thread AtmiCtx instances
/// live. All ATMI traffic funnels through here.
async fn atmi_blocking<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce() -> Result<R, String> + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("blocking task failed: {}", e))?
}

/// Call a service with a STRING request buffer and return the STRING reply.
fn call_service_string(ctx: &AtmiCtx, service: &str, data: &str) -> Result<String, String> {
    let mut req = ctx
        .tpalloc("STRING", "", data.len() + 1)
        .map_err(|e| format!("tpalloc failed: {}", e))?;
    let mut bytes = data.as_bytes().to_vec();
    bytes.push(0);
    req.set_bytes(&bytes)
        .map_err(|e| format!("failed to fill request buffer: {}", e))?;

    let mut rsp = ctx
        .tpalloc("STRING", "", 1024)
        .map_err(|e| format!("reply tpalloc failed: {}", e))?;
    ctx.tpcall(service, &req, &mut rsp, 0)
        .map_err(|e| format!("{}", e))?;

    Ok(buffer_to_string(&rsp))
}

fn buffer_to_string(buf: &TypedBuffer<'_>) -> String {
    let bytes = buf.as_bytes();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Call a service with a UBF request built from `request` and decode the
/// UBF reply into `R`.
fn call_service_ubf<T, R>(ctx: &AtmiCtx, service: &str, request: &T) -> Result<R, String>
where
    T: UbfSerialize,
    R: UbfDeserialize,
{
    let mut req = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("Failed to create UBF buffer: {}", e))?;
    req.ubf_write(request, true)
        .map_err(|e| format!("ENCODING_ERROR: {}", e))?;

    let mut rsp = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("Failed to create UBF reply buffer: {}", e))?;
    ctx.tpcall(service, &req, &mut rsp, 0)
        .map_err(|e| format!("{}", e))?;

    rsp.ubf_read().map_err(|e| format!("DECODING_ERROR: {}", e))
}

#[derive(Debug, Deserialize)]
struct HelloRequest {
    name: String,
}

#[derive(Debug, Serialize)]
struct ServiceResponse {
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

// Transaction request/response structures
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,

    #[ubf(field = T_ERROR_MSG_FLD)]
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
struct TransactionJsonResponse {
    transaction_id: String,
    status: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorDetail>,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

// Get transaction request
#[derive(Debug, Deserialize, Serialize, UbfSerialize, UbfDeserialize)]
struct GetTransactionRequest {
    #[ubf(field = T_TRANS_ID_FLD)]
    transaction_id: String,
}

// Empty UBF request (e.g. for LIST_TXN)
#[derive(Debug, UbfSerialize)]
struct EmptyRequest {}

// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Common string-service handler body (runs on the blocking pool).
async fn string_service_call(service: &'static str, payload: String, log: String) -> Response {
    let result = atmi_blocking(move || {
        with_ctx(|ctx| {
            tp_info!(ctx, "REST API: {}", log);
            call_service_string(ctx, service, &payload)
        })
    })
    .await;

    match result {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            Json(ServiceResponse {
                result,
                error: None,
            })
            .into_response()
        }
        Err(e) => {
            let msg = e.clone();
            let _ = atmi_blocking(move || {
                with_ctx(|ctx| {
                    tp_error!(ctx, "{} call failed: {}", service, msg);
                    Ok(())
                })
            })
            .await;
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ServiceResponse {
                    result: String::new(),
                    error: Some(format!("Service call failed: {}", e)),
                }),
            )
                .into_response()
        }
    }
}

// STATUS service endpoint
async fn call_status() -> Response {
    string_service_call("STATUS", String::new(), "Calling STATUS service".to_string()).await
}

// HELLO service endpoint
async fn call_hello(Json(payload): Json<HelloRequest>) -> Response {
    let request_json = serde_json::json!({
        "name": payload.name
    })
    .to_string();

    string_service_call(
        "HELLO",
        request_json,
        format!("Calling HELLO with name={}", payload.name),
    )
    .await
}

// ECHO service endpoint
async fn call_echo(body: String) -> Response {
    let log = format!("Calling ECHO with data: {}", body);
    string_service_call("ECHO", body, log).await
}

// DATAPROC service endpoint
async fn call_dataproc(body: String) -> Response {
    let log = format!("Calling DATAPROC with {} bytes", body.len());
    string_service_call("DATAPROC", body, log).await
}

/// Common UBF transaction-service handler body (runs on the blocking pool).
async fn transaction_service_call<T: UbfSerialize + Send + 'static>(
    service: &'static str,
    request: T,
    transaction_id: String,
    log: String,
) -> Response {
    let result: Result<TransactionResponse, String> = atmi_blocking(move || {
        with_ctx(|ctx| {
            tp_info!(ctx, "REST API: {}", log);
            call_service_ubf(ctx, service, &request)
        })
    })
    .await;

    match result {
        Ok(trans_response) => {
            // Convert to JSON response
            let json_response = TransactionJsonResponse {
                transaction_id: trans_response.transaction_id,
                status: trans_response.status,
                message: trans_response.message,
                error: match (trans_response.error_code, trans_response.error_message) {
                    (Some(code), Some(msg)) => Some(ErrorDetail { code, message: msg }),
                    _ => None,
                },
            };
            Json(json_response).into_response()
        }
        Err(e) => {
            let msg = e.clone();
            let _ = atmi_blocking(move || {
                with_ctx(|ctx| {
                    tp_error!(ctx, "{} call failed: {}", service, msg);
                    Ok(())
                })
            })
            .await;
            let (status, code) = if e.starts_with("ENCODING_ERROR") {
                (StatusCode::BAD_REQUEST, "ENCODING_ERROR")
            } else if e.starts_with("DECODING_ERROR") {
                (StatusCode::INTERNAL_SERVER_ERROR, "DECODING_ERROR")
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "SERVICE_ERROR")
            };
            (
                status,
                Json(TransactionJsonResponse {
                    transaction_id,
                    status: "ERROR".to_string(),
                    message: "Service call failed".to_string(),
                    error: Some(ErrorDetail {
                        code: code.to_string(),
                        message: e,
                    }),
                }),
            )
                .into_response()
        }
    }
}

// Oracle CREATE_TXN service endpoint
async fn create_oracle_transaction(Json(payload): Json<TransactionRequest>) -> Response {
    let transaction_id = payload.transaction_id.clone();
    let log = format!(
        "Creating Oracle transaction {} of type {} for account {}",
        transaction_id, payload.transaction_type, payload.account
    );
    transaction_service_call("CREATE_TXN", payload, transaction_id, log).await
}

// Oracle GET_TXN service endpoint
async fn get_oracle_transaction(Json(payload): Json<GetTransactionRequest>) -> Response {
    let transaction_id = payload.transaction_id.clone();
    let log = format!("Getting Oracle transaction {}", transaction_id);
    transaction_service_call("GET_TXN", payload, transaction_id, log).await
}

// Oracle LIST_TXN service endpoint
async fn list_oracle_transactions() -> Response {
    transaction_service_call(
        "LIST_TXN",
        EmptyRequest {},
        String::new(),
        "Listing Oracle transactions".to_string(),
    )
    .await
}

// TRANSACTION service endpoint with UBF (legacy, calls samplesvr_rust)
async fn call_transaction(Json(payload): Json<TransactionRequest>) -> Response {
    let transaction_id = payload.transaction_id.clone();
    let log = format!(
        "Processing transaction {} of type {} for account {}",
        transaction_id, payload.transaction_type, payload.account
    );
    transaction_service_call("TRANSACTION", payload, transaction_id, log).await
}

fn main() -> std::io::Result<()> {
    println!("REST Gateway (axum) starting...");

    // Bounded blocking pool: each blocking thread that calls into ATMI owns a
    // thread_local AtmiCtx with its own reply queue, so keep the pool small
    // (REST_WORKERS, default 2, max 4) — see module docs.
    let workers: usize = std::env::var("REST_WORKERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
        .clamp(1, 4);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8081);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .max_blocking_threads(workers)
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let app = Router::new()
            .route("/", get(health_check))
            .route("/api/status", get(call_status))
            .route("/api/hello", post(call_hello))
            .route("/api/echo", post(call_echo))
            .route("/api/dataproc", post(call_dataproc))
            .route("/api/transaction", post(call_transaction))
            // Oracle transaction endpoints
            .route("/api/oracle/create", post(create_oracle_transaction))
            .route("/api/oracle/get", post(get_oracle_transaction))
            .route("/api/oracle/list", get(list_oracle_transactions));

        println!("REST Gateway (axum) listening on http://0.0.0.0:{}", port);
        println!("Blocking workers (ATMI contexts): {}", workers);

        let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
        axum::serve(listener, app).await
    })
}
