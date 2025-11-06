use endurox_sys::ubf::UbfBuffer;
use endurox_sys::ubf_fields::*;
use endurox_sys::ubf_struct::{UbfError, UbfStruct as UbfStructTrait};
/// Example of using UbfStruct derive macro
///
/// Run with: cargo run --example derive_macro_example --features "ubf,derive"
use endurox_sys::UbfStruct; // Auto-generated field constants

/// Simple transaction struct using derive macro
#[derive(Debug, Clone, UbfStruct)]
struct Transaction {
    #[ubf(field = T_NAME_FLD)] // Auto-generated constant
    name: String,

    #[ubf(field = T_ID_FLD)] // Auto-generated constant
    id: i64,

    #[ubf(field = T_PRICE_FLD)] // Auto-generated constant
    amount: f64,

    #[ubf(field = T_STATUS_FLD, default = "pending")] // Auto-generated constant
    status: String,
}

/// User account with derive macro
#[derive(Debug, Clone, UbfStruct)]
struct UserAccount {
    #[ubf(field = T_NAME_FLD)] // Auto-generated constant
    username: String,

    #[ubf(field = T_ID_FLD)] // Auto-generated constant
    user_id: i64,

    #[ubf(field = T_PRICE_FLD)] // Auto-generated constant
    balance: f64,

    #[ubf(field = T_FLAG_FLD)] // Auto-generated constant
    active: bool,
}

/// Address struct for nested example
#[derive(Debug, Clone, UbfStruct)]
struct Address {
    #[ubf(field = T_STREET_FLD)] // Auto-generated constant
    street: String,

    #[ubf(field = T_CITY_FLD)] // Auto-generated constant
    city: String,

    #[ubf(field = T_ZIP_FLD)] // Auto-generated constant
    zip: String,
}

/// Customer with nested Address struct
#[derive(Debug, Clone, UbfStruct)]
struct Customer {
    #[ubf(field = T_NAME_FLD)] // Auto-generated constant
    name: String,

    #[ubf(field = T_ID_FLD)] // Auto-generated constant
    customer_id: i64,

    #[ubf(field = 0)] // Nested struct doesn't use a specific field ID
    address: Option<Address>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize ATMI context (required for UBF operations)
    unsafe {
        endurox_sys::ffi::tpinit(std::ptr::null_mut());
    }

    println!("=== UbfStruct Derive Macro Example ===\n");

    // Example 1: Create and convert Transaction
    println!("1. Transaction with derive macro:");
    let txn = Transaction {
        name: "Payment".to_string(),
        id: 12345,
        amount: 999.99,
        status: "completed".to_string(),
    };

    println!("   Original: {:?}", txn);

    // Convert to UBF
    let ubf = txn.to_ubf()?;
    println!("   Converted to UBF (used: {} bytes)", ubf.used());

    // Convert back
    let restored = Transaction::from_ubf(&ubf)?;
    println!("   Restored: {:?}", restored);
    println!();

    // Example 2: Test default value
    println!("2. Transaction with default status:");
    let mut ubf2 = UbfBuffer::new(1024)?;
    ubf2.add_string(T_NAME_FLD, "Transfer")?;
    ubf2.add_long(T_ID_FLD, 999)?;
    ubf2.add_double(T_PRICE_FLD, 50.0)?;
    // Note: no status field - should use default

    let txn2 = Transaction::from_ubf(&ubf2)?;
    println!("   Transaction: {:?}", txn2);
    println!("   Status (should be 'pending'): {}", txn2.status);
    println!();

    // Example 3: UserAccount
    println!("3. UserAccount with derive macro:");
    let user = UserAccount {
        username: "alice".to_string(),
        user_id: 42,
        balance: 1500.50,
        active: true,
    };

    println!("   Original: {:?}", user);

    let ubf_user = user.to_ubf()?;
    let restored_user = UserAccount::from_ubf(&ubf_user)?;

    println!("   Restored: {:?}", restored_user);
    println!("   Active: {}", restored_user.active);
    println!();

    // Example 4: Update existing buffer
    println!("4. Updating existing UBF buffer:");
    let mut ubf_mut = UbfBuffer::new(2048)?;

    let updated_txn = Transaction {
        name: "Refund".to_string(),
        id: 777,
        amount: 123.45,
        status: "processed".to_string(),
    };

    updated_txn.update_ubf(&mut ubf_mut)?;
    println!("   Updated buffer with transaction");

    let verified = Transaction::from_ubf(&ubf_mut)?;
    println!("   Verified: {:?}", verified);
    println!();

    // Example 5: Nested struct
    println!("5. Nested struct - Customer with Address:");
    let customer = Customer {
        name: "John Doe".to_string(),
        customer_id: 1001,
        address: Some(Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            zip: "12345".to_string(),
        }),
    };

    println!("   Original: {:?}", customer);

    let ubf_customer = customer.to_ubf()?;
    let restored_customer = Customer::from_ubf(&ubf_customer)?;

    println!("   Restored: {:?}", restored_customer);
    if let Some(addr) = &restored_customer.address {
        println!("   Address city: {}", addr.city);
    }
    println!();

    // Example 6: Error handling
    println!("6. Error handling:");
    let empty_buffer = UbfBuffer::new(1024)?;

    match Transaction::from_ubf(&empty_buffer) {
        Ok(t) => println!("   Got transaction: {:?}", t),
        Err(UbfError::FieldNotFound(field)) => {
            println!("   ✓ Expected error - missing field: {}", field);
        }
        Err(e) => println!("   Unexpected error: {}", e),
    }

    println!("\n=== All examples completed successfully ===");

    // Cleanup ATMI context
    unsafe {
        endurox_sys::ffi::tpterm();
    }

    Ok(())
}
