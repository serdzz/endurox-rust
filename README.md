# Enduro/X Rust Integration

A modern Rust integration for [Enduro/X](https://www.endurox.org/) - a high-performance middleware platform for building distributed transaction processing systems.

## Overview

This project provides Rust bindings and a complete example implementation for building Enduro/X services and clients. It includes:

- **endurox-sys** - Low-level FFI bindings and safe Rust wrappers for Enduro/X
- **samplesvr_rust** - Example Enduro/X server implementing multiple services
- **rest_gateway** - REST API gateway built with Axum that exposes Enduro/X services over HTTP

## Architecture

```
┌─────────────┐      HTTP/REST      ┌──────────────┐
│   Client    │ ──────────────────> │ rest_gateway │
│ (curl/web)  │                     │   (Axum)     │
└─────────────┘                     └──────┬───────┘
                                           │ Enduro/X
                                           │ tpcall()
                                           ▼
                                    ┌──────────────┐
                                    │ samplesvr_   │
                                    │    rust      │
                                    └──────────────┘
```

## Features

### endurox-sys

Safe Rust bindings for Enduro/X:

- **Server API**: `tpsvrinit`, `tpsvrdone`, service advertisement, `tpreturn`
- **Client API**: `tpinit`, `tpterm`, `tpcall`, `tpacall`, `tpgetrply`
- **Logging**: Integrated Enduro/X logging (`tplog_*`)
- **Buffer Management**: Safe wrappers for STRING and JSON buffers
- **Feature Flags**: Separate `server`, `client`, and `ubf` features for modular builds

### Sample Services

The `samplesvr_rust` server implements:

- **ECHO** - Echo back the input data
- **HELLO** - Personalized greeting service (JSON input/output)
- **STATUS** - Server status and health check
- **DATAPROC** - Data processing service with JSON support

### REST Gateway

HTTP/REST interface powered by Axum:

- **GET /** - Health check endpoint
- **GET /api/status** - Server status
- **POST /api/hello** - Call HELLO service with JSON
- **POST /api/echo** - Call ECHO service with plain text
- **POST /api/dataproc** - Call DATAPROC service with JSON

## Prerequisites

- **Rust** 1.70+ (toolchain installed via rustup)
- **Enduro/X** 8.0+ (installed from `.deb` packages)
- **Docker** (for containerized deployment)
- **Docker Compose** (for orchestration)

## Quick Start

### Using Docker (Recommended)

1. **Build the Docker image:**
   ```bash
   docker-compose build endurox_rust
   ```

2. **Start the services:**
   ```bash
   docker-compose up -d endurox_rust
   ```

3. **Test the REST API:**
   ```bash
   ./test_rest.sh
   ```

4. **View logs:**
   ```bash
   docker-compose logs -f endurox_rust
   # or check the logs directory
   tail -f logs/ULOG.*
   ```

### Manual Build

1. **Set up Enduro/X environment:**
   ```bash
   source setenv.sh
   ```

2. **Build the workspace:**
   ```bash
   cargo build --release
   ```

3. **Start Enduro/X:**
   ```bash
   xadmin start -y
   ```

4. **The services should now be running:**
   - `samplesvr_rust` - Enduro/X server process
   - `rest_gateway` - REST API on http://localhost:8080

## API Examples

### Health Check
```bash
curl http://localhost:8080/
```

### Status Service
```bash
curl http://localhost:8080/api/status
```

### Hello Service
```bash
curl -X POST http://localhost:8080/api/hello \
  -H "Content-Type: application/json" \
  -d '{"name":"World"}'
```

### Echo Service
```bash
curl -X POST http://localhost:8080/api/echo \
  -H "Content-Type: text/plain" \
  -d "Hello from REST Gateway!"
```

### Data Processing Service
```bash
curl -X POST http://localhost:8080/api/dataproc \
  -H "Content-Type: application/json" \
  -d '{"data":"test","count":123}'
```

## Development

### Project Structure

```
.
├── Cargo.toml              # Workspace definition
├── docker-compose.yml      # Docker orchestration
├── Dockerfile              # Container image
├── endurox-sys/           # Rust FFI bindings
│   ├── src/
│   │   ├── lib.rs         # Module exports
│   │   ├── ffi.rs         # Raw FFI declarations
│   │   ├── server.rs      # Server API
│   │   ├── client.rs      # Client API
│   │   └── log.rs         # Logging wrappers
│   └── Cargo.toml
├── samplesvr_rust/        # Example server
│   ├── src/
│   │   ├── main.rs        # Server entry point
│   │   └── services.rs    # Service implementations
│   └── Cargo.toml
├── rest_gateway/          # REST gateway
│   ├── src/
│   │   └── main.rs        # Axum server
│   └── Cargo.toml
├── conf/                  # Enduro/X configuration
├── setenv.sh             # Environment setup
└── test_rest.sh          # API test script
```

### Running Tests

```bash
# Run Clippy for linting
cargo clippy --release

# Run tests
cargo test

# Format code
cargo fmt
```

### Inside Docker

```bash
# Enter the container
docker-compose exec endurox_rust bash

# Check Enduro/X status
xadmin psc
xadmin ppm

# View service queue
xadmin mqlq

# Rebuild inside container
cargo build --release

# Restart services
xadmin stop -y && xadmin start -y
```

## Configuration

- **ndrxconfig.xml** - Enduro/X server configuration
- **conf/\*** - Service-specific configuration files
- **setenv.sh** - Environment variables for Enduro/X

## Troubleshooting

### Check Service Status
```bash
xadmin psc
```

### View Logs
```bash
tail -f logs/ULOG.*
```

### Restart Services
```bash
xadmin restart -s samplesvr_rust -i 1
```

### Clean Rebuild
```bash
cargo clean
cargo build --release
```

## License

This project is provided as-is for demonstration and development purposes.

## Contributing

Contributions are welcome! Please ensure code passes:
- `cargo fmt` - Code formatting
- `cargo clippy` - Linting
- `cargo test` - All tests

## Resources

- [Enduro/X Documentation](https://www.endurox.org/dokuwiki/)
- [Rust FFI Guide](https://doc.rust-lang.org/nomicon/ffi.html)
- [Axum Documentation](https://docs.rs/axum/latest/axum/)
