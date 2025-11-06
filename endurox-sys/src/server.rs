//! Server API - безопасные обертки для server functions

use crate::ffi::{self, TpSvcInfoRaw, TPFAIL, TPSUCCESS};
use libc::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::ptr;

/// Buffer wrapper для автоматического управления памятью
pub struct TpBuffer {
    ptr: *mut c_char,
    len: usize,
    allocated_size: usize, // Size allocated with tpalloc
}

impl TpBuffer {
    /// Создает новый STRING buffer
    pub fn new_string(content: &str) -> Result<Self, String> {
        let string_type = CString::new("STRING").map_err(|e| e.to_string())?;
        let allocated_size = content.len() + 1;
        let ptr =
            unsafe { ffi::tpalloc(string_type.as_ptr(), ptr::null(), allocated_size as c_long) };

        if ptr.is_null() {
            return Err("Failed to allocate buffer".to_string());
        }

        let c_content = CString::new(content).map_err(|e| e.to_string())?;
        unsafe {
            ptr::copy_nonoverlapping(c_content.as_ptr(), ptr, allocated_size);
        }

        Ok(TpBuffer {
            ptr,
            len: content.len(),
            allocated_size,
        })
    }

    /// Создает новый JSON buffer
    pub fn new_json(content: &str) -> Result<Self, String> {
        let json_type = CString::new("JSON").map_err(|e| e.to_string())?;
        let allocated_size = content.len() + 1;
        let ptr =
            unsafe { ffi::tpalloc(json_type.as_ptr(), ptr::null(), allocated_size as c_long) };

        if ptr.is_null() {
            return Err("Failed to allocate JSON buffer".to_string());
        }

        let c_content = CString::new(content).map_err(|e| e.to_string())?;
        unsafe {
            ptr::copy_nonoverlapping(c_content.as_ptr(), ptr, allocated_size);
        }

        Ok(TpBuffer {
            ptr,
            len: content.len(),
            allocated_size,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Передает владение указателем (для tpreturn)
    pub fn into_raw(self) -> *mut c_char {
        let ptr = self.ptr;
        std::mem::forget(self); // Не вызываем Drop
        ptr
    }
}

impl Drop for TpBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                ffi::tpfree(self.ptr);
            }
        }
    }
}

/// Регистрирует сервис
pub fn advertise_service(
    name: &str,
    handler: extern "C" fn(*mut TpSvcInfoRaw),
) -> Result<(), String> {
    let c_name = CString::new(name).map_err(|e| e.to_string())?;
    let c_funcname = CString::new("service_dispatcher").map_err(|e| e.to_string())?;

    let result = unsafe { ffi::tpadvertise_full(c_name.as_ptr(), handler, c_funcname.as_ptr()) };

    if result == -1 {
        let err_msg = unsafe {
            let tperrno = *ffi::_exget_tperrno_addr();
            let err_ptr = ffi::tpstrerror(tperrno);
            if !err_ptr.is_null() {
                CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
            } else {
                "Unknown error".to_string()
            }
        };
        return Err(err_msg);
    }

    Ok(())
}

/// Возвращает успешный результат
///
/// # Safety
/// Caller must ensure rqst is a valid pointer to TpSvcInfoRaw
pub unsafe fn tpreturn_success(rqst: *mut TpSvcInfoRaw, buffer: TpBuffer) {
    let req = &*rqst;
    let len = buffer.len();
    let ptr = buffer.into_raw();
    // Log buffer content for debugging
    if !ptr.is_null() {
        let c_str = CStr::from_ptr(ptr);
        crate::tplog_info(&format!(
            "tpreturn_success: buffer content=[{}]",
            c_str.to_string_lossy()
        ));
    }
    // Use request buffer if available, copy our data into it
    let ret_ptr = if !req.data.is_null() {
        // Reuse request buffer
        let ret_buf = ffi::tprealloc(req.data, (len + 1) as c_long);
        if !ret_buf.is_null() {
            ptr::copy_nonoverlapping(ptr, ret_buf, len + 1);
            // Free our temp buffer
            ffi::tpfree(ptr);
            ret_buf
        } else {
            ptr
        }
    } else {
        ptr
    };

    crate::tplog_info(&format!(
        "tpreturn_success: calling tpreturn with TPSUCCESS, rcode=1, ptr={:?}, len={}",
        ret_ptr, len
    ));
    // Use standard success code - service specific code in rcode
    ffi::tpreturn(TPSUCCESS, 1, ret_ptr, len as c_long, 0);
}

