use endurox_sys::ubf::*;

#[test]
fn test_ubf_buffer_creation() {
    let buffer = UbfBuffer::new(1024);
    assert!(buffer.is_ok());

    let buf = buffer.unwrap();
    assert!(!buf.as_ptr().is_null());
    assert_eq!(buf.size(), 1024);
}

#[test]
fn test_ubf_add_get_string() {
    // This test requires UBF field tables to be loaded
    // Will work in integration tests with proper Enduro/X setup
}
