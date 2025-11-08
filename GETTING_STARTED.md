# Getting Started with endurox-sys

Welcome! This guide will help you get started with using **endurox-sys** in your Rust projects.

## What is endurox-sys?

**endurox-sys** is a Rust FFI binding library for [Enduro/X](https://www.endurox.org/) middleware, providing safe wrappers around the XATMI API and UBF (Unified Buffer Format) operations.

## Installation

### Prerequisites

1. **Enduro/X** 8.0+ installed
   ```bash
   export NDRX_HOME=/opt/endurox
   export PATH=$NDRX_HOME/bin:$PATH
   ```

2. **Rust** 1.70+ (via rustup)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### Add to Your Project

Add **endurox-sys** to your `Cargo.toml`:

```toml
[dependencies]
endurox-sys = { version = "0.1.1", features = ["server", "client", "ubf", "derive"] }
```

### Feature Flags

Choose the features you need:

| Feature | Description | Use Case |
|---------|-------------|----------|
| `server` | Server API (`tpsvrinit`, `tpsvrdone`, service advertisement) | Building Enduro/X servers |
| `client` | Client API (`tpinit`, `tpterm`, `tpcall`) | Building Enduro/X clients |
| `ubf` | UBF buffer support | Working with UBF buffers |
| `derive` | `#[derive(UbfStruct)]` macro | Automatic UBF serialization |

**Example combinations:**

```toml
# Server with UBF support
endurox-sys = { version = "0.1.1", features = ["server", "ubf", "derive"] }

# Client only
endurox-sys = { version = "0.1.1", features = ["client", "ubf"] }

# Full features
endurox-sys = { version = "0.1.1", features = ["server", "client", "ubf", "derive"] }
```

## Quick Examples

### 1. Simple Enduro/X Client

```rust
use endurox_sys::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        // Initialize XATMI
        tpinit(std::ptr::null_mut())?;
        
        // Allocate STRING buffer
        let input = "Hello, Enduro/X!";
        let buf = tpalloc(
            b"STRING\0".as_ptr() as *mut i8,
            std::ptr::null_mut(),
            input.len() + 1
        );
        
        // Copy input data
        std::ptr::copy_nonoverlapping(
            input.as_ptr(),
            buf as *mut u8,
            input.len()
        );
        
        // Call service
        let mut len = 1024;
        tpcall(
            b"ECHO\0".as_ptr() as *mut i8,
            buf,
            0,
            &mut buf,
            &mut len,
            0
        )?;
        
        // Process response
        let response = std::ffi::CStr::from_ptr(buf as *const i8)
            .to_string_lossy();
        println!("Response: {}", response);
        
        // Cleanup
        tpfree(buf);
        tpterm();
    }
    
    Ok(())
}
```

### 2. Enduro/X Server with UBF

```rust
use endurox_sys::*;

#[derive(UbfStruct)]
struct Request {
    #[ubf(field = 1000)]
    name: String,
    
    #[ubf(field = 1001)]
    age: i64,
}

#[derive(UbfStruct)]
struct Response {
    #[ubf(field = 2000)]
    message: String,
    
    #[ubf(field = 2001)]
    status: String,
}

#[no_mangle]
pub extern "C" fn GREETING(p_svc: *mut TPSVCINFO) {
    unsafe {
        let svc = &*p_svc;
        
        // Deserialize request from UBF
        let req = match Request::from_ubf(svc.data as *mut UBFH) {
            Ok(r) => r,
            Err(e) => {
                tplog_error(&format!("Failed to parse request: {}", e));
                tpreturn(TPFAIL, 0, std::ptr::null_mut(), 0, 0);
                return;
            }
        };
        
        // Process request
        let resp = Response {
            message: format!("Hello, {}! You are {} years old.", req.name, req.age),
            status: "SUCCESS".to_string(),
        };
        
        // Serialize response to UBF
        if resp.to_ubf(svc.data as *mut UBFH).is_ok() {
            tpreturn(TPSUCCESS, 0, svc.data, 0, 0);
        } else {
            tpreturn(TPFAIL, 0, std::ptr::null_mut(), 0, 0);
        }
    }
}

fn main() {
    unsafe {
        // Register service and start server
        let mut services = vec![
            ("GREETING\0".as_ptr() as *mut i8, GREETING as *mut std::ffi::c_void),
        ];
        
        atmisrvnomain(
            services.len() as i32,
            services.as_mut_ptr() as *mut *mut i8,
            Some(tpsvrinit),
            Some(tpsvrdone)
        );
    }
}
```

### 3. Using UBF Derive Macro

```rust
use endurox_sys::*;

// Define your data structure
#[derive(Debug, UbfStruct)]
struct Transaction {
    #[ubf(field = 1050)]  // T_TRANS_TYPE_FLD
    transaction_type: String,
    
    #[ubf(field = 1051)]  // T_TRANS_ID_FLD
    transaction_id: String,
    
    #[ubf(field = 1052)]  // T_ACCOUNT_FLD
    account: String,
    
    #[ubf(field = 1010)]  // T_AMOUNT_FLD
    amount: i64,
    
    #[ubf(field = 1053)]  // T_CURRENCY_FLD
    currency: String,
    
    #[ubf(field = 1054)]  // Optional description
    description: Option<String>,
}

fn process_transaction(ubf_buf: *mut UBFH) -> Result<(), UbfError> {
    // Deserialize from UBF
    let txn = Transaction::from_ubf(ubf_buf)?;
    
    println!("Processing transaction: {:?}", txn);
    
    // Modify and serialize back
    let response = Transaction {
        transaction_type: "sale".to_string(),
        transaction_id: txn.transaction_id,
        account: txn.account,
        amount: txn.amount,
        currency: txn.currency,
        description: Some("Processed successfully".to_string()),
    };
    
    response.to_ubf(ubf_buf)?;
    Ok(())
}
```

## UBF Field Tables

To use UBF field constants (like `T_NAME_FLD`), you need to set up field tables:

### 1. Create Field Definition File

Create `ubftab/test.fd`:

```
$/* Field definitions for your application */

*base 1000

T_NAME_FLD     1000  string  -  Name field
T_ID_FLD       1001  long    -  ID field
T_AMOUNT_FLD   1002  long    -  Amount field (in cents)
T_STATUS_FLD   1003  string  -  Status field
T_MESSAGE_FLD  1004  string  -  Message field
```

### 2. Compile Field Table

```bash
mkfldhdr -d ubftab ubftab/test.fd
```

This generates `ubftab/test.fd.h`.

### 3. Build Your Project

Set `NDRX_APPHOME` so endurox-sys can find the field tables:

```bash
export NDRX_APPHOME=/path/to/your/app
cargo build --release
```

The build script will automatically:
- Look for `$NDRX_APPHOME/ubftab/*.fd.h`
- Generate Rust constants for your fields
- Include them in `endurox_sys::ubf_fields::*`

### 4. Use Field Constants

```rust
use endurox_sys::*;
use endurox_sys::ubf_fields::*;

#[derive(UbfStruct)]
struct MyStruct {
    #[ubf(field = T_NAME_FLD)]  // Use generated constant
    name: String,
    
    #[ubf(field = T_ID_FLD)]
    id: i64,
}
```

## Running Your Server

### 1. Configure Enduro/X

Add your server to `ndrxconfig.xml`:

```xml
<server name="myserver">
    <srvid>10</srvid>
    <min>1</min>
    <max>5</max>
    <cctag>default</cctag>
</server>
```

### 2. Start Enduro/X

```bash
# Source environment
. setenv.sh

# Start application
xadmin start -y

# Check status
xadmin psc
```

### 3. Test Your Service

```bash
# Using ud command
echo "Hello, World!" | ud ECHO

# Using UBF
ud GREETING <<EOF
T_NAME_FLD  John
T_AGE_FLD   30
EOF
```

## Docker Deployment

The example project includes Docker support:

```bash
# Build and start
docker-compose up -d

# View logs
docker-compose logs -f

# Test services
curl http://localhost:8080/api/status
```

## Next Steps

- Read the [full documentation](https://docs.rs/endurox-sys)
- Check out [example projects](../README.md#examples)
- Learn about [UBF Struct Guide](UBF_STRUCT_GUIDE.md)
- Explore [Transaction API examples](TRANSACTION_API.md)

## Resources

- **Documentation**: [docs.rs/endurox-sys](https://docs.rs/endurox-sys)
- **Crates.io**: [crates.io/crates/endurox-sys](https://crates.io/crates/endurox-sys)
- **Enduro/X**: [www.endurox.org](https://www.endurox.org/)
- **Examples**: See the main [README.md](README.md)

## Getting Help

- Check the [Troubleshooting section](README.md#troubleshooting)
- Review [Enduro/X documentation](https://www.endurox.org/dokuwiki/)
- File issues on the project repository

---

Happy coding with Enduro/X and Rust! 🚀
