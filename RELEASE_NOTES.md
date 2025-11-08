# Release Notes - v0.1.1

## 🎉 endurox-sys Published to crates.io!

We're excited to announce that **endurox-sys v0.1.1** is now available on [crates.io](https://crates.io/crates/endurox-sys)!

### What's New

#### Published to crates.io
- **endurox-sys** is now publicly available on crates.io
- Easy installation: `cargo add endurox-sys --features server,client,ubf,derive`
- API documentation available at [docs.rs/endurox-sys](https://docs.rs/endurox-sys)

#### Simplified Dependency Management
Previously, projects needed local path dependencies:
```toml
[dependencies]
endurox-sys = { path = "../endurox-sys", features = ["server", "ubf"] }
```

Now, simply use the published version:
```toml
[dependencies]
endurox-sys = { version = "0.1.1", features = ["server", "ubf", "derive"] }
```

#### UBF Field Table Discovery
- Added `NDRX_APPHOME` environment variable support
- Build script now searches `$NDRX_APPHOME/ubftab/*.fd.h` for field tables
- Falls back to `../ubftab/*.fd.h` for local development
- Gracefully handles missing field tables (generates empty constants)

### Available Features

- **`server`** - Server API support (`tpsvrinit`, `tpsvrdone`, service advertisement)
- **`client`** - Client API support (`tpinit`, `tpterm`, `tpcall`, `tpacall`, `tpgetrply`)
- **`ubf`** - UBF (Unified Buffer Format) support
- **`derive`** - Procedural macros for UBF struct serialization (`#[derive(UbfStruct)]`)

### Quick Start

```toml
[dependencies]
endurox-sys = { version = "0.1.1", features = ["server", "ubf", "derive"] }
```

```rust
use endurox_sys::*;

#[derive(UbfStruct)]
struct Request {
    #[ubf(field = 1000)]
    transaction_id: String,
    
    #[ubf(field = 1001)]
    amount: i64,
}

// Use in your Enduro/X server or client
```

### Documentation

- **Main Documentation**: [docs.rs/endurox-sys](https://docs.rs/endurox-sys)
- **Crates.io**: [crates.io/crates/endurox-sys](https://crates.io/crates/endurox-sys)
- **Examples**: See the [examples in the repository](https://github.com/yourusername/endurox-dev)

### Migration Guide

If you're upgrading from a local path dependency:

1. **Update your `Cargo.toml`**:
   ```diff
   [dependencies]
   - endurox-sys = { path = "../endurox-sys", features = ["server", "ubf"] }
   + endurox-sys = { version = "0.1.1", features = ["server", "ubf", "derive"] }
   ```

2. **Rebuild your project**:
   ```bash
   cargo clean
   cargo build --release
   ```

3. **Set `NDRX_APPHOME`** (if using UBF field tables):
   ```bash
   export NDRX_APPHOME=/path/to/your/app
   cargo build --release
   ```

### Breaking Changes

None. This release is fully backward compatible.

### Bug Fixes

- Fixed UBF field table discovery in deployment environments
- Build script now properly uses `NDRX_APPHOME` environment variable

### Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for complete list of changes.

---

**Published**: November 8, 2025  
**Version**: 0.1.1  
**License**: MIT
