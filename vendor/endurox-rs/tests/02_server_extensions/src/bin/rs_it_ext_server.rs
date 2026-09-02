use endurox_rs::{
    ubf_fields, AtmiCtx, AtmiResult, PollerEvent, ServerHooks, TpReturnStatus, TpSvcInfo, UbfValue,
};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

static READ_FD: AtomicI32 = AtomicI32::new(-1);
static WRITE_FD: AtomicI32 = AtomicI32::new(-1);
static B4POLL_COUNT: AtomicUsize = AtomicUsize::new(0);
static POLLER_COUNT: AtomicUsize = AtomicUsize::new(0);
static PERIOD_COUNT: AtomicUsize = AtomicUsize::new(0);
static B4POLL_CTX_COUNT: AtomicUsize = AtomicUsize::new(0);
static POLLER_CTX_COUNT: AtomicUsize = AtomicUsize::new(0);
static PERIOD_CTX_COUNT: AtomicUsize = AtomicUsize::new(0);
static WRITE_PENDING: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);

fn rs_ext_b4poll_cb(ctx: &AtmiCtx) -> i32 {
    B4POLL_COUNT.fetch_add(1, Ordering::SeqCst);
    if ctx.tpgetsrvid().is_ok() {
        B4POLL_CTX_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    if !WRITE_PENDING.swap(true, Ordering::SeqCst) {
        let write_fd = WRITE_FD.load(Ordering::SeqCst);
        if write_fd >= 0 {
            let byte = [1_u8; 1];
            let _ = unsafe { libc::write(write_fd, byte.as_ptr().cast(), byte.len()) };
        }
    }

    0
}

fn rs_ext_poller_cb(ctx: &AtmiCtx, _event: PollerEvent) -> i32 {
    POLLER_COUNT.fetch_add(1, Ordering::SeqCst);
    if ctx.tpgetsrvid().is_ok() {
        POLLER_CTX_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    let read_fd = READ_FD.load(Ordering::SeqCst);
    if read_fd >= 0 {
        let mut byte = [0_u8; 1];
        let _ = unsafe { libc::read(read_fd, byte.as_mut_ptr().cast(), byte.len()) };
    }
    WRITE_PENDING.store(false, Ordering::SeqCst);

    0
}

fn rs_ext_period_cb(ctx: &AtmiCtx) -> i32 {
    PERIOD_COUNT.fetch_add(1, Ordering::SeqCst);
    if ctx.tpgetsrvid().is_ok() {
        PERIOD_CTX_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    0
}

fn rs_ext_install(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };

    let should_install = !INSTALLED.swap(true, Ordering::SeqCst);

    if should_install {
        let read_fd = READ_FD.load(Ordering::SeqCst);
        if ctx
            .tpext_addpollerfd(read_fd, libc::POLLIN as u32, 0, rs_ext_poller_cb)
            .and_then(|_| ctx.tpext_addb4pollcb(rs_ext_b4poll_cb))
            .and_then(|_| ctx.tpext_addperiodcb(1, rs_ext_period_cb))
            .is_err()
        {
            INSTALLED.store(false, Ordering::SeqCst);
            ctx.tpreturn_ubf(TpReturnStatus::Fail, 1, ubf, 0);
            return;
        }
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

fn rs_ext_status(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut ubf = match svc.take_data_ubf() {
        Some(b) => b,
        None => return,
    };

    let b4poll = B4POLL_COUNT.load(Ordering::SeqCst);
    let poller = POLLER_COUNT.load(Ordering::SeqCst);
    let period = PERIOD_COUNT.load(Ordering::SeqCst);
    let b4poll_ctx = B4POLL_CTX_COUNT.load(Ordering::SeqCst);
    let poller_ctx = POLLER_CTX_COUNT.load(Ordering::SeqCst);
    let period_ctx = PERIOD_CTX_COUNT.load(Ordering::SeqCst);
    let ok = b4poll > 0
        && poller > 0
        && period > 0
        && b4poll_ctx > 0
        && poller_ctx > 0
        && period_ctx > 0;

    let should_uninstall = ok && INSTALLED.swap(false, Ordering::SeqCst);

    if should_uninstall {
        let _ = ctx.tpext_delb4pollcb();
        let _ = ctx.tpext_delperiodcb();
        let read_fd = READ_FD.load(Ordering::SeqCst);
        let _ = ctx.tpext_delpollerfd(read_fd);
    }

    let rsp = format!(
        "b4poll={b4poll};poller={poller};period={period};b4ctx={b4poll_ctx};\
pollerctx={poller_ctx};periodctx={period_ctx};ok={ok}"
    );

    if ubf
        .bchg(ubf_fields::T_STRING_2_FLD, 0, UbfValue::String(rsp), true)
        .is_err()
    {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 2, ubf, 0);
        return;
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

fn rs_ext_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    let mut fds = [-1; 2];
    let pipe_rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if pipe_rc != 0 {
        return Err(ctx.atmi_last_error());
    }

    READ_FD.store(fds[0], Ordering::SeqCst);
    WRITE_FD.store(fds[1], Ordering::SeqCst);
    B4POLL_COUNT.store(0, Ordering::SeqCst);
    POLLER_COUNT.store(0, Ordering::SeqCst);
    PERIOD_COUNT.store(0, Ordering::SeqCst);
    B4POLL_CTX_COUNT.store(0, Ordering::SeqCst);
    POLLER_CTX_COUNT.store(0, Ordering::SeqCst);
    PERIOD_CTX_COUNT.store(0, Ordering::SeqCst);
    WRITE_PENDING.store(false, Ordering::SeqCst);
    INSTALLED.store(false, Ordering::SeqCst);

    ctx.tpadvertise("RS_EXT_INSTALL", rs_ext_install)?;
    ctx.tpadvertise("RS_EXT_STATUS", rs_ext_status)?;

    Ok(())
}

fn rs_ext_done(_ctx: &AtmiCtx) {
    let read_fd = READ_FD.swap(-1, Ordering::SeqCst);
    if read_fd >= 0 {
        let _ = unsafe { libc::close(read_fd) };
    }

    let write_fd = WRITE_FD.swap(-1, Ordering::SeqCst);
    if write_fd >= 0 {
        let _ = unsafe { libc::close(write_fd) };
    }
}

fn main() {
    let ctx = match AtmiCtx::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to create context: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = ctx.tp_run(ServerHooks::new(rs_ext_init).done(rs_ext_done)) {
        eprintln!("extension server failed: {e}");
        std::process::exit(1);
    }
}
