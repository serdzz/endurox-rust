use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TpContext, TpTranId, TypedBuffer, TypedUbf};
use core::ffi::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::ptr;
use std::time::{Duration, Instant};

#[cfg(endurox_pollable)]
struct PendingCall<'ctx> {
    ctx: &'ctx AtmiCtx,
    cd: i32,
    armed: bool,
}

#[cfg(endurox_pollable)]
impl<'ctx> PendingCall<'ctx> {
    fn new(ctx: &'ctx AtmiCtx, cd: i32) -> Self {
        Self {
            ctx,
            cd,
            armed: true,
        }
    }

    fn complete(&mut self) {
        self.armed = false;
    }
}

#[cfg(endurox_pollable)]
impl Drop for PendingCall<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.ctx.tpcancel(self.cd);
        }
    }
}

impl AtmiCtx {
    /// Synchronous RPC call with separate input and output buffers.
    ///
    /// This mirrors the C API: `idata` is the request buffer, and `odata` is the
    /// reply buffer. Enduro/X may reallocate `odata`; on success this wrapper is
    /// updated to the returned pointer.
    pub fn tpcall(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;
        let ilen = idata.len() as c_long;
        let mut reply = odata.as_ptr();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpcall(
                c_svc.as_ptr() as *mut c_char,
                idata.as_ptr(),
                ilen,
                &mut reply,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpcall(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                idata.as_ptr(),
                ilen,
                &mut reply,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            odata.replace_ptr(reply);
            odata.set_len(olen as usize);
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Asynchronous RPC call.  Returns a call descriptor used with `tpgetrply`.
    pub fn tpacall(&self, svc: &str, data: &TypedBuffer<'_>, flags: i64) -> AtmiResult<i32> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;
        let ilen = data.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpacall(
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                ilen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpacall(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                ilen,
                flags as c_long,
            )
        };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    /// Retrieve the reply for a previous `tpacall`.
    ///
    /// `cd` is updated by the framework when `TPGETANY` is used.
    pub fn tpgetrply(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let mut c_cd = *cd as c_int;
        let mut odata = data.as_ptr();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetrply(&mut c_cd, &mut odata, &mut olen, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpgetrply(
                self.c_ctx_ptr(),
                &mut c_cd,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        // Adopt the descriptor and buffer on *every* path, not just on success.
        // `ndrx_tpgetrply` assigns `*cd = rply->cd` and runs
        // `ndrx_mbuf_prepare_incoming(.., char **odata, long *olen, ..)` -- which
        // may reallocate -- before it raises TPESVCFAIL or TPETIME. Keeping the
        // old pointer on those paths leaves it dangling and leaks the
        // replacement, and drops the descriptor that says which call failed.
        *cd = c_cd as i32;
        data.replace_ptr(odata);
        data.set_len(olen.max(0) as usize);

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Cancel a pending asynchronous call descriptor returned by `tpacall`.
    pub fn tpcancel(&self, cd: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcancel(cd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcancel(self.c_ctx_ptr(), cd as c_int) };

        self.rc_to_result(rc)
    }

    /// Deadline for one reply wait, taken from Enduro/X's own effective call
    /// timeout rather than from a caller-supplied duration.
    ///
    /// `tpgblktime(0)` resolves `tpsblktime(TPBLK_NEXT)`, then `tpsblktime`/
    /// `tptoutset` thread settings, then `NDRX_TOUT`. Read it **before**
    /// `tpacall`: a `TPBLK_NEXT` setting is one-shot and the call consumes it.
    ///
    /// The deadline matters beyond reporting `TPETIME`. Enduro/X only expires
    /// call descriptors inside `tpgetrply` (`call_scan_tout`), so a poll loop
    /// with no timer would never wake to let that run and would wait forever.
    pub(crate) fn reply_deadline(&self) -> AtmiResult<Option<Instant>> {
        let secs = self.tpgblktime(0)?;
        if secs <= 0 {
            return Ok(None);
        }
        Instant::now()
            .checked_add(Duration::from_secs(secs as u64))
            .ok_or_else(|| {
                AtmiError::new(raw::TPEINVAL, "Enduro/X call timeout exceeds Instant range")
            })
            .map(Some)
    }

    /// Synchronous call that uses the async/reply-queue path only on pollable
    /// Enduro/X builds.
    ///
    /// On `EX_USE_EPOLL` and `EX_USE_KQUEUE` builds this performs `tpacall`,
    /// waits for readiness on the internal reply queue descriptor, then drains
    /// the requested call descriptor with `tpgetrply(TPNOBLOCK)`. Other queue
    /// backends are not externally pollable, so this falls back to the normal
    /// blocking `tpcall` path (`Otpcall` when `ctx-send` is enabled).
    ///
    /// Timeouts come from `NDRX_TOUT` / `tptoutset` / `tpsblktime`, exactly as
    /// for [`AtmiCtx::tpcall`]. There is no per-call timeout argument.
    pub fn tpcall_polled(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(endurox_pollable))]
        {
            self.tpcall(svc, idata, odata, flags)
        }

        #[cfg(endurox_pollable)]
        {
            self.tpcall_pollable(svc, idata, odata, flags)
        }
    }

    #[cfg(endurox_pollable)]
    fn tpcall_pollable(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        if flags & raw::TPNOREPLY as i64 != 0 {
            // The public tpcall() rejects this (libatmi/atmi.c:330), so the
            // polled variant must too rather than silently returning without a
            // reply. tpacall would hand back descriptor 0, which can never be
            // collected.
            return Err(AtmiError::new(
                raw::TPEINVAL,
                "TPNOREPLY cannot be used with tpcall()",
            ));
        }

        // TPNOTIME disables Enduro/X's own call timeout, so imposing the
        // tpgblktime deadline here would cancel a call the caller asked to wait
        // on indefinitely.
        let deadline = if flags & raw::TPNOTIME as i64 != 0 {
            None
        } else {
            self.reply_deadline()?
        };
        let cd = self.tpacall(svc, idata, flags)?;
        let mut pending = PendingCall::new(self, cd);
        let result = self.tpgetrply_polled(&mut pending.cd, odata, flags, deadline);
        if result.is_ok() {
            pending.complete();
        }
        result
    }

    #[cfg(endurox_pollable)]
    fn tpgetrply_polled(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
        deadline: Option<Instant>,
    ) -> AtmiResult<()> {
        let reply_fd = self.reply_queue_fd()?;
        let get_flags = flags | raw::TPNOBLOCK as i64;

        loop {
            match self.tpgetrply(cd, data, get_flags) {
                Ok(()) => return Ok(()),
                Err(err) if err.code == raw::TPEBLOCK => {}
                Err(err) => return Err(err),
            }

            if !self.poll_reply_queue(reply_fd, deadline)? {
                // The timer fired. Give Enduro/X one more chance to expire the
                // descriptor itself so the caller sees its bookkeeping, and only
                // synthesize TPETIME if it still reports nothing.
                match self.tpgetrply(cd, data, get_flags) {
                    Ok(()) => return Ok(()),
                    Err(err) if err.code == raw::TPEBLOCK => {
                        return Err(AtmiError::new(raw::TPETIME, "polled tpcall timed out"))
                    }
                    Err(err) => return Err(err),
                }
            }
        }
    }

    pub(crate) fn reply_queue_fd(&self) -> AtmiResult<c_int> {
        #[cfg(not(endurox_pollable))]
        {
            Err(AtmiError::new(
                raw::TPEINVAL,
                "async integration requires an Enduro/X EX_USE_EPOLL or EX_USE_KQUEUE build",
            ))
        }

        #[cfg(endurox_pollable)]
        {
            #[cfg(not(feature = "ctx-send"))]
            let reply_fd = unsafe { raw::tpext_getreplyqfd() };

            #[cfg(feature = "ctx-send")]
            let reply_fd = unsafe { raw::Otpext_getreplyqfd(self.c_ctx_ptr()) };

            if reply_fd < 0 {
                Err(self.atmi_last_error())
            } else {
                Ok(reply_fd)
            }
        }
    }

    #[cfg(endurox_pollable)]
    fn poll_reply_queue(&self, reply_fd: c_int, deadline: Option<Instant>) -> AtmiResult<bool> {
        let mut pfd = libc::pollfd {
            fd: reply_fd,
            events: libc::POLLIN,
            revents: 0,
        };

        loop {
            let timeout_ms = match deadline {
                Some(d) => d
                    .checked_duration_since(Instant::now())
                    .map(|remaining| {
                        let millis = remaining.as_millis();
                        if remaining.is_zero() {
                            0
                        } else {
                            millis.max(1).min(c_int::MAX as u128) as c_int
                        }
                    })
                    .unwrap_or(0),
                None => -1,
            };
            pfd.revents = 0;
            let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if rc > 0 {
                if (pfd.revents & libc::POLLIN) != 0 {
                    return Ok(true);
                }
                let error_events = libc::POLLERR | libc::POLLHUP | libc::POLLNVAL;
                if (pfd.revents & error_events) != 0 {
                    return Err(AtmiError::new(
                        raw::TPEOS,
                        format!(
                            "poll on Enduro/X reply queue returned events {:#x}",
                            pfd.revents
                        ),
                    ));
                }
                continue;
            }
            if rc == 0 {
                return Ok(false);
            }
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(AtmiError::new(
                raw::TPEOS,
                format!("poll on Enduro/X reply queue failed: {err}"),
            ));
        }
    }

    pub fn tpconnect(
        &self,
        svc: &str,
        data: &TypedBuffer<'_>,
        len: usize,
        flags: i64,
    ) -> AtmiResult<i32> {
        let c_svc = CString::new(svc).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpconnect(
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpconnect(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
            )
        };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    pub fn tpdiscon(&self, cd: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpdiscon(cd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpdiscon(self.c_ctx_ptr(), cd as c_int) };

        self.rc_to_result(rc)
    }

    pub fn tprecv(
        &self,
        cd: i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<(usize, i64)> {
        let mut odata = data.as_ptr();
        let mut olen: c_long = 0;
        let mut revent: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tprecv(
                cd as c_int,
                &mut odata,
                &mut olen,
                flags as c_long,
                &mut revent,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otprecv(
                self.c_ctx_ptr(),
                cd as c_int,
                &mut odata,
                &mut olen,
                flags as c_long,
                &mut revent,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            data.replace_ptr(odata);
            Ok((olen as usize, revent as i64))
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpsend(
        &self,
        cd: i32,
        data: &TypedBuffer<'_>,
        len: usize,
        flags: i64,
    ) -> AtmiResult<i64> {
        let mut revent: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpsend(
                cd as c_int,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
                &mut revent,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpsend(
                self.c_ctx_ptr(),
                cd as c_int,
                data.as_ptr(),
                len as c_long,
                flags as c_long,
                &mut revent,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(revent as i64)
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpabort(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpabort(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpabort(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpscmt(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpscmt(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpscmt(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpbegin(&self, timeout: u64, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpbegin(timeout as _, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpbegin(self.c_ctx_ptr(), timeout as _, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpcommit(&self, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpcommit(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpcommit(self.c_ctx_ptr(), flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Suspend the current global transaction. Returns a `TpTranId` that can
    /// later be passed to `tpresume` to rejoin the transaction.
    pub fn tpsuspend(&self, flags: i64) -> AtmiResult<TpTranId> {
        let mut tranid: raw::TPTRANID = unsafe { std::mem::zeroed() };

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsuspend(&mut tranid, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsuspend(self.c_ctx_ptr(), &mut tranid, flags as c_long) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(TpTranId(tranid))
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Resume a previously suspended global transaction.
    pub fn tpresume(&self, tranid: &TpTranId, flags: i64) -> AtmiResult<()> {
        let mut inner = tranid.0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpresume(&mut inner, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpresume(self.c_ctx_ptr(), &mut inner, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Open the XA resource manager associated with this context.
    pub fn tpopen(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpopen() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpopen(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Close the XA resource manager associated with this context.
    pub fn tpclose(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpclose() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpclose(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    pub fn tpgetlev(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetlev() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetlev(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    pub fn tperrordetail(&self, flags: i64) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tperrordetail(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otperrordetail(self.c_ctx_ptr(), flags as c_long) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    pub fn tpstrerrordetail(&self, err: i32, flags: i64) -> AtmiResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let ptr = unsafe { raw::tpstrerrordetail(err as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let ptr =
            unsafe { raw::Otpstrerrordetail(self.c_ctx_ptr(), err as c_int, flags as c_long) };

        if ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    pub fn tpecodestr(&self, err: i32) -> AtmiResult<String> {
        #[cfg(not(feature = "ctx-send"))]
        let ptr = unsafe { raw::tpecodestr(err as c_int) };

        #[cfg(feature = "ctx-send")]
        let ptr = unsafe { raw::Otpecodestr(self.c_ctx_ptr(), err as c_int) };

        if ptr.is_null() {
            Err(self.atmi_last_error())
        } else {
            Ok(unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned())
        }
    }

    pub fn tpgetnodeid(&self) -> AtmiResult<i64> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetnodeid() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetnodeid(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_long {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i64)
        }
    }

    pub fn tpsubscribe(
        &self,
        eventexpr: &str,
        filter: Option<&str>,
        ctl: Option<&mut crate::TpEvCtl>,
        flags: i64,
    ) -> AtmiResult<i64> {
        let c_expr = CString::new(eventexpr).map_err(|_| self.atmi_last_error())?;
        let c_filter = filter
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;
        let filter_ptr = c_filter
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());
        let ctl_ptr = ctl.map_or(ptr::null_mut(), |ctl| ctl.as_mut_ptr());

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpsubscribe(
                c_expr.as_ptr() as *mut c_char,
                filter_ptr,
                ctl_ptr,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpsubscribe(
                self.c_ctx_ptr(),
                c_expr.as_ptr() as *mut c_char,
                filter_ptr,
                ctl_ptr,
                flags as c_long,
            )
        };

        if rc == raw::EXFAIL as c_long {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i64)
        }
    }

    pub fn tpunsubscribe(&self, subscription: i64, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpunsubscribe(subscription as c_long, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpunsubscribe(self.c_ctx_ptr(), subscription as c_long, flags as c_long)
        };

        self.rc_to_result(rc)
    }

    pub fn tppost(&self, eventname: &str, data: &TypedBuffer<'_>, flags: i64) -> AtmiResult<()> {
        let c_event = CString::new(eventname).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tppost(
                c_event.as_ptr() as *mut c_char,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otppost(
                self.c_ctx_ptr(),
                c_event.as_ptr() as *mut c_char,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Initialize an application thread with no authentication (null TPINIT).
    pub fn tpappthrinit(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpappthrinit(ptr::null_mut()) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpappthrinit(self.c_ctx_ptr(), ptr::null_mut()) };

        self.rc_to_result(rc)
    }

    pub fn tpappthrterm(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpappthrterm() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpappthrterm(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    pub fn tpchkauth(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpchkauth() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpchkauth(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    /// Send an unsolicited message to a specific client.
    pub fn tpnotify(
        &self,
        clientid: &mut crate::ClientId,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpnotify(clientid, data.as_ptr(), 0, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpnotify(
                self.c_ctx_ptr(),
                clientid,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Broadcast an unsolicited message to matching clients.
    pub fn tpbroadcast(
        &self,
        lmid: Option<&str>,
        usrname: Option<&str>,
        cltname: Option<&str>,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_lmid = lmid
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;
        let c_usr = usrname
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;
        let c_clt = cltname
            .map(CString::new)
            .transpose()
            .map_err(|_| self.atmi_last_error())?;

        let p_lmid = c_lmid
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());
        let p_usr = c_usr
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());
        let p_clt = c_clt
            .as_ref()
            .map(|v| v.as_ptr() as *mut c_char)
            .unwrap_or(ptr::null_mut());

        #[cfg(not(feature = "ctx-send"))]
        let rc =
            unsafe { raw::tpbroadcast(p_lmid, p_usr, p_clt, data.as_ptr(), 0, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpbroadcast(
                self.c_ctx_ptr(),
                p_lmid,
                p_usr,
                p_clt,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    pub fn tpchkunsol(&self) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpchkunsol() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpchkunsol(self.c_ctx_ptr()) };

        self.rc_to_result(rc)
    }

    pub fn tptoutset(&self, tout: i32) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptoutset(tout as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptoutset(self.c_ctx_ptr(), tout as c_int) };

        self.rc_to_result(rc)
    }

    pub fn tptoutget(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tptoutget() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otptoutget(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    pub fn tpimport<'ctx>(&'ctx self, payload: &[u8], flags: i64) -> AtmiResult<TypedBuffer<'ctx>> {
        let mut obuf: *mut c_char = ptr::null_mut();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpimport(
                payload.as_ptr() as *mut c_char,
                payload.len() as c_long,
                &mut obuf,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpimport(
                self.c_ctx_ptr(),
                payload.as_ptr() as *mut c_char,
                payload.len() as c_long,
                &mut obuf,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            let _ = olen;
            Ok(unsafe { TypedBuffer::from_raw(self, obuf) })
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpexport(&self, ibuf: &TypedBuffer<'_>, flags: i64) -> AtmiResult<Vec<u8>> {
        let mut out = vec![0u8; 65536];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpexport(
                ibuf.as_ptr(),
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpexport(
                self.c_ctx_ptr(),
                ibuf.as_ptr(),
                0,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            out.truncate(olen as usize);
            Ok(out)
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub(crate) unsafe fn tpgetconn(&self) -> *mut ::std::os::raw::c_void {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::tpgetconn()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Otpgetconn(self.c_ctx_ptr())
        }
    }

    pub(crate) fn tpgetcallinfo(
        &self,
        msg: *const c_char,
        cibuf: *mut *mut raw::UBFH,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetcallinfo(msg, cibuf, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetcallinfo(self.c_ctx_ptr(), msg, cibuf, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub(crate) fn tpsetcallinfo(
        &self,
        msg: *const c_char,
        cibuf: *mut raw::UBFH,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsetcallinfo(msg, cibuf, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsetcallinfo(self.c_ctx_ptr(), msg, cibuf, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// Populate a UBF buffer from a JSON string.
    pub fn tpjsontoubf(&self, ubf: &mut TypedUbf<'_>, json: &str) -> AtmiResult<()> {
        use std::ffi::CString;
        let c_json = CString::new(json).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpjsontoubf(ubf.as_ubfh(), c_json.as_ptr() as *mut c_char) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpjsontoubf(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                c_json.as_ptr() as *mut c_char,
            )
        };

        self.rc_to_result(rc)
    }

    /// Serialize a UBF buffer to a JSON string.
    pub fn tpubftojson(&self, ubf: &TypedUbf<'_>) -> AtmiResult<String> {
        // Allocate a reasonably-sized output buffer; grow on first call if needed.
        let mut out = vec![0u8; 65536];

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpubftojson(
                ubf.as_ubfh(),
                out.as_mut_ptr() as *mut c_char,
                out.len() as c_int,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpubftojson(
                self.c_ctx_ptr(),
                ubf.as_ubfh(),
                out.as_mut_ptr() as *mut c_char,
                out.len() as c_int,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            let end = out.iter().position(|&b| b == 0).unwrap_or(out.len());
            Ok(String::from_utf8_lossy(&out[..end]).into_owned())
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub(crate) fn tpviewtojson(
        &self,
        cstruct: *mut c_char,
        view: *mut c_char,
        buffer: *mut c_char,
        bufsize: i32,
        flags: i64,
    ) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc =
            unsafe { raw::tpviewtojson(cstruct, view, buffer, bufsize as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpviewtojson(
                self.c_ctx_ptr(),
                cstruct,
                view,
                buffer,
                bufsize as c_int,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    pub(crate) unsafe fn tpjsontoview(
        &self,
        view: *mut c_char,
        buffer: *mut c_char,
    ) -> AtmiResult<*mut c_char> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = raw::tpjsontoview(view, buffer);

        #[cfg(feature = "ctx-send")]
        let rc = raw::Otpjsontoview(self.c_ctx_ptr(), view, buffer);

        if rc.is_null() {
            Err(self.atmi_last_error())
        } else {
            Ok(rc)
        }
    }

    /// Enqueue a buffer into a persistent queue.
    pub fn tpenqueue(
        &self,
        qspace: &str,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_qspace = CString::new(qspace).map_err(|_| self.atmi_last_error())?;
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpenqueue(
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpenqueue(
                self.c_ctx_ptr(),
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Dequeue a buffer from a persistent queue. Returns the dequeued buffer.
    pub fn tpdequeue<'ctx>(
        &'ctx self,
        qspace: &str,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        flags: i64,
    ) -> AtmiResult<TypedBuffer<'ctx>> {
        let c_qspace = CString::new(qspace).map_err(|_| self.atmi_last_error())?;
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();
        let mut odata: *mut c_char = ptr::null_mut();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpdequeue(
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpdequeue(
                self.c_ctx_ptr(),
                c_qspace.as_ptr() as *mut c_char,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(unsafe { TypedBuffer::from_raw(self, odata) })
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpenqueueex(
        &self,
        nodeid: i16,
        srvid: i16,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        data: &TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpenqueueex(
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpenqueueex(
                self.c_ctx_ptr(),
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                data.as_ptr(),
                0,
                flags as c_long,
            )
        };

        self.rc_to_result(rc)
    }

    /// Dequeue a buffer by node/server ID. Returns the dequeued buffer.
    pub fn tpdequeueex<'ctx>(
        &'ctx self,
        nodeid: i16,
        srvid: i16,
        qname: &str,
        ctl: &mut crate::TpQCtl,
        flags: i64,
    ) -> AtmiResult<TypedBuffer<'ctx>> {
        let c_qname = CString::new(qname).map_err(|_| self.atmi_last_error())?;
        let ctl_ptr = ctl.as_mut_ptr();
        let mut odata: *mut c_char = ptr::null_mut();
        let mut olen: c_long = 0;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpdequeueex(
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpdequeueex(
                self.c_ctx_ptr(),
                nodeid,
                srvid,
                c_qname.as_ptr() as *mut c_char,
                ctl_ptr,
                &mut odata,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(unsafe { TypedBuffer::from_raw(self, odata) })
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// Capture the current ATMI context handle.
    pub fn tpgetctxt(&self) -> AtmiResult<TpContext> {
        let mut out: raw::TPCONTEXT_T = ptr::null_mut();

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetctxt(&mut out, 0) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetctxt(self.c_ctx_ptr(), &mut out, 0) };

        if rc == raw::TPMULTICONTEXTS as c_int {
            Ok(TpContext(out))
        } else if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            // TPNULLCONTEXT. Enduro/X reports "no context" without setting
            // tperrno, so atmi_last_error() would report a stale code here.
            Err(AtmiError::new(
                raw::TPEPROTO,
                "no ATMI context is associated with the current thread",
            ))
        }
    }

    /// Activate a previously captured ATMI context handle on the current thread.
    pub fn tpsetctxt(&self, context: TpContext, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsetctxt(context.0, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsetctxt(self.c_ctx_ptr(), context.0, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpencrypt(&self, input: &[u8], flags: i64) -> AtmiResult<Vec<u8>> {
        let mut out = vec![0u8; input.len().saturating_mul(2).max(256)];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpencrypt(
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpencrypt(
                self.c_ctx_ptr(),
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            out.truncate(olen as usize);
            Ok(out)
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpdecrypt(&self, input: &[u8], flags: i64) -> AtmiResult<Vec<u8>> {
        let mut out = vec![0u8; input.len().max(256)];
        let mut olen = out.len() as c_long;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpdecrypt(
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpdecrypt(
                self.c_ctx_ptr(),
                input.as_ptr() as *mut c_char,
                input.len() as c_long,
                out.as_mut_ptr() as *mut c_char,
                &mut olen,
                flags as c_long,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            out.truncate(olen as usize);
            Ok(out)
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpsprio(&self, prio: i32, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsprio(prio as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsprio(self.c_ctx_ptr(), prio as c_int, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpgprio(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgprio() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgprio(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    pub fn tpsblktime(&self, tout: i32, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsblktime(tout as c_int, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsblktime(self.c_ctx_ptr(), tout as c_int, flags as c_long) };

        self.rc_to_result(rc)
    }

    pub fn tpgblktime(&self, flags: i64) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgblktime(flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgblktime(self.c_ctx_ptr(), flags as c_long) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }
}
