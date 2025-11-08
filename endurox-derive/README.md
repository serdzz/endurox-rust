# endurox-derive

Procedural macros for [Enduro/X](https://www.endurox.org/) middleware integration in Rust.

This crate provides derive macros for automatic serialization and deserialization of Rust structs to/from UBF (Unified Buffer Format) buffers used by Enduro/X.

## Features

- `#[derive(UbfStructDerive)]` - Automatic UBF serialization/deserialization
- Field attribute `#[ubf_field(id)]` - Map struct fields to UBF field IDs
- Support for nested structs and arrays
- Type-safe conversions between Rust types and UBF types

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
endurox-sys = { version = "0.1", features = ["derive"] }
```

### Basic Example

```rust
use endurox_sys::{UbfStruct, UbfStructDerive};

#[derive(UbfStructDerive)]
struct Transaction {
    #[ubf_field(100)]
    transaction_id: String,
    
    #[ubf_field(101)]
    amount: i64,
    
    #[ubf_field(102)]
    status: String,
}

// Automatic conversion to/from UBF buffers
fn process_transaction(ubf_buf: *mut UBFH) -> Result<(), UbfError> {
    let txn = Transaction::from_ubf(ubf_buf)?;
    
    // Process transaction...
    
    txn.to_ubf(ubf_buf)?;
    Ok(())
}
```

### Field Mapping

The `#[ubf_field(id)]` attribute maps struct fields to UBF field IDs:

```rust
#[derive(UbfStructDerive)]
struct Request {
    #[ubf_field(1000)]  // Maps to UBF field ID 1000
    request_id: String,
    
    #[ubf_field(1001)]
    data: Vec<u8>,
}
```

### Supported Types

- `String` - Maps to `BFLD_STRING`
- `i16`, `i32`, `i64` - Maps to `BFLD_SHORT`, `BFLD_LONG`, `BFLD_LONG`
- `f32`, `f64` - Maps to `BFLD_FLOAT`, `BFLD_DOUBLE`
- `Vec<u8>` - Maps to `BFLD_CARRAY`
- Arrays and nested structs (with limitations)

## Requirements

- Enduro/X installed and configured
- `endurox-sys` crate with UBF feature enabled

## Documentation

For complete API documentation, see [docs.rs/endurox-derive](https://docs.rs/endurox-derive).

## License

Licensed under the MIT license. See [LICENSE](../LICENSE) for details.

## Related Crates

- [`endurox-sys`](https://crates.io/crates/endurox-sys) - Low-level FFI bindings to Enduro/X
