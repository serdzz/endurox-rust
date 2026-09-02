use core::ffi::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};

use crate::{raw, AtmiCtx, AtmiError, TypedBuffer, UbfError, UbfResult};

pub const BVACCESS_NOTNULL: i64 = 0x00000001;
const VIEW_NAME_LEN: usize = 33;
const VIEW_CNAME_LEN: usize = 256;

/// Value that can be written into a VIEW field.
pub enum ViewValue {
    Short(i16),
    Long(i64),
    Int(i64),
    Char(i8),
    Float(f32),
    Double(f64),
    String(String),
    Carray(Vec<u8>),
}

pub trait IntoViewValue {
    fn into_view_value(self) -> ViewValue;
}

impl IntoViewValue for ViewValue {
    fn into_view_value(self) -> ViewValue {
        self
    }
}

impl IntoViewValue for i16 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Short(self)
    }
}

impl IntoViewValue for i64 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Long(self)
    }
}

impl IntoViewValue for isize {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Long(self as i64)
    }
}

impl IntoViewValue for i32 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Int(self as i64)
    }
}

impl IntoViewValue for u64 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Long(self as i64)
    }
}

impl IntoViewValue for usize {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Long(self as i64)
    }
}

impl IntoViewValue for u32 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Long(self as i64)
    }
}

impl IntoViewValue for u16 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Short(self as i16)
    }
}

impl IntoViewValue for u8 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Short(self as i16)
    }
}

impl IntoViewValue for i8 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Char(self)
    }
}

impl IntoViewValue for f32 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Float(self)
    }
}

impl IntoViewValue for f64 {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Double(self)
    }
}

impl IntoViewValue for String {
    fn into_view_value(self) -> ViewValue {
        ViewValue::String(self)
    }
}

impl IntoViewValue for &str {
    fn into_view_value(self) -> ViewValue {
        ViewValue::String(self.to_string())
    }
}

impl IntoViewValue for Vec<u8> {
    fn into_view_value(self) -> ViewValue {
        ViewValue::Carray(self)
    }
}

/// XATMI VIEW-typed buffer.
#[derive(Debug)]
pub struct TypedView<'ctx> {
    view: String,
    inner: TypedBuffer<'ctx>,
}

/// Iterator state for [`TypedView::bvnext`].
#[derive(Debug)]
pub struct BvNextState {
    inner: raw::Bvnext_state_t,
}

impl Default for BvNextState {
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

impl<'ctx> TypedView<'ctx> {
    pub fn from_typed(view: impl Into<String>, buf: TypedBuffer<'ctx>) -> Self {
        Self {
            view: view.into(),
            inner: buf,
        }
    }

    pub fn into_inner(self) -> TypedBuffer<'ctx> {
        self.inner
    }

    #[inline]
    pub fn bvname(&self) -> &str {
        &self.view
    }

    #[inline]
    fn view_cstring(&self) -> UbfResult<CString> {
        CString::new(self.view.as_str())
            .map_err(|e| UbfError::new(UbfError::BEINVAL, e.to_string()))
    }

    #[inline]
    fn cname_cstring(cname: &str) -> UbfResult<CString> {
        CString::new(cname).map_err(|e| UbfError::new(UbfError::BEINVAL, e.to_string()))
    }

    fn cbvget(
        &self,
        cname: &str,
        occ: i32,
        buf: *mut c_char,
        len: &mut raw::BFLDLEN,
        usrtype: c_int,
        flags: i64,
    ) -> UbfResult<()> {
        let view = self.view_cstring()?;
        let cname = Self::cname_cstring(cname)?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::CBvget(
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                occ as raw::BFLDOCC,
                buf,
                len,
                usrtype,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OCBvget(
                self.inner.ctx.c_ctx_ptr(),
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                occ as raw::BFLDOCC,
                buf,
                len,
                usrtype,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.inner.ctx.ubf_last_error())
        }
    }

    pub fn bvget_i16(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<i16> {
        let mut val: i16 = 0;
        let mut len = std::mem::size_of::<i16>() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            &mut val as *mut i16 as *mut c_char,
            &mut len,
            raw::BFLD_SHORT as c_int,
            flags,
        )?;
        Ok(val)
    }

    pub fn bvget_i64(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<i64> {
        let mut val: i64 = 0;
        let mut len = std::mem::size_of::<i64>() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            &mut val as *mut i64 as *mut c_char,
            &mut len,
            raw::BFLD_LONG as c_int,
            flags,
        )?;
        Ok(val)
    }

    pub fn bvget_i32(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<i32> {
        let mut val: i64 = 0;
        let mut len = std::mem::size_of::<i64>() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            &mut val as *mut i64 as *mut c_char,
            &mut len,
            raw::BFLD_INT as c_int,
            flags,
        )?;
        Ok(val as i32)
    }

    pub fn bvget_char(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<i8> {
        let mut val: i8 = 0;
        let mut len = std::mem::size_of::<i8>() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            &mut val as *mut i8 as *mut c_char,
            &mut len,
            raw::BFLD_CHAR as c_int,
            flags,
        )?;
        Ok(val)
    }

