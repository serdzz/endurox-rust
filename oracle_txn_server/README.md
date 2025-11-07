# Oracle Transaction Server

Enduro/X transaction server with Oracle Database integration using Diesel ORM.

## Features

- **Diesel ORM** - Type-safe database access with Oracle support
- **Connection Pooling** - R2D2 connection pool for efficient database access
- **UBF Integration** - Seamless UBF buffer serialization/deserialization
- **XA Transactions** - Full Enduro/X XA transaction support with Oracle
- **Three Services**:
  - `CREATE_TXN` - Create new transaction record
  - `GET_TXN` - Retrieve transaction by ID
  - `LIST_TXN` - List all transactions

## Prerequisites

1. **Oracle Instant Client** - Install Oracle instant client libraries
   ```bash
   # On macOS (example)
   brew install oracle-instantclient
   
   # On Linux
   # Download from Oracle website and install
   ```

2. **Oracle Database** - Running Oracle instance (local or remote)
   - XE Edition works fine for development
   - Default connection: `oracle://endurox:endurox@localhost:1521/XEPDB1`

3. **Enduro/X** - Installed and configured
   ```bash
   export NDRX_HOME=/opt/endurox
   export PATH=$NDRX_HOME/bin:$PATH
   ```

## Setup

### 1. Configure Database Connection

Copy the example environment file:
```bash
cp .env.example .env
```

Edit `.env` with your Oracle connection details:
```env
DATABASE_URL=oracle://username:password@hostname:port/service_name
```

### 2. Run Database Migrations

Install diesel CLI if not already installed:
```bash
cargo install diesel_cli --no-default-features --features oracle
```

Run migrations:
```bash
cd oracle_txn_server
diesel migration run
```

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

You can also call these services through the REST gateway:

```bash
# Create transaction
curl -X POST http://localhost:8080/api/oracle/create \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_id": "TXN-12345",
    "transaction_type": "sale",
    "account": "ACC-9876",
    "amount": 15000,
    "currency": "USD",
    "description": "Payment for order #12345"
  }'

# Get transaction
curl -X POST http://localhost:8080/api/oracle/get \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_id": "TXN-12345"
  }'
```

## Architecture

```
┌─────────────┐         ┌──────────────────┐         ┌──────────────┐
│   Client    │   UBF   │ oracle_txn_server│   SQL   │   Oracle DB  │
│             │ ──────> │                  │ ──────> │              │
│ (ud/REST)   │         │  + Diesel ORM    │         │   + XA       │
│             │ <────── │  + Connection    │ <────── │              │
└─────────────┘   UBF   │    Pool          │   SQL   └──────────────┘
                        └──────────────────┘
```

### Data Flow

1. **Client** sends UBF request to service
2. **Server** decodes UBF to Rust struct using `#[derive(UbfStruct)]`
3. **Diesel** executes type-safe SQL query on Oracle DB
4. **XA Layer** manages distributed transaction (if enabled)
5. **Server** encodes result to UBF and returns to client

## XA Transactions

To use XA transactions in service calls:

```rust
use crate::xa;

pub fn create_transaction_with_xa(
    request: &ServiceRequest,
    pool: &DbPool,
) -> ServiceResult {
    // Start XA transaction
    xa::with_transaction(|| {
        // All database operations here are part of XA transaction
        let mut conn = crate::db::get_connection(pool)?;
        
        diesel::insert_into(transactions::table)
            .values(&new_txn)
            .execute(&mut conn)
            .map_err(|e| e.to_string())?;
            
        Ok(())
    }).map_err(|e| create_error_response("", "XA_ERROR", &e))?;
    
    create_success_response(txn_id, "Transaction committed")
}
```

## Database Schema

The transactions table structure:

```sql
CREATE TABLE transactions (
    id VARCHAR2(50) PRIMARY KEY,
    transaction_type VARCHAR2(50) NOT NULL,
    account VARCHAR2(50) NOT NULL,
    amount NUMBER NOT NULL,
    currency VARCHAR2(10) NOT NULL,
    description VARCHAR2(500),
    status VARCHAR2(20) NOT NULL,
    message VARCHAR2(500),
    error_code VARCHAR2(50),
    error_message VARCHAR2(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);
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

### Migration Issues

If migrations fail:
```bash
# Check migration status
diesel migration list

# Revert last migration
diesel migration revert

# Regenerate schema
diesel migration redo
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

Add indexes for frequently queried fields:
```sql
CREATE INDEX idx_transactions_account ON transactions(account);
CREATE INDEX idx_transactions_type ON transactions(transaction_type);
```

## Security Notes

- **Never commit** `.env` file with real credentials
- Use Oracle Wallet for production credentials
- Enable SSL/TLS for database connections
- Implement proper access control in Enduro/X configuration
- Rotate database passwords regularly

## License

Same as the parent Enduro/X Rust project.
