use crate::raw;
use crate::AtmiCtx;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_long};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Always = 1,
    Error = 2,
    Warn = 3,
    Info = 4,
    Debug = 5,
    Dump = 6,
}

impl From<LogLevel> for c_int {
    fn from(l: LogLevel) -> Self {
        l as c_int
    }
}

fn call_logex(
    f: unsafe extern "C" fn(c_int, *const c_char, c_long, *const c_char),
    level: LogLevel,
    file: &'static str,
    line: u32,
    msg: &str,
) {
    // sanitize and convert
    let c_file = match CString::new(file) {
        Ok(s) => s,
        Err(_) => return,
    };
    let c_msg = match CString::new(msg.replace('\0', "")) {
        Ok(s) => s,
        Err(_) => return,
    };
    unsafe {
        f(
            level.into(),
            c_file.as_ptr(),
            line as c_long,
            c_msg.as_ptr(),
        )
    }
}

impl AtmiCtx {
    #[inline]
    pub fn tplog_str(&self, level: LogLevel, file: &'static str, line: u32, msg: &str) {
        call_logex(raw::tplogex, level, file, line, msg);
    }

    #[inline]
    pub fn ndrxlog_str(&self, level: LogLevel, file: &'static str, line: u32, msg: &str) {
        call_logex(raw::ndrxlogex, level, file, line, msg);
    }

    #[inline]
    pub fn ubflog_str(&self, level: LogLevel, file: &'static str, line: u32, msg: &str) {
        call_logex(raw::ubflogex, level, file, line, msg);
    }
}

// ---- TP logger macros ----
#[macro_export]
macro_rules! tp_log {
    ($ctx:expr, $lvl:expr, $($arg:tt)*) => {{
        let __msg = format!($($arg)*);
        $ctx.tplog_str($lvl, file!(), line!(), &__msg);
    }};
}
#[macro_export]
macro_rules! tp_always { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Always, $($arg)*); } }
#[macro_export]
macro_rules! tp_error  { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Error,  $($arg)*); } }
#[macro_export]
macro_rules! tp_warn   { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Warn,   $($arg)*); } }
#[macro_export]
macro_rules! tp_info   { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Info,   $($arg)*); } }
#[macro_export]
macro_rules! tp_debug  { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Debug,  $($arg)*); } }
#[macro_export]
macro_rules! tp_dump   { ($ctx:expr, $($arg:tt)*) => { $crate::tp_log!($ctx, $crate::LogLevel::Dump,   $($arg)*); } }

// ---- NDRX logger macros ----
#[macro_export]
macro_rules! ndrx_log {
    ($ctx:expr, $lvl:expr, $($arg:tt)*) => {{
        let __msg = format!($($arg)*);
        $ctx.ndrxlog_str($lvl, file!(), line!(), &__msg);
    }};
}
#[macro_export]
macro_rules! ndrx_always { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Always, $($arg)*); } }
#[macro_export]
macro_rules! ndrx_error  { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Error,  $($arg)*); } }
#[macro_export]
macro_rules! ndrx_warn   { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Warn,   $($arg)*); } }
#[macro_export]
macro_rules! ndrx_info   { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Info,   $($arg)*); } }
#[macro_export]
macro_rules! ndrx_debug  { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Debug,  $($arg)*); } }
#[macro_export]
macro_rules! ndrx_dump   { ($ctx:expr, $($arg:tt)*) => { $crate::ndrx_log!($ctx, $crate::LogLevel::Dump,   $($arg)*); } }

// ---- UBF logger macros ----
#[macro_export]
macro_rules! ubf_log {
    ($ctx:expr, $lvl:expr, $($arg:tt)*) => {{
        let __msg = format!($($arg)*);
        $ctx.ubflog_str($lvl, file!(), line!(), &__msg);
    }};
}
#[macro_export]
macro_rules! ubf_always { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Always, $($arg)*); } }
#[macro_export]
macro_rules! ubf_error  { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Error,  $($arg)*); } }
#[macro_export]
macro_rules! ubf_warn   { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Warn,   $($arg)*); } }
#[macro_export]
macro_rules! ubf_info   { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Info,   $($arg)*); } }
#[macro_export]
macro_rules! ubf_debug  { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Debug,  $($arg)*); } }
#[macro_export]
macro_rules! ubf_dump   { ($ctx:expr, $($arg:tt)*) => { $crate::ubf_log!($ctx, $crate::LogLevel::Dump,   $($arg)*); } }
