//! UBF (Unified Buffer Format) safe API
//!
//! This module provides safe Rust wrappers around Enduro/X UBF API.
//! UBF is a typed, self-describing buffer format for structured data.

use crate::ffi;
use libc::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::fmt;
use std::ptr;

/// UBF Buffer - safe wrapper around Enduro/X UBF buffer
pub struct UbfBuffer {
    ptr: *mut c_char,
    size: usize,
}

impl UbfBuffer {
    /// Allocate a new UBF buffer
    pub fn new(size: usize) -> Result<Self, String> {
        let ubf_type = CString::new("UBF").map_err(|e| e.to_string())?;
        let ptr = unsafe { ffi::tpalloc(ubf_type.as_ptr(), ptr::null(), size as c_long) };

        if ptr.is_null() {
            return Err("Failed to allocate UBF buffer".to_string());
        }

        // Initialize the UBF buffer
        let result = unsafe { ffi::Binit(ptr, size as c_long) };
        if result == -1 {
            unsafe {
                ffi::tpfree(ptr);
            }
            return Err("Failed to initialize UBF buffer".to_string());
        }

        Ok(UbfBuffer { ptr, size })
    }

    /// Add a string field
    pub fn add_string(&mut self, field_id: i32, value: &str) -> Result<(), String> {
        let c_value = CString::new(value).map_err(|e| e.to_string())?;
        let result = unsafe {
            ffi::Badd(
                self.ptr,
                field_id,
                c_value.as_ptr(),
                0, // 0 for null-terminated strings
            )
        };

        if result == -1 {
            return Err(format!("Failed to add string field {}", field_id));
        }

        Ok(())
    }

    /// Add a long field
    pub fn add_long(&mut self, field_id: i32, value: i64) -> Result<(), String> {
        let val = value as c_long;
        let result = unsafe {
            ffi::Badd(
                self.ptr,
                field_id,
                &val as *const c_long as *const c_char,
                0, // 0 = use field type from field ID
            )
        };

        if result == -1 {
            return Err(format!("Failed to add long field {}", field_id));
        }

        Ok(())
    }

    /// Add a double field
    pub fn add_double(&mut self, field_id: i32, value: f64) -> Result<(), String> {
        let result = unsafe {
            ffi::Badd(
                self.ptr,
                field_id,
                &value as *const f64 as *const c_char,
                0, // 0 = use field type from field ID
            )
        };

        if result == -1 {
            return Err(format!("Failed to add double field {}", field_id));
        }

        Ok(())
    }

    /// Change a string field at specific occurrence
    pub fn change_string(&mut self, field_id: i32, occ: i32, value: &str) -> Result<(), String> {
        let c_value = CString::new(value).map_err(|e| e.to_string())?;
        let result = unsafe { ffi::Bchg(self.ptr, field_id, occ, c_value.as_ptr(), 0) };

        if result == -1 {
            return Err(format!(
                "Failed to change string field {} at occ {}",
                field_id, occ
            ));
        }

        Ok(())
    }

    /// Get a string field
    pub fn get_string(&self, field_id: i32, occ: i32) -> Result<String, String> {
        let mut buf = vec![0u8; 1024];
        let mut len = buf.len() as c_int;

        let result = unsafe {
            ffi::CBget(
                self.ptr,
                field_id,
                occ,
                buf.as_mut_ptr() as *mut c_char,
                &mut len,
                ffi::BFLD_STRING,
            )
        };

        if result == -1 {
            return Err(format!(
                "Failed to get string field {} at occ {}",
                field_id, occ
            ));
        }

        // Convert C string to Rust String
        let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
        Ok(c_str.to_string_lossy().into_owned())
    }

    /// Get a long field
    pub fn get_long(&self, field_id: i32, occ: i32) -> Result<i64, String> {
        let mut value: c_long = 0;
        let mut len = std::mem::size_of::<c_long>() as c_int;

        let result = unsafe {
            ffi::CBget(
                self.ptr,
                field_id,
                occ,
                &mut value as *mut c_long as *mut c_char,
                &mut len,
                ffi::BFLD_LONG,
            )
        };

        if result == -1 {
            return Err(format!(
                "Failed to get long field {} at occ {}",
                field_id, occ
            ));
        }

        Ok(value as i64)
    }

    /// Get a double field
    pub fn get_double(&self, field_id: i32, occ: i32) -> Result<f64, String> {
        let mut value: f64 = 0.0;
        let mut len = std::mem::size_of::<f64>() as c_int;

        let result = unsafe {
            ffi::CBget(
                self.ptr,
                field_id,
                occ,
                &mut value as *mut f64 as *mut c_char,
                &mut len,
                ffi::BFLD_DOUBLE,
            )
        };

        if result == -1 {
            return Err(format!(
                "Failed to get double field {} at occ {}",
                field_id, occ
            ));
        }

        Ok(value)
    }

