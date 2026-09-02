#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
use std::ffi::CStr;

pub use endurox_rs_derive::{UbfDeserialize, UbfSerialize};

pub(crate) mod raw {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[doc(hidden)]
pub mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/test.rs"));
}

#[cfg(feature = "async")]
mod async_atmi;
mod atmictx;
mod atmictx_log;
mod atmictx_srv;
mod atmictx_ubf;
mod atmictx_xatmi;
mod errors;
mod flags;
mod nstdutil;
mod tpsvcinfo;
mod typed_buf;
mod typed_ubf;
mod typed_view;
mod types;
mod ubf_serde;

#[cfg(feature = "async")]
pub use async_atmi::{AsyncAtmiCtx, AsyncReplyDriver};
#[cfg(feature = "async-io")]
pub use async_atmi::{AsyncIoAtmiCtx, AsyncIoReplyDriver};
#[cfg(feature = "tokio")]
pub use async_atmi::{TokioAtmiCtx, TokioReplyDriver};
pub use atmictx::AtmiCtx;
pub use atmictx_log::LogLevel;
pub use atmictx_srv::{
    PollerEvent, RustBeforePollCallback, RustPeriodCallback, RustPollerCallback,
    RustServerDoneHook, RustServerInitHook, RustServerThreadDoneHook, RustServerThreadInitHook,
    RustServiceCallback, ServerHooks, TpReturnStatus,
};
pub use atmictx_ubf::{BFldLocInfo, UbfExprCallback, UbfExprCallback2, UbfExprTree, UbfFieldType};
pub use errors::{AtmiError, AtmiResult, NstdError, NstdResult, UbfError, UbfResult};
pub use flags::{
    TPBLK_ALL, TPBLK_NEXT, TPCONV, TPGETANY, TPNOBLOCK, TPNOCHANGE, TPNOREPLY, TPNOTIME, TPNOTRAN,
    TPRECVONLY, TPSENDONLY, TPSIGRSTRT, TPTRAN, TPTRANSUSPEND,
};
pub use nstdutil::NdrxStdCfgStr;
pub use tpsvcinfo::TpSvcInfo;
pub use typed_buf::{TpTypeInfo, TypedBuffer};
pub use typed_ubf::{
    BorrowedBuffer, BorrowedUbf, IntoUbfValue, TypedUbf, UbfField, UbfGetValue, UbfIterator,
    UbfValue,
};
pub use typed_view::{BvNextState, IntoViewValue, TypedView, ViewValue, BVACCESS_NOTNULL};
pub use types::{ClientId, TpContext, TpTranId};
pub use ubf_serde::{
    ubf_read_adhoc, ubf_read_nested, ubf_write_adhoc, ubf_write_nested, UbfAdhoc, UbfCarray,
    UbfDeserialize, UbfFieldDeserialize, UbfFieldSerialize, UbfSerialize,
};

pub const TPQCORRID: i64 = raw::TPQCORRID as i64;
pub const TPQFAILUREQ: i64 = raw::TPQFAILUREQ as i64;
pub const TPQBEFOREMSGID: i64 = raw::TPQBEFOREMSGID as i64;
pub const TPQGETBYMSGIDOLD: i64 = raw::TPQGETBYMSGIDOLD as i64;
pub const TPQMSGID: i64 = raw::TPQMSGID as i64;
pub const TPQPRIORITY: i64 = raw::TPQPRIORITY as i64;
pub const TPQTOP: i64 = raw::TPQTOP as i64;
pub const TPQWAIT: i64 = raw::TPQWAIT as i64;
pub const TPQREPLYQ: i64 = raw::TPQREPLYQ as i64;
pub const TPQTIME_ABS: i64 = raw::TPQTIME_ABS as i64;
pub const TPQTIME_REL: i64 = raw::TPQTIME_REL as i64;
pub const TPQGETBYCORRIDOLD: i64 = raw::TPQGETBYCORRIDOLD as i64;
pub const TPQPEEK: i64 = raw::TPQPEEK as i64;
pub const TPQDELIVERYQOS: i64 = raw::TPQDELIVERYQOS as i64;
pub const TPQREPLYQOS: i64 = raw::TPQREPLYQOS as i64;
pub const TPQEXPTIME_ABS: i64 = raw::TPQEXPTIME_ABS as i64;
pub const TPQEXPTIME_REL: i64 = raw::TPQEXPTIME_REL as i64;
pub const TPQEXPTIME_NONE: i64 = raw::TPQEXPTIME_NONE as i64;
pub const TPQGETBYMSGID: i64 = raw::TPQGETBYMSGID as i64;
pub const TPQGETBYCORRID: i64 = raw::TPQGETBYCORRID as i64;
pub const TPQASYNC: i64 = raw::TPQASYNC as i64;
pub const TPQKEEPORIG: i64 = raw::TPQKEEPORIG as i64;
pub const TPQQOSDEFAULTPERSIST: i64 = raw::TPQQOSDEFAULTPERSIST as i64;
pub const TPQQOSPERSISTENT: i64 = raw::TPQQOSPERSISTENT as i64;
pub const TPQQOSNONPERSISTENT: i64 = raw::TPQQOSNONPERSISTENT as i64;

/// Event subscription control block used by [`AtmiCtx::tpsubscribe`].
///
/// This is an opaque Rust wrapper around the Enduro/X `TPEVCTL` structure.
/// Use [`Default::default`] when no fields need to be customized.
pub struct TpEvCtl {
    inner: raw::TPEVCTL,
}

impl Default for TpEvCtl {
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

impl TpEvCtl {
    #[inline]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut raw::TPEVCTL {
        &mut self.inner
    }
}

