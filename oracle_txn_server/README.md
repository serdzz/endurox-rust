# Oracle Transaction Server

Enduro/X transaction server with Oracle Database integration using Diesel ORM and diesel-oci driver.

## Features

- **Diesel ORM** - Type-safe database queries with diesel-oci 0.4.0
- **Connection Pooling** - r2d2 connection pool (max 10 connections) for efficient database access
- **Type Safety** - Compile-time query validation prevents SQL errors
- **Schema Migrations** - Database schema versioning with Diesel migrations
- **Automatic Transactions** - Diesel handles commit/rollback automatically
- **UBF Integration** - Seamless UBF buffer serialization/deserialization using derive macros
- **REST API Integration** - Full REST gateway support with automatic JSON ↔ UBF conversion
- **Three Services**:
  - `CREATE_TXN` - Create new transaction record
  - `GET_TXN` - Retrieve transaction by ID
  - `LIST_TXN` - List all transactions (max 100, ordered by created_at DESC)

## Technology Stack

- **ORM**: Diesel 2.1.0
- **Oracle Driver**: diesel-oci 0.4.0
- **Connection Pool**: r2d2 0.8
- **Serialization**: serde, serde_json
- **Database**: Oracle Database XE 21c

## Prerequisites

1. **Oracle Instant Client** - Required for diesel-oci
   ```bash
   # On macOS (example)
   brew install oracle-instantclient
   
   # On Linux (Docker handles this automatically)
   # Download from Oracle website and install if running locally
   ```

2. **Oracle Database** - Running Oracle instance (local or Docker)
   - XE 21c Edition included in docker-compose.yml
   - Default connection: `oracle://ctp:ctp@oracledb:1521/XE`
   - Docker exposes port 11521 → 1521

3. **Enduro/X** - Installed and configured
   ```bash
   export NDRX_HOME=/opt/endurox
   export PATH=$NDRX_HOME/bin:$PATH
   ```

4. **Python 3** (optional) - For migration management with `migrate.py`
   ```bash
   pip install oracledb
   ```

## Setup

### 1. Using Docker (Recommended)

The easiest way to run the Oracle transaction server is with Docker Compose:

```bash
# Build and start all services
docker-compose up -d

# Wait for Oracle to initialize (1-2 minutes on first start)
docker-compose logs -f oracledb

# Verify oracle_txn_server is running
docker-compose logs endurox_rust | grep oracle_txn_server
```

The docker-compose.yml automatically:
- Starts Oracle Database XE 21c
- Runs database initialization scripts from `db/oracle/`
- Creates CTP user and transactions table
- Starts oracle_txn_server with correct DATABASE_URL

### 2. Manual Setup

If running outside Docker, set the DATABASE_URL environment variable:

```bash
export DATABASE_URL=oracle://username:password@hostname:port/service_name
```

Create the database schema manually (see Database Schema section below).

### Database Migrations

The project uses Diesel migrations for schema management. Migrations are located in `migrations/` directory.

#### Using Diesel CLI (Recommended for Development)

**Note**: Diesel CLI doesn't fully support Oracle migrations, but you can use the Python migration tool instead.

#### Using Python Migration Tool (migrate.py)

For easier Oracle migration management, use the included `migrate.py` script:

**Installation:**
```bash
# Install Oracle Python driver
pip install oracledb

# Make script executable
chmod +x migrate.py
```

**Usage:**
```bash
# Check migration status
./migrate.py status

# Apply all pending migrations
./migrate.py run

# Rollback last migration
./migrate.py rollback

# Rollback last N migrations
./migrate.py rollback 2

# Rollback all migrations (reset database)
./migrate.py reset
```

**Example Output:**
```
$ python3 migrate.py status
Using DATABASE_URL: localhost:11521/XE

Migration Status:
------------------------------------------------------------
✓ Applied    2025-11-07-000000_create_txn_opt_lock
------------------------------------------------------------
Total: 1 migrations, 1 applied, 0 pending

$ python3 migrate.py run
Using DATABASE_URL: localhost:11521/XE

Applying 1 migration(s)...
⬆ Running migration: 2025-11-07-000000_create_txn_opt_lock
✓ Applied: 2025-11-07-000000_create_txn_opt_lock

✓ Successfully applied 1 migration(s)
```

