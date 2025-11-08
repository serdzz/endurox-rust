# Enduro/X Rust Integration

A modern Rust integration for [Enduro/X](https://www.endurox.org/) - a high-performance middleware platform for building distributed transaction processing systems.

## Overview

This project provides Rust bindings and a complete example implementation for building Enduro/X services and clients. It includes:

- **[endurox-sys](https://crates.io/crates/endurox-sys)** - Low-level FFI bindings and safe Rust wrappers for Enduro/X (published on crates.io)
- **endurox-derive** - Procedural macros for automatic UBF struct serialization (bundled with endurox-sys)
- **samplesvr_rust** - Example Enduro/X server implementing STRING/JSON and UBF services
- **ubfsvr_rust** - UBF (Unified Buffer Format) server with example services
- **oracle_txn_server** - Transaction server with Oracle Database integration
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

### endurox-sys [![Crates.io](https://img.shields.io/crates/v/endurox-sys.svg)](https://crates.io/crates/endurox-sys)

Safe Rust bindings for Enduro/X, available on [crates.io](https://crates.io/crates/endurox-sys):

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

#### oracle_txn_server (Oracle Database Services with Diesel ORM)

- **CREATE_TXN** - Create transaction in Oracle database
- **GET_TXN** - Retrieve transaction by ID
- **LIST_TXN** - List all transactions (max 100)

**Technology Stack:**
- **ORM**: Diesel 2.1.0 with diesel-oci 0.4.0
- **Connection Pool**: r2d2 (max 10 connections)
- **Features**: Type-safe queries, automatic transactions, schema migrations

### REST Gateway

HTTP/REST interface powered by Actix-web:

#### Basic Endpoints
- **GET /** - Health check endpoint
- **GET /api/status** - Server status
- **POST /api/hello** - Call HELLO service with JSON
- **POST /api/echo** - Call ECHO service with plain text
- **POST /api/dataproc** - Call DATAPROC service with JSON
- **POST /api/transaction** - Process transactions with UBF (calls samplesvr_rust)

#### Oracle Transaction Endpoints
- **POST /api/oracle/create** - Create transaction in Oracle DB (calls CREATE_TXN)
- **POST /api/oracle/get** - Get transaction by ID (calls GET_TXN)
- **GET /api/oracle/list** - List all transactions (calls LIST_TXN)

## Prerequisites

- **Rust** 1.70+ (toolchain installed via rustup)
- **Enduro/X** 8.0+ (installed from `.deb` packages)
- **Docker** (for containerized deployment)
- **Docker Compose** (for orchestration)
- **Oracle Database** XE 21c (optional, runs in Docker)

## Using endurox-sys in Your Project

Add to your `Cargo.toml`:

```toml
[dependencies]
endurox-sys = { version = "0.1.1", features = ["server", "client", "ubf", "derive"] }
```

Available features:
- **`server`** - Server API support (`tpsvrinit`, `tpsvrdone`, service advertisement)
- **`client`** - Client API support (`tpinit`, `tpterm`, `tpcall`, `tpacall`, `tpgetrply`)
- **`ubf`** - UBF (Unified Buffer Format) support
- **`derive`** - Procedural macros for UBF struct serialization (`#[derive(UbfStruct)]`)

See the [endurox-sys documentation](https://docs.rs/endurox-sys) for detailed API reference.

## Quick Start

### Using Docker (Recommended)

1. **Build the Docker image:**
   ```bash
   docker-compose build endurox_rust
   ```

2. **Start all services (including Oracle DB):**
   ```bash
   docker-compose up -d
   ```
   
   This will:
   - Start Oracle Database XE 21c
   - Wait for Oracle to be healthy (1-2 minutes on first start)
   - Run database initialization scripts from `db/oracle/`
   - Start Enduro/X services

   Or start only Enduro/X (without Oracle):
   ```bash
   docker-compose up -d endurox_rust
   ```

3. **Test the REST API:**
   ```bash
   # Test basic endpoints
   ./test_rest.sh
   
   # Test Oracle transaction endpoints
   ./test_oracle_rest.sh
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

### Oracle Transaction Services

The Oracle transaction server provides database-backed transaction management:

**Create transaction:**
```bash
curl -X POST http://localhost:8080/api/oracle/create \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "sale",
    "transaction_id": "TXN001",
    "account": "ACC123",
    "amount": 10050,
    "currency": "USD",
    "description": "Payment via REST API"
  }'
```

Response:
```json
{
  "transaction_id": "TXN001",
  "status": "SUCCESS",
  "message": "Transaction TXN001 created successfully"
}
```

**Get transaction:**
```bash
curl -X POST http://localhost:8080/api/oracle/get \
  -H "Content-Type: application/json" \
  -d '{"transaction_id": "TXN001"}'
```

**List all transactions:**
```bash
curl -X GET http://localhost:8080/api/oracle/list
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

## Database Integration

### Oracle Database

The project includes Oracle Database XE 21c integration for transaction processing:

**Connection Details:**
- **Host**: `oracledb` (internal) / `localhost` (from host)
- **Port**: `1521` (internal) / `11521` (host)
- **SID**: `XE`
- **User**: `ctp` / **Password**: `ctp`
- **Connection String**: `oracle://ctp:ctp@oracledb:1521/XE`

**Features:**
- Automatic database initialization on first start
- Health checks ensure DB is ready before app starts
- Sample tables and test data created automatically
- Persistent data stored in Docker volume

**Initialization Scripts:**

Place `.sql` files in `db/oracle/` directory. They run automatically on first container start:
```bash
db/oracle/
└── 01_init.sql  # Creates CTP user and test tables
```

**Database Operations:**
```bash
# View database logs
docker-compose logs oracledb

# Connect to database from host
sqlplus ctp/ctp@localhost:11521/XE

# Reset database (removes all data)
docker-compose down
docker volume rm endurox-dev_oracle
docker-compose up -d
```

See [db/README.md](db/README.md) for complete database documentation.

### Performance Benchmarks

The Oracle transaction server uses Diesel ORM with excellent performance characteristics:

**Benchmark Results (Diesel ORM):**
- **GET_TXN**: 1,677 requests/sec (similar to native driver)
- **LIST_TXN**: 1,134 requests/sec (37% slower than native, but acceptable)
- **CREATE_TXN**: ~85 requests/sec sequential (similar to native driver)
- **Zero failures**: All tests completed successfully

**Performance Analysis:**
- GET and CREATE operations have negligible ORM overhead (~5% difference)
- LIST operations are 37% slower due to row deserialization (100 rows)
- Database commit overhead dominates CREATE performance (2-3ms per txn)
- Trade-off: Diesel provides type safety and maintainability with minimal performance impact

**Run benchmarks yourself:**
```bash
# Benchmark all endpoints
./benchmark_oracle_rest_v2.sh

# Test individual endpoints
ab -n 1000 -c 10 http://localhost:8080/api/oracle/list
```

See [DIESEL_BENCHMARK_RESULTS.md](DIESEL_BENCHMARK_RESULTS.md) for detailed performance analysis and comparison with native Oracle driver.

## Development
### Project Structure

```
.
├── Cargo.toml              # Workspace definition
├── docker-compose.yml      # Docker orchestration
├── Dockerfile              # Container image
├── samplesvr_rust/         # STRING/JSON server
│   ├── src/
│   │   ├── main.rs         # Server entry point
│   │   └── services.rs     # Service implementations
│   └── Cargo.toml
├── ubfsvr_rust/            # UBF server
│   ├── src/
│   │   └── main.rs         # UBF services
│   └── Cargo.toml
├── oracle_txn_server/      # Oracle transaction server (Diesel ORM)
│   ├── src/
│   │   ├── main.rs         # Server entry point
│   │   ├── services.rs     # Transaction services
│   │   ├── db.rs           # Diesel connection pool
│   │   ├── models.rs       # Database models
│   │   └── schema.rs       # Diesel schema (auto-generated)
│   ├── migrations/         # Database migrations
│   ├── diesel.toml         # Diesel configuration
│   └── Cargo.toml
├── ubf_test_client/        # UBF test client
│   ├── src/
│   │   └── main.rs         # Test runner
│   └── Cargo.toml
├── rest_gateway/           # REST gateway
│   ├── src/
│   │   └── main.rs         # Actix-web server
│   └── Cargo.toml
├── ubftab/                 # UBF field tables
│   └── test.fd             # Field definitions
├── conf/                   # Enduro/X configuration
├── setenv.sh               # Environment setup
├── test_rest.sh            # REST API test script
├── test_oracle_rest.sh     # Oracle transaction API test script
├── benchmark_oracle_rest_v2.sh  # Oracle benchmark script
├── BENCHMARK_RESULTS.md    # Native driver benchmark results
└── DIESEL_BENCHMARK_RESULTS.md  # Diesel ORM benchmark results
```

### Running Tests

```bash
# Run Clippy for linting (with strict mode)
cargo clippy --all-targets --all-features -- -D warnings

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

### LD_PRELOAD Issue

If you encounter errors like `undefined symbol: ndrx_Bget_long` or library loading issues, you need to set `LD_PRELOAD`:

```bash
export LD_PRELOAD=/opt/endurox/lib/libnstd.so
```

See [LD_PRELOAD_ISSUE.md](LD_PRELOAD_ISSUE.md) for detailed explanation and solutions.

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

## Documentation

📖 **[Complete Documentation Index](DOCUMENTATION_INDEX.md)** - Full guide to all project documentation

### Quick Links

- **[Getting Started Guide](GETTING_STARTED.md)** - Quick start guide for using endurox-sys
- **[Release Notes](RELEASE_NOTES.md)** - Latest release information (v0.1.1)
- **[Changelog](CHANGELOG.md)** - Complete version history
- **[UBF Struct Guide](UBF_STRUCT_GUIDE.md)** - Guide to UBF struct serialization
- **[Transaction API](TRANSACTION_API.md)** - Complex JSON ↔ UBF conversion examples
- **[Oracle Integration](oracle_txn_server/README.md)** - Oracle Database integration with Diesel ORM
- **[Performance Benchmarks](DIESEL_BENCHMARK_RESULTS.md)** - Detailed performance analysis

## Contributing

Contributions are welcome! Please ensure code passes:
- `cargo fmt` - Code formatting
- `cargo clippy` - Linting
- `cargo test` - All tests

## Resources

- [Enduro/X Documentation](https://www.endurox.org/dokuwiki/)
- [endurox-sys on crates.io](https://crates.io/crates/endurox-sys)
- [endurox-sys API Documentation](https://docs.rs/endurox-sys)
- [Rust FFI Guide](https://doc.rust-lang.org/nomicon/ffi.html)
- [Actix-web Documentation](https://actix.rs/)
