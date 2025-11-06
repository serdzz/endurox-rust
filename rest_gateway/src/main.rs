use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use endurox_sys::client::EnduroxClient;
use endurox_sys::{tplog_info, tplog_error};
use endurox_sys::ubf::UbfBuffer;
use endurox_sys::ubf_struct::UbfStruct;
use endurox_sys::ubf_fields::*;
use endurox_sys::UbfStruct as UbfStructDerive;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// Wrapper для EnduroxClient с синхронизацией
struct AppState {
    client: Arc<Mutex<EnduroxClient>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
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

// Health check endpoint
async fn health_check() -> impl Responder {
    "OK"
}

// STATUS service endpoint
async fn call_status(data: web::Data<AppState>) -> impl Responder {
    tplog_info("REST API: Calling STATUS service");
    
    let client = data.client.lock().unwrap();
    match client.call_service_blocking("STATUS", "") {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            HttpResponse::Ok().json(ServiceResponse {
                result,
                error: None,
            })
        }
        Err(e) => {
            tplog_error(&format!("STATUS call failed: {}", e));
            HttpResponse::InternalServerError().json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })
        }
    }
}

// HELLO service endpoint
async fn call_hello(
    data: web::Data<AppState>,
    payload: web::Json<HelloRequest>,
) -> impl Responder {
    tplog_info(&format!("REST API: Calling HELLO with name={}", payload.name));
    
    let request_json = serde_json::json!({
        "name": payload.name
    })
    .to_string();
    
    let client = data.client.lock().unwrap();
    match client.call_service_blocking("HELLO", &request_json) {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            HttpResponse::Ok().json(ServiceResponse {
                result,
                error: None,
            })
        }
        Err(e) => {
            tplog_error(&format!("HELLO call failed: {}", e));
            HttpResponse::InternalServerError().json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })
        }
    }
}

// ECHO service endpoint
async fn call_echo(
    data: web::Data<AppState>,
    body: String,
) -> impl Responder {
    tplog_info(&format!("REST API: Calling ECHO with data: {}", body));
    
    let client = data.client.lock().unwrap();
    match client.call_service_blocking("ECHO", &body) {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            HttpResponse::Ok().json(ServiceResponse {
                result,
                error: None,
            })
        }
        Err(e) => {
            tplog_error(&format!("ECHO call failed: {}", e));
            HttpResponse::InternalServerError().json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })
        }
    }
}

// DATAPROC service endpoint
async fn call_dataproc(
    data: web::Data<AppState>,
    body: String,
) -> impl Responder {
    tplog_info(&format!(
        "REST API: Calling DATAPROC with {} bytes",
        body.len()
    ));
    
    let client = data.client.lock().unwrap();
    match client.call_service_blocking("DATAPROC", &body) {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            HttpResponse::Ok().json(ServiceResponse {
                result,
                error: None,
            })
        }
        Err(e) => {
            tplog_error(&format!("DATAPROC call failed: {}", e));
            HttpResponse::InternalServerError().json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })
        }
    }
}

// TRANSACTION service endpoint with UBF
async fn call_transaction(
    data: web::Data<AppState>,
    payload: web::Json<TransactionRequest>,
) -> impl Responder {
    let transaction_id = payload.transaction_id.clone();
    tplog_info(&format!(
        "REST API: Processing transaction {} of type {} for account {}",
        transaction_id, payload.transaction_type, payload.account
    ));

    // Encode request to UBF
    let mut ubf_buf = match UbfBuffer::new(1024) {
        Ok(buf) => buf,
        Err(e) => {
            tplog_error(&format!("Failed to create UBF buffer: {}", e));
            return HttpResponse::InternalServerError().json(TransactionJsonResponse {
                transaction_id: transaction_id.clone(),
                status: "ERROR".to_string(),
                message: "Failed to create UBF buffer".to_string(),
                error: Some(ErrorDetail {
                    code: "INTERNAL_ERROR".to_string(),
                    message: e.to_string(),
                }),
            });
        }
    };

    if let Err(e) = payload.update_ubf(&mut ubf_buf) {
        tplog_error(&format!("Failed to encode request to UBF: {}", e));
        return HttpResponse::BadRequest().json(TransactionJsonResponse {
            transaction_id: transaction_id.clone(),
            status: "ERROR".to_string(),
            message: "Failed to encode request".to_string(),
            error: Some(ErrorDetail {
                code: "ENCODING_ERROR".to_string(),
                message: e.to_string(),
            }),
        });
    }

    // Call TRANSACTION service with UBF buffer
    let buffer_data = ubf_buf.as_bytes().to_vec();
    let client = data.client.lock().unwrap();
    
    match client.call_service_ubf_blocking("TRANSACTION", &buffer_data) {
        Ok(response_data) => {
            // Decode UBF response
            let response_buf = match UbfBuffer::from_bytes(&response_data) {
                Ok(buf) => buf,
                Err(e) => {
                    tplog_error(&format!("Failed to parse UBF response: {}", e));
                    return HttpResponse::InternalServerError().json(TransactionJsonResponse {
                        transaction_id: transaction_id.clone(),
                        status: "ERROR".to_string(),
                        message: "Failed to parse response".to_string(),
                        error: Some(ErrorDetail {
                            code: "PARSING_ERROR".to_string(),
                            message: e.to_string(),
                        }),
                    });
                }
            };

            let trans_response = match TransactionResponse::from_ubf(&response_buf) {
                Ok(resp) => resp,
                Err(e) => {
                    tplog_error(&format!("Failed to decode UBF response: {}", e));
                    return HttpResponse::InternalServerError().json(TransactionJsonResponse {
                        transaction_id: transaction_id.clone(),
                        status: "ERROR".to_string(),
                        message: "Failed to decode response".to_string(),
                        error: Some(ErrorDetail {
                            code: "DECODING_ERROR".to_string(),
                            message: e.to_string(),
                        }),
                    });
                }
            };

            // Convert to JSON response
            let json_response = TransactionJsonResponse {
                transaction_id: trans_response.transaction_id,
                status: trans_response.status,
                message: trans_response.message,
                error: match (trans_response.error_code, trans_response.error_message) {
                    (Some(code), Some(msg)) => Some(ErrorDetail {
                        code,
                        message: msg,
                    }),
                    _ => None,
                },
            };

            HttpResponse::Ok().json(json_response)
        }
        Err(e) => {
            tplog_error(&format!("TRANSACTION call failed: {}", e));
            HttpResponse::InternalServerError().json(TransactionJsonResponse {
                transaction_id: transaction_id.clone(),
                status: "ERROR".to_string(),
                message: "Service call failed".to_string(),
                error: Some(ErrorDetail {
                    code: "SERVICE_ERROR".to_string(),
                    message: e,
                }),
            })
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize Enduro/X client
    let client = match EnduroxClient::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to initialize Enduro/X client: {}", e);
            std::process::exit(1);
        }
    };

    tplog_info("REST Gateway starting...");

    let app_state = web::Data::new(AppState {
        client: Arc::new(Mutex::new(client)),
    });

    println!("REST Gateway listening on http://0.0.0.0:8080");
    tplog_info("REST Gateway listening on http://0.0.0.0:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(health_check))
            .route("/api/status", web::get().to(call_status))
            .route("/api/hello", web::post().to(call_hello))
            .route("/api/echo", web::post().to(call_echo))
            .route("/api/dataproc", web::post().to(call_dataproc))
            .route("/api/transaction", web::post().to(call_transaction))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
