use crate::{raw, AtmiError, AtmiResult, NstdError, TypedBuffer, TypedUbf, UbfError};
#[cfg(not(feature = "ctx-send"))]
use core::ffi::c_char;
use core::ffi::{c_int, c_long};

#[cfg(feature = "ctx-send")]
use std::cell::Cell;
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ptr,
};

// --- Marker selection -------------------------------------------------------
#[cfg(not(feature = "ctx-send"))]
type CtxMarker = std::rc::Rc<()>; // -> !Send & !Sync

#[cfg(feature = "ctx-send")]
type CtxMarker = Cell<()>; // -> Send & !Sync

#[cfg(feature = "ctx-send")]
type CtxHandle = raw::TPCONTEXT_T;

/// Per-thread XATMI context.
///
/// By default the context is neither `Send` nor `Sync`. With the `ctx-send`
/// feature enabled it becomes `Send`, but remains `!Sync`.
#[derive(Debug)]
pub struct AtmiCtx {
    _marker: PhantomData<CtxMarker>,
    // Service callbacks borrow libatmisrv's worker context. Such a view must
    // neither terminate nor free the worker TLS when it goes out of scope.
    borrowed: bool,

    #[cfg(feature = "ctx-send")]
    handle: Cell<CtxHandle>,
}

// SAFETY: `ctx-send` creates the handle detached (`tpnewctxt(0, 0)`). Operations
// that access this stored handle use Enduro/X's Object API, which attaches it
// only for that call and detaches it before returning. `Cell` deliberately
// keeps `AtmiCtx` !Sync, so the handle cannot be used concurrently. Buffers
// borrow their context and therefore also prevent moving it while they exist.
#[cfg(feature = "ctx-send")]
unsafe impl Send for AtmiCtx {}

impl AtmiCtx {
    /// Create a new ATMI context handle.
    pub fn new() -> Result<Self, AtmiError> {
        #[cfg(not(feature = "ctx-send"))]
        {
            Ok(AtmiCtx {
                _marker: PhantomData,
                borrowed: false,
            })
        }

        #[cfg(feature = "ctx-send")]
        {
            unsafe {
                let handle = raw::tpnewctxt(0, 0);

                if handle.is_null() {
                    return Err(AtmiError::new(
                        raw::TPESYSTEM,
                        "failed to allocate ATMI context",
                    ));
                }

                Ok(AtmiCtx {
                    _marker: PhantomData,
                    borrowed: false,
                    handle: Cell::new(handle),
                })
            }
        }
    }

