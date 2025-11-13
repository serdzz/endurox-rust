# Docker Database Setup Guide

The docker-compose.yml now supports both PostgreSQL and Oracle databases using Docker profiles.

## Quick Start

### Option 1: PostgreSQL (Recommended for development)

```bash
# Start with PostgreSQL
docker-compose --profile postgres up -d

# Or set default in .env file
echo "COMPOSE_PROFILES=postgres" > .env
docker-compose up -d
```

The application will automatically:
- Start PostgreSQL container
- Run PostgreSQL migrations
- Connect to postgres://endurox:endurox@postgres:5432/endurox

### Option 2: Oracle

```bash
# Start with Oracle
docker-compose --profile oracle up -d

# Set DATABASE_URL for Oracle
docker-compose --profile oracle up -d \
  -e DATABASE_URL=oracle://ctp:ctp@oracledb:1521/XE
```

### Option 3: Both Databases

```bash
# Start both databases (useful for testing)
docker-compose --profile all up -d
```

## Configuration

### Environment Variables

Create a `.env` file in the project root:

**For PostgreSQL:**
```env
COMPOSE_PROFILES=postgres
DATABASE_URL=postgres://endurox:endurox@postgres:5432/endurox
TIMEZONE=UTC
```

**For Oracle:**
```env
COMPOSE_PROFILES=oracle
DATABASE_URL=oracle://ctp:ctp@oracledb:1521/XE
TIMEZONE=UTC
```

### Custom Database URL

Override the database URL at runtime:

```bash
# PostgreSQL with custom credentials
docker-compose --profile postgres up -d \
  -e DATABASE_URL=postgres://myuser:mypass@postgres:5432/mydb

# Oracle with custom settings
docker-compose --profile oracle up -d \
  -e DATABASE_URL=oracle://system:oracle123@oracledb:1521/XE
```

## Database Access

### PostgreSQL

```bash
# Connect from host
psql -h localhost -U endurox -d endurox

# Connect from container
docker-compose exec postgres psql -U endurox -d endurox

# Run SQL file
docker-compose exec -T postgres psql -U endurox -d endurox < script.sql
```

**Connection details:**
- Host: localhost
- Port: 5432
- User: endurox
- Password: endurox
- Database: endurox

### Oracle

```bash
# Connect from host (requires Oracle client)
sqlplus ctp/ctp@localhost:11521/XE

# Connect from endurox_rust container
docker-compose exec endurox_rust sqlplus ctp/ctp@oracledb:1521/XE
```

**Connection details:**
- Host: localhost
- Port: 11521 (mapped from container 1521)
- User: ctp
- Password: ctp
- Service: XE

## Running Migrations

Migrations run automatically on container startup. To run manually:

### PostgreSQL Migrations

```bash
# Inside container
docker-compose exec endurox_rust bash
export DATABASE_URL=postgres://endurox:endurox@postgres:5432/endurox
python3 /app/oracle_txn_server/migrate_universal.py status
python3 /app/oracle_txn_server/migrate_universal.py run

# From host (if you have Python and drivers installed)
export DATABASE_URL=postgres://endurox:endurox@localhost:5432/endurox
pip install psycopg2-binary
./oracle_txn_server/migrate_universal.py run
```

### Oracle Migrations

```bash
# Inside container
docker-compose exec endurox_rust bash
export DATABASE_URL=oracle://ctp:ctp@oracledb:1521/XE
python3 /app/oracle_txn_server/migrate_universal.py status
python3 /app/oracle_txn_server/migrate_universal.py run

# From host (if you have Python and drivers installed)
export DATABASE_URL=oracle://ctp:ctp@localhost:11521/XE
pip install oracledb
./oracle_txn_server/migrate_universal.py run
```

## Data Persistence

Database data is stored in Docker volumes:

```bash
# List volumes
docker volume ls | grep endurox

# Inspect volume
docker volume inspect endurox-postgres-data
docker volume inspect endurox-oracle-data

# Remove volumes (WARNING: This deletes all data)
docker-compose down -v
```

## Switching Databases

To switch from one database to another:

```bash
# Stop current setup
docker-compose down

# Start with different profile
docker-compose --profile postgres up -d
# or
docker-compose --profile oracle up -d
```

Note: Each database has its own volume, so data persists when switching.

## Troubleshooting

### PostgreSQL Issues

**Connection refused:**
```bash
# Check if PostgreSQL is running
docker-compose ps postgres

# Check logs
docker-compose logs postgres

# Test connection
docker-compose exec postgres pg_isready -U endurox
```

**Permission denied:**
```bash
# Check volume permissions
docker-compose exec postgres ls -la /var/lib/postgresql/data
```

### Oracle Issues

**ORA-12154: TNS:could not resolve:**
```bash
# Check if Oracle is running
docker-compose ps oracledb

# Check logs
docker-compose logs oracledb

# Wait for Oracle to fully initialize (can take 1-2 minutes)
docker-compose logs -f oracledb | grep "DATABASE IS READY"
```

**Long startup time:**
Oracle XE takes 1-2 minutes to initialize on first start. Be patient and watch the logs.

### Application Issues

**Migration failed:**
```bash
# Check DATABASE_URL is set correctly
docker-compose exec endurox_rust env | grep DATABASE_URL

# Check database is reachable
docker-compose exec endurox_rust bash
# For PostgreSQL:
psql -h postgres -U endurox -d endurox -c "SELECT 1"
# For Oracle:
sqlplus -L ctp/ctp@oracledb:1521/XE <<< "SELECT 1 FROM DUAL;"
```

**Wrong database type:**
Make sure DATABASE_URL scheme matches the running database:
- PostgreSQL: `postgres://` or `postgresql://`
- Oracle: `oracle://`

## Performance Comparison

### PostgreSQL
✅ Fast startup (~5 seconds)
✅ Lightweight (~30MB RAM)
✅ Standard SQL support
✅ Excellent for development

### Oracle
⚠️ Slow startup (~60-120 seconds)
⚠️ Heavy (~2GB RAM)
✅ Enterprise features
✅ Required for production Oracle environments

## Recommendation

**For Development:** Use PostgreSQL
- Faster iteration
- Lighter resource usage
- Easier setup

**For Production:** Match your production database
- Test with Oracle if deploying to Oracle
- Test with PostgreSQL if deploying to PostgreSQL

## Examples

### Development Workflow

```bash
# Day-to-day development with PostgreSQL
echo "COMPOSE_PROFILES=postgres" > .env
docker-compose up -d

# Check application logs
docker-compose logs -f endurox_rust

# Test REST API
curl http://localhost:8080/status

# Stop when done
docker-compose down
```

### Testing with Both Databases

```bash
# Test PostgreSQL
docker-compose --profile postgres up -d
docker-compose exec endurox_rust ./test_rest.sh
docker-compose down

# Test Oracle
docker-compose --profile oracle up -d
docker-compose exec -e DATABASE_URL=oracle://ctp:ctp@oracledb:1521/XE \
  endurox_rust ./test_rest.sh
docker-compose down
```

## Additional Resources

- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [Oracle XE Documentation](https://docs.oracle.com/en/database/oracle/oracle-database/)
- [Docker Compose Profiles](https://docs.docker.com/compose/profiles/)
- [Migration Guide](./oracle_txn_server/MIGRATION_GUIDE.md)
