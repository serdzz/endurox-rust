//! UBF Struct support - convert between Rust structs and UBF buffers
//!
//! This module provides trait-based conversion between Rust structs and UBF buffers
//! with JSON-like marshal/unmarshal API

use crate::ubf::UbfBuffer;
use crate::ubf_fields::*;  // Auto-generated field constants
use std::fmt;
use serde::{Serialize, Deserialize};

/// Trait for types that can be converted to/from UBF buffers
pub trait UbfStruct: Sized {
    /// Convert from UBF buffer to struct
    fn from_ubf(buf: &UbfBuffer) -> Result<Self, UbfError>;
    
    /// Convert from struct to UBF buffer
    fn to_ubf(&self) -> Result<UbfBuffer, UbfError>;
    
    /// Update existing UBF buffer with struct data
    fn update_ubf(&self, buf: &mut UbfBuffer) -> Result<(), UbfError>;
}

/// UBF conversion errors
#[derive(Debug, Clone)]
pub enum UbfError {
    /// Field not found in buffer
    FieldNotFound(String),
    /// Type conversion error
    TypeError(String),
    /// Buffer allocation error
    AllocationError(String),
    /// Invalid field value
    InvalidValue(String),
}

impl fmt::Display for UbfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UbfError::FieldNotFound(field) => write!(f, "Field not found: {}", field),
            UbfError::TypeError(msg) => write!(f, "Type error: {}", msg),
            UbfError::AllocationError(msg) => write!(f, "Allocation error: {}", msg),
            UbfError::InvalidValue(msg) => write!(f, "Invalid value: {}", msg),
        }
    }
}

impl std::error::Error for UbfError {}

/// Marshal Rust value to UBF buffer
/// 
/// Converts a Rust type to UBF buffer. For structs with #[ubf] attributes,
/// uses the field mappings. For plain types, serializes to JSON in T_DATA_FLD.
pub fn marshal<T: Serialize>(value: &T) -> Result<UbfBuffer, UbfError> {
    // Serialize to JSON
    let json = serde_json::to_string(value)
        .map_err(|e| UbfError::TypeError(format!("JSON serialization failed: {}", e)))?;
    
    // Create UBF buffer and store JSON in T_DATA_FLD
    let mut buf = UbfBuffer::new(json.len() + 1024)
        .map_err(UbfError::AllocationError)?;
    
    buf.add_string(T_DATA_FLD, &json)
        .map_err(|e| UbfError::TypeError(format!("Failed to add JSON: {}", e)))?;
    
    Ok(buf)
}

/// Unmarshal UBF buffer to Rust value
/// 
/// Converts UBF buffer to Rust type. For structs with #[ubf] attributes,
/// uses the field mappings. For plain types, deserializes from JSON in T_DATA_FLD.
pub fn unmarshal<T: for<'de> Deserialize<'de>>(buf: &UbfBuffer) -> Result<T, UbfError> {
    // Get JSON from T_DATA_FLD
    let json = buf.get_string(T_DATA_FLD, 0)
        .map_err(|e| UbfError::FieldNotFound(format!("T_DATA_FLD: {}", e)))?;
    
    // Deserialize from JSON
    serde_json::from_str(&json)
        .map_err(|e| UbfError::TypeError(format!("JSON deserialization failed: {}", e)))
}

/// Example struct with UBF mapping
/// 
/// ```
/// use endurox_sys::ubf_struct::{UbfStruct, UserData};
/// 
/// let user = UserData {
///     name: "John Doe".to_string(),
///     id: 12345,
///     balance: 100.50,
///     active: true,
/// };
/// 
/// // Convert to UBF
/// let ubf = user.to_ubf()?;
/// 
/// // Convert from UBF
/// let user2 = UserData::from_ubf(&ubf)?;
/// ```
#[derive(Debug, Clone)]
pub struct UserData {
    pub name: String,
    pub id: i64,
    pub balance: f64,
    pub active: bool,
}


