# Oracle REST Gateway Integration

## Overview

This document describes the integration between the REST gateway and the Oracle Transaction Server (`oracle_txn_server`), enabling REST API access to Oracle database operations through Enduro/X services.

## Architecture

```
┌────────────┐   HTTP/REST    ┌──────────────┐   Enduro/X    ┌─────────────────┐   Oracle    ┌──────────┐
│   Client   │ ────────────> │ rest_gateway │ ───tpcall──> │oracle_txn_server│ ──────────> │ Oracle   │
│ (curl/web) │               │ (Actix-web)  │   (UBF)      │   (Rust)        │   (SQL)     │ Database │
└────────────┘               └──────────────┘              └─────────────────┘             └──────────┘
```

## Implementation

### REST Endpoints Added

Three new endpoints were added to `rest_gateway/src/main.rs`:

1. **POST /api/oracle/create** - Create transaction in Oracle DB
   - Calls Enduro/X service: `CREATE_TXN`
   - Input: JSON with transaction details
   - Output: JSON with status and message

2. **POST /api/oracle/get** - Get transaction by ID
   - Calls Enduro/X service: `GET_TXN`
   - Input: JSON with transaction_id
   - Output: JSON with transaction details

3. **GET /api/oracle/list** - List all transactions
   - Calls Enduro/X service: `LIST_TXN`
   - Input: None
   - Output: JSON with transaction count

### Request/Response Flow

1. **Client → REST Gateway (JSON)**
   ```json
   {
     "transaction_type": "sale",
     "transaction_id": "TXN001",
     "account": "ACC123",
     "amount": 10050,
     "currency": "USD",
     "description": "Payment via REST"
   }
   ```

2. **REST Gateway → Oracle Server (UBF)**
   - JSON deserialized to Rust struct
   - Struct encoded to UBF buffer using `UbfStructDerive` macro
   - UBF buffer sent via `tpcall()` to service

3. **Oracle Server → Database (SQL)**
   - UBF buffer decoded to Rust struct
   - SQL INSERT/SELECT executed
   - Oracle connection pool managed automatically
   - **Transaction committed** to persist changes

4. **Response Chain (UBF → JSON)**
   - SQL result encoded to UBF buffer
   - UBF returned via `tpreturn()`
   - REST gateway decodes UBF to struct
   - Struct serialized to JSON response

### Code Changes

#### 1. rest_gateway/src/main.rs

Added three handler functions:
- `create_oracle_transaction()` - Handler for CREATE_TXN
- `get_oracle_transaction()` - Handler for GET_TXN
- `list_oracle_transactions()` - Handler for LIST_TXN

Added helper function:
- `process_transaction_response()` - Shared UBF response processing

Added request structure:
- `GetTransactionRequest` - UBF-mapped struct for GET requests

Registered new routes in `main()`:
```rust
.route("/api/oracle/create", web::post().to(create_oracle_transaction))
.route("/api/oracle/get", web::post().to(get_oracle_transaction))
.route("/api/oracle/list", web::get().to(list_oracle_transactions))
```

#### 2. oracle_txn_server/src/services.rs

Added database commit after INSERT:
```rust
// Commit the transaction
if let Err(e) = conn.commit() {
    tplog_error(&format!("Failed to commit transaction: {}", e));
    return create_error_response(&req.transaction_id, "DB_COMMIT_ERROR", &e.to_string());
}
```

This was critical - Oracle doesn't auto-commit, so transactions were being inserted but rolled back.

#### 3. db/oracle/01_init.sql

Added transactions table schema:
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
```

Added indexes for performance:
- `idx_transactions_type` - On transaction_type
- `idx_transactions_status` - On status
- `idx_transactions_created` - On created_at DESC

#### 4. docker-compose.yml

Fixed Oracle init scripts mount path:
```yaml
volumes:
  - ./db/oracle:/docker-entrypoint-initdb.d  # Changed from /docker-entrypoint-initdb.d/setup
