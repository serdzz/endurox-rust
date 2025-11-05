# UBF Struct Guide

Complete guide for using typed Rust structs with Enduro/X UBF buffers.

## Overview

The `ubf_struct` module provides two main patterns for working with UBF:

1. **Marshal/Unmarshal Pattern** - Serialize entire struct to JSON in `T_DATA_FLD`
2. **Field Mapping Pattern** - Map struct fields directly to UBF field IDs

## Marshal/Unmarshal Pattern

This pattern serializes the entire Rust struct to JSON and stores it in the `T_DATA_FLD` UBF field.

### Basic Example

```rust
use endurox_sys::ubf_struct::{marshal, unmarshal, UbfError};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct RequestData {
    operation: String,
    user_id: i64,
    amount: f64,
    metadata: Option<String>,
}

fn example() -> Result<(), UbfError> {
    // Create data
    let request = RequestData {
        operation: "transfer".to_string(),
        user_id: 12345,
        amount: 100.50,
        metadata: Some("urgent".to_string()),
    };
    
    // Marshal to UBF buffer
    let ubf_buffer = marshal(&request)?;
    
    // Unmarshal back to struct
    let restored: RequestData = unmarshal(&ubf_buffer)?;
    
    assert_eq!(request, restored);
    Ok(())
}
```

### Advantages

- ✅ Simple API - just add `#[derive(Serialize, Deserialize)]`
- ✅ Supports nested structures
- ✅ Supports Option, Vec, and other complex types
- ✅ Easy to version and extend

### When to Use

- Complex data structures
- Rapid prototyping
- When you don't need to access individual fields in UBF

## Field Mapping Pattern

This pattern maps individual struct fields to specific UBF field IDs.

### Manual Implementation

```rust
use endurox_sys::ubf::{UbfBuffer};
use endurox_sys::ubf_struct::{UbfStruct, UbfError};

// Field IDs from test.fd
const T_NAME_FLD: i32 = 1002;
const T_ID_FLD: i32 = 1012;
const T_PRICE_FLD: i32 = 1021;
const T_STATUS_FLD: i32 = 1004;

struct Transaction {
    name: String,
    id: i64,
    amount: f64,
    status: String,
}

impl UbfStruct for Transaction {
    fn from_ubf(buf: &UbfBuffer) -> Result<Self, UbfError> {
        let name = buf.get_string(T_NAME_FLD, 0)
            .map_err(|e| UbfError::FieldNotFound(format!("T_NAME_FLD: {}", e)))?;
        
        let id = buf.get_long(T_ID_FLD, 0)
            .map_err(|e| UbfError::FieldNotFound(format!("T_ID_FLD: {}", e)))?;
        
        let amount = buf.get_double(T_PRICE_FLD, 0)
            .map_err(|e| UbfError::FieldNotFound(format!("T_PRICE_FLD: {}", e)))?;
        
        let status = buf.get_string(T_STATUS_FLD, 0)
            .unwrap_or_else(|_| "pending".to_string());
        
        Ok(Transaction { name, id, amount, status })
    }
    
    fn to_ubf(&self) -> Result<UbfBuffer, UbfError> {
        let mut buf = UbfBuffer::new(2048)
            .map_err(|e| UbfError::AllocationError(e))?;
        self.update_ubf(&mut buf)?;
        Ok(buf)
    }
    
    fn update_ubf(&self, buf: &mut UbfBuffer) -> Result<(), UbfError> {
        buf.add_string(T_NAME_FLD, &self.name)
            .map_err(|e| UbfError::TypeError(format!("name: {}", e)))?;
        buf.add_long(T_ID_FLD, self.id)
            .map_err(|e| UbfError::TypeError(format!("id: {}", e)))?;
        buf.add_double(T_PRICE_FLD, self.amount)
            .map_err(|e| UbfError::TypeError(format!("amount: {}", e)))?;
        buf.add_string(T_STATUS_FLD, &self.status)
            .map_err(|e| UbfError::TypeError(format!("status: {}", e)))?;
        Ok(())
    }
}
```

### Derive Macro

The `UbfStruct` derive macro automatically implements the trait for you:

