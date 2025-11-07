# Oracle Transaction Server

Enduro/X transaction server with Oracle Database integration using native Oracle driver.

## Features

- **Native Oracle Driver** - Direct Oracle database access with `oracle` crate
- **Connection Pooling** - R2D2 connection pool for efficient database access
- **UBF Integration** - Seamless UBF buffer serialization/deserialization using derive macros
- **REST API Integration** - Full REST gateway support with automatic JSON ↔ UBF conversion
- **Three Services**:
  - `CREATE_TXN` - Create new transaction record with automatic commit
  - `GET_TXN` - Retrieve transaction by ID
  - `LIST_TXN` - List all transactions (max 100)

## Prerequisites

1. **Oracle Instant Client** - Install Oracle instant client libraries
   ```bash
   # On macOS (example)
   brew install oracle-instantclient
   
   # On Linux
   # Download from Oracle website and install
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

This creates the `transactions` table with the following schema:
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
┌─────────────┐  HTTP/REST  ┌──────────────┐  Enduro/X  ┌─────────────────┐  Oracle  ┌──────────┐
│   Client    │ ──────────> │ rest_gateway │ ──tpcall─> │oracle_txn_server│ ───SQL─> │ Oracle   │
│ (curl/web)  │             │ (Actix-web)  │   (UBF)    │   + Connection  │          │ Database │
│             │ <────────── │              │ <────────  │     Pool        │ <──────  │ XE 21c   │
└─────────────┘    JSON     └──────────────┘    UBF     └─────────────────┘  Result  └──────────┘
```

### Data Flow

1. **Client** sends HTTP/JSON request to REST gateway
2. **REST Gateway** converts JSON to Rust struct, then encodes to UBF using `#[derive(UbfStructDerive)]`
3. **Enduro/X** transfers UBF buffer via IPC to oracle_txn_server using `tpcall()`
4. **Oracle Server** decodes UBF to Rust struct using `UbfStruct::from_ubf()`
5. **Oracle Driver** executes SQL query with connection pooling
6. **Database** commits transaction explicitly (`conn.commit()`)
7. **Response** flows back through the same chain: SQL result → UBF → JSON

## Transaction Management

The server uses explicit commit for transaction persistence:

```rust
// Execute SQL
match conn.execute(schema::CREATE_TRANSACTION, &[...]) {
    Ok(_) => {
        // IMPORTANT: Explicit commit required for Oracle
        if let Err(e) = conn.commit() {
            tplog_error(&format!("Failed to commit transaction: {}", e));
            return create_error_response(&req.transaction_id, "DB_COMMIT_ERROR", &e.to_string());
        }
        
        create_success_response(&req.transaction_id, &message)
    }
    Err(e) => {
        create_error_response(&req.transaction_id, "DB_INSERT_ERROR", &e.to_string())
    }
}
```

**Note**: Oracle doesn't auto-commit by default, so explicit `commit()` is required after INSERT/UPDATE operations.

## Database Schema

The transactions table is automatically created by `db/oracle/01_init.sql`:

```sql
CREATE TABLE ctp.transactions (
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
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Performance indexes
CREATE INDEX idx_transactions_type ON ctp.transactions(transaction_type);
CREATE INDEX idx_transactions_status ON ctp.transactions(status);
CREATE INDEX idx_transactions_created ON ctp.transactions(created_at DESC);
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

- **REST Integration**: See [ORACLE_REST_INTEGRATION.md](../ORACLE_REST_INTEGRATION.md) for detailed implementation
- **Main README**: See [../README.md](../README.md) for complete project documentation
- **Docker Usage**: See [../DOCKER_USAGE.md](../DOCKER_USAGE.md) for Docker Compose guide

## License

Same as the parent Enduro/X Rust project.
