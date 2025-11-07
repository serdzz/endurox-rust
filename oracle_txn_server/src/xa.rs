use endurox_sys::ffi;
use endurox_sys::{tplog_error, tplog_info};

/// Start an XA transaction
pub fn begin_transaction() -> Result<(), String> {
    let ret = unsafe { ffi::tpbegin(60, 0) }; // 60 second timeout
    
    if ret == -1 {
        let err = unsafe { ffi::tperrno };
        tplog_error(&format!("Failed to begin transaction: error={}", err));
        return Err(format!("tpbegin failed with error {}", err));
    }
    
    tplog_info("XA transaction started");
    Ok(())
}

/// Commit an XA transaction
pub fn commit_transaction() -> Result<(), String> {
    let ret = unsafe { ffi::tpcommit(0) };
    
    if ret == -1 {
        let err = unsafe { ffi::tperrno };
        tplog_error(&format!("Failed to commit transaction: error={}", err));
        return Err(format!("tpcommit failed with error {}", err));
    }
    
    tplog_info("XA transaction committed");
    Ok(())
}

/// Abort/rollback an XA transaction
pub fn abort_transaction() -> Result<(), String> {
    let ret = unsafe { ffi::tpabort(0) };
    
    if ret == -1 {
        let err = unsafe { ffi::tperrno };
        tplog_error(&format!("Failed to abort transaction: error={}", err));
        return Err(format!("tpabort failed with error {}", err));
    }
    
    tplog_info("XA transaction aborted");
    Ok(())
}

/// Check if currently in a transaction
pub fn is_in_transaction() -> bool {
    unsafe { ffi::tpgetlev() > 0 }
}

/// Get current transaction level
pub fn get_transaction_level() -> i32 {
    unsafe { ffi::tpgetlev() }
}

/// Execute a function within an XA transaction
/// Automatically commits on success or aborts on error
pub fn with_transaction<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    begin_transaction()?;
    
    match f() {
        Ok(result) => {
            commit_transaction()?;
            Ok(result)
        }
        Err(e) => {
            tplog_error(&format!("Transaction failed: {}", e));
            abort_transaction()?;
            Err(e)
        }
    }
}
