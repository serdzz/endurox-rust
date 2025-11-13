# Database Migrations

This project supports both PostgreSQL and Oracle databases with separate migration files for each.

## Directory Structure

```
migrations/
├── postgres/
│   └── 2025-11-07-000000_create_transactions/
│       ├── up.sql
│       └── down.sql
└── oracle/
    └── 2025-11-07-000000_create_transactions/
        ├── up.sql
        └── down.sql
```

## Migration Tool

The `migrate_universal.py` script automatically detects your database type from the `DATABASE_URL` environment variable and applies the appropriate migrations.

### Prerequisites

**For PostgreSQL:**
```bash
pip install psycopg2-binary
```

**For Oracle:**
```bash
pip install oracledb
```

### Usage

```bash
# Set your database URL
export DATABASE_URL=postgres://user:pass@localhost:5432/database
# or
export DATABASE_URL=oracle://user:pass@localhost:1521/service

# Check migration status
./migrate_universal.py status

# Apply all pending migrations
./migrate_universal.py run

# Rollback last migration
./migrate_universal.py rollback

# Rollback last N migrations
./migrate_universal.py rollback 2

# Reset database (rollback all migrations)
./migrate_universal.py reset
```

### Example Output

```
$ ./migrate_universal.py status
Detected database type: POSTGRES
Connection: endurox@localhost:5432/endurox

Migration Status:
------------------------------------------------------------
✓ Applied    2025-11-07-000000_create_transactions
------------------------------------------------------------
Total: 1 migrations, 1 applied, 0 pending
```

## Database Differences

### PostgreSQL
- Uses standard SQL types: `VARCHAR`, `BIGINT`, `TIMESTAMP`
- Trigger function written in PL/pgSQL
- Automatically updates `updated_at` on row modification

### Oracle
- Uses Oracle-specific types: `VARCHAR2`, `NUMBER`, `TIMESTAMP`
- Trigger written in PL/SQL
- Automatically updates `updated_at` on row modification

## Schema

Both databases create the same `transactions` table with the following structure:

| Column           | Type          | Description                    |
|------------------|---------------|--------------------------------|
| id               | VARCHAR(100)  | Primary key, transaction ID    |
| transaction_type | VARCHAR(50)   | Type of transaction (e.g., "sale") |
| account          | VARCHAR(100)  | Account identifier             |
| amount           | BIGINT/NUMBER | Transaction amount             |
| currency         | VARCHAR(10)   | Currency code (e.g., "USD")    |
| description      | VARCHAR(500)  | Optional description           |
| status           | VARCHAR(50)   | Transaction status             |
| message          | VARCHAR(500)  | Status message                 |
| error_code       | VARCHAR(50)   | Error code if failed           |
| error_message    | VARCHAR(500)  | Error message if failed        |
| created_at       | TIMESTAMP     | Creation timestamp             |
| updated_at       | TIMESTAMP     | Last update timestamp          |

## Indexes

Three indexes are created for query performance:
- `idx_transactions_type` - On `transaction_type`
- `idx_transactions_status` - On `status`
- `idx_transactions_created` - On `created_at DESC`

## Adding New Migrations

To add a new migration:

1. Create migration directories for both databases:
```bash
mkdir -p migrations/postgres/YYYY-MM-DD-NNNNNN_migration_name
mkdir -p migrations/oracle/YYYY-MM-DD-NNNNNN_migration_name
```

2. Create `up.sql` and `down.sql` in each directory with database-specific SQL

3. Ensure version numbers match across both databases

4. Test with both database types before committing
