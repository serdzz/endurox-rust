# Changelog

## [Unreleased] - 2025-11-06

### Added

#### Transaction API with Complex UBF Integration
- **New `/api/transaction` endpoint** in REST gateway
  - Accepts JSON transaction data
  - Converts to UBF using derive macros
  - Validates transaction type on server
  - Returns detailed error messages

#### UBF Improvements
- Added `as_bytes()` and `from_bytes()` methods to `UbfBuffer`
- Implemented `Option<String>` support in `UbfStruct` derive macro
- Added `call_service_ubf_blocking()` method to `EnduroxClient`

#### New UBF Fields
Added transaction-specific fields to `ubftab/test.fd`:
- `T_TRANS_TYPE_FLD` (1050) - Transaction type
- `T_TRANS_ID_FLD` (1051) - Transaction ID
- `T_ACCOUNT_FLD` (1052) - Account number
- `T_CURRENCY_FLD` (1053) - Currency code
- `T_DESC_FLD` (1054) - Description (optional)
- `T_ERROR_CODE_FLD` (1055) - Error code
- `T_ERROR_MSG_FLD` (1056) - Error message

#### New Services
- **TRANSACTION** service in `samplesvr_rust`
  - Validates transaction type must be "sale"
  - Returns structured error responses
  - Full UBF encode/decode with derive macros

#### Documentation
- Created `TRANSACTION_API.md` - Complete guide to transaction endpoint
- Updated `README.md` with transaction examples
- Added architecture diagrams showing JSON ↔ UBF flow

### Changed

#### REST Gateway Rewrite
- **Migrated from Axum to Actix-web**
  - Better support for complex async functions
  - More flexible handler signatures
  - Improved error handling

- **Synchronous Enduro/X client**
  - Uses `Mutex` for thread-safe access
  - Simpler implementation than tokio channels
  - Better integration with blocking FFI calls

#### Service Response Handling
- Changed error responses to always return SUCCESS at transport level
- Error details now in UBF buffer fields (standard pattern)
- Improved error code and message structure

### Technical Details

#### Derive Macro Enhancements
```rust
// Now supports Option<String> fields
#[derive(UbfStruct)]
struct Example {
    #[ubf(field = T_NAME_FLD)]
    name: String,
    
    #[ubf(field = T_DESC_FLD)]
    description: Option<String>,  // ← New!
}
```

#### UBF Buffer Operations
```rust
// New methods for serialization
let bytes = ubf_buf.as_bytes();  // Get buffer as byte slice
let restored = UbfBuffer::from_bytes(bytes)?;  // Create from bytes
```

#### Type-Safe UBF Calls
```rust
// New UBF-specific service call
let response = client.call_service_ubf_blocking(
    "TRANSACTION",
    &buffer_data
)?;
```

### Examples

#### Successful Transaction
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

Response:
```json
{
  "transaction_id": "TXN-12345",
  "status": "SUCCESS",
  "message": "Transaction TXN-12345 processed successfully"
}
```

#### Failed Validation
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

Response:
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

### Testing

All tests pass successfully:
```bash
$ ./test_rest.sh
Testing Enduro/X REST Gateway (Rust)
=====================================

✓ Health Check (GET /)
✓ STATUS endpoint (GET /api/status)
✓ HELLO endpoint (POST /api/hello)
✓ ECHO endpoint (POST /api/echo)
✓ DATAPROC endpoint (POST /api/dataproc)
✓ TRANSACTION endpoint with SALE transaction
✓ TRANSACTION endpoint with invalid type (proper error)

Tests completed
```

### Migration Notes

If upgrading from previous version:

1. **REST Gateway**: Now uses Actix-web instead of Axum
   - Update dependencies in `Cargo.toml`
   - Handler signatures use `impl Responder`
   - State access via `web::Data<AppState>`

2. **UBF Fields**: Regenerate field tables
   ```bash
   mkfldhdr -d ubftab ubftab/test.fd
   ```

3. **Docker**: Rebuild image
   ```bash
   docker-compose build
   docker-compose up -d
   ```

### Performance

- Zero-copy UBF buffer operations where possible
- Minimal allocations in hot paths
- Efficient JSON ↔ UBF conversion
- Thread-safe client with mutex (negligible overhead for typical loads)

### Future Work

Potential enhancements for consideration:
- Nested struct support in UBF derive
- Array field support
- Custom validation attributes
- Async Enduro/X client wrapper
- Transaction batching
- Connection pooling for high-load scenarios

---

## [0.1.0] - Previous Release

Initial release with:
- Basic Enduro/X FFI bindings
- STRING and JSON services
- UBF server and client
- REST gateway with Axum
- UBF derive macro (numeric fields only)
