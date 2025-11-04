//! Enduro/X logging functions

use libc::c_int;
use std::ffi::CString;

// Log levels
const LOG_ERROR: c_int = 1;
const LOG_WARN: c_int = 2;
const LOG_INFO: c_int = 3;
const LOG_DEBUG: c_int = 4;

/// Log info message
pub fn tplog_info(msg: &str) {
    log_message(LOG_INFO, msg);
}

/// Log error message
pub fn tplog_error(msg: &str) {
    log_message(LOG_ERROR, msg);
}

/// Log warning message
pub fn tplog_warn(msg: &str) {
    log_message(LOG_WARN, msg);
}

/// Log debug message
pub fn tplog_debug(msg: &str) {
    log_message(LOG_DEBUG, msg);
}

fn log_message(level: c_int, msg: &str) {
    if let Ok(c_msg) = CString::new(msg.to_string()) {
        unsafe {
            crate::ffi::tplog(level, c_msg.as_ptr());
        }
    }
}
