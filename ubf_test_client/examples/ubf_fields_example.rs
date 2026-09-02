/// Example using auto-generated UBF field constants
///
/// This demonstrates the correct way to use UBF fields with proper type encoding
use endurox_rs::{AtmiCtx, UbfValue};

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::*; // Import auto-generated constants

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize ATMI context
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;

    println!("=== UBF Fields Example with Auto-Generated Constants ===\n");

    // Create UBF buffer
    let mut buf = ctx.tpalloc_ubf(1024)?;

    // Add fields using auto-generated constants
    // These constants already have proper type encoding
    println!("Adding fields to UBF buffer...");
    buf.bchg(
        T_NAME_FLD,
        0,
        UbfValue::String("John Doe".to_string()),
        true,
    )?;
    println!("  T_NAME_FLD ({}): \"John Doe\"", T_NAME_FLD);

    buf.bchg(T_ID_FLD, 0, UbfValue::Long(12345), true)?;
    println!("  T_ID_FLD ({}): 12345", T_ID_FLD);

    buf.bchg(T_PRICE_FLD, 0, UbfValue::Double(999.99), true)?;
    println!("  T_PRICE_FLD ({}): 999.99", T_PRICE_FLD);

    buf.bchg(
        T_STATUS_FLD,
        0,
        UbfValue::String("ACTIVE".to_string()),
        true,
    )?;
    println!("  T_STATUS_FLD ({}): \"ACTIVE\"", T_STATUS_FLD);

    buf.bchg(T_COUNT_FLD, 0, UbfValue::Long(42), true)?;
    println!("  T_COUNT_FLD ({}): 42", T_COUNT_FLD);

    let size = buf.bsizeof()?;
    let used = ctx.bused(&buf)?;
    println!("\nBuffer info:");
    println!("  Size: {} bytes", size);
    println!("  Used: {} bytes", used);
    println!("  Unused: {} bytes", size.saturating_sub(used));

    // Print UBF buffer contents
    println!("\nBuffer contents:");
    buf.bprint()?;

    // Read fields back
    println!("\nReading fields back...");
    let name = buf.bget_string(T_NAME_FLD, 0)?;
    println!("  Name: {}", name);

    let id = buf.bget_long(T_ID_FLD, 0)?;
    println!("  ID: {}", id);

    let price = buf.bget_double(T_PRICE_FLD, 0)?;
    println!("  Price: {:.2}", price);

    let status = buf.bget_string(T_STATUS_FLD, 0)?;
    println!("  Status: {}", status);

    let count = buf.bget_long(T_COUNT_FLD, 0)?;
    println!("  Count: {}", count);

    // Verify data integrity
    assert_eq!(name, "John Doe");
    assert_eq!(id, 12345);
    assert!((price - 999.99).abs() < 0.01);
    assert_eq!(status, "ACTIVE");
    assert_eq!(count, 42);

    println!("\n✅ All fields read correctly!");

    // Cleanup
    ctx.tpterm()?;

    Ok(())
}