    /// Check if field is present
    pub fn is_present(&self, field_id: i32, occ: i32) -> bool {
        unsafe { ffi::Bpres(self.ptr, field_id, occ) == 1 }
    }

    /// Delete a field occurrence
    pub fn delete(&mut self, field_id: i32, occ: i32) -> Result<(), String> {
        let result = unsafe { ffi::Bdel(self.ptr, field_id, occ) };

        if result == -1 {
            return Err(format!(
                "Failed to delete field {} at occ {}",
                field_id, occ
            ));
        }

        Ok(())
    }

    /// Get field name by ID
    pub fn field_name(field_id: i32) -> Result<String, String> {
        let name_ptr = unsafe { ffi::Bfname(field_id) };

        if name_ptr.is_null() {
            return Err(format!("Field ID {} not found", field_id));
        }

        let c_str = unsafe { CStr::from_ptr(name_ptr) };
        Ok(c_str.to_string_lossy().into_owned())
    }

    /// Get field ID by name
    pub fn field_id(field_name: &str) -> Result<i32, String> {
        let c_name = CString::new(field_name).map_err(|e| e.to_string())?;
        let field_id = unsafe { ffi::Bfldid(c_name.as_ptr()) };

        if field_id == -1 {
            return Err(format!("Field name '{}' not found", field_name));
        }

        Ok(field_id)
    }

    /// Get used buffer size
    pub fn used(&self) -> usize {
        unsafe { ffi::Bused(self.ptr) as usize }
    }

    /// Get unused buffer size
    pub fn unused(&self) -> usize {
        unsafe { ffi::Bunused(self.ptr) as usize }
    }

    /// Get total buffer size
    pub fn size(&self) -> usize {
        unsafe { ffi::Bsizeof(self.ptr) as usize }
    }

    /// Print buffer to stdout (for debugging)
    pub fn print(&self) -> Result<(), String> {
        let result = unsafe { ffi::Bprint(self.ptr) };

        if result == -1 {
            return Err("Failed to print UBF buffer".to_string());
        }

        Ok(())
    }

    /// Get raw pointer (for FFI)
    pub fn as_ptr(&self) -> *mut c_char {
        self.ptr
    }

    /// Get buffer as byte slice
    pub fn as_bytes(&self) -> &[u8] {
        let used_size = self.used();
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, used_size) }
    }

    /// Create UbfBuffer from byte slice
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let size = data.len();
        let ubf_type = CString::new("UBF").map_err(|e| e.to_string())?;
        let ptr = unsafe { ffi::tpalloc(ubf_type.as_ptr(), ptr::null(), size as c_long) };

        if ptr.is_null() {
            return Err("Failed to allocate UBF buffer".to_string());
        }

        // Copy data
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, size);
        }

        Ok(UbfBuffer { ptr, size })
    }

    /// Get raw pointer and consume the buffer (for tpreturn)
    pub fn into_raw(self) -> *mut c_char {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }

    /// Create UbfBuffer from raw pointer (unsafe - caller must ensure validity)
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is a valid pointer to a UBF buffer allocated by Balloc or tpalloc.
    pub unsafe fn from_raw(ptr: *mut c_char) -> Self {
        let size = ffi::Bsizeof(ptr) as usize;
        UbfBuffer { ptr, size }
    }
}

impl Drop for UbfBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                ffi::tpfree(self.ptr);
            }
        }
    }
}

impl fmt::Debug for UbfBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UbfBuffer")
            .field("ptr", &self.ptr)
            .field("size", &self.size())
            .field("used", &self.used())
            .field("unused", &self.unused())
            .finish()
    }
}

/// UBF field iterator
pub struct UbfIterator {
    buffer_ptr: *mut c_char,
    current_field_id: c_int,
    current_occ: c_int,
}

impl UbfIterator {
    pub fn new(buffer: &UbfBuffer) -> Self {
        UbfIterator {
            buffer_ptr: buffer.ptr,
            current_field_id: 0,
            current_occ: 0,
        }
    }
}

impl Iterator for UbfIterator {
    type Item = (i32, i32); // (field_id, occurrence)

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = vec![0u8; 1024];
        let mut len = buf.len() as c_int;

        let result = unsafe {
            ffi::Bnext(
                self.buffer_ptr,
                &mut self.current_field_id,
                &mut self.current_occ,
                buf.as_mut_ptr() as *mut c_char,
                &mut len,
            )
        };

        if result == 1 {
            Some((self.current_field_id, self.current_occ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
