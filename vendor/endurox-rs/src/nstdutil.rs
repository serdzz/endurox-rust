use crate::{raw, AtmiCtx, NstdError, NstdResult};
use core::ffi::{c_char, c_int};
use std::{
    ffi::{CStr, CString},
    ptr,
};

/// Single key/value entry parsed from an Enduro/X standard configuration string.
///
/// `value` is `None` when the source string contained a bare key (no `=`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NdrxStdCfgStr {
    pub key: String,
    pub value: Option<String>,
}

impl AtmiCtx {
    /// Parse an Enduro/X standard configuration string into a `Vec` of
    /// key/value entries.
    ///
    /// Wraps the C `ndrx_stdcfgstr_parse` (or `Ondrx_stdcfgstr_parse` under
    /// the `ctx-send` feature). The intermediate C linked list is freed with
    /// `ndrx_stdcfgstr_free` before returning, so the caller only sees plain
    /// owned Rust values.
    pub fn ndrx_stdcfgstr_parse(&self, input: &str) -> NstdResult<Vec<NdrxStdCfgStr>> {
        let input_c =
            CString::new(input).map_err(|_| NstdError::new(0, "input contains NUL byte"))?;

        let mut parsed: *mut raw::ndrx_stdcfgstr_t = ptr::null_mut();

        let rc = unsafe { raw::ndrx_stdcfgstr_parse(input_c.as_ptr(), &mut parsed) };

        if rc != raw::EXSUCCEED as c_int {
            return Err(self.nstd_last_error());
        }

        let result = unsafe { collect_entries(parsed) };

        unsafe { raw::ndrx_stdcfgstr_free(parsed) };

        Ok(result)
    }
}

unsafe fn collect_entries(head: *mut raw::ndrx_stdcfgstr_t) -> Vec<NdrxStdCfgStr> {
    let mut out = Vec::new();
    let mut cur = head;
    while !cur.is_null() {
        let node = &*cur;
        out.push(NdrxStdCfgStr {
            key: c_array_to_string(&node.key),
            value: c_ptr_to_string(node.value),
        });
        cur = node.next;
    }
    out
}

fn c_array_to_string(src: &[c_char]) -> String {
    let len = src.iter().position(|&b| b == 0).unwrap_or(src.len());
    let bytes = unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, len) };
    String::from_utf8_lossy(bytes).into_owned()
}

unsafe fn c_ptr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(CStr::from_ptr(p).to_string_lossy().into_owned())
    }
}
