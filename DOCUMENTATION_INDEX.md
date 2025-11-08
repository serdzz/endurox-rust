# Documentation Index

Complete guide to all documentation in the Enduro/X Rust Integration project.

## 📚 Getting Started

- **[README.md](README.md)** - Main project documentation
  - Overview and architecture
  - Quick start with Docker
  - API examples
  - Configuration and troubleshooting

- **[GETTING_STARTED.md](GETTING_STARTED.md)** - Beginner's guide
  - Installation instructions
  - Feature flags explained
  - Quick code examples
  - UBF field table setup
  - Running your first server

- **[RELEASE_NOTES.md](RELEASE_NOTES.md)** - Latest release (v0.1.1)
  - What's new in v0.1.1
  - Migration guide
  - crates.io publication announcement

## 📖 Core Documentation

### endurox-sys Library

- **[endurox-sys/README.md](endurox-sys/README.md)** - Core FFI bindings
  - Complete API reference
  - Environment variables (`NDRX_HOME`, `NDRX_APPHOME`)
  - Features: `server`, `client`, `ubf`, `derive`
  - Usage examples

- **[endurox-derive/README.md](endurox-derive/README.md)** - Derive macros
  - `#[derive(UbfStruct)]` macro
  - Field mapping with `#[ubf(field = id)]`
  - Supported types

### UBF (Unified Buffer Format)

- **[UBF_STRUCT_GUIDE.md](UBF_STRUCT_GUIDE.md)** - Comprehensive UBF guide
  - Derive macro usage
  - Field constants vs numeric IDs
  - Optional fields with `Option<T>`
  - Nested structs
  - Examples and best practices

- **[UBF_FIELDS_GUIDE.md](UBF_FIELDS_GUIDE.md)** - Field table management
  - Creating `.fd` field definition files
  - Compiling with `mkfldhdr`
  - Field constants generation
  - Using field constants in code

## 🏗️ Components

### REST Gateway

- **REST API Integration** (in main README)
  - Actix-web based gateway
  - HTTP endpoints
  - JSON ↔ UBF conversion
  - Error handling

### Sample Servers

- **samplesvr_rust** - STRING/JSON services
  - ECHO, HELLO, STATUS, DATAPROC services
  - TRANSACTION service with validation

- **ubfsvr_rust** - UBF services
  - UBFECHO, UBFTEST, UBFADD, UBFGET services

### Oracle Database Integration

- **[oracle_txn_server/README.md](oracle_txn_server/README.md)** - Complete guide
  - Diesel ORM integration
  - Database schema and migrations
  - CREATE_TXN, GET_TXN, LIST_TXN services
  - Migration tool (`migrate.py`)
  - Performance benchmarks
  - XA transaction configuration

## 🔧 Advanced Topics

### Transaction Processing

- **[TRANSACTION_API.md](TRANSACTION_API.md)** - Complex transactions
  - JSON ↔ UBF conversion flow
  - Transaction validation
  - Error handling patterns
  - Architecture diagrams

### Database & ORM

- **[ORACLE_REST_INTEGRATION.md](ORACLE_REST_INTEGRATION.md)** (if exists)
  - REST gateway to Oracle integration
  - Request/response flow
  - Error handling

- **[DIESEL_MIGRATION_NOTE.md](DIESEL_MIGRATION_NOTE.md)** - Database migrations
  - Diesel migration system
  - Optimistic locking with `Recver` column
  - PL/SQL triggers
  - Migration workflow

### Performance

- **[BENCHMARK_RESULTS.md](BENCHMARK_RESULTS.md)** - Native driver benchmarks
  - Direct Oracle driver performance
  - GET/LIST/CREATE operations
  - Baseline measurements

- **[DIESEL_BENCHMARK_RESULTS.md](DIESEL_BENCHMARK_RESULTS.md)** - Diesel ORM benchmarks
  - Diesel vs native driver comparison
  - Throughput and latency analysis
  - Performance trade-offs
  - Optimization recommendations

## 🐳 Deployment

### Docker

- **[DOCKER_USAGE.md](DOCKER_USAGE.md)** - Docker deployment
  - docker-compose.yml configuration
  - Building images
  - Running containers
  - Service orchestration

- **[LINUX_WITHOUT_DOCKER.md](LINUX_WITHOUT_DOCKER.md)** - Native Linux setup
  - Manual installation
  - Environment configuration
  - Building from source

## 🔍 Troubleshooting

- **[LD_PRELOAD_ISSUE.md](LD_PRELOAD_ISSUE.md)** - Library loading issues
  - `undefined symbol` errors
  - Setting `LD_PRELOAD`
  - Environment configuration
  - Solutions and workarounds

- **[MULTIPLE_FD_FILES.md](MULTIPLE_FD_FILES.md)** - Multiple field tables
  - Managing multiple `.fd` files
  - Build script behavior
  - Field ID conflicts

## 📝 Project Management

- **[CHANGELOG.md](CHANGELOG.md)** - Version history
  - Complete changelog
  - All releases
  - Breaking changes
  - Migration notes

- **[SOLUTION_SUMMARY.md](SOLUTION_SUMMARY.md)** - Architecture overview
  - System design
  - Component interaction
  - Technology stack

## 🧪 Development

### Testing

- **test_rest.sh** - REST API tests
- **test_oracle_rest.sh** - Oracle transaction tests
- **benchmark_oracle_rest_v2.sh** - Performance benchmarks

### Build Configuration

- **Cargo.toml** - Workspace configuration
- **Dockerfile** - Container image definition
- **docker-compose.yml** - Multi-service orchestration
- **setenv.sh** - Environment setup script

## 🔗 External Resources

- **[endurox-sys on crates.io](https://crates.io/crates/endurox-sys)** - Published package
- **[API Documentation](https://docs.rs/endurox-sys)** - Online API docs
- **[Enduro/X Official Docs](https://www.endurox.org/dokuwiki/)** - Enduro/X documentation
- **[Diesel Documentation](https://diesel.rs/)** - Diesel ORM guide
- **[Actix-web Documentation](https://actix.rs/)** - Actix-web framework

## 📋 Quick Reference

### New to the Project?
1. Start with [README.md](README.md)
2. Follow [GETTING_STARTED.md](GETTING_STARTED.md)
3. Check [RELEASE_NOTES.md](RELEASE_NOTES.md) for latest changes

### Working with UBF?
1. Read [UBF_STRUCT_GUIDE.md](UBF_STRUCT_GUIDE.md)
2. Review [UBF_FIELDS_GUIDE.md](UBF_FIELDS_GUIDE.md)
3. See examples in [TRANSACTION_API.md](TRANSACTION_API.md)

### Database Integration?
1. Check [oracle_txn_server/README.md](oracle_txn_server/README.md)
2. Review [DIESEL_MIGRATION_NOTE.md](DIESEL_MIGRATION_NOTE.md)
3. See benchmarks in [DIESEL_BENCHMARK_RESULTS.md](DIESEL_BENCHMARK_RESULTS.md)

### Troubleshooting?
1. Check [LD_PRELOAD_ISSUE.md](LD_PRELOAD_ISSUE.md)
2. See troubleshooting section in [README.md](README.md#troubleshooting)
3. Review [oracle_txn_server/README.md](oracle_txn_server/README.md#troubleshooting)

---

**Last Updated**: November 8, 2025  
**Version**: 0.1.1