impl UbfStruct for UserData {
    fn from_ubf(buf: &UbfBuffer) -> Result<Self, UbfError> {
        let name = buf.get_string(T_NAME_FLD, 0)
            .map_err(|e| UbfError::FieldNotFound(format!("T_NAME_FLD: {}", e)))?;
        
        let id = buf.get_long(T_ID_FLD, 0)
            .map_err(|e| UbfError::FieldNotFound(format!("T_ID_FLD: {}", e)))?;
        
        let balance = buf.get_double(T_PRICE_FLD, 0)
            .map_err(|e| UbfError::FieldNotFound(format!("T_PRICE_FLD: {}", e)))?;
        
        let active = if buf.is_present(T_FLAG_FLD, 0) {
            // For simplicity, treat any presence as true
            true
        } else {
            false
        };
        
        Ok(UserData {
            name,
            id,
            balance,
            active,
        })
    }
    
    fn to_ubf(&self) -> Result<UbfBuffer, UbfError> {
        let mut buf = UbfBuffer::new(1024)
            .map_err(UbfError::AllocationError)?;
        
        self.update_ubf(&mut buf)?;
        Ok(buf)
    }
    
    fn update_ubf(&self, buf: &mut UbfBuffer) -> Result<(), UbfError> {
        buf.add_string(T_NAME_FLD, &self.name)
            .map_err(|e| UbfError::TypeError(format!("name: {}", e)))?;
        
        buf.add_long(T_ID_FLD, self.id)
            .map_err(|e| UbfError::TypeError(format!("id: {}", e)))?;
        
        buf.add_double(T_PRICE_FLD, self.balance)
            .map_err(|e| UbfError::TypeError(format!("balance: {}", e)))?;
        
        if self.active {
            buf.add_long(T_FLAG_FLD, 1)
                .map_err(|e| UbfError::TypeError(format!("active: {}", e)))?;
        }
        
        Ok(())
    }
}

/// Generic UBF struct builder
pub struct UbfStructBuilder {
    buffer: UbfBuffer,
}

impl UbfStructBuilder {
    /// Create new builder with specified size
    pub fn new(size: usize) -> Result<Self, UbfError> {
        let buffer = UbfBuffer::new(size)
            .map_err(UbfError::AllocationError)?;
        Ok(UbfStructBuilder { buffer })
    }
    
    /// Add string field
    pub fn with_string(mut self, field_id: i32, value: &str) -> Result<Self, UbfError> {
        self.buffer.add_string(field_id, value)
            .map_err(UbfError::TypeError)?;
        Ok(self)
    }
    
    /// Add long field
    pub fn with_long(mut self, field_id: i32, value: i64) -> Result<Self, UbfError> {
        self.buffer.add_long(field_id, value)
            .map_err(UbfError::TypeError)?;
        Ok(self)
    }
    
    /// Add double field
    pub fn with_double(mut self, field_id: i32, value: f64) -> Result<Self, UbfError> {
        self.buffer.add_double(field_id, value)
            .map_err(UbfError::TypeError)?;
        Ok(self)
    }
    
    /// Build and return the UBF buffer
    pub fn build(self) -> UbfBuffer {
        self.buffer
    }
}

/// Example: Simple data struct that maps to UBF JSON field
/// 
/// This demonstrates marshal/unmarshal pattern where entire struct
/// is serialized to JSON and stored in T_DATA_FLD
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestData {
    pub operation: String,
    pub user_id: i64,
    pub amount: f64,
    pub metadata: Option<String>,
}

