use crate::raw::*;
use crate::{raw, AtmiCtx, BorrowedUbf, TypedUbf, UbfResult};
use core::ffi::{c_char, c_int, c_long, c_void};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

/// Fast-add location state used by [`TypedUbf::badd_fast`](crate::TypedUbf::badd_fast).
#[derive(Debug)]
pub struct BFldLocInfo {
    pub(crate) inner: raw::Bfld_loc_info_t,
}

impl Default for BFldLocInfo {
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

/// Compiled UBF boolean expression tree.
#[derive(Debug)]
pub struct UbfExprTree<'ctx> {
    ptr: *mut c_char,
    ctx: &'ctx AtmiCtx,
}

impl<'ctx> UbfExprTree<'ctx> {
    #[inline]
    pub(crate) fn as_ptr(&self) -> *mut c_char {
        self.ptr
    }

    fn free(&mut self) {
        if !self.ptr.is_null() {
            self.ctx.btreefree_value(self.ptr);
            self.ptr = std::ptr::null_mut();
        }
    }
}

impl Drop for UbfExprTree<'_> {
    fn drop(&mut self) {
        self.free();
    }
}

/// UBF expression callback registered with `Bboolsetcbf`.
pub type UbfExprCallback = fn(&TypedUbf<'_>, &str) -> i64;

/// UBF expression callback registered with `Bboolsetcbf2`.
pub type UbfExprCallback2 = fn(&TypedUbf<'_>, &str, &str) -> i64;

struct OutputState {
    bytes: Vec<u8>,
}

struct ReadState<'a> {
    bytes: &'a [u8],
    offset: usize,
}

fn expr_callbacks() -> &'static Mutex<HashMap<String, UbfExprCallback>> {
    static CALLBACKS: OnceLock<Mutex<HashMap<String, UbfExprCallback>>> = OnceLock::new();
    CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn expr_callbacks2() -> &'static Mutex<HashMap<String, UbfExprCallback2>> {
    static CALLBACKS: OnceLock<Mutex<HashMap<String, UbfExprCallback2>>> = OnceLock::new();
    CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe extern "C" fn bfprint_output_callback(
    buffer: *mut *mut c_char,
    datalen: c_long,
    dataptr1: *mut c_void,
    _do_write: *mut c_int,
    _outf: *mut raw::FILE,
    _fid: c_int,
) -> c_int {
    if buffer.is_null() {
        return raw::EXFAIL;
    }
    if (*buffer).is_null() || dataptr1.is_null() || datalen < 0 {
        return raw::EXFAIL;
    }

    let state = &mut *(dataptr1 as *mut OutputState);
    if !state.bytes.is_empty() {
        state.bytes.pop();
    }
    let data = std::slice::from_raw_parts(*buffer as *const u8, datalen as usize);
    state.bytes.extend_from_slice(data);
    raw::EXSUCCEED as c_int
}

unsafe extern "C" fn read_callback(
    buffer: *mut c_char,
    bufsz: c_long,
    dataptr1: *mut c_void,
) -> c_long {
    if buffer.is_null() || dataptr1.is_null() || bufsz <= 0 {
        return 0;
    }

    let state = &mut *(dataptr1 as *mut ReadState<'_>);
    if state.offset >= state.bytes.len() {
        return 0;
    }

    let remaining = state.bytes.len() - state.offset;
    let to_copy = remaining.min(bufsz as usize);
    std::ptr::copy_nonoverlapping(
        state.bytes[state.offset..].as_ptr(),
        buffer as *mut u8,
        to_copy,
    );
    state.offset += to_copy;
    to_copy as c_long
}

unsafe extern "C" fn write_callback(
    buffer: *mut c_char,
    bufsz: c_long,
    dataptr1: *mut c_void,
) -> c_long {
    if buffer.is_null() || dataptr1.is_null() || bufsz < 0 {
        return raw::EXFAIL as c_long;
    }

    let state = &mut *(dataptr1 as *mut OutputState);
    let data = std::slice::from_raw_parts(buffer as *const u8, bufsz as usize);
    state.bytes.extend_from_slice(data);
    bufsz
}

unsafe extern "C" fn expr_callback_proxy(p_ub: *mut raw::UBFH, funcname: *mut c_char) -> c_long {
    expr_callback_proxy_impl(p_ub, funcname, std::ptr::null_mut())
}

unsafe extern "C" fn expr_callback_proxy2(
    p_ub: *mut raw::UBFH,
    funcname: *mut c_char,
    arg1: *mut c_char,
) -> c_long {
    expr_callback_proxy_impl(p_ub, funcname, arg1)
}

unsafe fn expr_callback_proxy_impl(
    p_ub: *mut raw::UBFH,
    funcname: *mut c_char,
    arg1: *mut c_char,
) -> c_long {
    if p_ub.is_null() || funcname.is_null() {
        return 0;
    }

    let Ok(ctx) = AtmiCtx::new() else {
        return 0;
    };
    let ubf = TypedUbf::borrowed_from_raw(&ctx, p_ub as *mut c_char);
    let name = CStr::from_ptr(funcname).to_string_lossy().into_owned();

    catch_unwind(AssertUnwindSafe(|| {
        if arg1.is_null() {
            expr_callbacks()
                .lock()
                .ok()
                .and_then(|callbacks| callbacks.get(&name).copied())
                .map(|cb| cb(&ubf, &name))
                .unwrap_or(0)
        } else {
            let arg = CStr::from_ptr(arg1).to_string_lossy().into_owned();
            expr_callbacks2()
                .lock()
                .ok()
                .and_then(|callbacks| callbacks.get(&name).copied())
                .map(|cb| cb(&ubf, &name, &arg))
                .unwrap_or(0)
        }
    }))
    .unwrap_or(0) as c_long
}

/// UBF field type for safe field-id construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UbfFieldType {
    /// `BFLD_SHORT`.
    Short,
    /// `BFLD_LONG`.
    Long,
    /// `BFLD_CHAR`.
    Char,
    /// `BFLD_FLOAT`.
    Float,
    /// `BFLD_DOUBLE`.
    Double,
    /// `BFLD_STRING`.
    String,
    /// `BFLD_CARRAY`.
    Carray,
    /// `BFLD_PTR`.
    Ptr,
    /// `BFLD_UBF`.
    Ubf,
    /// `BFLD_VIEW`.
    View,
}

impl UbfFieldType {
    #[inline]
    fn as_raw(self) -> c_int {
        match self {
            UbfFieldType::Short => raw::BFLD_SHORT as c_int,
            UbfFieldType::Long => raw::BFLD_LONG as c_int,
            UbfFieldType::Char => raw::BFLD_CHAR as c_int,
            UbfFieldType::Float => raw::BFLD_FLOAT as c_int,
            UbfFieldType::Double => raw::BFLD_DOUBLE as c_int,
            UbfFieldType::String => raw::BFLD_STRING as c_int,
            UbfFieldType::Carray => raw::BFLD_CARRAY as c_int,
            UbfFieldType::Ptr => raw::BFLD_PTR as c_int,
            UbfFieldType::Ubf => raw::BFLD_UBF as c_int,
            UbfFieldType::View => raw::BFLD_VIEW as c_int,
        }
    }

    #[inline]
    pub(crate) fn from_raw(raw_type: c_int) -> Option<Self> {
        match raw_type as u32 {
            raw::BFLD_SHORT => Some(UbfFieldType::Short),
            raw::BFLD_LONG => Some(UbfFieldType::Long),
            raw::BFLD_CHAR => Some(UbfFieldType::Char),
            raw::BFLD_FLOAT => Some(UbfFieldType::Float),
            raw::BFLD_DOUBLE => Some(UbfFieldType::Double),
            raw::BFLD_STRING => Some(UbfFieldType::String),
            raw::BFLD_CARRAY => Some(UbfFieldType::Carray),
            raw::BFLD_PTR => Some(UbfFieldType::Ptr),
            raw::BFLD_UBF => Some(UbfFieldType::Ubf),
            raw::BFLD_VIEW => Some(UbfFieldType::View),
            _ => None,
        }
    }
}

impl AtmiCtx {
    #[inline]
    fn ubf_unit_result(&self, rc: c_int) -> UbfResult<()> {
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.ubf_last_error())
        }
    }

    #[inline]
    fn ubf_count_result<T>(&self, rc: T) -> UbfResult<usize>
    where
        T: Into<i64> + Copy,
    {
        let value = rc.into();
        if value < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(value as usize)
        }
    }

    #[inline]
    pub(crate) fn ndrx_bget_ferror_addr(&self) -> *mut c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::ndrx_Bget_Ferror_addr()
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::Ondrx_Bget_Ferror_addr(self.c_ctx_ptr())
        }
    }

    #[inline]
    pub(crate) fn bstrerror(&self, err: c_int) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bstrerror(err)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBstrerror(self.c_ctx_ptr(), err)
        }
    }

    #[inline]
    pub(crate) fn bchg_ubf_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        value: &TypedUbf<'_>,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bchg(
                ubf.as_ubfh(),
                bfldid,
                occ,
                value.as_ubfh() as *mut c_char,
                0,
            )
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBchg(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                value.as_ubfh() as *mut c_char,
                0,
            )
        }
    }

    #[inline]
    pub(crate) fn cbadd_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        buf: *mut c_char,
        len: BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBadd(ubf.as_ubfh(), bfldid, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBadd(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, buf, len, usrtype)
        }
    }

    #[inline]
    pub(crate) fn cbchg_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBchg(ubf.as_ubfh(), bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBchg(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                buf,
                len,
                usrtype,
            )
        }
    }

    #[inline]
    pub(crate) fn cbget_value(
        &self,
        ubf: &TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: &mut BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBget(ubf.as_ubfh(), bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBget(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                buf,
                len,
                usrtype,
            )
        }
    }

    #[inline]
    pub(crate) fn cbget_borrowed_ubf_value(
        &self,
        ubf: &BorrowedUbf<'_, '_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: &mut BFLDLEN,
        usrtype: c_int,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBget(ubf.as_ubfh(), bfldid, occ, buf, len, usrtype)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBget(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                buf,
                len,
                usrtype,
            )
        }
    }

    #[inline]
    pub(crate) fn bfind_value(
        &self,
        ubf: &TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        len: &mut BFLDLEN,
    ) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bfind(ubf.as_ubfh(), bfldid, occ, len)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBfind(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ, len)
        }
    }

    #[inline]
    pub(crate) fn bnext_value(
        &self,
        ubf: &TypedUbf<'_>,
        bfldid: &mut BFLDID,
        occ: &mut BFLDOCC,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bnext(
                ubf.as_ubfh(),
                bfldid,
                occ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBnext(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                occ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        }
    }

    #[inline]
    pub(crate) fn baddfast_value(
        &self,
        ubf: &mut TypedUbf<'_>,
        bfldid: BFLDID,
        buf: *mut c_char,
        len: BFLDLEN,
        usrtype: c_int,
        loc: &mut BFldLocInfo,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::CBaddfast(ubf.as_ubfh(), bfldid, buf, len, usrtype, &mut loc.inner)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OCBaddfast(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                bfldid,
                buf,
                len,
                usrtype,
                &mut loc.inner,
            )
        }
    }

    #[inline]
    pub(crate) fn bget_raw_value(
        &self,
        ubf: &TypedUbf<'_>,
        bfldid: BFLDID,
        occ: BFLDOCC,
        buf: *mut c_char,
        len: &mut BFLDLEN,
    ) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bget(ubf.as_ubfh(), bfldid, occ, buf, len)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBget(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ, buf, len)
        }
    }

    #[inline]
    pub(crate) fn bboolco_value(&self, expr: *mut c_char) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bboolco(expr)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBboolco(self.c_ctx_ptr(), expr)
        }
    }

    #[inline]
    pub(crate) fn bboolev_value(&self, ubf: &TypedUbf<'_>, tree: &UbfExprTree<'_>) -> c_int {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bboolev(ubf.as_ubfh(), tree.as_ptr())
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBboolev(self.c_ctx_ptr(), ubf.as_ubfh(), tree.as_ptr())
        }
    }

    #[inline]
    pub(crate) fn bfloatev_value(&self, ubf: &TypedUbf<'_>, tree: &UbfExprTree<'_>) -> f64 {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bfloatev(ubf.as_ubfh(), tree.as_ptr())
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBfloatev(self.c_ctx_ptr(), ubf.as_ubfh(), tree.as_ptr())
        }
    }

    #[inline]
    pub(crate) fn btreefree_value(&self, tree: *mut c_char) {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Btreefree(tree)
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBtreefree(self.c_ctx_ptr(), tree)
        }
    }

    pub(crate) fn bfprintcb_value(&self, ubf: &TypedUbf<'_>) -> UbfResult<String> {
        let mut state = OutputState { bytes: Vec::new() };

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bfprintcb(
                ubf.as_ubfh(),
                Some(bfprint_output_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBfprintcb(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                Some(bfprint_output_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            return Err(self.ubf_last_error());
        }

        if state.bytes.last().copied() == Some(0) {
            state.bytes.pop();
        }
        String::from_utf8(state.bytes)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEUNIX, e.to_string()))
    }

    pub(crate) fn bwritecb_value(&self, ubf: &TypedUbf<'_>) -> UbfResult<Vec<u8>> {
        let mut state = OutputState { bytes: Vec::new() };

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bwritecb(
                ubf.as_ubfh(),
                Some(write_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBwritecb(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                Some(write_callback),
                &mut state as *mut OutputState as *mut c_void,
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            Err(self.ubf_last_error())
        } else {
            Ok(state.bytes)
        }
    }

    pub(crate) fn breadcb_value(&self, ubf: &mut TypedUbf<'_>, dump: &[u8]) -> UbfResult<()> {
        let mut data = dump.to_vec();
        let mode = CString::new("rb").expect("static mode has no NUL");
        let file =
            unsafe { libc::fmemopen(data.as_mut_ptr() as *mut c_void, data.len(), mode.as_ptr()) };
        if file.is_null() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEUNIX,
                "failed to open memory stream",
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bread(ubf.as_ubfh(), file as *mut raw::FILE) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBread(self.c_ctx_ptr(), ubf.as_ubfh(), file as *mut raw::FILE) };

        unsafe {
            libc::fclose(file);
        }

        self.ubf_unit_result(rc)
    }

    pub(crate) fn bextreadcb_value(&self, ubf: &mut TypedUbf<'_>, text: &str) -> UbfResult<()> {
        if self.bextread_text_rust(ubf, text).is_ok() {
            return Ok(());
        }

        let normalized = self.normalize_bextread_text(text);
        let mut data = CString::new(normalized)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?
            .into_bytes_with_nul();
        let mode = CString::new("r").expect("static mode has no NUL");
        let file =
            unsafe { libc::fmemopen(data.as_mut_ptr() as *mut c_void, data.len(), mode.as_ptr()) };
        if file.is_null() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEUNIX,
                "failed to open memory stream",
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bextread(ubf.as_ubfh(), file as *mut raw::FILE) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBextread(self.c_ctx_ptr(), ubf.as_ubfh(), file as *mut raw::FILE) };

        unsafe {
            libc::fclose(file);
        }

        self.ubf_unit_result(rc)
    }

    fn bextread_text_rust(&self, ubf: &mut TypedUbf<'_>, text: &str) -> UbfResult<()> {
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let (field, value) = line.split_once('\t').ok_or_else(|| {
                crate::UbfError::new(crate::UbfError::BEINVAL, "missing field/value separator")
            })?;
            let bfldid = self.bextread_field_id(field)?;
            match self.bfldtype(bfldid)? {
                UbfFieldType::Short => ubf.badd(
                    bfldid,
                    value.parse::<i16>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::Long => ubf.badd(
                    bfldid,
                    value.parse::<i64>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::Char => {
                    let ch = value.as_bytes().first().copied().unwrap_or_default() as i8;
                    ubf.badd(bfldid, ch, true)?
                }
                UbfFieldType::Float => ubf.badd(
                    bfldid,
                    value.parse::<f32>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::Double => ubf.badd(
                    bfldid,
                    value.parse::<f64>().map_err(|e| {
                        crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string())
                    })?,
                    true,
                )?,
                UbfFieldType::String => ubf.badd(bfldid, value, true)?,
                UbfFieldType::Carray => ubf.badd(bfldid, value.as_bytes().to_vec(), true)?,
                UbfFieldType::Ptr | UbfFieldType::Ubf | UbfFieldType::View => {
                    return Err(crate::UbfError::new(
                        crate::UbfError::BEINVAL,
                        "BExtRead for ptr/ubf/view fields is not supported by Rust parser",
                    ));
                }
            }
        }
        Ok(())
    }

    fn bextread_field_id(&self, field: &str) -> UbfResult<i32> {
        if let Some(id) = field
            .strip_prefix("((BFLDID32)")
            .and_then(|s| s.strip_suffix(')'))
            .and_then(|id| id.parse::<i32>().ok())
        {
            return Ok(id);
        }
        self.bfldid(field)
    }

    fn normalize_bextread_text(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for line in text.lines() {
            if let Some((field, rest)) = line.split_once('\t') {
                let name = field
                    .strip_prefix("((BFLDID32)")
                    .and_then(|s| s.strip_suffix(')'))
                    .and_then(|id| id.parse::<i32>().ok())
                    .and_then(|id| self.bfname(id as BFLDID).ok());
                if let Some(name) = name {
                    out.push_str(&name);
                } else {
                    out.push_str(field);
                }
                out.push('\t');
                out.push_str(rest);
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        out
    }

    /// Return whether two UBF buffers contain the same fields and values.
    pub fn bcmp(&self, ubf1: &TypedUbf<'_>, ubf2: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bcmp(ubf1.as_ubfh(), ubf2.as_ubfh()) == 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBcmp(self.c_ctx_ptr(), ubf1.as_ubfh(), ubf2.as_ubfh()) == 0
        }
    }

    /// Append all fields from `src` into `dst`.
    pub fn bconcat(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bconcat(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBconcat(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Copy the full contents of `src` into `dst`.
    pub fn bcpy(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bcpy(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBcpy(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Delete one occurrence of a field from a UBF buffer.
    pub fn bdel(&self, ubf: &mut TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdel(ubf.as_ubfh(), bfldid, occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdel(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) };

        self.ubf_unit_result(rc)
    }

    /// Delete all occurrences of a field from a UBF buffer.
    pub fn bdelall(&self, ubf: &mut TypedUbf<'_>, bfldid: BFLDID) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdelall(ubf.as_ubfh(), bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdelall(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid) };

        self.ubf_unit_result(rc)
    }

    /// Delete all fields listed in `fldlist` from a UBF buffer.
    ///
    /// A terminating `0` is appended for the C API; callers should pass field
    /// numbers only.
    pub fn bdelete(&self, ubf: &mut TypedUbf<'_>, fldlist: &[i32]) -> UbfResult<()> {
        let mut fields: Vec<BFLDID> = fldlist.iter().copied().map(|f| f as BFLDID).collect();
        fields.push(0);

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bdelete(ubf.as_ubfh(), fields.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBdelete(self.c_ctx_ptr(), ubf.as_ubfh(), fields.as_mut_ptr()) };

        self.ubf_unit_result(rc)
    }

    /// Return the number of index slots used by a UBF buffer.
    pub fn bidxused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bidxused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBidxused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Build or rebuild the UBF index.
    pub fn bindex(&self, ubf: &mut TypedUbf<'_>, occ: BFLDOCC) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bindex(ubf.as_ubfh(), occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBindex(self.c_ctx_ptr(), ubf.as_ubfh(), occ) };

        self.ubf_unit_result(rc)
    }

    /// Return whether a buffer is a valid UBF buffer.
    pub fn bisubf(&self, ubf: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bisubf(ubf.as_ubfh()) != 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBisubf(self.c_ctx_ptr(), ubf.as_ubfh()) != 0
        }
    }

    /// Join fields from `src` into `dest`.
    pub fn bjoin(&self, dest: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bjoin(dest.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBjoin(self.c_ctx_ptr(), dest.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Return the stored length of a field occurrence.
    pub fn blen(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Blen(ubf.as_ubfh(), bfldid, occ) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBlen(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) };

        self.ubf_count_result(rc)
    }

    /// Return the total number of field occurrences in a UBF buffer.
    pub fn bnum(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bnum(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBnum(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Return the number of occurrences for one field.
    pub fn boccur(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Boccur(ubf.as_ubfh(), bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBoccur(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid) };

        self.ubf_count_result(rc)
    }

    /// Outer-join fields from `src` into `dest`.
    pub fn bojoin(&self, dest: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bojoin(dest.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBojoin(self.c_ctx_ptr(), dest.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Return whether a field occurrence is present.
    pub fn bpres(&self, ubf: &TypedUbf<'_>, bfldid: BFLDID, occ: BFLDOCC) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bpres(ubf.as_ubfh(), bfldid, occ) != 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBpres(self.c_ctx_ptr(), ubf.as_ubfh(), bfldid, occ) != 0
        }
    }

    /// Return the UBF type for a field id.
    pub fn bfldtype(&self, bfldid: BFLDID) -> UbfResult<UbfFieldType> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bfldtype(bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBfldtype(self.c_ctx_ptr(), bfldid) };

        if rc < 0 {
            return Err(self.ubf_last_error());
        }

        UbfFieldType::from_raw(rc)
            .ok_or_else(|| crate::UbfError::new(crate::UbfError::BEINVAL, "unknown UBF field type"))
    }

    /// Resolve a field name to its typed field id.
    pub fn bfldid(&self, field_name: &str) -> UbfResult<i32> {
        let name = CString::new(field_name)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        let mut rc = unsafe { raw::Bfldid(name.as_ptr() as *mut c_char) };

        #[cfg(feature = "ctx-send")]
        let mut rc = unsafe { raw::OBfldid(self.c_ctx_ptr(), name.as_ptr() as *mut c_char) };

        if rc <= 0 {
            #[cfg(not(feature = "ctx-send"))]
            unsafe {
                raw::Bflddbload();
                rc = raw::Bflddbid(name.as_ptr() as *mut c_char);
            }

            #[cfg(feature = "ctx-send")]
            unsafe {
                raw::OBflddbload(self.c_ctx_ptr());
                rc = raw::OBflddbid(self.c_ctx_ptr(), name.as_ptr() as *mut c_char);
            }
        }

        if rc <= 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Resolve a typed field id to its field name.
    pub fn bfname(&self, bfldid: BFLDID) -> UbfResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let mut ptr = unsafe { raw::Bfname(bfldid) };

        #[cfg(feature = "ctx-send")]
        let mut ptr = unsafe { raw::OBfname(self.c_ctx_ptr(), bfldid) };

        if ptr.is_null() {
            #[cfg(not(feature = "ctx-send"))]
            unsafe {
                raw::Bflddbload();
                ptr = raw::Bflddbname(bfldid);
            }

            #[cfg(feature = "ctx-send")]
            unsafe {
                raw::OBflddbload(self.c_ctx_ptr());
                ptr = raw::OBflddbname(self.c_ctx_ptr(), bfldid);
            }
        }

        if ptr.is_null() {
            Err(self.ubf_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    /// Return the untyped field number portion of a typed field id.
    pub fn bfldno(&self, bfldid: BFLDID) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bfldno(bfldid) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBfldno(self.c_ctx_ptr(), bfldid) };

        rc as i32
    }

    /// Return the Enduro/X textual field type descriptor.
    pub fn btype(&self, bfldid: BFLDID) -> UbfResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let ptr = unsafe { raw::Btype(bfldid) };

        #[cfg(feature = "ctx-send")]
        let ptr = unsafe { raw::OBtype(self.c_ctx_ptr(), bfldid) };

        if ptr.is_null() {
            Err(self.ubf_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    /// Reinitialize a UBF buffer with a given UBF length.
    pub fn binit(&self, ubf: &mut TypedUbf<'_>, len: usize) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Binit(ubf.as_ubfh(), len as raw::BFLDLEN) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBinit(self.c_ctx_ptr(), ubf.as_ubfh(), len as raw::BFLDLEN) };

        self.ubf_unit_result(rc)
    }

    /// Load UBF field table database from `FLDTBLDIR`/`FIELDTBLS`.
    pub fn bflddbload(&self) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bflddbload() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBflddbload(self.c_ctx_ptr()) };

        self.ubf_unit_result(rc)
    }

    /// Compile a UBF boolean expression.
    pub fn bboolco(&self, expr: &str) -> UbfResult<UbfExprTree<'_>> {
        let expr = CString::new(expr)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;
        let _ = self.bflddbload();
        let ptr = self.bboolco_value(expr.as_ptr() as *mut c_char);

        if ptr.is_null() {
            Err(self.ubf_last_error())
        } else {
            Ok(UbfExprTree { ptr, ctx: self })
        }
    }

    /// Explicitly free a compiled UBF boolean expression tree.
    pub fn btreefree(&self, mut tree: UbfExprTree<'_>) {
        tree.free();
    }

    /// Print a compiled boolean expression tree to a string.
    ///
    /// Wraps the C `Bboolpr`/`OBboolpr` `FILE*` API by capturing its output via
    /// an in-memory stream from `open_memstream(3)`.
    pub fn bboolpr(&self, tree: &UbfExprTree<'_>) -> UbfResult<String> {
        let mut buf_ptr: *mut c_char = std::ptr::null_mut();
        let mut buf_size: libc::size_t = 0;
        let file = unsafe { libc::open_memstream(&mut buf_ptr, &mut buf_size) };
        if file.is_null() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEUNIX,
                "failed to open memory stream",
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bboolpr(tree.as_ptr(), file as *mut raw::FILE);
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBboolpr(self.c_ctx_ptr(), tree.as_ptr(), file as *mut raw::FILE);
        }

        unsafe {
            libc::fclose(file);
        }

        if buf_ptr.is_null() {
            return Ok(String::new());
        }

        let bytes = unsafe { std::slice::from_raw_parts(buf_ptr as *const u8, buf_size) }.to_vec();
        unsafe {
            libc::free(buf_ptr as *mut c_void);
        }

        String::from_utf8(bytes)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEUNIX, e.to_string()))
    }

    /// Register a Rust callback for UBF boolean expression evaluation.
    pub fn bboolsetcbf(&self, funcname: &str, callback: UbfExprCallback) -> UbfResult<()> {
        if funcname.is_empty() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEINVAL,
                "function name is empty",
            ));
        }
        let c_funcname = CString::new(funcname)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bboolsetcbf(
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy),
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBboolsetcbf(
                self.c_ctx_ptr(),
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy),
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            Err(self.ubf_last_error())
        } else {
            expr_callbacks()
                .lock()
                .expect("UBF expression callback registry poisoned")
                .insert(funcname.to_string(), callback);
            Ok(())
        }
    }

    /// Register a Rust callback with one string argument for boolean evaluation.
    pub fn bboolsetcbf2(&self, funcname: &str, callback: UbfExprCallback2) -> UbfResult<()> {
        if funcname.is_empty() {
            return Err(crate::UbfError::new(
                crate::UbfError::BEINVAL,
                "function name is empty",
            ));
        }
        let c_funcname = CString::new(funcname)
            .map_err(|e| crate::UbfError::new(crate::UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bboolsetcbf2(
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy2),
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBboolsetcbf2(
                self.c_ctx_ptr(),
                c_funcname.as_ptr() as *mut c_char,
                Some(expr_callback_proxy2),
            )
        };

        if rc != raw::EXSUCCEED as c_int {
            Err(self.ubf_last_error())
        } else {
            expr_callbacks2()
                .lock()
                .expect("UBF expression callback registry poisoned")
                .insert(funcname.to_string(), callback);
            Ok(())
        }
    }

    /// Project a UBF buffer in place to the fields listed in `fldlist`.
    ///
    /// A terminating `0` is appended for the C API; callers should pass field
    /// numbers only.
    pub fn bproj(&self, ubf: &mut TypedUbf<'_>, fldlist: &[i32]) -> UbfResult<()> {
        let mut fields: Vec<BFLDID> = fldlist.iter().copied().map(|f| f as BFLDID).collect();
        fields.push(0);

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bproj(ubf.as_ubfh(), fields.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBproj(self.c_ctx_ptr(), ubf.as_ubfh(), fields.as_mut_ptr()) };

        self.ubf_unit_result(rc)
    }

    /// Copy a projection of `src` into `dst`.
    pub fn bprojcpy(
        &self,
        dst: &mut TypedUbf<'_>,
        src: &TypedUbf<'_>,
        fldlist: &[i32],
    ) -> UbfResult<()> {
        let mut fields: Vec<BFLDID> = fldlist.iter().copied().map(|f| f as BFLDID).collect();
        fields.push(0);

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bprojcpy(dst.as_ubfh(), src.as_ubfh(), fields.as_mut_ptr()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBprojcpy(
                self.c_ctx_ptr(),
                dst.as_ubfh(),
                src.as_ubfh(),
                fields.as_mut_ptr(),
            )
        };

        self.ubf_unit_result(rc)
    }

    /// Return the allocated size of a UBF buffer in bytes.
    pub fn bsizeof(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bsizeof(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBsizeof(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Return whether `ubf1` is a subset of `ubf2`.
    pub fn bsubset(&self, ubf1: &TypedUbf<'_>, ubf2: &TypedUbf<'_>) -> bool {
        #[cfg(not(feature = "ctx-send"))]
        unsafe {
            raw::Bsubset(ubf1.as_ubfh(), ubf2.as_ubfh()) != 0
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            raw::OBsubset(self.c_ctx_ptr(), ubf1.as_ubfh(), ubf2.as_ubfh()) != 0
        }
    }

    /// Remove the index from a UBF buffer.
    pub fn bunindex(&self, ubf: &mut TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bunindex(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBunindex(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Return the unused byte count in a UBF buffer.
    pub fn bunused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bunused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBunused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Update `dst` with fields from `src`.
    pub fn bupdate(&self, dst: &mut TypedUbf<'_>, src: &TypedUbf<'_>) -> UbfResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bupdate(dst.as_ubfh(), src.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBupdate(self.c_ctx_ptr(), dst.as_ubfh(), src.as_ubfh()) };

        self.ubf_unit_result(rc)
    }

    /// Return the used byte count in a UBF buffer.
    pub fn bused(&self, ubf: &TypedUbf<'_>) -> UbfResult<usize> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bused(ubf.as_ubfh()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBused(self.c_ctx_ptr(), ubf.as_ubfh()) };

        self.ubf_count_result(rc)
    }

    /// Return a typed field id from a field type and field number.
    pub fn bmkfldid_typed(&self, field_type: UbfFieldType, field_no: i32) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bmkfldid(field_type.as_raw(), field_no as BFLDID) };

        #[cfg(feature = "ctx-send")]
        let rc =
            unsafe { raw::OBmkfldid(self.c_ctx_ptr(), field_type.as_raw(), field_no as BFLDID) };

        rc as i32
    }

    /// Return a typed field id from a raw Enduro/X field type and field number.
    pub fn bmkfldid(&self, field_type: i32, field_no: i32) -> UbfResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bmkfldid(field_type as c_int, field_no as BFLDID) };

        #[cfg(feature = "ctx-send")]
        let rc =
            unsafe { raw::OBmkfldid(self.c_ctx_ptr(), field_type as c_int, field_no as BFLDID) };

        if rc < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(rc as i32)
        }
    }
}
