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
# Use 16 workers
export REST_WORKERS=16

# Default: num_cpus * 2
```

**Important**: Each worker thread creates its own Enduro/X client connection using thread-local storage. This eliminates mutex contention and enables true parallel request processing.

More workers = better concurrency for handling simultaneous requests. Recommended settings:
- **Development**: 2-4 workers
- **Production**: 8-16 workers (or num_cpus * 2)
- **High load**: 16-32 workers

### Backend Service Instances

For optimal performance, configure multiple instances of backend services in `ndrxconfig.xml`:

```xml
<server name="oracle_txn_server">
    <min>5</min>
    <max>5</max>
    <srvid>11</srvid>
    <sysopt>-e ${NDRX_APPHOME}/log/oracle_txn_server.log -r</sysopt>
</server>
```

This allows Enduro/X to distribute load across multiple service instances.

### Performance Tuning

For optimal performance:

1. **REST Gateway**: Configure workers
```bash
export REST_WORKERS=16
./target/release/rest_gateway
```

2. **Backend Services**: Run multiple instances
```bash
# In ndrxconfig.xml, set min/max to 5 or more
# Then restart Enduro/X
xadmin stop
xadmin start
```

3. **Verify load distribution**:
```bash
xadmin psc
```

Expected performance with 16 workers + 5 backend instances:
- **GET_TXN**: ~1700-1800 req/sec
- **LIST_TXN**: ~1600-1900 req/sec
- **CREATE_TXN**: ~100-150 req/sec (with parallel benchmark)

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

### Sequential Benchmark

Basic benchmark with sequential requests:

```bash
./benchmark_oracle_rest_v2.sh
```

### Parallel Benchmark

Test concurrent load with parallel CREATE requests:

```bash
# 10 concurrent connections, 100 total requests
./benchmark_create_parallel.sh 10 100

# 20 concurrent connections, 200 total requests
./benchmark_create_parallel.sh 20 200
```

The parallel benchmark:
- Simulates real-world concurrent load
- Tests throughput under parallel requests
- Shows how well load distributes across backend instances
- Measures average response time and success rate

## Architecture

### Thread-Local Client Design

The gateway uses thread-local storage for Enduro/X clients:

```rust
thread_local! {
    static CLIENT: RefCell<Option<EnduroxClient>> = RefCell::new(None);
}
```

**Benefits**:
- **Zero contention**: No mutex locks between threads
- **True parallelism**: Multiple requests processed simultaneously
- **Auto-initialization**: Each worker creates client on first use
- **Resource efficiency**: One client per worker thread

### Components

- **Actix-Web**: HTTP server with configurable worker pool
- **Thread-local EnduroxClient**: Per-thread client connections
- **UBF encoding/decoding**: Structured data exchange
- **Enduro/X load balancing**: Distributes requests across service instances

### Request Flow

1. HTTP request arrives at Actix-Web worker
2. Worker gets/creates thread-local EnduroxClient
3. Request encoded to UBF format
4. Enduro/X routes to available service instance
5. Service processes and returns UBF response
6. Response decoded to JSON and returned to client

Each worker operates independently without blocking others.
