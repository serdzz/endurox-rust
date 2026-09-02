/// Example of using UBF serde derive macros from endurox-rs
///
/// Run with: cargo run --example derive_macro_example
use endurox_rs::{AtmiCtx, TypedUbf, UbfResult, UbfValue};
use endurox_rs::{UbfDeserialize, UbfSerialize};

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::*;

/// Simple transaction struct using derive macros
#[derive(Debug, Clone, UbfSerialize, UbfDeserialize)]
struct Transaction {
    #[ubf(field = T_NAME_FLD)] // Auto-generated constant
    name: String,

    #[ubf(field = T_ID_FLD)] // Auto-generated constant
    id: i64,

    #[ubf(field = T_PRICE_FLD)] // Auto-generated constant
    amount: f64,

    #[ubf(field = T_STATUS_FLD)] // Auto-generated constant
    status: Option<String>, // may be absent - defaulted to "pending" below
}

impl Transaction {
    fn status_or_default(&self) -> &str {
        self.status.as_deref().unwrap_or("pending")
    }
}

/// User account with derive macros
#[derive(Debug, Clone, UbfSerialize, UbfDeserialize)]
struct UserAccount {
    #[ubf(field = T_NAME_FLD)] // Auto-generated constant
    username: String,

    #[ubf(field = T_ID_FLD)] // Auto-generated constant
    user_id: i64,

    #[ubf(field = T_PRICE_FLD)] // Auto-generated constant
    balance: f64,

    #[ubf(field = T_FLAG_FLD)] // Auto-generated constant
    active: i16,
}

/// Address struct for nested example
#[derive(Debug, Clone, UbfSerialize, UbfDeserialize)]
struct Address {
    #[ubf(field = T_STREET_FLD)] // Auto-generated constant
    street: String,

    #[ubf(field = T_CITY_FLD)] // Auto-generated constant
    city: String,

    #[ubf(field = T_ZIP_FLD)] // Auto-generated constant
    zip: String,
}

/// Customer with address serialized into the same flat buffer
#[derive(Debug, Clone, UbfSerialize, UbfDeserialize)]
struct Customer {
    #[ubf(field = T_NAME_FLD)] // Auto-generated constant
    name: String,

    #[ubf(field = T_ID_FLD)] // Auto-generated constant
    customer_id: i64,

    #[ubf(field = T_STREET_FLD)]
    street: String,

    #[ubf(field = T_CITY_FLD)]
    city: String,

    #[ubf(field = T_ZIP_FLD)]
    zip: String,
}

fn roundtrip<'ctx, T>(ctx: &'ctx AtmiCtx, value: &T) -> UbfResult<TypedUbf<'ctx>>
where
    T: endurox_rs::UbfSerialize,
{
    let mut ubf = ctx
        .tpalloc_ubf(2048)
        .map_err(|e| endurox_rs::UbfError::new(endurox_rs::UbfError::BMALLOC, e.to_string()))?;
    ubf.ubf_write(value, true)?;
    Ok(ubf)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize ATMI context (required for UBF operations)
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;

    println!("=== UBF Serde Derive Macro Example ===\n");

    // Example 1: Create and convert Transaction
    println!("1. Transaction with derive macro:");
    let txn = Transaction {
        name: "Payment".to_string(),
        id: 12345,
        amount: 999.99,
        status: Some("completed".to_string()),
    };

    println!("   Original: {:?}", txn);

    // Convert to UBF
    let ubf = roundtrip(&ctx, &txn)?;
    println!("   Converted to UBF (used: {} bytes)", ctx.bused(&ubf)?);

    // Convert back
    let restored: Transaction = ubf.ubf_read()?;
    println!("   Restored: {:?}", restored);
    println!();

    // Example 2: Test default value
    println!("2. Transaction with default status:");
    let mut ubf2 = ctx.tpalloc_ubf(1024)?;
    ubf2.bchg(
        T_NAME_FLD,
        0,
        UbfValue::String("Transfer".to_string()),
        true,
    )?;
    ubf2.bchg(T_ID_FLD, 0, UbfValue::Long(999), true)?;
    ubf2.bchg(T_PRICE_FLD, 0, UbfValue::Double(50.0), true)?;
    // Note: no status field - should use default

    let txn2: Transaction = ubf2.ubf_read()?;
    println!("   Transaction: {:?}", txn2);
    println!(
        "   Status (should be 'pending'): {}",
        txn2.status_or_default()
    );
    println!();

    // Example 3: UserAccount
    println!("3. UserAccount with derive macro:");
    let user = UserAccount {
        username: "alice".to_string(),
        user_id: 42,
        balance: 1500.50,
        active: 1,
    };

    println!("   Original: {:?}", user);

    let ubf_user = roundtrip(&ctx, &user)?;
    let restored_user: UserAccount = ubf_user.ubf_read()?;

    println!("   Restored: {:?}", restored_user);
    println!("   Active: {}", restored_user.active);
    println!();

    // Example 4: Update existing buffer
    println!("4. Updating existing UBF buffer:");
    let mut ubf_mut = ctx.tpalloc_ubf(2048)?;

    let updated_txn = Transaction {
        name: "Refund".to_string(),
        id: 777,
        amount: 123.45,
        status: Some("processed".to_string()),
    };

    updated_txn.ubf_serialize(&mut ubf_mut, true)?;
    println!("   Updated buffer with transaction");

    let verified: Transaction = ubf_mut.ubf_read()?;
    println!("   Verified: {:?}", verified);
    println!();

    // Example 5: Composed struct - Customer carries flat Address fields
    println!("5. Composed struct - Customer with Address fields:");
    let customer = Customer {
        name: "John Doe".to_string(),
        customer_id: 1001,
        street: "123 Main St".to_string(),
        city: "Springfield".to_string(),
        zip: "12345".to_string(),
    };

    println!("   Original: {:?}", customer);

    let ubf_customer = roundtrip(&ctx, &customer)?;
    let restored_customer: Customer = ubf_customer.ubf_read()?;
    println!("   Restored: {:?}", restored_customer);

    // The same buffer can also be read as the Address sub-structure
    let restored_address: Address = ubf_customer.ubf_read()?;
    println!("   Address city: {}", restored_address.city);
    println!();

    // Example 6: Error handling
    println!("6. Error handling:");
    let empty_buffer = ctx.tpalloc_ubf(1024)?;

    match empty_buffer.ubf_read::<Transaction>() {
        Ok(t) => println!("   Got transaction: {:?}", t),
        Err(e) => println!("   ✓ Expected error - missing field: {}", e),
    }

    println!("\n=== All examples completed successfully ===");
    Ok(())
}