```rust
use endurox_sys::UbfStruct;
use endurox_sys::ubf_fields::*;  // Auto-generated field constants

#[derive(Debug, Clone, UbfStruct)]
struct Transaction {
    #[ubf(field = T_NAME_FLD)]  // Using field constant
    name: String,
    
    #[ubf(field = T_ID_FLD)]  // Field constant
    id: i64,
    
    #[ubf(field = T_PRICE_FLD)]  // Field constant
    amount: f64,
    
    #[ubf(field = T_STATUS_FLD, default = "pending")]  // With default value
    status: String,
}

// Alternative: using numeric field IDs
#[derive(Debug, Clone, UbfStruct)]
struct Payment {
    #[ubf(field = 1002)]  // Numeric field ID
    name: String,
    
    #[ubf(field = 1012)]  // Numeric field ID  
    id: i64,
}

// Usage
let txn = Transaction {
    name: "Payment".to_string(),
    id: 12345,
    amount: 999.99,
    status: "completed".to_string(),
};

// Convert to UBF
let ubf = txn.to_ubf()?;

// Convert back from UBF
let restored = Transaction::from_ubf(&ubf)?;

// Update existing buffer
let mut existing_ubf = UbfBuffer::new(1024)?;
txn.update_ubf(&mut existing_ubf)?;
```

**Supported Types:**
- `String` - mapped to UBF string fields
- `i64`, `i32` - mapped to UBF long fields
- `f64`, `f32` - mapped to UBF double fields
- `bool` - mapped to UBF long fields (0/1), checked with `is_present()`
- **Nested structs** - any type implementing `UbfStruct`

