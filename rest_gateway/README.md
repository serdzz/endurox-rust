# REST Gateway

REST API gateway for Enduro/X services with support for Oracle transaction operations.

## Features

- RESTful API endpoints for Enduro/X services
- Support for STRING and UBF buffer types
- Oracle transaction management (CREATE, GET, LIST)
- Automatic UBF encoding/decoding
- JSON request/response handling

## Configuration

### Workers

The number of worker threads can be configured using the `REST_WORKERS` environment variable:

```bash
# Use 8 workers
export REST_WORKERS=8

# Default: num_cpus * 2
```

More workers = better concurrency for handling simultaneous requests. Recommended settings:
- **Development**: 2-4 workers
- **Production**: 8-16 workers (or num_cpus * 2)
- **High load**: 16-32 workers

### Performance Tuning

For optimal performance in benchmarks:

```bash
# Set workers before starting
export REST_WORKERS=16

# Start the gateway
./target/release/rest_gateway
```

Expected performance with 16 workers:
- GET_TXN: ~1700-1800 req/sec
- LIST_TXN: ~1600-1900 req/sec
- CREATE_TXN: ~40-50 req/sec (limited by database INSERT)

## API Endpoints

### Health Check
```bash
GET /
```

### Oracle Transactions

#### Create Transaction
```bash
POST /api/oracle/create
Content-Type: application/json

{
  "transaction_type": "sale",
  "transaction_id": "TXN-001",
  "account": "ACC-001",
  "amount": 10000,
  "currency": "USD",
  "description": "Payment"
}
```

#### Get Transaction
```bash
POST /api/oracle/get
Content-Type: application/json

{
  "transaction_id": "TXN-001"
}
```

#### List Transactions
```bash
GET /api/oracle/list
```

### Legacy Endpoints

#### Status
```bash
GET /api/status
```

#### Hello
```bash
POST /api/hello
Content-Type: application/json

{
  "name": "World"
}
```

#### Echo
```bash
POST /api/echo
Content-Type: text/plain

Hello World
```

## Building

```bash
cargo build --release --bin rest_gateway
```

## Running

```bash
# Development
cargo run --bin rest_gateway

# Production
export REST_WORKERS=16
./target/release/rest_gateway
```

## Benchmarking

Run the benchmark script to test performance:

```bash
./benchmark_oracle_rest_v2.sh
```

The benchmark tests:
- GET operations (read performance)
- LIST operations (scan performance)
- CREATE operations (write performance)

## Architecture

The gateway uses:
- **Actix-Web** for HTTP server
- **Enduro/X Client API** for service calls
- **UBF** for structured data exchange
- **Worker pool** for concurrent request handling

Each worker maintains its own Enduro/X client connection, allowing parallel service calls without blocking.