/// Возвращает тот же buffer что пришел
///
/// # Safety
/// Caller must ensure rqst is a valid pointer to TpSvcInfoRaw
pub unsafe fn tpreturn_echo(rqst: *mut TpSvcInfoRaw) {
    let req = &*rqst;
    // Pass 0 for length - Enduro/X calculates it automatically
    ffi::tpreturn(TPSUCCESS, 0, req.data, 0, 0);
}

/// Возвращает ошибку
///
/// # Safety
/// Caller must ensure rqst is a valid pointer to TpSvcInfoRaw
pub unsafe fn tpreturn_fail(rqst: *mut TpSvcInfoRaw) {
    let req = &*rqst;
    ffi::tpreturn(TPFAIL, 0, req.data, 0, 0);
}

/// Читает данные из запроса
///
/// # Safety
/// Caller must ensure rqst is a valid pointer to TpSvcInfoRaw
pub unsafe fn get_request_data(rqst: *mut TpSvcInfoRaw) -> Result<Vec<u8>, String> {
    let req = &*rqst;
    if req.data.is_null() || req.len <= 0 {
        return Ok(Vec::new());
    }

    let slice = std::slice::from_raw_parts(req.data as *const u8, req.len as usize);
    Ok(slice.to_vec())
}

/// Получает имя сервиса
///
/// # Safety
/// Caller must ensure rqst is a valid pointer to TpSvcInfoRaw
pub unsafe fn get_service_name(rqst: *mut TpSvcInfoRaw) -> Result<String, String> {
    let req = &*rqst;
    let name_bytes: Vec<u8> = req
        .name
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();

    String::from_utf8(name_bytes).map_err(|e| e.to_string())
}

/// Точка входа для server binary
pub fn run_server(
    tpsvrinit: extern "C" fn(c_int, *mut *mut c_char) -> c_int,
    tpsvrdone: extern "C" fn(),
) -> ! {
    // Экспортируем функции для libatmisrvnomain
    unsafe {
        G_tpsvrinit__ = tpsvrinit;
        G_tpsvrdone__ = tpsvrdone;
    }

    // Вызываем ndrx_main
    let args: Vec<CString> = std::env::args()
        .map(|arg| CString::new(arg).unwrap())
        .collect();
    let mut c_args: Vec<*mut c_char> = args.iter().map(|arg| arg.as_ptr() as *mut c_char).collect();
    c_args.push(ptr::null_mut());

    unsafe {
        let result = ffi::ndrx_main(c_args.len() as c_int - 1, c_args.as_mut_ptr());
        std::process::exit(result);
    }
}

// Глобальные указатели для libatmisrvnomain
type TpsvrInitFn = extern "C" fn(c_int, *mut *mut c_char) -> c_int;
type TpsvrDoneFn = extern "C" fn();

#[no_mangle]
pub static mut G_tpsvrinit__: TpsvrInitFn = stub_tpsvrinit;

#[no_mangle]
pub static mut G_tpsvrdone__: TpsvrDoneFn = stub_tpsvrdone;

extern "C" fn stub_tpsvrinit(_: c_int, _: *mut *mut c_char) -> c_int {
    -1
}

extern "C" fn stub_tpsvrdone() {}

// Дополнительные указатели
type TpsvrInitPtr = *mut extern "C" fn(c_int, *mut *mut c_char) -> c_int;
type TpsvrDonePtr = *mut extern "C" fn();

#[no_mangle]
pub static mut ndrx_G_tpsvrinit_sys: TpsvrInitPtr = ptr::null_mut();

#[no_mangle]
pub static mut ndrx_G_tpsvrthrinit: TpsvrInitPtr = ptr::null_mut();

#[no_mangle]
pub static mut ndrx_G_tpsvrthrdone: TpsvrDonePtr = ptr::null_mut();
