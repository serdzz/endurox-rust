//! Raw FFI биндинги к Enduro/X C API

use libc::{c_char, c_int, c_long, c_void};

// Return codes (from xatmi.h)
pub const TPFAIL: c_int = 0x00000001;
pub const TPSUCCESS: c_int = 0x00000002;

// Flags
pub const TPNOBLOCK: c_long = 0x00000001;
pub const TPNOTRAN: c_long = 0x00000008;
pub const TPSIGRSTRT: c_long = 0x00000010;
pub const TPNOTIME: c_long = 0x00000020;

// Service info structure  - must match C TPSVCINFO layout
// typedef struct {
//     char name[XATMI_SERVICE_NAME_LENGTH+1];  // 31 chars
//     char *data;
//     long len;
//     long flags;
//     int cd;
//     long appkey;
//     CLIENTID cltid;  // struct with char clientdata[96]
//     char fname[XATMI_SERVICE_NAME_LENGTH+1]; // 31 chars
// } TPSVCINFO;
#[repr(C)]
pub struct TpSvcInfoRaw {
    pub name: [c_char; 32], // XATMI_SERVICE_NAME_LENGTH+1 (31, padded to 32)
    pub data: *mut c_char,
    pub len: c_long,
    pub flags: c_long,
    pub cd: c_int,
    pub appkey: c_long,
    pub cltid: [c_char; 96], // CLIENTID.clientdata[NDRX_MAX_ID_SIZE]
    pub fname: [c_char; 32], // XATMI_SERVICE_NAME_LENGTH+1 (31, padded to 32)
}

extern "C" {
    // Server functions
    #[cfg(feature = "server")]
    pub fn ndrx_main(argc: c_int, argv: *mut *mut c_char) -> c_int;

    #[cfg(feature = "server")]
    pub fn tpadvertise_full(
        svcname: *const c_char,
        func: extern "C" fn(*mut TpSvcInfoRaw),
        funcname: *const c_char,
    ) -> c_int;

    #[cfg(feature = "server")]
    pub fn tpreturn(rval: c_int, rcode: c_long, data: *mut c_char, len: c_long, flags: c_long);

    // Client functions
    #[cfg(feature = "client")]
    pub fn tpinit(tpinfo: *mut c_void) -> c_int;

    #[cfg(feature = "client")]
    pub fn tpterm() -> c_int;

    #[cfg(feature = "client")]
    pub fn tpcall(
        svc: *const c_char,
        idata: *mut c_char,
        ilen: c_long,
        odata: *mut *mut c_char,
        olen: *mut c_long,
        flags: c_long,
    ) -> c_int;

    #[cfg(feature = "client")]
    pub fn tpacall(svc: *const c_char, data: *mut c_char, len: c_long, flags: c_long) -> c_int;

    #[cfg(feature = "client")]
    pub fn tpgetrply(
        cd: *mut c_int,
        data: *mut *mut c_char,
        len: *mut c_long,
        flags: c_long,
    ) -> c_int;

    // Buffer management
    pub fn tpalloc(typ: *const c_char, subtyp: *const c_char, size: c_long) -> *mut c_char;
    pub fn tprealloc(ptr: *mut c_char, size: c_long) -> *mut c_char;
    pub fn tpfree(ptr: *mut c_char);

    // Error handling
    pub fn tpstrerror(err: c_int) -> *const c_char;
    pub fn _exget_tperrno_addr() -> *const c_int;

    // Logging
    pub fn tplog(lev: c_int, format: *const c_char, ...);
    pub fn userlog(format: *const c_char, ...);

    // UBF API
    #[cfg(feature = "ubf")]
    pub fn Binit(p_ub: *mut c_char, len: c_long) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Badd(p_ub: *mut c_char, bfldid: c_int, buf: *const c_char, len: c_int) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bchg(
        p_ub: *mut c_char,
        bfldid: c_int,
        occ: c_int,
        buf: *const c_char,
        len: c_int,
    ) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bget(
        p_ub: *mut c_char,
        bfldid: c_int,
        occ: c_int,
        buf: *mut c_char,
        len: *mut c_int,
    ) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn CBget(
        p_ub: *mut c_char,
        bfldid: c_int,
        occ: c_int,
        buf: *mut c_char,
        len: *mut c_int,
        usrtype: c_int,
    ) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bpres(p_ub: *mut c_char, bfldid: c_int, occ: c_int) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bdel(p_ub: *mut c_char, bfldid: c_int, occ: c_int) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bproj(p_ub: *mut c_char, fldlist: *const c_int) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bfprint(p_ub: *mut c_char, outf: *mut c_void) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bprint(p_ub: *mut c_char) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Blen(p_ub: *mut c_char, bfldid: c_int, occ: c_int) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bused(p_ub: *mut c_char) -> c_long;

    #[cfg(feature = "ubf")]
    pub fn Bunused(p_ub: *mut c_char) -> c_long;

    #[cfg(feature = "ubf")]
    pub fn Bsizeof(p_ub: *mut c_char) -> c_long;

    #[cfg(feature = "ubf")]
    pub fn Bfldid(fldname: *const c_char) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bfname(bfldid: c_int) -> *const c_char;

    #[cfg(feature = "ubf")]
    pub fn Bfldtype(bfldid: c_int) -> c_int;

    #[cfg(feature = "ubf")]
    pub fn Bnext(
        p_ub: *mut c_char,
        bfldid: *mut c_int,
        occ: *mut c_int,
        buf: *mut c_char,
        len: *mut c_int,
    ) -> c_int;
}

// UBF field types
#[cfg(feature = "ubf")]
pub const BFLD_SHORT: c_int = 0;
#[cfg(feature = "ubf")]
pub const BFLD_LONG: c_int = 1;
#[cfg(feature = "ubf")]
pub const BFLD_CHAR: c_int = 2;
#[cfg(feature = "ubf")]
pub const BFLD_FLOAT: c_int = 3;
#[cfg(feature = "ubf")]
pub const BFLD_DOUBLE: c_int = 4;
#[cfg(feature = "ubf")]
pub const BFLD_STRING: c_int = 5;
#[cfg(feature = "ubf")]
pub const BFLD_CARRAY: c_int = 6;
