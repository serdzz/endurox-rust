use endurox_rs::{
    ubf_fields, AtmiCtx, AtmiResult, ServerHooks, TpReturnStatus, TpSvcInfo, UbfValue,
};

use std::sync::atomic::{AtomicUsize, Ordering};

static THRINIT_COUNT: AtomicUsize = AtomicUsize::new(0);
static THRINIT_CTX_COUNT: AtomicUsize = AtomicUsize::new(0);
static THRDONE_COUNT: AtomicUsize = AtomicUsize::new(0);

fn return_echo(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>, prefix: &str) {
    let mut ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };

    let req_fld = ubf_fields::T_STRING_FLD;
    let rsp_fld = ubf_fields::T_STRING_2_FLD;

    let req = match ubf.bget_string(req_fld, 0) {
        Ok(v) => v,
        Err(_) => {
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 1, ubf, 0);
            return;
        }
    };

    let rsp = format!("{prefix}:{req}");
    if ubf.bchg(rsp_fld, 0, UbfValue::String(rsp), true).is_err() {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 2, ubf, 0);
        return;
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

/// Per-worker-thread init hook. This server runs mindispatchthreads=2, so it
/// must fire once on each dispatch thread, on that thread, with that worker's
/// own context.
fn rs_it_thread_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    THRINIT_COUNT.fetch_add(1, Ordering::SeqCst);
    if ctx.tpgetsrvid().is_ok() {
        THRINIT_CTX_COUNT.fetch_add(1, Ordering::SeqCst);
    }
    Ok(())
}

/// Per-worker-thread shutdown hook.
///
/// This only runs as the server stops, so a live service call can never observe
/// it. Each worker appends a line to a marker file instead, which `run.sh`
/// checks after `xadmin stop`.
fn rs_it_thread_done(ctx: &AtmiCtx) {
    let n = THRDONE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
    let Ok(path) = std::env::var("NDRX_RS_THRDONE_MARKER") else {
        return;
    };
    // The context must still be usable here: Enduro/X calls this before the
    // worker's tpterm().
    let srvid = ctx.tpgetsrvid().unwrap_or(-1);
    // Build the whole line first and emit it with one write_all. Both workers
    // append here concurrently, and the write! macro issues several syscalls,
    // so formatting straight into the file interleaves their output and tears
    // the lines. A single short write to an O_APPEND fd is atomic.
    let line = format!("thrdone={n};srvid={srvid}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

/// Reports how many worker threads ran the Rust thread-init hook.
fn rs_it_thrinfo(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };
    let rsp = format!(
        "thrinit={};thrinitctx={};thrdone={}",
        THRINIT_COUNT.load(Ordering::SeqCst),
        THRINIT_CTX_COUNT.load(Ordering::SeqCst),
        THRDONE_COUNT.load(Ordering::SeqCst),
    );
    if ubf
        .bchg(ubf_fields::T_STRING_2_FLD, 0, UbfValue::String(rsp), true)
        .is_err()
    {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 1, ubf, 0);
        return;
    }
    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

fn rs_it_echo(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    return_echo(ctx, svc, "RUST-SERVER");
}

fn rs_it_thread(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let thread = format!("{:?}", std::thread::current().id());
    std::thread::sleep(std::time::Duration::from_millis(300));
    return_echo(ctx, svc, &thread);
}

fn rs_it_forward_final(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    return_echo(ctx, svc, "RUST-FORWARDED");
}

fn rs_it_forward(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };
    ctx.tpforward_ubf("RS_IT_FORWARD_FINAL", ubf, 0);
}

fn rs_it_inner_ubf(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut outer = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };

    let inner_ubf_fld = ubf_fields::T_UBF_FLD;
    let inner_req_fld = ubf_fields::T_STRING_3_FLD;
    let rsp_fld = ubf_fields::T_STRING_2_FLD;

    let req = match outer
        .bget_ubf(inner_ubf_fld, 0)
        .and_then(|inner| inner.bget_string(inner_req_fld, 0))
    {
        Ok(v) => v,
        Err(_) => {
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 3, outer, 0);
            return;
        }
    };

    let rsp = format!("RUST-INNER:{req}");
    if outer.bchg(rsp_fld, 0, UbfValue::String(rsp), true).is_err() {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 4, outer, 0);
        return;
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, outer, 0);
}

fn rs_it_dynamic(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    return_echo(ctx, svc, "RUST-DYNAMIC");
}

fn rs_it_control(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };

    let cmd_fld = ubf_fields::T_STRING_FLD;
    let target_fld = ubf_fields::T_STRING_2_FLD;
    let result_fld = ubf_fields::T_STRING_3_FLD;

    let cmd = match ubf.bget_string(cmd_fld, 0) {
        Ok(v) => v,
        Err(_) => {
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 1, ubf, 0);
            return;
        }
    };
    let target = match ubf.bget_string(target_fld, 0) {
        Ok(v) => v,
        Err(_) => {
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 2, ubf, 0);
            return;
        }
    };

    let outcome = match cmd.as_str() {
        "advertise" => ctx.tpadvertise(&target, rs_it_dynamic),
        "unadvertise" => ctx.tpunadvertise(&target),
        _ => {
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 3, ubf, 0);
            return;
        }
    };

    let status = match outcome {
        Ok(()) => "OK".to_string(),
        Err(e) => format!("ERR:{}", e.code),
    };
    if ubf
        .bchg(result_fld, 0, UbfValue::String(status), true)
        .is_err()
    {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 4, ubf, 0);
        return;
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

fn rs_it_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    ctx.tpadvertise("RS_IT_ECHO", rs_it_echo)?;
    ctx.tpadvertise("RS_IT_THREAD", rs_it_thread)?;
    ctx.tpadvertise("RS_IT_THRINFO", rs_it_thrinfo)?;
    ctx.tpadvertise("RS_IT_FORWARD", rs_it_forward)?;
    ctx.tpadvertise("RS_IT_FORWARD_FINAL", rs_it_forward_final)?;
    ctx.tpadvertise("RS_IT_INNER_UBF", rs_it_inner_ubf)?;
    ctx.tpadvertise("RS_IT_CONTROL", rs_it_control)?;
    Ok(())
}

fn rs_it_done(_ctx: &AtmiCtx) {}

fn main() {
    let ctx = match AtmiCtx::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to create context: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = ctx.tp_run(
        ServerHooks::new(rs_it_init)
            .done(rs_it_done)
            .thread_init(rs_it_thread_init)
            .thread_done(rs_it_thread_done),
    ) {
        eprintln!("server failed: {e}");
        std::process::exit(1);
    }
}
