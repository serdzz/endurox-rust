use endurox_rs::{tp_error, tp_info, AtmiCtx};

/// Start an XA transaction
pub fn begin_transaction(ctx: &AtmiCtx) -> Result<(), String> {
    if let Err(e) = ctx.tpbegin(60, 0) {
        // 60 second timeout
        tp_error!(ctx, "Failed to begin transaction: {}", e);
        return Err(format!("tpbegin failed: {}", e));
    }

    tp_info!(ctx, "XA transaction started");
    Ok(())
}

/// Commit an XA transaction
pub fn commit_transaction(ctx: &AtmiCtx) -> Result<(), String> {
    if let Err(e) = ctx.tpcommit(0) {
        tp_error!(ctx, "Failed to commit transaction: {}", e);
        return Err(format!("tpcommit failed: {}", e));
    }

    tp_info!(ctx, "XA transaction committed");
    Ok(())
}

/// Abort/rollback an XA transaction
pub fn abort_transaction(ctx: &AtmiCtx) -> Result<(), String> {
    if let Err(e) = ctx.tpabort(0) {
        tp_error!(ctx, "Failed to abort transaction: {}", e);
        return Err(format!("tpabort failed: {}", e));
    }

    tp_info!(ctx, "XA transaction aborted");
    Ok(())
}

/// Check if currently in a transaction
pub fn is_in_transaction(ctx: &AtmiCtx) -> bool {
    ctx.tpgetlev().map(|lev| lev > 0).unwrap_or(false)
}

/// Get current transaction level
pub fn get_transaction_level(ctx: &AtmiCtx) -> i32 {
    ctx.tpgetlev().unwrap_or(0)
}

/// Execute a function within an XA transaction
/// Automatically commits on success or aborts on error
pub fn with_transaction<F, T>(ctx: &AtmiCtx, f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    begin_transaction(ctx)?;

    match f() {
        Ok(result) => {
            commit_transaction(ctx)?;
            Ok(result)
        }
        Err(e) => {
            tp_error!(ctx, "Transaction failed: {}", e);
            abort_transaction(ctx)?;
            Err(e)
        }
    }
}