**Features:**
- Uses Diesel-compatible migration tracking table (`__diesel_schema_migrations`)
- Supports up/down migrations from `migrations/<version>/up.sql` and `down.sql`
- Handles PL/SQL blocks (DECLARE/BEGIN/END, CREATE TRIGGER)
- Automatic connection to Oracle database (configured for Docker setup)
- Safe rollback functionality
- Clear status reporting

**Configuration:**

The script supports DATABASE_URL environment variable or falls back to individual settings:

```bash
# Using DATABASE_URL (recommended)
export DATABASE_URL=oracle://ctp:ctp@localhost:11521/XE
python3 migrate.py run

# Or using individual environment variables
export ORACLE_HOST=localhost
export ORACLE_PORT=11521
export ORACLE_USER=ctp
export ORACLE_PASSWORD=ctp
export ORACLE_SERVICE=XE
python3 migrate.py run
```

Defaults: `localhost:11521`, user `ctp`, password `ctp`, service `XE`

#### Migration Files

Current migration: `migrations/2025-11-07-000000_create_txn_opt_lock/`

**up.sql** - Creates the transactions table with optimistic locking:
```sql
-- Create transactions table with optimistic locking support
CREATE TABLE transactions (
    Recver NUMBER(10) DEFAULT 0 NOT NULL,
    id VARCHAR2(100) PRIMARY KEY,
    transaction_type VARCHAR2(50) NOT NULL,
    account VARCHAR2(100) NOT NULL,
    amount NUMBER(19,2) NOT NULL,
    currency VARCHAR2(10) NOT NULL,
    description VARCHAR2(500),
    status VARCHAR2(50) NOT NULL,
    message VARCHAR2(500),
    error_code VARCHAR2(50),
    error_message VARCHAR2(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX idx_transactions_type ON transactions(transaction_type);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_created ON transactions(created_at DESC);

-- Create trigger to auto-increment Recver on any UPDATE
CREATE OR REPLACE TRIGGER trg_transactions_optimistic_lock
BEFORE UPDATE ON transactions
FOR EACH ROW
BEGIN
    :NEW.Recver := :OLD.Recver + 1;
END;
```

**down.sql** - Drops trigger, indexes and table:
```sql
DROP TRIGGER trg_transactions_optimistic_lock;
DROP INDEX idx_transactions_created;
DROP INDEX idx_transactions_status;
DROP INDEX idx_transactions_type;
DROP TABLE transactions;
```

**Optimistic Locking:**
- `Recver` column stores version number for each row (starts at 0)
- Automatically incremented by trigger on every UPDATE
- Enables optimistic concurrency control for Enduro/X transactions

### Database Schema

The `transactions` table has the following schema:
- `Recver` - Version number for optimistic locking (auto-incremented on UPDATE)
- `id` - Transaction ID (primary key)
- `transaction_type` - Type of transaction (e.g., "sale")
- `account` - Account number
- `amount` - Transaction amount
- `currency` - Currency code
- `description` - Optional description
- `status` - Transaction status ("SUCCESS" or "ERROR")
- `message` - Status message
- `error_code` - Error code if failed
- `error_message` - Error message if failed
- `created_at` - Creation timestamp
- `updated_at` - Update timestamp (auto-updated)

### 3. Build the Server

```bash
cargo build --release
```

The binary will be at `target/release/oracle_txn_server`.

### 4. Configure Enduro/X

Add the server to your Enduro/X configuration (`ndrxconfig.xml`):

```xml
<server name="oracle_txn_server">
    <srvid>100</srvid>
    <min>1</min>
    <max>5</max>
    <cctag>default</cctag>
</server>
```

### 5. Configure XA Resource Manager

Create or update `$NDRX_HOME/udataobj/RM`:
```
Oracle_XA;xaosw;Oracle_XA+Acc=P/username/password+SesTm=30+LogDir=/tmp
```