/// Example: Complex struct with multiple UBF field mappings
/// 
/// In a real derive macro implementation, this would use:
/// ```ignore
/// #[derive(UbfStruct)]
/// struct Transaction {
///     #[ubf(field = "T_NAME_FLD")]
///     name: String,
///     #[ubf(field = "T_ID_FLD")]
///     id: i64,
///     #[ubf(field = "T_PRICE_FLD")]
///     amount: f64,
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub name: String,
    pub id: i64,
    pub amount: f64,
    pub status: String,
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
        
        Ok(Transaction {
            name,
            id,
            amount,
            status,
        })
    }
    
    fn to_ubf(&self) -> Result<UbfBuffer, UbfError> {
        let mut buf = UbfBuffer::new(2048)
            .map_err(UbfError::AllocationError)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ubf_struct_builder() {
        let result = UbfStructBuilder::new(1024)
            .and_then(|b| b.with_string(T_NAME_FLD, "Test"))
            .and_then(|b| b.with_long(T_ID_FLD, 123))
            .and_then(|b| b.with_double(T_PRICE_FLD, 45.67));
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_marshal_unmarshal_json() {
        // Create test data
        let data = RequestData {
            operation: "transfer".to_string(),
            user_id: 12345,
            amount: 100.50,
            metadata: Some("test transaction".to_string()),
        };
        
        // Marshal to UBF
        let ubf = marshal(&data).expect("Marshal should succeed");
        
        // Verify buffer was created
        assert!(ubf.used() > 0);
        
        // Unmarshal back
        let restored: RequestData = unmarshal(&ubf).expect("Unmarshal should succeed");
        
        // Verify data matches
        assert_eq!(data, restored);
        assert_eq!(restored.operation, "transfer");
        assert_eq!(restored.user_id, 12345);
        assert_eq!(restored.amount, 100.50);
    }
    
    #[test]
    fn test_marshal_unmarshal_nested() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct Address {
            street: String,
            city: String,
            zip: String,
        }
        
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct Person {
            name: String,
            age: u32,
            address: Address,
        }
        
        let person = Person {
            name: "John Doe".to_string(),
            age: 30,
            address: Address {
                street: "123 Main St".to_string(),
                city: "New York".to_string(),
                zip: "10001".to_string(),
            },
        };
        
        // Marshal
        let ubf = marshal(&person).expect("Marshal should succeed");
        
        // Unmarshal
        let restored: Person = unmarshal(&ubf).expect("Unmarshal should succeed");
        
        // Verify
        assert_eq!(person, restored);
        assert_eq!(restored.address.city, "New York");
    }
    
    #[test]
    fn test_transaction_ubf_struct() {
        let txn = Transaction {
            name: "Payment".to_string(),
            id: 999,
            amount: 250.75,
            status: "completed".to_string(),
        };
        
        // Convert to UBF
        let ubf = txn.to_ubf().expect("to_ubf should succeed");
        
        // Convert back
        let restored = Transaction::from_ubf(&ubf).expect("from_ubf should succeed");
        
        // Verify
        assert_eq!(txn.name, restored.name);
        assert_eq!(txn.id, restored.id);
        assert_eq!(txn.amount, restored.amount);
        assert_eq!(txn.status, restored.status);
    }
    
    #[test]
    fn test_user_data_round_trip() {
        let user = UserData {
            name: "Test User".to_string(),
            id: 42,
            balance: 1000.00,
            active: true,
        };
        
        let ubf = user.to_ubf().expect("to_ubf failed");
        let restored = UserData::from_ubf(&ubf).expect("from_ubf failed");
        
        assert_eq!(user.name, restored.name);
        assert_eq!(user.id, restored.id);
        assert_eq!(user.balance, restored.balance);
        assert_eq!(user.active, restored.active);
    }
    
    #[test]
    fn test_marshal_unmarshal_with_optional() {
        let data1 = RequestData {
            operation: "query".to_string(),
            user_id: 777,
            amount: 0.0,
            metadata: None,
        };
        
        let ubf = marshal(&data1).unwrap();
        let restored: RequestData = unmarshal(&ubf).unwrap();
        
        assert_eq!(data1, restored);
        assert!(restored.metadata.is_none());
    }
    
    #[test]
    fn test_unmarshal_error_handling() {
        let empty_buffer = UbfBuffer::new(1024).unwrap();
        
        // Should fail - no T_DATA_FLD
        let result: Result<RequestData, _> = unmarshal(&empty_buffer);
        assert!(result.is_err());
        
        match result {
            Err(UbfError::FieldNotFound(_)) => {}
            _ => panic!("Expected FieldNotFound error"),
        }
    }
    
    #[test]
    fn test_builder_pattern() {
        let ubf = UbfStructBuilder::new(2048)
            .and_then(|b| b.with_string(T_NAME_FLD, "Alice"))
            .and_then(|b| b.with_long(T_ID_FLD, 100))
            .and_then(|b| b.with_double(T_PRICE_FLD, 99.99))
            .and_then(|b| b.with_string(T_STATUS_FLD, "active"))
            .map(|b| b.build())
            .expect("Builder should succeed");
        
        // Debug: check what we get when reading T_NAME_FLD
        let name_val = ubf.get_string(T_NAME_FLD, 0);
        eprintln!("DEBUG: T_NAME_FLD={}, get_string result={:?}", T_NAME_FLD, name_val);
        
        let txn = Transaction::from_ubf(&ubf).expect("Should parse transaction");
        assert_eq!(txn.name, "Alice");
        assert_eq!(txn.id, 100);
        assert_eq!(txn.amount, 99.99);
        assert_eq!(txn.status, "active");
    }
}
