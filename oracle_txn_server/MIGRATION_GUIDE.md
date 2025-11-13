# Migration Guide: Multi-Database Support

## Overview

The oracle_txn_server now supports both PostgreSQL and Oracle databases with automatic database detection and separate migration files for each platform.

## What Changed

### 1. Database Support
- **Before**: Oracle-only support
- **After**: PostgreSQL and Oracle support with automatic detection from `DATABASE_URL`

### 2. Migration Structure
```
migrations/
├── postgres/               # PostgreSQL-specific migrations
│   └── YYYY-MM-DD-NNNNNN_name/
│       ├── up.sql
│       └── down.sql
├── oracle/                 # Oracle-specific migrations
│   └── YYYY-MM-DD-NNNNNN_name/
│       ├── up.sql
│       └── down.sql
└── README.md              # Migration documentation
```

### 3. Migration Tool
- **New**: `migrate_universal.py` - Universal migration tool
- **Old**: `migrate.py` - Oracle-only tool (still available)

## Quick Start

### PostgreSQL Setup

```bash
# Install PostgreSQL driver
pip install psycopg2-binary

# Set DATABASE_URL
export DATABASE_URL=postgres://endurox:endurox@localhost:5432/endurox

# Run migrations
./migrate_universal.py run
```

### Oracle Setup

```bash
# Install Oracle driver
pip install oracledb

# Set DATABASE_URL
export DATABASE_URL=oracle://endurox:endurox@localhost:1521/XEPDB1

# Run migrations
./migrate_universal.py run
```

## Migration Commands

```bash
# Check status
./migrate_universal.py status

# Apply pending migrations
./migrate_universal.py run

# Rollback last migration
./migrate_universal.py rollback

# Rollback last 2 migrations
./migrate_universal.py rollback 2

# Reset database (rollback all)
./migrate_universal.py reset
```

## Key Differences Between Databases

### Data Types
| Concept      | PostgreSQL | Oracle      |
|--------------|------------|-------------|
| String       | VARCHAR    | VARCHAR2    |
| Integer      | BIGINT     | NUMBER(19)  |
| Timestamp    | TIMESTAMP  | TIMESTAMP   |

### Triggers
**PostgreSQL** uses PL/pgSQL functions:
```sql
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_transactions_updated_at
BEFORE UPDATE ON transactions
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();
```

**Oracle** uses PL/SQL:
```sql
CREATE OR REPLACE TRIGGER trg_transactions_updated_at
BEFORE UPDATE ON transactions
FOR EACH ROW
BEGIN
    :NEW.updated_at := CURRENT_TIMESTAMP;
END;
```

## Testing

Both databases can be tested locally:

### PostgreSQL with Docker
```bash
docker run -d \
  --name postgres-endurox \
  -e POSTGRES_USER=endurox \
  -e POSTGRES_PASSWORD=endurox \
  -e POSTGRES_DB=endurox \
  -p 5432:5432 \
  postgres:15

export DATABASE_URL=postgres://endurox:endurox@localhost:5432/endurox
./migrate_universal.py run
```

### Oracle with Docker
```bash
docker run -d \
  --name oracle-endurox \
  -e ORACLE_PASSWORD=endurox \
  -p 1521:1521 \
  gvenzl/oracle-xe:21-slim

export DATABASE_URL=oracle://endurox:endurox@localhost:1521/XEPDB1
./migrate_universal.py run
```

## Troubleshooting

### PostgreSQL Issues

**Connection refused:**
```bash
# Check PostgreSQL is running
psql -h localhost -U endurox -d endurox -c "SELECT 1"
```

**psycopg2 installation issues:**
```bash
# On macOS with Homebrew PostgreSQL
pip install psycopg2-binary
```

### Oracle Issues

**ORA-12154: TNS:could not resolve the connect identifier:**
```bash
# Verify Oracle is running and accessible
tnsping localhost:1521/XEPDB1
```

**oracledb installation issues:**
```bash
# Install Oracle Instant Client if needed
# macOS:
brew install oracle-instantclient

# Linux:
# Download from Oracle website
```

## Migration Compatibility

The old `migrate.py` tool is still available for Oracle-only deployments:
```bash
export DATABASE_URL=oracle://user:pass@host:port/service
python migrate.py run
```

However, it only works with the old migration structure in `migrations/YYYY-MM-DD-NNNNNN_name/`.

## Best Practices

1. **Always test migrations on both databases** before committing
2. **Keep migration versions synchronized** between postgres/ and oracle/
3. **Use DATABASE_URL** for configuration instead of hardcoding connection details
4. **Run migrations in CI/CD** to catch schema drift early
5. **Back up production databases** before running migrations

## CI/CD Integration

The migrations can be automated in GitHub Actions or GitLab CI:

```yaml
# Example for PostgreSQL
- name: Run migrations
  env:
    DATABASE_URL: postgres://user:pass@db:5432/database
  run: |
    pip install psycopg2-binary
    ./oracle_txn_server/migrate_universal.py run
```

## Future Improvements

- [ ] Add support for MySQL/MariaDB
- [ ] Create migration generator tool
- [ ] Add migration dry-run mode
- [ ] Support for migration dependencies
- [ ] Parallel migration execution
