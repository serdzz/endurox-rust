use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use endurox_sys::client::EnduroxClient;
use endurox_sys::{tplog_info, tplog_error};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

// Message type for Enduro/X worker thread
struct ServiceRequest {
    service: String,
    data: String,
    response_tx: oneshot::Sender<Result<String, String>>,
}

// Async wrapper для EnduroxClient
struct AsyncEnduroxClient {
    request_tx: mpsc::UnboundedSender<ServiceRequest>,
}

impl AsyncEnduroxClient {
    fn new() -> Self {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<ServiceRequest>();

        // Spawn dedicated thread for Enduro/X operations
        std::thread::spawn(move || {
            // Initialize Enduro/X client IN THIS THREAD
            let client = match EnduroxClient::new() {
                Ok(c) => c,
                Err(e) => {
                    tplog_error(&format!("Failed to initialize Enduro/X client in worker thread: {}", e));
                    return;
                }
            };
            
            tplog_info("Enduro/X client initialized in worker thread");
            
            // Process requests
            while let Some(req) = request_rx.blocking_recv() {
                let result = client.call_service_blocking(&req.service, &req.data);
                let _ = req.response_tx.send(result);
            }
        });

        Self { request_tx }
    }

    async fn call_service(&self, service: &str, data: &str) -> Result<String, String> {
        let (response_tx, response_rx) = oneshot::channel();
        
        self.request_tx
            .send(ServiceRequest {
                service: service.to_string(),
                data: data.to_string(),
                response_tx,
            })
            .map_err(|e| format!("Failed to send request: {}", e))?;
        
        response_rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))?
    }
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

// Application state
struct AppState {
    endurox_client: Arc<AsyncEnduroxClient>,
}

#[tokio::main]
async fn main() {
    // Initialize Enduro/X client (will be done in dedicated thread)
    let endurox_client = Arc::new(AsyncEnduroxClient::new());

    tplog_info("REST Gateway starting...");

    let state = Arc::new(AppState { endurox_client });

    // Build router
    let app = Router::new()
        .route("/", get(health_check))
        .route("/api/status", get(call_status))
        .route("/api/hello", post(call_hello))
        .route("/api/echo", post(call_echo))
        .route("/api/dataproc", post(call_dataproc))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");

    tplog_info("REST Gateway listening on http://0.0.0.0:8080");
    println!("REST Gateway listening on http://0.0.0.0:8080");

    axum::serve(listener, app).await.expect("Server failed");
}

// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

// STATUS service endpoint
async fn call_status(
    State(state): State<Arc<AppState>>,
) -> Response {
    tplog_info("REST API: Calling STATUS service");

    match state.endurox_client.call_service("STATUS", "").await {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            Json(ServiceResponse {
                result,
                error: None,
            }).into_response()
        }
        Err(e) => {
            tplog_error(&format!("STATUS call failed: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR, 
             Json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })).into_response()
        }
    }
}

// HELLO service endpoint
async fn call_hello(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HelloRequest>,
) -> Response {
    tplog_info(&format!("REST API: Calling HELLO with name={}", payload.name));

    let request_json = serde_json::json!({
        "name": payload.name
    })
    .to_string();

    match state
        .endurox_client
        .call_service("HELLO", &request_json)
        .await
    {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            Json(ServiceResponse {
                result,
                error: None,
            }).into_response()
        }
        Err(e) => {
            tplog_error(&format!("HELLO call failed: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR,
             Json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })).into_response()
        }
    }
}

// ECHO service endpoint
async fn call_echo(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Response {
    tplog_info(&format!("REST API: Calling ECHO with data: {}", body));

    match state.endurox_client.call_service("ECHO", &body).await {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            Json(ServiceResponse {
                result,
                error: None,
            }).into_response()
        }
        Err(e) => {
            tplog_error(&format!("ECHO call failed: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR,
             Json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })).into_response()
        }
    }
}

// DATAPROC service endpoint
async fn call_dataproc(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Response {
    tplog_info(&format!(
        "REST API: Calling DATAPROC with {} bytes",
        body.len()
    ));

    match state.endurox_client.call_service("DATAPROC", &body).await {
        Ok(result) => {
            let result = result.trim_end_matches('\0').to_string();
            Json(ServiceResponse {
                result,
                error: None,
            }).into_response()
        }
        Err(e) => {
            tplog_error(&format!("DATAPROC call failed: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR,
             Json(ServiceResponse {
                result: String::new(),
                error: Some(format!("Service call failed: {}", e)),
            })).into_response()
        }
    }
}
