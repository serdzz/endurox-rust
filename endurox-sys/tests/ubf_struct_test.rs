use endurox_sys::ubf::*;
use endurox_sys::ubf_struct::*;
use serde::{Deserialize, Serialize};

// Test constants for UBF field IDs
const T_NAME_FLD: i32 = 1001;
const T_ID_FLD: i32 = 1002;
const T_PRICE_FLD: i32 = 1003;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct RequestData {
    operation: String,
    user_id: i64,
    amount: f64,
    metadata: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Transaction {
    name: String,
    id: i64,
    amount: f64,
    status: String,
}

impl UbfStruct for Transaction {
    fn from_ubf(buf: &UbfBuffer) -> Result<Self, UbfError> {
        let name = buf
            .get_string(T_NAME_FLD, 0)
            .map_err(|e| UbfError::TypeError(format!("name: {}", e)))?;

        let id = buf
            .get_long(T_ID_FLD, 0)
            .map_err(|e| UbfError::TypeError(format!("id: {}", e)))?;

        let amount = buf
            .get_double(T_PRICE_FLD, 0)
            .map_err(|e| UbfError::TypeError(format!("amount: {}", e)))?;

        let status = buf
            .get_string(1004, 0) // T_STATUS_FLD
            .map_err(|e| UbfError::TypeError(format!("status: {}", e)))?;

        Ok(Transaction {
            name,
            id,
            amount,
            status,
        })
    }

    fn to_ubf(&self) -> Result<UbfBuffer, UbfError> {
        let mut buf = UbfBuffer::new(1024).map_err(UbfError::AllocationError)?;
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

        buf.add_string(1004, &self.status) // T_STATUS_FLD
            .map_err(|e| UbfError::TypeError(format!("status: {}", e)))?;

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
struct UserData {
    name: String,
    id: i64,
    balance: f64,
    active: bool,
}

impl UbfStruct for UserData {
    fn from_ubf(buf: &UbfBuffer) -> Result<Self, UbfError> {
        let name = buf
            .get_string(T_NAME_FLD, 0)
            .map_err(|e| UbfError::TypeError(format!("name: {}", e)))?;

        let id = buf
            .get_long(T_ID_FLD, 0)
            .map_err(|e| UbfError::TypeError(format!("id: {}", e)))?;

        let balance = buf
            .get_double(T_PRICE_FLD, 0)
            .map_err(|e| UbfError::TypeError(format!("balance: {}", e)))?;

        let active_long = buf
            .get_long(1005, 0)
            .map_err(|e| UbfError::TypeError(format!("active: {}", e)))?;

        Ok(UserData {
            name,
            id,
            balance,
            active: active_long != 0,
        })
    }

    fn to_ubf(&self) -> Result<UbfBuffer, UbfError> {
        let mut buf = UbfBuffer::new(1024).map_err(UbfError::AllocationError)?;
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

        buf.add_long(1005, if self.active { 1 } else { 0 })
            .map_err(|e| UbfError::TypeError(format!("active: {}", e)))?;

        Ok(())
    }
}

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
        .and_then(|b| b.with_string(1004, "active"))
        .map(|b| b.build())
        .expect("Builder should succeed");

    let txn = Transaction::from_ubf(&ubf).expect("Should parse transaction");
    assert_eq!(txn.name, "Alice");
    assert_eq!(txn.id, 100);
    assert_eq!(txn.amount, 99.99);
    assert_eq!(txn.status, "active");
}