Update `$NDRX_HOME/udataobj/ULOG` configuration for XA:
```
@oracle
NDRX_XA_RES_ID=1
NDRX_XA_OPEN_STR=Oracle_XA+Acc=P/username/password+SesTm=30+LogDir=/tmp
NDRX_XA_CLOSE_STR=
NDRX_XA_DRIVERLIB=liboramysql.so
NDRX_XA_RMLIB=libclntsh.so
NDRX_XA_LAZY_INIT=1
```

## Usage

### Start the Server

```bash
# Source Enduro/X environment
. setenv.sh

# Start the application
xadmin start

# Check status
xadmin psc
```

### Service Calls

#### CREATE_TXN - Create Transaction

```bash
# Using ud command
ud CREATE_TXN <<EOF
T_TRANS_ID_FLD  TXN-$(date +%s)
T_TRANS_TYPE_FLD    sale
T_ACCOUNT_FLD   ACC-12345
T_AMOUNT_FLD    10000
T_CURRENCY_FLD  USD
T_DESC_FLD      Test transaction
EOF
```

Response:
```
T_TRANS_ID_FLD  TXN-1234567890
T_STATUS_FLD    SUCCESS
T_MESSAGE_FLD   Transaction TXN-1234567890 created successfully
```

#### GET_TXN - Get Transaction

```bash
ud GET_TXN <<EOF
T_TRANS_ID_FLD  TXN-1234567890
EOF
```

#### LIST_TXN - List Transactions

```bash
ud LIST_TXN <<EOF
EOF
```

### REST Gateway Integration

The recommended way to interact with the Oracle transaction server is through the REST gateway:

#### Create Transaction
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

#### Get Transaction
```bash
curl -X POST http://localhost:8080/api/oracle/get \
  -H "Content-Type: application/json" \
  -d '{"transaction_id": "TXN001"}'
```

#### List All Transactions
```bash
curl -X GET http://localhost:8080/api/oracle/list
```

Response:
```json
{
  "transaction_id": "",
  "status": "SUCCESS",
  "message": "Found 3 transactions"
}
```

#### Run Complete Test Suite
```bash
./test_oracle_rest.sh
```

## Architecture

```
┌─────────────┐  HTTP/REST  ┌──────────────┐  Enduro/X  ┌─────────────────┐  Diesel   ┌──────────┐
│   Client    │ ──────────> │ rest_gateway │ ──tpcall─> │oracle_txn_server│ ─Diesel─> │ Oracle   │
│ (curl/web)  │             │ (Actix-web)  │   (UBF)    │  + Diesel ORM  │   ORM    │ Database │
│             │ <────────── │              │ <────────  │   + r2d2 Pool  │ <──────  │ XE 21c   │
└─────────────┘    JSON     └──────────────┘    UBF     └─────────────────┘ diesel-oci └──────────┘
```

### Data Flow

1. **Client** sends HTTP/JSON request to REST gateway
2. **REST Gateway** converts JSON to Rust struct, then encodes to UBF using `#[derive(UbfStructDerive)]`
3. **Enduro/X** transfers UBF buffer via IPC to oracle_txn_server using `tpcall()`
4. **Oracle Server** decodes UBF to Rust struct using `UbfStruct::from_ubf()`
5. **Diesel ORM** builds type-safe SQL query with query builder
6. **diesel-oci** executes query via Oracle Instant Client
7. **Database** processes query and returns results
8. **Diesel** deserializes results into Rust structs (automatic type mapping)
9. **Response** flows back through the same chain: Diesel Result → UBF → JSON

### Components

