# Transaction API with UBF

This document demonstrates the complex UBF integration in the REST gateway using the `/api/transaction` endpoint.

## Overview

The transaction endpoint showcases:
- **JSON → UBF → JSON** conversion pipeline
- **Automatic struct serialization** using `#[derive(UbfStruct)]`
- **Complex validation logic** on the server side
- **Error handling** with detailed error codes and messages

## Architecture

```
┌──────────┐         ┌──────────────┐         ┌──────────────┐
│  Client  │  JSON   │ rest_gateway │   UBF   │ samplesvr_   │
│          │ ──────> │ (Actix-web)  │ ──────> │    rust      │
│  (curl)  │         │              │         │              │
│          │ <────── │              │ <────── │              │
└──────────┘  JSON   └──────────────┘   UBF   └──────────────┘
```

### Data Flow

1. **Client** sends JSON request to `/api/transaction`
2. **REST Gateway** converts JSON to UBF using `UbfStruct::update_ubf()`
3. **Enduro/X** routes UBF buffer to `TRANSACTION` service
4. **samplesvr_rust** decodes UBF, validates, and encodes response
5. **REST Gateway** converts UBF response back to JSON
6. **Client** receives JSON response

## Request Structure

```rust
#[derive(Debug, Deserialize, Serialize, UbfStruct)]
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
```

## Response Structure

```rust
#[derive(Debug, Serialize, Deserialize, UbfStruct)]
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
```

## Examples

### Successful Sale Transaction

**Request:**
```bash
curl -X POST http://localhost:8080/api/transaction \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "sale",
    "transaction_id": "TXN-12345",
    "account": "ACC-9876",
    "amount": 15000,
    "currency": "USD",
    "description": "Payment for order #12345"
  }'
```

**Response:**
```json
{
  "transaction_id": "TXN-12345",
  "status": "SUCCESS",
  "message": "Transaction TXN-12345 processed successfully"
}
```

### Failed Validation (Wrong Type)

**Request:**
```bash
curl -X POST http://localhost:8080/api/transaction \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "refund",
    "transaction_id": "TXN-12346",
    "account": "ACC-9876",
    "amount": 5000,
    "currency": "USD"
  }'
```

**Response:**
```json
{
  "transaction_id": "TXN-12346",
  "status": "ERROR",
  "message": "Transaction validation failed",
  "error": {
    "code": "INVALID_TYPE",
    "message": "Expected 'sale' transaction type, got 'refund'"
  }
}
```

## Server-Side Validation

The service validates that `transaction_type` must be `"sale"`:

```rust
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
            Some(format!("Expected 'sale' transaction type, got '{}'", trans_req.transaction_type)),
        )
    } else {
        tplog_info(&format!("Transaction {} validated successfully", trans_req.transaction_id));
        (
            "SUCCESS".to_string(),
            format!("Transaction {} processed successfully", trans_req.transaction_id),
            None,
            None,
        )
    };
```

## UBF Fields Used

The transaction uses the following UBF fields from `ubftab/test.fd`:

| Field Constant        | Field ID | Type   | Description              |
|-----------------------|----------|--------|--------------------------|
| T_TRANS_TYPE_FLD     | 1050     | string | Transaction type         |
| T_TRANS_ID_FLD       | 1051     | string | Transaction identifier   |
| T_ACCOUNT_FLD        | 1052     | string | Account number           |
| T_AMOUNT_FLD         | 1014     | long   | Transaction amount       |
| T_CURRENCY_FLD       | 1053     | string | Currency code            |
| T_DESC_FLD           | 1054     | string | Optional description     |
| T_STATUS_FLD         | 1004     | string | Response status          |
| T_MESSAGE_FLD        | 1003     | string | Response message         |
| T_ERROR_CODE_FLD     | 1055     | string | Error code (if any)      |
| T_ERROR_MSG_FLD      | 1056     | string | Error message (if any)   |

## Key Features

### 1. Automatic UBF Serialization

The `#[derive(UbfStruct)]` macro automatically generates code to:
- Convert struct fields to UBF buffer fields
- Handle optional fields with `Option<T>`
- Map field names to UBF field IDs using constants

### 2. Type Safety

Strong typing ensures:
- Correct field types (String, i64, etc.)
- Compile-time validation of field mappings
- No manual buffer manipulation required

### 3. Error Handling

Comprehensive error handling at multiple levels:
- **Encoding errors** - Invalid data during JSON → UBF conversion
- **Transport errors** - Service call failures
- **Decoding errors** - Invalid data during UBF → JSON conversion
- **Business logic errors** - Application-level validation failures

### 4. Optional Fields

Support for optional fields using `Option<T>`:
```rust
#[ubf(field = T_DESC_FLD)]
#[serde(skip_serializing_if = "Option::is_none")]
description: Option<String>,
```

## Testing

Run the full test suite:
```bash
./test_rest.sh
```

This will test all endpoints including both successful and failed transaction scenarios.

## Implementation Details

### REST Gateway (Actix-web)

```rust
async fn call_transaction(
    data: web::Data<AppState>,
    payload: web::Json<TransactionRequest>,
) -> impl Responder {
    // 1. Create UBF buffer
    let mut ubf_buf = UbfBuffer::new(1024)?;
    
    // 2. Encode JSON to UBF
    payload.update_ubf(&mut ubf_buf)?;
    
    // 3. Call service
    let response_data = client.call_service_ubf_blocking("TRANSACTION", &buffer_data)?;
    
    // 4. Decode UBF response
    let response_buf = UbfBuffer::from_bytes(&response_data)?;
    let trans_response = TransactionResponse::from_ubf(&response_buf)?;
    
    // 5. Convert to JSON and return
    HttpResponse::Ok().json(json_response)
}
```

### Server Implementation

```rust
pub fn transaction_service(request: &ServiceRequest) -> ServiceResult {
    // 1. Get UBF buffer from request
    let ubf_buf = &request.ubf_buffer?;
    
    // 2. Decode UBF to struct
    let trans_req = TransactionRequest::from_ubf(ubf_buf)?;
    
    // 3. Validate business logic
    if trans_req.transaction_type != "sale" {
        return error_with_details(...);
    }
    
    // 4. Create response struct
    let response = TransactionResponse { ... };
    
    // 5. Encode to UBF
    let mut response_buf = UbfBuffer::new(1024)?;
    response.update_ubf(&mut response_buf)?;
    
    // 6. Return UBF buffer
    ServiceResult::success_ubf(response_buf)
}
```

## Benefits

1. **Type Safety** - Compile-time guarantees for data structure mapping
2. **Maintainability** - Clear struct definitions instead of manual buffer manipulation
3. **Productivity** - Automatic serialization reduces boilerplate
4. **Reliability** - Comprehensive error handling at every layer
5. **Performance** - Zero-copy buffer operations where possible

## Future Enhancements

Possible improvements:
- Support for nested UBF structures
- Array field support
- Custom validation macros
- Async service calls in REST gateway
- Transaction batching support
