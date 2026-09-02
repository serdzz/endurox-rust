use endurox_rs::{ubf_fields, AtmiCtx, AtmiError, TpQCtl, TypedUbf, TPQCORRID, TPQGETBYCORRID};

const QSPACE: &str = "SAMPLESPACE";
const QNAME: &str = "TESTQ";

// Diagnostic returned in TpQCtl when tpdequeue finds no message; matches
// the C constant `QMENOMSG` from `xa_cmn.h`.
const QMENOMSG: i64 = -11;

fn main() {
    let rc = match run() {
        Ok(()) => 0,
        Err(msg) => {
            eprintln!("{msg}");
            1
        }
    };
    std::process::exit(rc);
}

fn run() -> Result<(), String> {
    let scenario = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "enqueue-dequeue".to_string());

    let ctx = AtmiCtx::new().map_err(|e| format!("failed to create AtmiCtx: {e}"))?;
    ctx.tpinit().map_err(|e| format!("tpinit failed: {e}"))?;

    let result = match scenario.as_str() {
        "enqueue-dequeue" => run_enqueue_dequeue(&ctx),
        "corrid" => run_corrid(&ctx),
        "fifo" => run_fifo(&ctx),
        "tx-commit" => run_tx_commit(&ctx),
        "tx-abort" => run_tx_abort(&ctx),
        "tx-suspend-resume" => run_tx_suspend_resume(&ctx),
        other => Err(format!("unknown scenario `{other}`")),
    };

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    result
}

fn enqueue_str(ctx: &AtmiCtx, value: &str) -> Result<(), String> {
    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;
    buf.bchg(ubf_fields::T_STRING_FLD, 0, value, true)
        .map_err(|e| format!("bchg failed: {e}"))?;
    let mut ctl = TpQCtl::default();
    ctx.tpenqueue(QSPACE, QNAME, &mut ctl, &buf, 0)
        .map_err(|e| format!("tpenqueue `{value}` failed: {e}"))
}

fn dequeue_str(ctx: &AtmiCtx) -> Result<String, String> {
    let mut ctl = TpQCtl::default();
    let buf = ctx
        .tpdequeue(QSPACE, QNAME, &mut ctl, 0)
        .map_err(|e| format!("tpdequeue failed: {e}"))?;
    let ubf = TypedUbf::from_typed(buf);
    ubf.bget_string(ubf_fields::T_STRING_FLD, 0)
        .map_err(|e| format!("bget_string failed: {e}"))
}

fn run_enqueue_dequeue(ctx: &AtmiCtx) -> Result<(), String> {
    enqueue_str(ctx, "HELLO-QUEUE")?;
    let val = dequeue_str(ctx)?;
    if val != "HELLO-QUEUE" {
        return Err(format!(
            "enqueue-dequeue: expected `HELLO-QUEUE`, got `{val}`"
        ));
    }
    Ok(())
}

