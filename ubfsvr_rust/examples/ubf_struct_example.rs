/// Example of using UBF struct pattern with endurox-rs derive macros
///
/// This demonstrates how to work with typed Rust structs instead of raw UBF buffers
use endurox_rs::{AtmiCtx, UbfDeserialize, UbfSerialize};

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::{T_FLAG_FLD, T_ID_FLD, T_NAME_FLD, T_PRICE_FLD};

#[derive(Debug, UbfSerialize, UbfDeserialize)]
struct UserData {
    #[ubf(field = T_NAME_FLD)]
    name: String,

    #[ubf(field = T_ID_FLD)]
    id: i64,

    #[ubf(field = T_PRICE_FLD)]
    balance: f64,

    #[ubf(field = T_FLAG_FLD)]
    active: i16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== UBF Struct Example ===\n");

    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;

    // Example 1: Create struct and convert to UBF
    println!("1. Creating UserData struct and converting to UBF:");
    let user = UserData {
        name: "Alice Smith".to_string(),
        id: 54321,
        balance: 1500.75,
        active: 1,
    };

    println!("   Original struct: {:?}", user);

    let mut ubf = ctx.tpalloc_ubf(1024)?;
    ubf.ubf_write(&user, true)?;
    println!(
        "   Converted to UBF buffer (size: {} bytes, used: {} bytes)",
        ubf.bsizeof()?,
        ctx.bused(&ubf)?
    );
    println!();

    // Example 2: Read UBF buffer back to struct
    println!("2. Reading UBF buffer back to struct:");
    let user2: UserData = ubf.ubf_read()?;
    println!("   Restored struct: {:?}", user2);
    println!("   Name: {}", user2.name);
    println!("   ID: {}", user2.id);
    println!("   Balance: ${:.2}", user2.balance);
    println!("   Active: {}", user2.active);
    println!();

    // Example 3: Update existing buffer
    println!("3. Updating existing UBF buffer:");
    let updated_user = UserData {
        name: "Charlie Brown".to_string(),
        id: 11111,
        balance: 500.25,
        active: 0,
    };

    ubf.ubf_write(&updated_user, true)?;
    println!("   Updated buffer with new user data");
    println!("   Used: {} bytes", ctx.bused(&ubf)?);

    // Verify
    let verified: UserData = ubf.ubf_read()?;
    println!("   Verified: {:?}", verified);
    println!();

    // Example 4: Pattern matching with Result
    println!("4. Error handling with Result:");
    let empty_buffer = ctx.tpalloc_ubf(1024)?;

    match empty_buffer.ubf_read::<UserData>() {
        Ok(user) => println!("   Got user: {:?}", user),
        Err(e) => println!("   Expected error - field not found: {}", e),
    }

    println!("\n=== Example completed successfully ===");
    Ok(())
}
