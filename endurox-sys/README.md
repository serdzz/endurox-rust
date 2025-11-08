# endurox-sys

Low-level Rust FFI bindings for [Enduro/X](https://www.endurox.org/) middleware.

Enduro/X is a high-performance, open-source middleware platform that implements the XATMI API. This crate provides safe and unsafe Rust bindings to the Enduro/X C API, enabling you to build distributed transaction processing applications in Rust.

## Features

- **XATMI API** - Complete bindings for `tpcall()`, `tpacall()`, `tpreturn()`, etc.
- **UBF Buffers** - Typed buffer manipulation with `Bchg()`, `Bget()`, etc.
- **Server Development** - Build Enduro/X servers with `atmisrvnomain()`
- **Client Development** - Create clients with `tpinit()`, `tpterm()`
- **Derive Macros** - Optional `#[derive(UbfStructDerive)]` for automatic serialization
- **Type Safety** - Safe wrappers around raw pointers and error handling

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
endurox-sys = { version = "0.1", features = ["ubf", "server"] }
```

### Prerequisites

- Enduro/X middleware installed and configured
- Required environment variables (see below)
- Oracle Instant Client (if using Oracle features)

### Environment Variables

#### `NDRX_HOME` (Required)
Points to the Enduro/X installation directory. Used by the build script to locate Enduro/X libraries and headers.

```bash
export NDRX_HOME=/opt/endurox
```

The build script uses this to:
- Link against Enduro/X libraries (`libatmi`, `libubf`, `libnstd`, etc.)
- Find header files during compilation

#### `NDRX_APPHOME` (Optional)
Points to your application's home directory. Used by the build script to locate UBF field table definitions (`ubftab/` directory) for generating Rust constants.

```bash
export NDRX_APPHOME=/path/to/your/app
```

When set, the build script looks for `*.fd.h` files in `$NDRX_APPHOME/ubftab/` and generates Rust constants for UBF field IDs. This allows you to use field constants like `T_NAME_FLD` directly in your code.

**Build-time behavior:**
- If `NDRX_APPHOME` is set: looks for `$NDRX_APPHOME/ubftab/*.fd.h`
- If not set: looks for `../ubftab/*.fd.h` (relative to crate directory, for local development)
- If no UBF tables found: generates empty constants file (build still succeeds)

**Example:**
```rust
use endurox_sys::*;

unsafe {
    // Use generated field constants
    Bchg(buffer, T_NAME_FLD, 0, name.as_ptr() as *mut i8, 0);
    Bchg(buffer, T_AMOUNT_FLD, 0, &amount as *const _ as *mut i8, 0);
}
```

## Usage

### Basic Client Example

```rust
use endurox_sys::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        // Initialize connection
        tpinit(std::ptr::null_mut())?;
        
        // Allocate UBF buffer
        let buf = tpalloc(b"UBF\0".as_ptr() as *mut i8, std::ptr::null_mut(), 1024);
        
        // Call service
        let mut len = 1024;
        tpcall(
            b"MYSERVICE\0".as_ptr() as *mut i8,
            buf,
            0,
            &mut buf,
            &mut len,
            0
        )?;
        
        // Free buffer and disconnect
        tpfree(buf);
        tpterm();
    }
    
    Ok(())
}
```

### Server Example with Derive

```rust
use endurox_sys::*;

#[derive(UbfStructDerive)]
struct Request {
    #[ubf_field(1000)]
    transaction_id: String,
    
    #[ubf_field(1001)]
    amount: i64,
}

#[derive(UbfStructDerive)]
struct Response {
    #[ubf_field(2000)]
    status: String,
    
    #[ubf_field(2001)]
    message: String,
}

#[no_mangle]
pub extern "C" fn MYSERVICE(p_svc: *mut TPSVCINFO) {
    unsafe {
        let svc = &*p_svc;
        
        // Deserialize request
        let req = match Request::from_ubf(svc.data as *mut UBFH) {
            Ok(r) => r,
            Err(_) => {
                tpreturn(TPFAIL, 0, std::ptr::null_mut(), 0, 0);
                return;
            }
        };
        
        // Process request
        let resp = Response {
            status: "SUCCESS".to_string(),
            message: format!("Processed {}", req.transaction_id),
        };
        
        // Serialize response
        if resp.to_ubf(svc.data as *mut UBFH).is_ok() {
            tpreturn(TPSUCCESS, 0, svc.data, 0, 0);
        } else {
            tpreturn(TPFAIL, 0, std::ptr::null_mut(), 0, 0);
        }
    }
}
```

## Features

### `ubf`
Enables UBF buffer API bindings:
- `Bchg()`, `Bget()`, `Badd()`, `Bdel()`
- Field ID and type management
- Buffer allocation and manipulation

### `server`
Enables server-side bindings:
- `atmisrvnomain()` - Main server entry point
- `tpsvrinit()`, `tpsvrdone()` - Server lifecycle hooks
- `tpadvertise()`, `tpunadvertise()` - Service advertisement

### `client`
Enables client-side bindings:
- `tpinit()`, `tpterm()` - Connection management
- `tpcall()`, `tpacall()`, `tpgetrply()` - Service calls

### `derive`
Enables `#[derive(UbfStructDerive)]` macro for automatic UBF serialization.
Requires the [`endurox-derive`](https://crates.io/crates/endurox-derive) crate.

## Safety

Most functions in this crate are marked `unsafe` as they interact with C FFI and raw pointers. Safe wrappers can be found in higher-level crates built on top of `endurox-sys`.

Always ensure:
- Buffers are properly allocated before use
- `tpinit()` is called before any XATMI operations
- Resources are freed with `tpfree()` and `tpterm()`
- Error codes are checked after each operation

## Error Handling

Use `tperrno()` and `tpstrerror()` to get error information:

```rust
use endurox_sys::*;

unsafe {
    if tpcall(...).is_err() {
        let errno = tperrno();
        let error = std::ffi::CStr::from_ptr(tpstrerror(errno))
            .to_string_lossy();
        eprintln!("Error: {}", error);
    }
}
```

## Documentation

For complete API documentation, see [docs.rs/endurox-sys](https://docs.rs/endurox-sys).

For Enduro/X documentation, visit [www.endurox.org/dokuwiki](https://www.endurox.org/dokuwiki).

## License

Licensed under the MIT license. See [LICENSE](../LICENSE) for details.

## Related Crates

- [`endurox-derive`](https://crates.io/crates/endurox-derive) - Derive macros for UBF serialization
- [`endurox`](https://crates.io/crates/endurox) - High-level safe API (coming soon)