**oracle_txn_server/src/**
- `main.rs` - Server initialization, service advertisement
- `services.rs` - Business logic for CREATE_TXN, GET_TXN, LIST_TXN
- `db.rs` - Diesel connection pool management (r2d2)
- `models.rs` - Rust structs with Diesel derives (Queryable, Insertable)
- `schema.rs` - Diesel schema (auto-generated by `diesel print-schema`)

**Diesel Benefits:**
- Type-safe queries validated at compile time
- Automatic serialization between SQL types and Rust structs
- Clean query builder API (no raw SQL strings)
- Connection pooling with r2d2
- Schema migrations for version control

## Transaction Management

Diesel ORM handles transactions automatically - no explicit commit required:

```rust
use diesel::prelude::*;
use crate::schema::transactions;

// Get connection from pool
let mut conn = match crate::db::get_connection(pool) {
    Ok(conn) => conn,
    Err(e) => {
        tplog_error(&format!("Failed to get DB connection: {}", e));
        return create_error_response(&req.transaction_id, "DB_ERROR", &e);
    }
};

// Insert transaction using Diesel - automatic commit
match diesel::insert_into(transactions::table)
    .values(&new_txn)
    .execute(&mut conn)
{
    Ok(_) => {
        tplog_info(&format!("Transaction {} created successfully", req.transaction_id));
        create_success_response(&req.transaction_id, &message)
    }
    Err(e) => {
        tplog_error(&format!("Failed to insert transaction: {}", e));
        create_error_response(&req.transaction_id, "DB_INSERT_ERROR", &e.to_string())
    }
}
```

**Benefits of Diesel ORM:**
- **Automatic Transactions**: Diesel handles commit/rollback automatically
- **Type Safety**: Compile-time query validation prevents SQL errors
- **Query Builder**: Clean, composable API for building complex queries
- **Connection Pooling**: r2d2 provides efficient connection management

**Example Queries:**
```rust
// Get transaction by ID
use crate::schema::transactions::dsl::*;

let result = transactions
    .filter(id.eq(&req.transaction_id))
    .first::<Transaction>(&mut conn);

// List transactions (ordered, limited)
let txn_list = transactions
    .order(created_at.desc())
    .limit(100)
    .load::<Transaction>(&mut conn)?;
```

## Database Schema

The transactions table is created by the migration system:

```sql
CREATE TABLE transactions (
    Recver NUMBER(10) DEFAULT 0 NOT NULL,
    id VARCHAR2(100) PRIMARY KEY,
    transaction_type VARCHAR2(50) NOT NULL,
    account VARCHAR2(100) NOT NULL,
    amount NUMBER(19,2) NOT NULL,
    currency VARCHAR2(10) NOT NULL,
    description VARCHAR2(500),
    status VARCHAR2(50) NOT NULL,
    message VARCHAR2(500),
    error_code VARCHAR2(50),
    error_message VARCHAR2(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Performance indexes
CREATE INDEX idx_transactions_type ON transactions(transaction_type);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_created ON transactions(created_at DESC);

-- Optimistic locking trigger
CREATE OR REPLACE TRIGGER trg_transactions_optimistic_lock
BEFORE UPDATE ON transactions
FOR EACH ROW
BEGIN
    :NEW.Recver := :OLD.Recver + 1;
END;
```

### Querying Transactions Directly

```bash
# Connect to Oracle in Docker
docker-compose exec -T oracledb sqlplus -S ctp/ctp@//localhost:1521/XE <<< \
  "SELECT id, transaction_type, amount, currency, status FROM transactions ORDER BY created_at;"
```

## Development

### Running Tests

```bash
cargo test
```

### Checking Code

```bash
cargo clippy
cargo fmt
```

### Viewing Logs

```bash
tail -f $NDRX_HOME/log/ULOG.*
```

## Troubleshooting

### Connection Issues

If you get "Failed to initialize database pool":
1. Check `DATABASE_URL` is set correctly
2. Verify Oracle instant client is installed
3. Check Oracle service is running
4. Test connection with sqlplus:
   ```bash
   sqlplus username/password@hostname:port/service_name
   ```

### XA Transaction Errors

If XA transactions fail:
1. Check RM configuration in `$NDRX_HOME/udataobj/RM`
2. Verify XA libraries are accessible
3. Check Oracle user has XA privileges:
   ```sql
   GRANT SELECT ON v_$xatrans$ TO username;
   GRANT SELECT ON pending_trans$ TO username;
   GRANT SELECT ON dba_2pc_pending TO username;
   GRANT EXECUTE ON dbms_xa TO username;
   ```

### Database Initialization

If you need to manually initialize the database:

```bash
# Connect as system user
docker-compose exec oracledb sqlplus / as sysdba

# Create user (if not exists)
ALTER SESSION SET "_ORACLE_SCRIPT"=true;
CREATE USER ctp IDENTIFIED BY ctp;
GRANT CONNECT, RESOURCE, CREATE SESSION, CREATE TABLE, UNLIMITED TABLESPACE TO ctp;

# Run init script
docker-compose exec -T oracledb sqlplus system/oracle123@//localhost:1521/XE < db/oracle/01_init.sql
```

## Performance Benchmarks

The Oracle transaction server has been benchmarked with Diesel ORM:

### Benchmark Results

| Operation | Throughput | Latency (mean) | Notes |
|-----------|------------|----------------|-------|
| **GET_TXN** | 1,677 req/sec | 5.96ms | Single row lookup by primary key |
| **LIST_TXN** | 1,134 req/sec | 8.81ms | 100 rows, ordered by created_at DESC |
| **CREATE_TXN** | ~85 req/sec | 11-12ms | Sequential insert with commit |

**Test Environment:**
- Concurrency: 10 concurrent requests
- Database: Oracle XE 21c in Docker
- Connection pool: r2d2 (max 10 connections)
- Zero failures across all tests

### Diesel vs Native Driver Comparison

| Operation | Native Driver | Diesel ORM | Difference |
|-----------|--------------|------------|------------|
| GET_TXN | 1,292-2,089 req/sec | 1,677 req/sec | **Similar** (±5%) |
| LIST_TXN | 1,781-1,907 req/sec | 1,134 req/sec | **-37%** slower |
| CREATE_TXN | ~88 req/sec | ~85 req/sec | **Similar** (±3%) |

**Analysis:**
- ✅ **GET operations**: Diesel overhead is negligible (~5% difference)
- ⚠️ **LIST operations**: 37% slower due to row deserialization overhead (100 rows)
- ✅ **CREATE operations**: Nearly identical - database commit dominates latency

**Trade-off:** Diesel sacrifices some LIST performance in exchange for:
- Type safety with compile-time query validation
- Better code maintainability
- Schema version control with migrations
- Automatic transaction management

### Running Benchmarks

```bash
# Run all benchmarks
../benchmark_oracle_rest_v2.sh

# Individual endpoint benchmarks
ab -n 1000 -c 10 -p get_req.json -T application/json http://localhost:8080/api/oracle/get
ab -n 1000 -c 10 http://localhost:8080/api/oracle/list
```

See [../DIESEL_BENCHMARK_RESULTS.md](../DIESEL_BENCHMARK_RESULTS.md) for detailed performance analysis.

## Performance Tuning

### Connection Pool

Adjust pool size in `src/db.rs`:
```rust
Pool::builder()
    .max_size(20)  // Increase for higher concurrency
    .build(manager)
```

### Database Indexes

The following indexes are created automatically:
- `idx_transactions_type` - On transaction_type
- `idx_transactions_status` - On status  
- `idx_transactions_created` - On created_at DESC

Add additional indexes for frequently queried fields:
```sql
CREATE INDEX idx_transactions_account ON ctp.transactions(account);
CREATE INDEX idx_transactions_currency ON ctp.transactions(currency);
```

## Security Notes

- **Never commit** production credentials to git
- Use Oracle Wallet for production credentials
- Enable SSL/TLS for database connections in production
- Implement proper access control in Enduro/X configuration
- Rotate database passwords regularly
- Docker Compose uses default credentials (ctp/ctp) - **change for production**

## Additional Documentation

- **Performance Analysis**: [../DIESEL_BENCHMARK_RESULTS.md](../DIESEL_BENCHMARK_RESULTS.md) - Detailed benchmark comparison (Diesel vs native driver)
- **Main README**: [../README.md](../README.md) - Complete project documentation
- **REST Integration**: [ORACLE_REST_INTEGRATION.md](../ORACLE_REST_INTEGRATION.md) - REST gateway implementation details (if exists)
- **Diesel Documentation**: [diesel.rs](https://diesel.rs/) - Official Diesel ORM documentation
- **diesel-oci**: [crates.io/crates/diesel-oci](https://crates.io/crates/diesel-oci) - Oracle driver for Diesel

## License

Same as the parent Enduro/X Rust project.