```

## Testing

### Test Script

Created `test_oracle_rest.sh` with 6 test cases:
1. Create transaction TXN100
2. Get transaction TXN100
3. Create transaction TXN101
4. List all transactions
5. Get non-existent transaction (error case)
6. Create with invalid transaction type (error case)

### Manual Testing

```bash
# Start services
docker-compose up -d

# Wait for initialization (Oracle takes ~1-2 minutes on first start)
sleep 120

# Create transaction
curl -X POST http://localhost:8080/api/oracle/create \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_type": "sale",
    "transaction_id": "TXN001",
    "account": "ACC123",
    "amount": 10050,
    "currency": "USD",
    "description": "Test transaction"
  }'

# Response:
# {"transaction_id":"TXN001","status":"SUCCESS","message":"Transaction TXN001 created successfully"}

# Get transaction
curl -X POST http://localhost:8080/api/oracle/get \
  -H "Content-Type: application/json" \
  -d '{"transaction_id":"TXN001"}'

# List transactions
curl -X GET http://localhost:8080/api/oracle/list
```

### Verification in Database

```bash
# Connect to Oracle
docker-compose exec -T oracledb sqlplus -S ctp/ctp@//localhost:1521/XE <<< \
  "SELECT id, transaction_type, amount, currency, status FROM transactions;"
```

## Issues Encountered and Resolved

### 1. Oracle User Creation in Pluggable Database
**Problem**: Initial script failed with `ORA-65096: invalid common user or role name`
**Solution**: Used `ALTER SESSION SET "_ORACLE_SCRIPT"=true;` to allow local user creation in pluggable database

### 2. Init Scripts Not Executing
**Problem**: Oracle container didn't run init scripts
**Solution**: Changed mount path from `/docker-entrypoint-initdb.d/setup` to `/docker-entrypoint-initdb.d`

### 3. Transactions Not Persisting
**Problem**: INSERT succeeded but data not visible in SELECT
**Solution**: Added explicit `conn.commit()` after INSERT operation

### 4. Container Restart Loops
**Problem**: `ndrxd` failed with "Duplicate startup, PID already exists"
**Solution**: Full restart with `docker-compose down && docker-compose up -d` to clear stale PIDs

## Features

### Implemented
- ✅ Three REST endpoints for Oracle operations
- ✅ JSON ↔ UBF conversion using derive macros
- ✅ Oracle connection pooling
- ✅ Transaction persistence with explicit commit
- ✅ Comprehensive error handling
- ✅ Test script with multiple scenarios
- ✅ Documentation in README

### Data Flow
- JSON requests automatically mapped to UBF using `#[derive(UbfStructDerive)]`
- UBF buffers efficiently transferred via Enduro/X IPC
- Oracle connection pool provides connection management
- Transactions explicitly committed for data persistence
- Errors properly propagated through all layers

### Performance Considerations
- Connection pooling minimizes Oracle connection overhead
- UBF provides efficient binary serialization
- Enduro/X IPC is optimized for low latency
- Indexes on transactions table support fast queries

## Future Enhancements

Potential improvements:
1. Add UPDATE and DELETE operations
2. Implement filtering/pagination for LIST endpoint
3. Add transaction statistics/aggregation endpoints
4. Support bulk operations
5. Add XA transaction support for distributed transactions
6. Implement caching layer for frequently accessed transactions

## Files Modified/Created

### Modified
- `rest_gateway/src/main.rs` - Added Oracle endpoints
- `oracle_txn_server/src/services.rs` - Added commit
- `db/oracle/01_init.sql` - Added transactions table
- `docker-compose.yml` - Fixed init scripts path
- `README.md` - Added Oracle endpoints documentation

### Created
- `test_oracle_rest.sh` - Test script for Oracle endpoints
- `ORACLE_REST_INTEGRATION.md` - This documentation

## Conclusion

The Oracle REST gateway integration successfully demonstrates:
- Clean separation of concerns (REST ↔ Business Logic ↔ Database)
- Efficient data serialization with UBF
- Reliable transaction management with explicit commits
- Comprehensive error handling across all layers
- Well-documented API with test scripts

The implementation provides a solid foundation for building production-ready transactional REST APIs with Enduro/X and Oracle Database.
