/// Example using auto-generated UBF field constants
/// 
/// This demonstrates the correct way to use UBF fields with proper type encoding

use endurox_sys::ubf::UbfBuffer;
use endurox_sys::ubf_fields::*;  // Import auto-generated constants

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize ATMI context
    unsafe {
        endurox_sys::ffi::tpinit(std::ptr::null_mut());
    }
    
    println!("=== UBF Fields Example with Auto-Generated Constants ===\n");
    
    // Create UBF buffer
    let mut buf = UbfBuffer::new(1024)?;
    
    // Add fields using auto-generated constants
    // These constants already have proper type encoding
    println!("Adding fields to UBF buffer...");
    buf.add_string(T_NAME_FLD, "John Doe")?;
    println!("  T_NAME_FLD ({}): \"John Doe\"", T_NAME_FLD);
    
    buf.add_long(T_ID_FLD, 12345)?;
    println!("  T_ID_FLD ({}): 12345", T_ID_FLD);
    
    buf.add_double(T_PRICE_FLD, 999.99)?;
    println!("  T_PRICE_FLD ({}): 999.99", T_PRICE_FLD);
    
    buf.add_string(T_STATUS_FLD, "ACTIVE")?;
    println!("  T_STATUS_FLD ({}): \"ACTIVE\"", T_STATUS_FLD);
    
    buf.add_long(T_COUNT_FLD, 42)?;
    println!("  T_COUNT_FLD ({}): 42", T_COUNT_FLD);
    
    println!("\nBuffer info:");
    println!("  Size: {} bytes", buf.size());
    println!("  Used: {} bytes", buf.used());
    println!("  Unused: {} bytes", buf.unused());
    
    // Print UBF buffer contents
    println!("\nBuffer contents:");
    buf.print()?;
    
    // Read fields back
    println!("\nReading fields back...");
    let name = buf.get_string(T_NAME_FLD, 0)?;
    println!("  Name: {}", name);
    
    let id = buf.get_long(T_ID_FLD, 0)?;
    println!("  ID: {}", id);
    
    let price = buf.get_double(T_PRICE_FLD, 0)?;
    println!("  Price: {:.2}", price);
    
    let status = buf.get_string(T_STATUS_FLD, 0)?;
    println!("  Status: {}", status);
    
    let count = buf.get_long(T_COUNT_FLD, 0)?;
    println!("  Count: {}", count);
    
    // Verify data integrity
    assert_eq!(name, "John Doe");
    assert_eq!(id, 12345);
    assert!((price - 999.99).abs() < 0.01);
    assert_eq!(status, "ACTIVE");
    assert_eq!(count, 42);
    
    println!("\n✅ All fields read correctly!");
    
    // Cleanup
    unsafe {
        endurox_sys::ffi::tpterm();
    }
    
    Ok(())
}