    pub fn bvget_f32(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<f32> {
        let mut val: f32 = 0.0;
        let mut len = std::mem::size_of::<f32>() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            &mut val as *mut f32 as *mut c_char,
            &mut len,
            raw::BFLD_FLOAT as c_int,
            flags,
        )?;
        Ok(val)
    }

    pub fn bvget_f64(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<f64> {
        let mut val: f64 = 0.0;
        let mut len = std::mem::size_of::<f64>() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            &mut val as *mut f64 as *mut c_char,
            &mut len,
            raw::BFLD_DOUBLE as c_int,
            flags,
        )?;
        Ok(val)
    }

    pub fn bvget_string(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<String> {
        let mut buf = vec![0u8; raw::NDRX_ATMI_MSG_MAX_SIZE as usize];
        let mut len = buf.len() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            buf.as_mut_ptr() as *mut c_char,
            &mut len,
            raw::BFLD_STRING as c_int,
            flags,
        )?;
        Ok(unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }
            .to_string_lossy()
            .into_owned())
    }

    pub fn bvget_bytes(&self, cname: &str, occ: i32, flags: i64) -> UbfResult<Vec<u8>> {
        let mut buf = vec![0u8; raw::NDRX_ATMI_MSG_MAX_SIZE as usize];
        let mut len = buf.len() as raw::BFLDLEN;
        self.cbvget(
            cname,
            occ,
            buf.as_mut_ptr() as *mut c_char,
            &mut len,
            raw::BFLD_CARRAY as c_int,
            flags,
        )?;
        buf.truncate(len as usize);
        Ok(buf)
    }

    pub fn bvchg(&mut self, cname: &str, occ: i32, value: impl IntoViewValue) -> UbfResult<()> {
        let view = self.view_cstring()?;
        let cname = Self::cname_cstring(cname)?;
        let mut value = value.into_view_value();
        let mut string_storage: Option<CString> = None;
        let mut empty_carray = [0u8; 1];

        let (ptr, len, usrtype) = match &mut value {
            ViewValue::Short(v) => (v as *mut i16 as *mut c_char, 0, raw::BFLD_SHORT),
            ViewValue::Long(v) => (v as *mut i64 as *mut c_char, 0, raw::BFLD_LONG),
            ViewValue::Int(v) => (v as *mut i64 as *mut c_char, 0, raw::BFLD_INT),
            ViewValue::Char(v) => (v as *mut i8 as *mut c_char, 0, raw::BFLD_CHAR),
            ViewValue::Float(v) => (v as *mut f32 as *mut c_char, 0, raw::BFLD_FLOAT),
            ViewValue::Double(v) => (v as *mut f64 as *mut c_char, 0, raw::BFLD_DOUBLE),
            ViewValue::String(v) => {
                let cstr = CString::new(v.as_str())
                    .map_err(|e| UbfError::new(UbfError::BEINVAL, e.to_string()))?;
                let ptr = cstr.as_ptr() as *mut c_char;
                string_storage = Some(cstr);
                (ptr, 0, raw::BFLD_STRING)
            }
            ViewValue::Carray(v) => {
                let ptr = if v.is_empty() {
                    empty_carray.as_mut_ptr() as *mut c_char
                } else {
                    v.as_mut_ptr() as *mut c_char
                };
                (ptr, v.len() as raw::BFLDLEN, raw::BFLD_CARRAY)
            }
        };
        let _ = &string_storage;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::CBvchg(
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                occ as raw::BFLDOCC,
                ptr,
                len,
                usrtype as c_int,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OCBvchg(
                self.inner.ctx.c_ctx_ptr(),
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                occ as raw::BFLDOCC,
                ptr,
                len,
                usrtype as c_int,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.inner.ctx.ubf_last_error())
        }
    }

    pub fn bvoccur(&self, cname: &str) -> UbfResult<(usize, usize, usize, usize, i32)> {
        let view = self.view_cstring()?;
        let cname = Self::cname_cstring(cname)?;
        let mut maxocc: raw::BFLDOCC = 0;
        let mut realocc: raw::BFLDOCC = 0;
        let mut dim_size: c_long = 0;
        let mut fldtype: c_int = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bvoccur(
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                &mut maxocc,
                &mut realocc,
                &mut dim_size,
                &mut fldtype,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBvoccur(
                self.inner.ctx.c_ctx_ptr(),
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                &mut maxocc,
                &mut realocc,
                &mut dim_size,
                &mut fldtype,
            )
        };

        if rc < 0 {
            Err(self.inner.ctx.ubf_last_error())
        } else {
            Ok((
                rc as usize,
                maxocc as usize,
                realocc as usize,
                dim_size as usize,
                fldtype as i32,
            ))
        }
    }

    pub fn bvsizeof(&self) -> UbfResult<usize> {
        self.inner.ctx.bvsizeof(&self.view)
    }

    pub fn bvsetoccur(&mut self, cname: &str, occ: i32) -> UbfResult<()> {
        let view = self.view_cstring()?;
        let cname = Self::cname_cstring(cname)?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bvsetoccur(
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                occ as raw::BFLDOCC,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBvsetoccur(
                self.inner.ctx.c_ctx_ptr(),
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
                cname.as_ptr() as *mut c_char,
                occ as raw::BFLDOCC,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.inner.ctx.ubf_last_error())
        }
    }

    pub fn tpviewtojson(&self, flags: i64) -> Result<String, AtmiError> {
        let view = CString::new(self.view.as_str())
            .map_err(|e| AtmiError::new(raw::TPEINVAL, e.to_string()))?;
        let size = self
            .bvsizeof()
            .unwrap_or(raw::NDRX_ATMI_MSG_MAX_SIZE as usize);
        let mut out = vec![0u8; size.saturating_mul(10).max(1024)];
        self.inner.ctx.tpviewtojson(
            self.inner.as_ptr(),
            view.as_ptr() as *mut c_char,
            out.as_mut_ptr() as *mut c_char,
            out.len() as i32,
            flags,
        )?;
        let end = out.iter().position(|&b| b == 0).unwrap_or(out.len());
        Ok(String::from_utf8_lossy(&out[..end]).into_owned())
    }

    pub fn bvnext(
        &self,
        state: &mut BvNextState,
        start: bool,
    ) -> UbfResult<Option<(String, i32, usize, usize)>> {
        let view = if start {
            Some(self.view_cstring()?)
        } else {
            None
        };
        let view_ptr = view
            .as_ref()
            .map(|s| s.as_ptr() as *mut c_char)
            .unwrap_or(std::ptr::null_mut());
        let mut cname = vec![0u8; VIEW_CNAME_LEN + 1];
        let mut fldtype: c_int = 0;
        let mut maxocc: raw::BFLDOCC = 0;
        let mut dim_size: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bvnext(
                &mut state.inner,
                view_ptr,
                cname.as_mut_ptr() as *mut c_char,
                &mut fldtype,
                &mut maxocc,
                &mut dim_size,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBvnext(
                self.inner.ctx.c_ctx_ptr(),
                &mut state.inner,
                view_ptr,
                cname.as_mut_ptr() as *mut c_char,
                &mut fldtype,
                &mut maxocc,
                &mut dim_size,
            )
        };

        match rc {
            1 => Ok(Some((
                unsafe { CStr::from_ptr(cname.as_ptr() as *const c_char) }
                    .to_string_lossy()
                    .into_owned(),
                fldtype as i32,
                maxocc as usize,
                dim_size as usize,
            ))),
            0 => Ok(None),
            _ => Err(self.inner.ctx.ubf_last_error()),
        }
    }

    pub fn bvcpy(&self, dst: &mut TypedView<'_>) -> UbfResult<usize> {
        let view = self.view_cstring()?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::Bvcpy(
                dst.inner.as_ptr(),
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::OBvcpy(
                self.inner.ctx.c_ctx_ptr(),
                dst.inner.as_ptr(),
                self.inner.as_ptr(),
                view.as_ptr() as *mut c_char,
            )
        };

        if rc < 0 {
            Err(self.inner.ctx.ubf_last_error())
        } else {
            Ok(rc as usize)
        }
    }

    pub fn tprealloc(&mut self, size: usize) -> Result<(), AtmiError> {
        self.inner.tprealloc(size)
    }
}

