use crate::{raw, AtmiCtx, AtmiResult};
use core::ffi::{c_char, c_long};
use std::ffi::CStr;

/// Result of [`TypedBuffer::tptypes`].
///
/// Mirrors the C `tptypes(3)` outputs: the buffer's reported allocation size,
/// the type name (e.g. `UBF`, `CARRAY`, `STRING`, `JSON`, `VIEW`), and the
/// subtype (e.g. the VIEW name). For types that have no subtype the
/// `subtype` field is an empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpTypeInfo {
    /// Buffer allocation size in bytes, as reported by `tptypes(3)`.
    pub size: usize,
    /// Buffer type name (e.g. `"UBF"`, `"CARRAY"`, `"STRING"`).
    pub type_name: String,
    /// Buffer subtype (e.g. VIEW name); empty when the buffer has no subtype.
    pub subtype: String,
}

/// Owned XATMI typed buffer allocated by `tpalloc`.
///
/// The buffer is tied to the [`AtmiCtx`] that allocated it and is released with
/// `tpfree` when dropped.
///
/// `len` carries the user data length used as `ilen`/`olen` for XATMI calls
/// (`tpcall`, `tpacall`, `tpsend`, `tpreturn`, ...). It is meaningful for
/// length-tracked buffer types (CARRAY, STRING) and may be left at `0` for
/// self-describing types (UBF, VIEW, JSON), where Enduro/X derives the length
/// from the buffer header.
#[derive(Debug)]
pub struct TypedBuffer<'ctx> {
    ptr: *mut c_char,
    pub(crate) ctx: &'ctx AtmiCtx,
    owned: bool,
    len: usize,
}

impl<'ctx> TypedBuffer<'ctx> {
    /// # Safety
    /// `raw` must be a valid `atmibuf*` allocated for this context and owned by the caller.
    pub(crate) unsafe fn from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: true,
            len: 0,
        }
    }

    /// # Safety
    /// `raw` must be a valid `atmibuf*` allocated for this context and owned by the caller.
    pub(crate) unsafe fn from_raw_with_len(
        ctx: &'ctx AtmiCtx,
        raw: *mut c_char,
        len: usize,
    ) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: true,
            len,
        }
    }

    /// # Safety
    /// `raw` must be a valid `atmibuf*` owned by the caller for at least `'ctx`.
    pub(crate) unsafe fn borrowed_from_raw(ctx: &'ctx AtmiCtx, raw: *mut c_char) -> Self {
        Self {
            ptr: raw,
            ctx,
            owned: false,
            len: 0,
        }
    }

    /// Transfer ownership of the underlying ATMI buffer pointer.
    ///
    /// The returned pointer will not be freed by this Rust value. Use this only
    /// when passing ownership to Enduro/X or immediately wrapping it in another
    /// owner.
    pub(crate) fn into_raw(self) -> *mut c_char {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }

    /// Return the current ATMI buffer pointer without transferring ownership.
    ///
    /// This is intended for low-level integration with APIs that are not yet
    /// represented by a safe Rust wrapper.
    #[inline]
    pub(crate) fn as_ptr(&self) -> *mut c_char {
        self.ptr
    }

    /// # Safety
    /// Retie this buffer to a *different* context.
    ///
    /// Only valid if the underlying ATMI/UBF API actually allows this buffer
    /// to be used under `new_ctx`. The lifetime re-tie is unchecked by Rust.
    pub(crate) unsafe fn move_to_context<'new>(self, new_ctx: &'new AtmiCtx) -> TypedBuffer<'new> {
        let len = self.len;
        TypedBuffer::from_raw_with_len(new_ctx, self.into_raw(), len)
    }

    /// Update the internal pointer after a C API may have reallocated the buffer.
    #[inline]
    pub(crate) fn replace_ptr(&mut self, new_ptr: *mut c_char) {
        self.ptr = new_ptr;
    }

    /// Current user data length in bytes used as `ilen`/`olen` for XATMI calls.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if [`Self::len`] is zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Set the user data length in bytes used as `ilen` for XATMI calls.
    ///
    /// Primarily for length-tracked buffer types (CARRAY, STRING). Self-describing
    /// buffers (UBF, VIEW) can leave this as `0`.
    #[inline]
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
    }

    /// View the user-data portion of the buffer as bytes (length tracked by `len()`).
    pub fn as_bytes(&self) -> &[u8] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
        }
    }

    /// Mutable byte view of the user-data portion of the buffer.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        if self.ptr.is_null() || self.len == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.ptr as *mut u8, self.len) }
        }
    }

    /// Copy `bytes` into the buffer, growing it via [`Self::tprealloc`] if needed,
    /// and update `len()` to `bytes.len()`.
    pub fn set_bytes(&mut self, bytes: &[u8]) -> AtmiResult<()> {
        let info = self.tptypes()?;
        if bytes.len() > info.size {
            self.tprealloc(bytes.len())?;
        }
        if !bytes.is_empty() {
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr as *mut u8, bytes.len());
            }
        }
        self.len = bytes.len();
        Ok(())
    }

    /// Query the buffer via `tptypes(3)`: returns size plus type and subtype.
    pub fn tptypes(&self) -> AtmiResult<TpTypeInfo> {
        let mut type_buf = [0i8; raw::XATMI_TYPE_LEN as usize];
        let mut subtype_buf = [0i8; raw::XATMI_SUBTYPE_LEN as usize];

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tptypes(
                self.ptr,
                type_buf.as_mut_ptr() as *mut c_char,
                subtype_buf.as_mut_ptr() as *mut c_char,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otptypes(
                self.ctx.c_ctx_ptr(),
                self.ptr,
                type_buf.as_mut_ptr() as *mut c_char,
                subtype_buf.as_mut_ptr() as *mut c_char,
            )
        };

        if rc < 0 {
            Err(self.ctx.atmi_last_error())
        } else {
            let type_name = unsafe { CStr::from_ptr(type_buf.as_ptr() as *const c_char) }
                .to_string_lossy()
                .into_owned();
            let subtype = unsafe { CStr::from_ptr(subtype_buf.as_ptr() as *const c_char) }
                .to_string_lossy()
                .into_owned();
            Ok(TpTypeInfo {
                size: rc as usize,
                type_name,
                subtype,
            })
        }
    }

    /// Reallocate this buffer with a new size using `tprealloc`.
    ///
    /// On success, `self` will point to the new buffer.
    /// On failure, `self` remains valid and unchanged, and the error is returned.
    pub fn tprealloc(&mut self, new_size: usize) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let new_ptr = unsafe { raw::tprealloc(self.ptr as *mut c_char, new_size as c_long) };

        #[cfg(feature = "ctx-send")]
        let new_ptr =
            unsafe { raw::Otprealloc(self.ctx.c_ctx_ptr(), self.ptr, new_size as c_long) };

        if new_ptr.is_null() {
            Err(self.ctx.atmi_last_error())
        } else {
            self.ptr = new_ptr;
            Ok(())
        }
    }
}

impl<'ctx> Drop for TypedBuffer<'ctx> {
    fn drop(&mut self) {
        if self.owned && !self.ptr.is_null() {
            #[cfg(not(feature = "ctx-send"))]
            unsafe {
                raw::tpfree(self.ptr)
            }

            #[cfg(feature = "ctx-send")]
            unsafe {
                raw::Otpfree(self.ctx.c_ctx_ptr(), self.ptr)
            }
        }
    }
}