fn run_corrid(ctx: &AtmiCtx) -> Result<(), String> {
    let corrid: [u8; 31] = {
        let mut c = [0u8; 31];
        c[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        c
    };

    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;
    buf.bchg(ubf_fields::T_STRING_FLD, 0, "CORRID-MSG", true)
        .map_err(|e| format!("bchg failed: {e}"))?;

    let mut enq_ctl = TpQCtl::default();
    enq_ctl
        .set_corrid(&corrid)
        .map_err(|e| format!("set_corrid failed: {e}"))?;
    enq_ctl.add_flags(TPQCORRID);
    ctx.tpenqueue(QSPACE, QNAME, &mut enq_ctl, &buf, 0)
        .map_err(|e| format!("tpenqueue (corrid) failed: {e}"))?;

    let mut deq_ctl = TpQCtl::default();
    deq_ctl
        .set_corrid(&corrid)
        .map_err(|e| format!("set_corrid (deq) failed: {e}"))?;
    deq_ctl.add_flags(TPQGETBYCORRID);
    let dequeued = ctx
        .tpdequeue(QSPACE, QNAME, &mut deq_ctl, 0)
        .map_err(|e| format!("tpdequeue (by corrid) failed: {e}"))?;
    let ubf = TypedUbf::from_typed(dequeued);
    let val = ubf
        .bget_string(ubf_fields::T_STRING_FLD, 0)
        .map_err(|e| format!("bget_string (corrid) failed: {e}"))?;

    if val != "CORRID-MSG" {
        return Err(format!("corrid: expected `CORRID-MSG`, got `{val}`"));
    }
    Ok(())
}

fn run_fifo(ctx: &AtmiCtx) -> Result<(), String> {
    let messages = ["FIFO-1", "FIFO-2", "FIFO-3"];
    for msg in &messages {
        enqueue_str(ctx, msg)?;
    }
    for expected in &messages {
        let val = dequeue_str(ctx)?;
        if &val != expected {
            return Err(format!("fifo order: expected `{expected}`, got `{val}`"));
        }
    }
    Ok(())
}

fn expect_empty_queue(ctx: &AtmiCtx, label: &str) -> Result<(), String> {
    let mut ctl = TpQCtl::default();
    match ctx.tpdequeue(QSPACE, QNAME, &mut ctl, 0) {
        Ok(_) => Err(format!("{label}: expected empty queue, but got a message")),
        Err(e) if e.code == AtmiError::TPEDIAGNOSTIC && ctl.diagnostic() == QMENOMSG => Ok(()),
        Err(e) => Err(format!(
            "{label}: expected TPEDIAGNOSTIC/QMENOMSG, got code={} diag={} ({e})",
            e.code,
            ctl.diagnostic()
        )),
    }
}

fn run_tx_commit(ctx: &AtmiCtx) -> Result<(), String> {
    ctx.tpopen().map_err(|e| format!("tpopen failed: {e}"))?;

    expect_empty_queue(ctx, "tx-commit precondition")?;

    ctx.tpbegin(60, 0)
        .map_err(|e| format!("tpbegin failed: {e}"))?;
    if let Err(e) = enqueue_str(ctx, "TX-COMMIT-MSG") {
        let _ = ctx.tpabort(0);
        let _ = ctx.tpclose();
        return Err(e);
    }
    ctx.tpcommit(0)
        .map_err(|e| format!("tpcommit failed: {e}"))?;

    let val = dequeue_str(ctx)?;
    if val != "TX-COMMIT-MSG" {
        let _ = ctx.tpclose();
        return Err(format!("tx-commit: expected `TX-COMMIT-MSG`, got `{val}`"));
    }

    expect_empty_queue(ctx, "tx-commit postcondition")?;
    ctx.tpclose().map_err(|e| format!("tpclose failed: {e}"))?;
    Ok(())
}

fn run_tx_abort(ctx: &AtmiCtx) -> Result<(), String> {
    ctx.tpopen().map_err(|e| format!("tpopen failed: {e}"))?;

    expect_empty_queue(ctx, "tx-abort precondition")?;

    ctx.tpbegin(60, 0)
        .map_err(|e| format!("tpbegin failed: {e}"))?;
    if let Err(e) = enqueue_str(ctx, "TX-ABORT-MSG") {
        let _ = ctx.tpabort(0);
        let _ = ctx.tpclose();
        return Err(e);
    }
    ctx.tpabort(0).map_err(|e| format!("tpabort failed: {e}"))?;

    expect_empty_queue(ctx, "tx-abort postcondition")?;
    ctx.tpclose().map_err(|e| format!("tpclose failed: {e}"))?;
    Ok(())
}

fn run_tx_suspend_resume(ctx: &AtmiCtx) -> Result<(), String> {
    ctx.tpopen().map_err(|e| format!("tpopen failed: {e}"))?;

    expect_empty_queue(ctx, "tx-suspend-resume precondition")?;

    // Outer tx: enqueue OUTER-MSG, then suspend.
    ctx.tpbegin(60, 0)
        .map_err(|e| format!("outer tpbegin failed: {e}"))?;
    if let Err(e) = enqueue_str(ctx, "OUTER-MSG") {
        let _ = ctx.tpabort(0);
        let _ = ctx.tpclose();
        return Err(format!("outer enqueue failed: {e}"));
    }
    let outer = ctx
        .tpsuspend(0)
        .map_err(|e| format!("tpsuspend failed: {e}"))?;

    // Inner tx (no current tx after suspend): enqueue INNER-MSG, commit.
    ctx.tpbegin(60, 0)
        .map_err(|e| format!("inner tpbegin failed: {e}"))?;
    if let Err(e) = enqueue_str(ctx, "INNER-MSG") {
        let _ = ctx.tpabort(0);
        let _ = ctx.tpresume(&outer, 0);
        let _ = ctx.tpabort(0);
        let _ = ctx.tpclose();
        return Err(format!("inner enqueue failed: {e}"));
    }
    ctx.tpcommit(0)
        .map_err(|e| format!("inner tpcommit failed: {e}"))?;

    // Resume outer and abort it — OUTER-MSG must NOT be visible.
    ctx.tpresume(&outer, 0)
        .map_err(|e| format!("tpresume failed: {e}"))?;
    ctx.tpabort(0)
        .map_err(|e| format!("outer tpabort failed: {e}"))?;

    let val = dequeue_str(ctx)?;
    if val != "INNER-MSG" {
        let _ = ctx.tpclose();
        return Err(format!(
            "tx-suspend-resume: expected `INNER-MSG`, got `{val}`"
        ));
    }

    expect_empty_queue(ctx, "tx-suspend-resume postcondition")?;
    ctx.tpclose().map_err(|e| format!("tpclose failed: {e}"))?;
    Ok(())
}
