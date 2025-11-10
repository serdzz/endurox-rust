//! Client API - safe wrappers for client functions

use crate::ffi;
use crate::{tplog_error, tplog_info};
use libc::{c_char, c_long};
use std::ffi::{CStr, CString};
use std::ptr;

/// Enduro/X client
pub struct EnduroxClient {
    initialized: bool,
}

impl EnduroxClient {
    /// Creates and initializes the client
    pub fn new() -> Result<Self, String> {
        unsafe {
            tplog_info("Calling tpinit...");
            let ret = ffi::tpinit(ptr::null_mut());
            if ret == -1 {
                let tperrno = *ffi::_exget_tperrno_addr();
                let err_ptr = ffi::tpstrerror(tperrno);
                let err_msg = if !err_ptr.is_null() {
                    CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                } else {
                    "Unknown error".to_string()
                };
                tplog_error(&format!(
                    "tpinit failed: ret={}, tperrno={}, msg={}",
                    ret, tperrno, err_msg
                ));
                return Err(format!("tpinit failed: {}", err_msg));
            }
            tplog_info(&format!("tpinit succeeded: ret={}", ret));
        }

        Ok(EnduroxClient { initialized: true })
    }

    /// Calls a service (blocking)
    pub fn call_service_blocking(&self, service: &str, data: &str) -> Result<String, String> {
        unsafe {
            tplog_info(&format!(
                "call_service_blocking: service={}, data_len={}",
                service,
                data.len()
            ));

            // Allocate STRING buffer for input
            let string_type = CString::new("STRING").map_err(|e| e.to_string())?;
            let send_buf = ffi::tpalloc(
                string_type.as_ptr(),
                ptr::null(),
                (data.len() + 1) as c_long,
            );

            if send_buf.is_null() {
                let tperrno = *ffi::_exget_tperrno_addr();
                let err_msg = format!("Failed to allocate send buffer, tperrno={}", tperrno);
                tplog_error(&err_msg);
                return Err(err_msg);
            }

            // Copy data to buffer
            let c_data = CString::new(data).map_err(|e| e.to_string())?;
            ptr::copy_nonoverlapping(c_data.as_ptr(), send_buf, data.len() + 1);

            // Make synchronous call with tpcall
            let c_service = CString::new(service).map_err(|e| e.to_string())?;
            let mut recv_buf: *mut c_char = ptr::null_mut();
            let mut recv_len: c_long = 0;

            tplog_info(&format!("Calling tpcall for service: {}", service));

            let ret = ffi::tpcall(
                c_service.as_ptr(),
                send_buf,
                (data.len() + 1) as c_long,
                &mut recv_buf,
                &mut recv_len,
                0, // Try with no flags first
            );

            ffi::tpfree(send_buf);

            tplog_info(&format!(
                "tpcall returned: ret={}, recv_buf={:?}, recv_len={}",
                ret, recv_buf, recv_len
            ));

            if ret == -1 {
                if !recv_buf.is_null() {
                    ffi::tpfree(recv_buf);
                }
                let tperrno = *ffi::_exget_tperrno_addr();
                let err_ptr = ffi::tpstrerror(tperrno);
                let err_msg = if !err_ptr.is_null() {
                    CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                } else {
                    "Unknown error".to_string()
                };
                tplog_error(&format!(
                    "tpcall failed: ret={}, tperrno={}, msg={}",
                    ret, tperrno, err_msg
                ));
                return Err(format!("tpcall failed: {}: {}", tperrno, err_msg));
            }

            // Convert response to string
            let response = if !recv_buf.is_null() && recv_len > 0 {
                let c_str = CStr::from_ptr(recv_buf);
                let result = c_str.to_string_lossy().into_owned();
                ffi::tpfree(recv_buf);
                result
            } else {
                if !recv_buf.is_null() {
                    ffi::tpfree(recv_buf);
                }
                String::new()
            };

            Ok(response)
        }
    }