/// Persistent queue control block used by queue enqueue/dequeue APIs.
///
/// Flags are explicit, matching Enduro/X `TPQCTL.flags`. Use setters for the
/// fixed-size fields so Rust performs length/NUL validation before C sees them.
pub struct TpQCtl {
    inner: raw::TPQCTL,
}

impl Default for TpQCtl {
    fn default() -> Self {
        Self {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

impl TpQCtl {
    #[inline]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut raw::TPQCTL {
        &mut self.inner
    }

    pub fn flags(&self) -> i64 {
        self.inner.flags as i64
    }

    pub fn set_flags(&mut self, flags: i64) -> &mut Self {
        self.inner.flags = flags as _;
        self
    }

    pub fn add_flags(&mut self, flags: i64) -> &mut Self {
        self.inner.flags |= flags as std::os::raw::c_long;
        self
    }

    pub fn clear_flags(&mut self, flags: i64) -> &mut Self {
        self.inner.flags &= !(flags as std::os::raw::c_long);
        self
    }

    pub fn deq_time(&self) -> i64 {
        self.inner.deq_time as i64
    }

    pub fn set_deq_time(&mut self, deq_time: i64) -> &mut Self {
        self.inner.deq_time = deq_time as _;
        self
    }

    pub fn priority(&self) -> i64 {
        self.inner.priority as i64
    }

    pub fn set_priority(&mut self, priority: i64) -> &mut Self {
        self.inner.priority = priority as _;
        self
    }

    pub fn diagnostic(&self) -> i64 {
        self.inner.diagnostic as i64
    }

    pub fn diagmsg(&self) -> String {
        fixed_cstr_to_string(&self.inner.diagmsg)
    }

    pub fn msgid(&self) -> &[u8] {
        fixed_bytes_until_nul(&self.inner.msgid)
    }

    pub fn set_msgid(&mut self, msgid: &[u8]) -> AtmiResult<&mut Self> {
        write_fixed_bytes(&mut self.inner.msgid, msgid, "msgid")?;
        Ok(self)
    }

    pub fn corrid(&self) -> &[u8] {
        fixed_bytes_until_nul(&self.inner.corrid)
    }

    pub fn set_corrid(&mut self, corrid: &[u8]) -> AtmiResult<&mut Self> {
        write_fixed_bytes(&mut self.inner.corrid, corrid, "corrid")?;
        Ok(self)
    }

    pub fn reply_queue(&self) -> String {
        fixed_cstr_to_string(&self.inner.replyqueue)
    }

    pub fn set_reply_queue(&mut self, replyqueue: &str) -> AtmiResult<&mut Self> {
        write_fixed_str(&mut self.inner.replyqueue, replyqueue, "replyqueue")?;
        Ok(self)
    }

    pub fn failure_queue(&self) -> String {
        fixed_cstr_to_string(&self.inner.failurequeue)
    }

    pub fn set_failure_queue(&mut self, failurequeue: &str) -> AtmiResult<&mut Self> {
        write_fixed_str(&mut self.inner.failurequeue, failurequeue, "failurequeue")?;
        Ok(self)
    }

    pub fn urcode(&self) -> i64 {
        self.inner.urcode as i64
    }

    pub fn set_urcode(&mut self, urcode: i64) -> &mut Self {
        self.inner.urcode = urcode as _;
        self
    }

    pub fn appkey(&self) -> i64 {
        self.inner.appkey as i64
    }

    pub fn set_appkey(&mut self, appkey: i64) -> &mut Self {
        self.inner.appkey = appkey as _;
        self
    }

    pub fn delivery_qos(&self) -> i64 {
        self.inner.delivery_qos as i64
    }

    pub fn set_delivery_qos(&mut self, delivery_qos: i64) -> &mut Self {
        self.inner.delivery_qos = delivery_qos as _;
        self
    }

    pub fn reply_qos(&self) -> i64 {
        self.inner.reply_qos as i64
    }

    pub fn set_reply_qos(&mut self, reply_qos: i64) -> &mut Self {
        self.inner.reply_qos = reply_qos as _;
        self
    }

    pub fn exp_time(&self) -> i64 {
        self.inner.exp_time as i64
    }

    pub fn set_exp_time(&mut self, exp_time: i64) -> &mut Self {
        self.inner.exp_time = exp_time as _;
        self
    }
}

fn write_fixed_str(dst: &mut [std::os::raw::c_char], value: &str, field: &str) -> AtmiResult<()> {
    if value.as_bytes().contains(&0) {
        return Err(AtmiError::new(
            raw::TPEINVAL,
            format!("{field} contains NUL byte"),
        ));
    }
    write_fixed_bytes(dst, value.as_bytes(), field)
}

fn write_fixed_bytes(
    dst: &mut [std::os::raw::c_char],
    value: &[u8],
    field: &str,
) -> AtmiResult<()> {
    if value.len() >= dst.len() {
        return Err(AtmiError::new(
            raw::TPEINVAL,
            format!("{field} too long: max {} bytes", dst.len() - 1),
        ));
    }
    dst.fill(0);
    for (out, byte) in dst.iter_mut().zip(value.iter().copied()) {
        *out = byte as std::os::raw::c_char;
    }
    Ok(())
}

fn fixed_bytes_until_nul(src: &[std::os::raw::c_char]) -> &[u8] {
    let len = src.iter().position(|&b| b == 0).unwrap_or(src.len());
    unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, len) }
}

fn fixed_cstr_to_string(src: &[std::os::raw::c_char]) -> String {
    if src.first().copied().unwrap_or_default() == 0 {
        String::new()
    } else {
        unsafe { CStr::from_ptr(src.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }
}
