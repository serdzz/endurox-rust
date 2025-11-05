/// Example of using UBF struct pattern
/// 
/// This demonstrates how to work with typed Rust structs instead of raw UBF buffers

use endurox_sys::ubf::UbfBuffer;
use endurox_sys::ubf_struct::{UbfStruct, UbfError, UserData, UbfStructBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== UBF Struct Example ===\n");
    
    // Example 1: Create struct and convert to UBF
    println!("1. Creating UserData struct and converting to UBF:");
    let user = UserData {
        name: "Alice Smith".to_string(),
        id: 54321,
        balance: 1500.75,
        active: true,
    };
    
    println!("   Original struct: {:?}", user);
    
    let ubf_buffer = user.to_ubf()?;
    println!("   Converted to UBF buffer (size: {} bytes, used: {} bytes)", 
             ubf_buffer.size(), ubf_buffer.used());
    println!();
    
    // Example 2: Read UBF buffer back to struct
    println!("2. Reading UBF buffer back to struct:");
    let user2 = UserData::from_ubf(&ubf_buffer)?;
    println!("   Restored struct: {:?}", user2);
    println!("   Name: {}", user2.name);
    println!("   ID: {}", user2.id);
    println!("   Balance: ${:.2}", user2.balance);
    println!("   Active: {}", user2.active);
    println!();
    
    // Example 3: Using UbfStructBuilder
    println!("3. Using UbfStructBuilder pattern:");
    let ubf = UbfStructBuilder::new(1024)?
        .with_string(1002, "Bob Johnson")?  // T_NAME_FLD
        .with_long(1012, 99999)?            // T_ID_FLD
        .with_double(1021, 2500.00)?        // T_PRICE_FLD
        .build();
    
    println!("   Built UBF buffer with builder pattern");
    println!("   Size: {} bytes, Used: {} bytes", ubf.size(), ubf.used());
    
    // Convert back to struct
    let user3 = UserData::from_ubf(&ubf)?;
    println!("   User from builder: {:?}", user3);
    println!();
    
    // Example 4: Update existing buffer
    println!("4. Updating existing UBF buffer:");
    let mut ubf_mut = UbfBuffer::new(1024)?;
    
    let updated_user = UserData {
        name: "Charlie Brown".to_string(),
        id: 11111,
        balance: 500.25,
        active: false,
    };
    
    updated_user.update_ubf(&mut ubf_mut)?;
    println!("   Updated buffer with new user data");
    println!("   Used: {} bytes", ubf_mut.used());
    
    // Verify
    let verified = UserData::from_ubf(&ubf_mut)?;
    println!("   Verified: {:?}", verified);
    println!();
    
    // Example 5: Pattern matching with Result
    println!("5. Error handling with Result:");
    let empty_buffer = UbfBuffer::new(1024)?;
    
    match UserData::from_ubf(&empty_buffer) {
        Ok(user) => println!("   Got user: {:?}", user),
        Err(UbfError::FieldNotFound(field)) => {
            println!("   Expected error - field not found: {}", field);
        }
        Err(e) => println!("   Other error: {}", e),
    }
    
    println!("\n=== Example completed successfully ===");
    Ok(())
}