impl AtmiCtx {
    pub fn tpalloc_view<'ctx>(
        &'ctx self,
        view: &str,
        size: usize,
    ) -> Result<TypedView<'ctx>, AtmiError> {
        let buf = self.tpalloc("VIEW", view, size)?;
        Ok(TypedView::from_typed(view, buf))
    }

    pub fn bvsizeof(&self, view: &str) -> UbfResult<usize> {
        let view =
            CString::new(view).map_err(|e| UbfError::new(UbfError::BEINVAL, e.to_string()))?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::Bvsizeof(view.as_ptr() as *mut c_char) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::OBvsizeof(self.c_ctx_ptr(), view.as_ptr() as *mut c_char) };

        if rc < 0 {
            Err(self.ubf_last_error())
        } else {
            Ok(rc as usize)
        }
    }

    pub fn tpjson_to_view<'ctx>(&'ctx self, json: &str) -> Result<TypedView<'ctx>, AtmiError> {
        let json = CString::new(json).map_err(|e| AtmiError::new(raw::TPEINVAL, e.to_string()))?;
        let mut view = vec![0u8; VIEW_NAME_LEN + 1];
        let raw = unsafe {
            self.tpjsontoview(
                view.as_mut_ptr() as *mut c_char,
                json.as_ptr() as *mut c_char,
            )?
        };
        let view = unsafe { CStr::from_ptr(view.as_ptr() as *const c_char) }
            .to_string_lossy()
            .into_owned();
        let buf = unsafe { TypedBuffer::from_raw(self, raw) };
        Ok(TypedView::from_typed(view, buf))
    }
}
