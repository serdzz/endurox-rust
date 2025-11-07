# Docker Compose Usage Guide

## Overview

This project uses Docker Compose to orchestrate multiple services:
- **endurox_rust**: Enduro/X middleware with Rust services
- **oracledb**: Oracle Database XE 21c (optional)

## Quick Start

### Start All Services (Default)

```bash
# Build and start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Check service status
docker-compose ps
```

This will:
1. Start Oracle Database
2. Wait for Oracle to be healthy (~1-2 minutes first time)
3. Run database initialization scripts
4. Start Enduro/X services

### Start Without Database

To run Enduro/X without Oracle:

1. **Option 1**: Comment out `depends_on` section in `docker-compose.yml`
2. **Option 2**: Start only specific service:
   ```bash
   docker-compose up -d endurox_rust
   ```

## Service Management

### Starting Services

```bash
# Start all services
docker-compose up -d

# Start specific service
docker-compose up -d endurox_rust

# Start with live logs
docker-compose up
```

### Stopping Services

```bash
# Stop all services (data preserved)
docker-compose stop

# Stop specific service
docker-compose stop endurox_rust

# Stop and remove containers (data preserved in volumes)
docker-compose down

# Stop, remove containers AND volumes (data deleted)
docker-compose down -v
```

### Restarting Services

```bash
# Restart all
docker-compose restart

# Restart specific service
docker-compose restart endurox_rust

# Restart when Oracle crashes
docker-compose restart oracledb
```

## Viewing Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f endurox_rust
docker-compose logs -f oracledb

# Last 100 lines
docker-compose logs --tail=100 endurox_rust

# Since specific time
docker-compose logs --since="2024-01-01T10:00:00" oracledb
```

## Executing Commands

### Inside Enduro/X Container

```bash
# Interactive shell
docker-compose exec endurox_rust bash

# Run single command
docker-compose exec endurox_rust xadmin psc

# Check Enduro/X status
docker-compose exec endurox_rust bash -c '. ./setenv.sh && xadmin psc'

# Run test client
docker-compose exec endurox_rust bash -c '. ./setenv.sh && /app/bin/ubf_test_client'
```

### Inside Oracle Container

```bash
# Connect as SYSTEM user
docker-compose exec oracledb sqlplus system/oracle123@XE

# Connect as CTP user
docker-compose exec oracledb sqlplus ctp/ctp@XE

# Run SQL script
docker-compose exec oracledb sqlplus ctp/ctp@XE @/docker-entrypoint-initdb.d/setup/01_init.sql
```

## Building and Rebuilding

```bash
# Build images
docker-compose build

# Build without cache (clean build)
docker-compose build --no-cache

# Rebuild and restart
docker-compose up -d --build

# Rebuild specific service
docker-compose build endurox_rust
```

## Database Operations

### Reset Database (Complete Wipe)

```bash
# Stop everything
docker-compose down

# Remove volume (deletes all data)
docker volume rm endurox-oracle-data

# Start fresh (will re-run init scripts)
docker-compose up -d
```

### Backup Database

```bash
# Export using Oracle tools
docker-compose exec oracledb expdp system/oracle123@XE \
  schemas=ctp directory=DATA_PUMP_DIR dumpfile=backup.dmp

# Copy backup to host
docker cp $(docker-compose ps -q oracledb):/opt/oracle/admin/XE/dpdump/backup.dmp ./backup.dmp
```

### Connect from Host

```bash
# Using sqlplus (if installed on host)
sqlplus ctp/ctp@localhost:11521/XE

# Using Docker
docker run -it --rm --network endurox-network \
  oracledb/sqlplus:latest ctp/ctp@oracledb:1521/XE
```

## Troubleshooting

### Oracle Won't Start

```bash
# Check logs
docker-compose logs oracledb | grep -i error

# Check if port is already in use
lsof -i :11521
netstat -an | grep 11521

