# Enduro/X Rust Integration

A modern Rust integration for [Enduro/X](https://www.endurox.org/) - a high-performance middleware platform for building distributed transaction processing systems.

## Overview

This project provides Rust bindings and a complete example implementation for building Enduro/X services and clients. It includes:

- **endurox-sys** - Low-level FFI bindings and safe Rust wrappers for Enduro/X
- **endurox-derive** - Procedural macros for automatic UBF struct serialization
- **samplesvr_rust** - Example Enduro/X server implementing STRING/JSON and UBF services
- **ubfsvr_rust** - UBF (Unified Buffer Format) server with example services
- **rest_gateway** - REST API gateway built with Actix-web that exposes Enduro/X services over HTTP
- **ubf_test_client** - Test client for UBF services

## Architecture

```
┌─────────────┐      HTTP/REST      ┌──────────────┐
│   Client    │ ──────────────────> │ rest_gateway │
│ (curl/web)  │                     │  (Actix-web) │
└─────────────┘                     └──────┬───────┘
                                           │ Enduro/X
                                           │ tpcall()
                                           │ (STRING/UBF)
                                           ▼
                                    ┌──────────────┐
                                    │ samplesvr_   │
                                    │    rust      │
                                    │ (JSON + UBF) │
                                    └──────────────┘
```

## Features

### endurox-sys

Safe Rust bindings for Enduro/X:

- **Server API**: `tpsvrinit`, `tpsvrdone`, service advertisement, `tpreturn`
- **Client API**: `tpinit`, `tpterm`, `tpcall`, `tpacall`, `tpgetrply`
- **UBF API**: Complete UBF (Unified Buffer Format) support with safe wrappers
  - `UbfBuffer` - Safe buffer management
  - Field operations: `add_string()`, `add_long()`, `add_double()`, `get_*()`, `is_present()`, `delete()`
  - Iteration support with `UbfIterator`
  - Field table compilation and loading
- **Logging**: Integrated Enduro/X logging (`tplog_*`)
- **Buffer Management**: Safe wrappers for STRING, JSON, and UBF buffers
- **Feature Flags**: Separate `server`, `client`, and `ubf` features for modular builds

### Sample Services

#### samplesvr_rust (STRING/JSON and UBF Services)

- **ECHO** - Echo back the input data
- **HELLO** - Personalized greeting service (JSON input/output)
- **STATUS** - Server status and health check
- **DATAPROC** - Data processing service with JSON support
- **TRANSACTION** - Complex transaction processing with UBF (validates sale transactions)

#### ubfsvr_rust (UBF Services)

- **UBFECHO** - Echo UBF buffer back
- **UBFTEST** - Test UBF operations with name field and response
- **UBFADD** - Create UBF buffer with multiple fields (string, long, double)
- **UBFGET** - Read and echo UBF fields

### REST Gateway

HTTP/REST interface powered by Actix-web:

- **GET /** - Health check endpoint
- **GET /api/status** - Server status
- **POST /api/hello** - Call HELLO service with JSON
- **POST /api/echo** - Call ECHO service with plain text
- **POST /api/dataproc** - Call DATAPROC service with JSON
- **POST /api/transaction** - Process transactions with UBF (JSON → UBF → JSON)

## Prerequisites

- **Rust** 1.70+ (toolchain installed via rustup)
- **Enduro/X** 8.0+ (installed from `.deb` packages)
- **Docker** (for containerized deployment)
- **Docker Compose** (for orchestration)

## Quick Start

### Using Docker (Recommended)

1. **Build the Docker image:**
   ```bash
   docker-compose build endurox_rust
   ```

2. **Start the services:**
   ```bash
   docker-compose up -d endurox_rust
   ```

3. **Test the REST API:**
   ```bash
   ./test_rest.sh
   ```

4. **Test UBF services:**
   ```bash
   docker-compose exec endurox_rust bash -c '. ./setenv.sh && /app/bin/ubf_test_client'
   ```

5. **View logs:**
   ```bash
   docker-compose logs -f endurox_rust
   # or check the logs directory
   tail -f logs/ULOG.*
   ```

### Manual Build

1. **Set up Enduro/X environment:**
   ```bash
   source setenv.sh
   ```

2. **Build the workspace:**
   ```bash
   cargo build --release
   ```

3. **Start Enduro/X:**
   ```bash
   xadmin start -y
   ```

4. **The services should now be running:**
   - `samplesvr_rust` - STRING/JSON services
   - `ubfsvr_rust` - UBF services
   - `rest_gateway` - REST API on http://localhost:8080

## API Examples

### Health Check
```bash
curl http://localhost:8080/
```

### Status Service
```bash
curl http://localhost:8080/api/status
```

### Hello Service
```bash
curl -X POST http://localhost:8080/api/hello \
  -H "Content-Type: application/json" \
  -d '{"name":"World"}'
```

### Echo Service
```bash
curl -X POST http://localhost:8080/api/echo \
  -H "Content-Type: text/plain" \
  -d "Hello from REST Gateway!"
```

### Data Processing Service
```bash
curl -X POST http://localhost:8080/api/dataproc \
  -H "Content-Type: application/json" \
  -d '{"data":"test","count":123}'
```

### Transaction Service (UBF)

The transaction service demonstrates complex JSON ↔ UBF conversion with validation:

**Valid sale transaction:**
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

**Invalid transaction (non-sale type):**
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

## UBF Examples

### Running UBF Test Client

```bash
docker-compose exec endurox_rust bash -c '. ./setenv.sh && /app/bin/ubf_test_client'
```

This will execute 4 tests:
1. **UBFADD** - Creates a UBF buffer with multiple fields
2. **UBFTEST** - Sends a name and receives a greeting
3. **UBFECHO** - Echoes the UBF buffer back
4. **UBFGET** - Sends multiple fields and reads them

### UBF Struct Derive Macro

The project includes an automatic derive macro for UBF struct conversion. You can use either numeric field IDs or field constants:

```rust
use endurox_sys::UbfStruct;
use endurox_sys::ubf_fields::*;  // Auto-generated field constants

#[derive(UbfStruct)]
struct Transaction {
    #[ubf(field = T_NAME_FLD)]  // Using field constant
    name: String,
    
    #[ubf(field = T_ID_FLD)]  // Field constant
    id: i64,
    
    #[ubf(field = T_PRICE_FLD)]  // Field constant
    amount: f64,
    
    #[ubf(field = T_STATUS_FLD)]  // Optional string field
    status: Option<String>,
    
    #[ubf(field = T_DESC_FLD)]  // Optional description
    description: Option<String>,
}

// Alternative: using numeric field IDs
#[derive(UbfStruct)]
struct Payment {
    #[ubf(field = 1002)]  // Numeric field ID
    name: String,
    
    #[ubf(field = 1012)]  // Numeric field ID
    id: i64,
}

// Nested structs are also supported (including optional)
#[derive(UbfStruct)]
struct Address {
    #[ubf(field = T_STREET_FLD)]
    street: String,
    #[ubf(field = T_CITY_FLD)]
    city: String,
}

#[derive(UbfStruct)]
struct Customer {
    #[ubf(field = T_NAME_FLD)]
    name: String,
    #[ubf(field = 0)]  // Optional nested struct
    address: Option<Address>,
}

// Usage
let txn = Transaction { name: "Payment".into(), id: 123, amount: 99.99, status: "completed".into() };
let ubf = txn.to_ubf()?;  // Convert to UBF
let restored = Transaction::from_ubf(&ubf)?;  // Convert from UBF
```

**Running the derive macro example:**
```bash
docker-compose exec endurox_rust bash /app/test_derive.sh
```

See [UBF_STRUCT_GUIDE.md](UBF_STRUCT_GUIDE.md) for complete documentation.

For a complete example of JSON ↔ UBF conversion with validation, see [TRANSACTION_API.md](TRANSACTION_API.md).

### UBF Field Table

The project includes a UBF field table (`ubftab/test.fd`) with the following fields:

- **String fields**: T_STRING_FLD, T_NAME_FLD, T_MESSAGE_FLD, T_STATUS_FLD, T_DATA_FLD, T_STREET_FLD, T_CITY_FLD, T_ZIP_FLD, T_TRANS_TYPE_FLD, T_TRANS_ID_FLD, T_ACCOUNT_FLD, T_CURRENCY_FLD, T_DESC_FLD, T_ERROR_CODE_FLD, T_ERROR_MSG_FLD
- **Long fields**: T_LONG_FLD, T_COUNT_FLD, T_ID_FLD, T_CODE_FLD, T_AMOUNT_FLD
- **Double fields**: T_DOUBLE_FLD, T_PRICE_FLD, T_BALANCE_FLD
- **Short fields**: T_SHORT_FLD, T_FLAG_FLD
- **Char fields**: T_CHAR_FLD

## Development

### Project Structure
.
├── Cargo.toml              # Workspace definition
├── docker-compose.yml      # Docker orchestration
├── Dockerfile              # Container image
|── endurox-sys/           # Rust FFI bindings
│   ├── src/
│   │   ├── lib.rs         # Module exports
│   │   ├── ffi.rs         # Raw FFI declarations
│   │   ├── server.rs      # Server API
│   │   ├── client.rs      # Client API
│   │   ├── ubf.rs         # UBF API
│   │   ├── ubf_struct.rs  # UBF struct trait & helpers
│   │   └── log.rs         # Logging wrappers
│   └── Cargo.toml
├── endurox-derive/       # Proc-macro for UbfStruct
│   ├── src/
│   │   └── lib.rs         # Derive macro implementation
│   └── Cargo.toml
├── samplesvr_rust/        # STRING/JSON server
│   ├── src/
│   │   ├── main.rs        # Server entry point
│   │   └── services.rs    # Service implementations
│   └── Cargo.toml
├── ubfsvr_rust/           # UBF server
│   ├── src/
│   │   └── main.rs        # UBF services
│   └── Cargo.toml
├── ubf_test_client/       # UBF test client
│   ├── src/
│   │   └── main.rs        # Test runner
│   └── Cargo.toml
├── rest_gateway/          # REST gateway
│   ├── src/
│   │   └── main.rs        # Actix-web server
│   └── Cargo.toml
├── ubftab/                # UBF field tables
│   └── test.fd           # Field definitions
├── conf/                  # Enduro/X configuration
├── setenv.sh             # Environment setup
├── test_rest.sh          # REST API test script
└── test_ubf.sh           # UBF test script
```
└── test_rest.sh          # API test script
```

### Running Tests

```bash
# Run Clippy for linting
cargo clippy --release

# Run tests
cargo test

# Format code
cargo fmt
```

### Inside Docker

```bash
# Enter the container
docker-compose exec endurox_rust bash

# Check Enduro/X status
xadmin psc
xadmin ppm

# View service queue
xadmin mqlq

# Rebuild inside container
cargo build --release

# Restart services
xadmin stop -y && xadmin start -y
```

## Configuration

- **ndrxconfig.xml** - Enduro/X server configuration
- **conf/\*** - Service-specific configuration files
- **setenv.sh** - Environment variables for Enduro/X

## Troubleshooting

### Check Service Status
```bash
xadmin psc
```

### View Logs
```bash
tail -f logs/ULOG.*
```

### Restart Services
```bash
# Restart STRING/JSON server
xadmin restart -s samplesvr_rust -i 1

# Restart UBF server
xadmin restart -s ubfsvr_rust -i 1
```

### Clean Rebuild
```bash
cargo clean
cargo build --release
```

## License

This project is provided as-is for demonstration and development purposes.

## Contributing

Contributions are welcome! Please ensure code passes:
- `cargo fmt` - Code formatting
- `cargo clippy` - Linting
- `cargo test` - All tests

## Resources

- [Enduro/X Documentation](https://www.endurox.org/dokuwiki/)
- [Rust FFI Guide](https://doc.rust-lang.org/nomicon/ffi.html)
- [Axum Documentation](https://docs.rs/axum/latest/axum/)