    /// Call service with UBF buffer (blocking)
    pub fn call_service_ubf_blocking(
        &self,
        service: &str,
        buffer_data: &[u8],
    ) -> Result<Vec<u8>, String> {
        unsafe {
            tplog_info(&format!(
                "call_service_ubf_blocking: service={}, data_len={}",
                service,
                buffer_data.len()
            ));

            // Allocate UBF buffer for input
            let ubf_type = CString::new("UBF").map_err(|e| e.to_string())?;
            let send_buf =
                ffi::tpalloc(ubf_type.as_ptr(), ptr::null(), buffer_data.len() as c_long);

            if send_buf.is_null() {
                let tperrno = *ffi::_exget_tperrno_addr();
                let err_msg = format!("Failed to allocate UBF send buffer, tperrno={}", tperrno);
                tplog_error(&err_msg);
                return Err(err_msg);
            }

            // Copy data to buffer
            ptr::copy_nonoverlapping(buffer_data.as_ptr(), send_buf as *mut u8, buffer_data.len());

            // Make synchronous call with tpcall
            let c_service = CString::new(service).map_err(|e| e.to_string())?;
            let mut recv_buf: *mut c_char = send_buf;
            let mut recv_len: c_long = 0;

            tplog_info(&format!("Calling tpcall for UBF service: {}", service));

            let ret = ffi::tpcall(
                c_service.as_ptr(),
                send_buf,
                0, // 0 for UBF - length determined automatically
                &mut recv_buf,
                &mut recv_len,
                0,
            );

            tplog_info(&format!(
                "tpcall returned: ret={}, recv_buf={:?}, recv_len={}",
                ret, recv_buf, recv_len
            ));

            if ret == -1 {
                if !recv_buf.is_null() && recv_buf != send_buf {
                    ffi::tpfree(recv_buf);
                } else if !send_buf.is_null() {
                    ffi::tpfree(send_buf);
                }
                let tperrno = *ffi::_exget_tperrno_addr();
                let err_ptr = ffi::tpstrerror(tperrno);
                let err_msg = if !err_ptr.is_null() {
                    CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                } else {
                    "Unknown error".to_string()
                };
                tplog_error(&format!(
                    "tpcall failed: ret={}, tperrno={}, msg={}",
                    ret, tperrno, err_msg
                ));
                return Err(format!("tpcall failed: {}: {}", tperrno, err_msg));
            }

            // Get buffer size and convert to Vec<u8>
            let used_size = if !recv_buf.is_null() {
                ffi::Bused(recv_buf) as usize
            } else {
                0
            };

            let response = if !recv_buf.is_null() && used_size > 0 {
                let data = std::slice::from_raw_parts(recv_buf as *const u8, used_size).to_vec();
                ffi::tpfree(recv_buf);
                data
            } else {
                if !recv_buf.is_null() {
                    ffi::tpfree(recv_buf);
                }
                Vec::new()
            };

            Ok(response)
        }
    }

    /// Call service with raw buffer (for UBF)
    ///
    /// # Safety
    ///
    /// The caller must ensure that `send_buf` is a valid pointer to a buffer allocated by tpalloc.
    pub unsafe fn call_service_raw(
        &self,
        service: &str,
        send_buf: *mut c_char,
    ) -> Result<*mut c_char, String> {
        unsafe {
            tplog_info(&format!("call_service_raw: service={}", service));

            let c_service = CString::new(service).map_err(|e| e.to_string())?;
            let mut recv_buf: *mut c_char = send_buf;
            let mut recv_len: c_long = 0;

            let ret = ffi::tpcall(
                c_service.as_ptr(),
                send_buf,
                0, // 0 for UBF - length determined automatically
                &mut recv_buf,
                &mut recv_len,
                0,
            );

            if ret == -1 {
                if !recv_buf.is_null() && recv_buf != send_buf {
                    ffi::tpfree(recv_buf);
                }
                let tperrno = *ffi::_exget_tperrno_addr();
                let err_ptr = ffi::tpstrerror(tperrno);
                let err_msg = if !err_ptr.is_null() {
                    CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                } else {
                    "Unknown error".to_string()
                };
                tplog_error(&format!("tpcall failed: {}", err_msg));
                return Err(err_msg);
            }

            Ok(recv_buf)
        }
    }
}

impl Drop for EnduroxClient {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                ffi::tpterm();
            }
        }
    }
}