# Remove and recreate
docker-compose down -v
docker-compose up -d
```

### Enduro/X Won't Start

```bash
# Check if waiting for Oracle
docker-compose ps

# View detailed logs
docker-compose logs endurox_rust

# Check Enduro/X logs
tail -f logs/ULOG.*

# Restart manually
docker-compose exec endurox_rust bash -c '. ./setenv.sh && xadmin stop -y && xadmin start -y'
```

### Health Check Failing

```bash
# Check Oracle status
docker-compose exec oracledb lsnrctl status

# Manual health check
docker-compose exec oracledb sqlplus -L system/oracle123@//localhost:1521/XE <<< "SELECT 1 FROM DUAL;"

# Increase health check timeout in docker-compose.yml
# Change: start_period: 60s -> start_period: 120s
```

### Network Issues

```bash
# Recreate network
docker network rm endurox-network
docker-compose up -d

# Check network
docker network inspect endurox-network

# Test connectivity
docker-compose exec endurox_rust ping oracledb
```

## Performance Tuning

### Oracle Performance

Add to `docker-compose.yml` under `oracledb.environment`:
```yaml
ORACLE_SGA_SIZE: 2048M
ORACLE_PGA_SIZE: 512M
```

### Resource Limits

Add to service definition:
```yaml
deploy:
  resources:
    limits:
      cpus: '2'
      memory: 4G
    reservations:
      cpus: '1'
      memory: 2G
```

## Development Workflow

### Typical Development Cycle

```bash
# 1. Make code changes
vim samplesvr_rust/src/main.rs

# 2. Rebuild
docker-compose build endurox_rust

# 3. Restart
docker-compose up -d endurox_rust

# 4. View logs
docker-compose logs -f endurox_rust

# 5. Test
./test_rest.sh
```

### Live Development

Mount source as volume for live reloading:
```yaml
services:
  endurox_rust:
    volumes:
      - ./logs:/app/log
      - ./samplesvr_rust/src:/app/samplesvr_rust/src
      - ./rest_gateway/src:/app/rest_gateway/src
```

Then rebuild inside container:
```bash
docker-compose exec endurox_rust bash
cargo build --release
xadmin restart -s samplesvr_rust -i 1
```

## Production Considerations

⚠️ **This setup is for DEVELOPMENT only**

For production:
1. Use strong passwords (not `ctp/ctp`)
2. Enable TLS/SSL
3. Use secrets management (Docker secrets or Vault)
4. Set up proper backup strategy
5. Use Oracle Enterprise Edition
6. Configure monitoring and alerting
7. Use proper volume drivers (not local)
8. Set resource limits
9. Enable container security scanning

## Environment Variables

Available variables (set in `.env` or export):

```bash
# Hostname
HOST_NAME=endurod8

# Timezone
TIMEZONE=Europe/Moscow

# Oracle
ORACLE_PASSWORD=oracle123

# Enduro/X
DATABASE_URL=oracle://ctp:ctp@oracledb:1521/XE
```

Create `.env` file:
```bash
cat > .env <<EOF
HOST_NAME=myserver
TIMEZONE=UTC
EOF
```

## Advanced Usage

### Multiple Environments

```bash
# Development
docker-compose -f docker-compose.yml up -d

# Production
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d

# Testing
docker-compose -f docker-compose.yml -f docker-compose.test.yml up -d
```

### Scale Services

```bash
# Run 3 instances of endurox_rust (requires load balancer)
docker-compose up -d --scale endurox_rust=3
```

### Custom Network

```yaml
networks:
  endurox-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.28.0.0/16
```

## Useful Commands

```bash
# Show disk usage
docker system df

# Clean up unused data
docker system prune

# Clean up volumes
docker volume prune

# Show resource usage
docker stats

# Export logs
docker-compose logs > logs/docker-compose.log

# Get IP addresses
docker-compose exec endurox_rust hostname -i
docker-compose exec oracledb hostname -i
```