    /// Join the application as a client by calling `tpinit`.
    pub fn tpinit(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpinit(ptr::null_mut()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpinit(self.c_ctx_ptr(), ptr::null_mut()) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Leave the application by calling `tpterm`.
    pub fn tpterm(&self) -> AtmiResult<()> {
        if self.borrowed {
            return Err(AtmiError::new(
                raw::TPEPROTO,
                "cannot terminate a borrowed libatmisrv worker context",
            ));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpterm() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpterm(self.c_ctx_ptr()) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Return last ATMI error for the current thread/context.
    pub fn atmi_last_error(&self) -> AtmiError {
        unsafe {
            #[cfg(not(feature = "ctx-send"))]
            let err_ptr = raw::_exget_tperrno_addr();

            #[cfg(feature = "ctx-send")]
            let err_ptr = raw::O_exget_tperrno_addr(self.c_ctx_ptr());

            let code = *err_ptr;

            #[cfg(not(feature = "ctx-send"))]
            let msg_ptr = raw::tpstrerror(code);

            #[cfg(feature = "ctx-send")]
            let msg_ptr = raw::Otpstrerror(self.c_ctx_ptr(), code);

            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            AtmiError::new(code as u32, message)
        }
    }

    /// Return last UBF error for the current thread/context.
    pub fn ubf_last_error(&self) -> UbfError {
        unsafe {
            let err_ptr = self.ndrx_bget_ferror_addr();
            let code = *err_ptr;
            let msg_ptr = self.bstrerror(code);
            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            UbfError::new(code as u32, message)
        }
    }

    /// Return the last NSTD error for the current thread/context.
    pub fn nstd_last_error(&self) -> NstdError {
        unsafe {
            let err_ptr = raw::_Nget_Nerror_addr(); // *const i32 or *mut i32
            let code = *err_ptr;
            let msg_ptr = raw::Nstrerror(code); // *const c_char
            let message = CStr::from_ptr(msg_ptr).to_string_lossy().into_owned();
            NstdError::new(code as u32, message)
        }
    }

    /// Allocate a typed XATMI buffer tied to this context.
    pub fn tpalloc<'ctx>(
        &'ctx self,
        type_: &str,
        subtype: &str,
        size: usize,
    ) -> AtmiResult<TypedBuffer<'ctx>> {
        let type_c = CString::new(type_)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "type_ contains NUL byte"))?;
        let subtype_c = CString::new(subtype)
            .map_err(|_| AtmiError::new(raw::TPEINVAL, "subtype contains NUL byte"))?;

        #[cfg(not(feature = "ctx-send"))]
        let ptr = unsafe {
            raw::tpalloc(
                type_c.as_ptr() as *mut c_char,
                subtype_c.as_ptr() as *mut c_char,
                size as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let ptr = unsafe {
            raw::Otpalloc(
                self.c_ctx_ptr(),
                type_c.as_ptr(),
                subtype_c.as_ptr(),
                size as c_long,
            )
        };

        if ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            let buf = unsafe { TypedBuffer::from_raw(self, ptr) };
            Ok(buf)
        }
    }

    /// Allocate a CARRAY (binary array) buffer tied to this context, copy the
    /// provided bytes in, and set `len()` to `bytes.len()`.
    pub fn tpalloc_carray<'ctx>(&'ctx self, bytes: &[u8]) -> AtmiResult<TypedBuffer<'ctx>> {
        let size = bytes.len().max(1);
        let mut buf = self.tpalloc("CARRAY", "", size)?;
        if !bytes.is_empty() {
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_ptr() as *mut u8, bytes.len());
            }
        }
        buf.set_len(bytes.len());
        Ok(buf)
    }

    /// Allocate a UBF buffer tied to this context.
    pub fn tpalloc_ubf<'ctx>(&'ctx self, size: usize) -> AtmiResult<TypedUbf<'ctx>> {
        let type_c = CString::new("UBF").unwrap();
        let subtype_c = CString::new("").unwrap();

        #[cfg(not(feature = "ctx-send"))]
        let raw_ptr = unsafe {
            raw::tpalloc(
                type_c.as_ptr() as *mut c_char,
                subtype_c.as_ptr() as *mut c_char,
                size as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let raw_ptr = unsafe {
            raw::Otpalloc(
                self.c_ctx_ptr(),
                type_c.as_ptr(),
                subtype_c.as_ptr(),
                size as c_long,
            )
        };

        if raw_ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            let ubf = unsafe { TypedUbf::from_raw(self, raw_ptr) };
            Ok(ubf)
        }
    }

    /*
    fn ubf_last_error() -> AtmiError { ... }
    fn nstd_last_error() -> AtmiError { ... }
    */

    #[cfg(feature = "ctx-send")]
    #[inline]
    pub(crate) fn c_ctx_ptr(&self) -> *mut raw::TPCONTEXT_T {
        self.handle.as_ptr()
    }

    /// Create a callback-scoped view of the current libatmisrv worker context.
    ///
    /// With `ctx-send`, the worker TLS is detached into an Object API handle
    /// and restored when this value is dropped. Without `ctx-send`, calls use
    /// the worker's current TLS directly and Drop is a no-op.
    ///
    /// # Safety
    ///
    /// The current thread must be inside a libatmisrv service callback, and the
    /// returned value must not outlive that callback.
    pub(crate) unsafe fn borrow_current_worker() -> AtmiResult<Self> {
        #[cfg(not(feature = "ctx-send"))]
        {
            Ok(Self {
                _marker: PhantomData,
                borrowed: true,
            })
        }

        #[cfg(feature = "ctx-send")]
        {
            let mut handle: raw::TPCONTEXT_T = ptr::null_mut();
            let rc = raw::tpgetctxt(&mut handle, 0);
            if rc == raw::TPMULTICONTEXTS as c_int && !handle.is_null() {
                Ok(Self {
                    _marker: PhantomData,
                    borrowed: true,
                    handle: Cell::new(handle),
                })
            } else if rc == raw::EXFAIL as c_int {
                Err(Self::current_thread_atmi_error())
            } else {
                Err(AtmiError::new(
                    raw::TPEPROTO,
                    "libatmisrv worker callback has no active ATMI context",
                ))
            }
        }
    }

    #[cfg(feature = "ctx-send")]
    unsafe fn current_thread_atmi_error() -> AtmiError {
        let code = *raw::_exget_tperrno_addr();
        let message = CStr::from_ptr(raw::tpstrerror(code))
            .to_string_lossy()
            .into_owned();
        AtmiError::new(code as u32, message)
    }
}

impl Drop for AtmiCtx {
    fn drop(&mut self) {
        #[cfg(not(feature = "ctx-send"))]
        if !self.borrowed {
            unsafe {
                raw::tpterm();
            }
        }

        #[cfg(feature = "ctx-send")]
        unsafe {
            let handle = self.handle.get();
            if !handle.is_null() {
                if self.borrowed {
                    // Restore libatmisrv's worker TLS before returning through
                    // the C dispatcher. The worker owns and terminates it.
                    let _ = raw::tpsetctxt(handle, 0);
                } else {
                    // tpfreectxt only runs tpterm automatically for a context
                    // currently attached to this thread. Object API contexts
                    // are detached between calls, so terminate first.
                    let _ = raw::Otpterm(self.handle.as_ptr());
                    raw::tpfreectxt(handle);
                }
                self.handle.set(ptr::null_mut());
            }
        }
    }
}
