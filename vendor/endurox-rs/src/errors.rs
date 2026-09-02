use crate::raw;
use std::{borrow::Cow, error::Error, fmt};

// --- ATMI Errors -------------------------------------------------------------

macro_rules! gen_error_consts {
    ($($name:ident),* $(,)?) => {
        $(pub const $name: u32 = raw::$name;)*
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtmiError {
    pub code: u32,
    pub message: Cow<'static, str>,
}

/* ATMI error */
impl AtmiError {
    pub fn new(code: u32, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    // List of errors codes
    gen_error_consts! {
        TPMINVAL,
        TPEABORT,
        TPEBADDESC,
        TPEBLOCK,
        TPEINVAL,
        TPELIMIT,
        TPENOENT,
        TPEOS,
        TPEPERM,
        TPEPROTO,
        TPESVCERR,
        TPESVCFAIL,
        TPESYSTEM,
        TPETIME,
        TPETRAN,
        TPGOTSIG,
        TPERMERR,
        TPEITYPE,
        TPEOTYPE,
        TPERELEASE,
        TPEHAZARD,
        TPEHEURISTIC,
        TPEEVENT,
        TPEMATCH,
        TPEDIAGNOSTIC,
        TPEMIB,
        TPERFU26,
        TPERFU27,
        TPERFU28,
        TPERFU29,
        TPINITFAIL,
        TPMAXVAL
    }
}

impl fmt::Display for AtmiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[code {}] {}", self.code, self.message)
    }
}

impl Error for AtmiError {}

pub type AtmiResult<T> = Result<T, AtmiError>;

// --- UBF Errors --------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbfError {
    pub code: u32,
    pub message: Cow<'static, str>,
}

/* ATMI error */
impl UbfError {
    pub fn new(code: u32, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    // List of errors codes
    gen_error_consts! {
        BMINVAL,
        BERFU0,
        BALIGNERR,
        BNOTFLD,
        BNOSPACE,
        BNOTPRES,
        BBADFLD,
        BTYPERR,
        BEUNIX,
        BBADNAME,
        BMALLOC,
        BSYNTAX,
        BFTOPEN,
        BFTSYNTAX,
        BEINVAL,
        BERFU1,
        BBADTBL,
        BBADVIEW,
        BVFSYNTAX,
        BVFOPEN,
        BBADACM,
        BNOCNAME,
        BEBADOP,
        BMAXVAL
    }
}

impl fmt::Display for UbfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[code {}] {}", self.code, self.message)
    }
}

impl Error for UbfError {}

pub type UbfResult<T> = Result<T, UbfError>;

// --- NSTD Errors -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NstdError {
    pub code: u32,
    pub message: Cow<'static, str>,
}

/* ATMI error */
impl NstdError {
    pub fn new(code: u32, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for NstdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[code {}] {}", self.code, self.message)
    }
}

impl Error for NstdError {}

pub type NstdResult<T> = Result<T, NstdError>;