**Attributes:**
- `#[ubf(field = CONSTANT)]` - Use auto-generated field constant (recommended)
- `#[ubf(field = 1234)]` - Use numeric field ID
- `#[ubf(field = T_NAME_FLD, default = "value")]` - Provide default value for optional fields (String only)
- `#[ubf(field = 0)]` - For nested structs (field ID is not used, the nested struct's fields are used)

### Nested Structs

You can nest structs that implement `UbfStruct` within other structs:

```rust
use endurox_sys::UbfStruct;
use endurox_sys::ubf_fields::*;

// Define nested struct
#[derive(Debug, Clone, UbfStruct)]
struct Address {
    #[ubf(field = T_STREET_FLD)]
    street: String,
    
    #[ubf(field = T_CITY_FLD)]
    city: String,
    
    #[ubf(field = T_ZIP_FLD)]
    zip: String,
}

// Use nested struct in parent
#[derive(Debug, Clone, UbfStruct)]
struct Customer {
    #[ubf(field = T_NAME_FLD)]
    name: String,
    
    #[ubf(field = T_ID_FLD)]
    customer_id: i64,
    
    #[ubf(field = 0)]  // Nested struct - field ID not used
    address: Address,
}

// Usage
let customer = Customer {
    name: "John Doe".to_string(),
    customer_id: 1001,
    address: Address {
        street: "123 Main St".to_string(),
        city: "Springfield".to_string(),
        zip: "12345".to_string(),
    },
};

// All fields (including nested) are serialized to the same UBF buffer
let ubf = customer.to_ubf()?;
let restored = Customer::from_ubf(&ubf)?;

assert_eq!(customer.address.city, restored.address.city);
```

**How it works:**
- Nested structs are flattened into the same UBF buffer
- All fields from both parent and nested structs are stored at the top level
- Field IDs must be unique across all nested structures
- The field ID for the nested struct field itself (specified with `#[ubf(field = 0)]`) is ignored

**Running the example:**
```bash
docker-compose exec endurox_rust bash /app/test_derive.sh
```

### Advantages

- ✅ Direct UBF field access
- ✅ Efficient - no JSON serialization overhead
- ✅ Interoperable with non-Rust services
- ✅ Can access fields individually without deserializing entire struct

### When to Use

- Performance-critical code
- Interoperability with existing UBF services
- When you need fine-grained control over field mapping

## Builder Pattern

For dynamic UBF buffer construction:

```rust
use endurox_sys::ubf_struct::{UbfStructBuilder, UbfError};

fn build_example() -> Result<(), UbfError> {
    let ubf = UbfStructBuilder::new(2048)?
        .with_string(1002, "John Doe")?
        .with_long(1012, 12345)?
        .with_double(1021, 99.99)?
        .build();
    
    Ok(())
}
```

## Using in Services

### Server Side

```rust
extern "C" fn service_handler(rqst: *mut TpSvcInfoRaw) {
    unsafe {
        let req = &*rqst;
        let ubf = UbfBuffer::from_raw(req.data);
        
        // Unmarshal request
        let request: RequestData = match unmarshal(&ubf) {
            Ok(r) => r,
            Err(e) => {
                tplog_error(&format!("Failed to unmarshal: {}", e));
                tpreturn_fail(rqst);
                return;
            }
        };
        
        // Process request
        let response = process_request(&request);
        
        // Marshal response
        let response_ubf = match marshal(&response) {
            Ok(buf) => buf,
            Err(e) => {
                tplog_error(&format!("Failed to marshal: {}", e));
                tpreturn_fail(rqst);
                return;
            }
        };
        
        // Return response
        let ptr = response_ubf.into_raw();
        tpreturn_success(rqst, ptr);
    }
}
```

### Client Side

```rust
use endurox_sys::client::EnduroxClient;
use endurox_sys::ubf_struct::{marshal, unmarshal};

fn call_service() -> Result<ResponseData, Box<dyn std::error::Error>> {
    let client = EnduroxClient::new()?;
    
    // Prepare request
    let request = RequestData {
        operation: "query".to_string(),
        user_id: 123,
        amount: 0.0,
        metadata: None,
    };
    
    // Marshal to UBF
    let ubf_request = marshal(&request)?;
    
    // Call service
    let response_ptr = client.call_service_raw("MYSERVICE", ubf_request.into_raw())?;
    
    // Unmarshal response
    let response_ubf = unsafe { UbfBuffer::from_raw(response_ptr) };
    let response: ResponseData = unmarshal(&response_ubf)?;
    
    Ok(response)
}
```

## Error Handling

```rust
use endurox_sys::ubf_struct::{UbfError, unmarshal};

fn handle_errors(ubf: &UbfBuffer) {
    match unmarshal::<MyStruct>(ubf) {
        Ok(data) => println!("Success: {:?}", data),
        Err(UbfError::FieldNotFound(field)) => {
            eprintln!("Missing field: {}", field);
        }
        Err(UbfError::TypeError(msg)) => {
            eprintln!("Type conversion error: {}", msg);
        }
        Err(UbfError::AllocationError(msg)) => {
            eprintln!("Allocation failed: {}", msg);
        }
        Err(UbfError::InvalidValue(msg)) => {
            eprintln!("Invalid value: {}", msg);
        }
    }
}
```

## Testing

Run tests with Enduro/X environment:

```bash
# In Docker
docker-compose exec endurox_rust bash -c '. ./setenv.sh && cargo test ubf_struct'

# Local (requires Enduro/X)
source setenv.sh
cargo test --package endurox-sys ubf_struct
```

## Examples

See:
- `endurox-sys/src/ubf_struct.rs` - Full implementation with tests
- `ubf_test_client/examples/derive_macro_example.rs` - Derive macro examples
- `ubfsvr_rust/examples/ubf_struct_example.rs` - Standalone examples
- `ubf_test_client/src/main.rs` - Client usage

## Field Table Reference

From `ubftab/test.fd`:
|| Field Name | Field ID | Type | Description |
|-----------|----------|------|-------------|
| T_STRING_FLD | 1001 | string | String field |
| T_NAME_FLD | 1002 | string | Name field |
| T_MESSAGE_FLD | 1003 | string | Message field |
| T_STATUS_FLD | 1004 | string | Status field |
| T_DATA_FLD | 1005 | string | Data field (used for JSON) |
| T_STREET_FLD | 1006 | string | Street field |
| T_CITY_FLD | 1007 | string | City field |
| T_ZIP_FLD | 1008 | string | ZIP code field |
| T_LONG_FLD | 1010 | long | Long integer field |
| T_COUNT_FLD | 1011 | long | Count field |
| T_ID_FLD | 1012 | long | ID field |
| T_CODE_FLD | 1013 | long | Code field |
| T_DOUBLE_FLD | 1020 | double | Double field |
| T_PRICE_FLD | 1021 | double | Price field |
| T_SHORT_FLD | 1030 | short | Short field |
| T_FLAG_FLD | 1031 | short | Flag field |

## Best Practices

1. **Use marshal/unmarshal for complex types**
   - Nested structures
   - Collections (Vec, HashMap)
   - Optional fields

2. **Use field mapping for simple, flat structures**
   - High-performance requirements
   - Interoperability with C services
   - Direct field access needed

3. **Always handle errors**
   - Check `Result` types
   - Log errors appropriately
   - Provide meaningful error messages

4. **Consider buffer sizes**
   - Marshal: `json.len() + 1024`
   - Field mapping: Estimate based on fields
   - Use `used()` to check actual usage

5. **Test with real UBF environment**
   - Unit tests work locally
   - Integration tests need Enduro/X
   - Use Docker for consistent testing
