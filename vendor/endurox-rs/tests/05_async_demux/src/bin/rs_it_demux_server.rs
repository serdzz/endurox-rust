//! Server for the reply-demux regression test.
//!
//! `RS_DEMUX_SLOW` deliberately lags so that `RS_DEMUX_FAST`'s reply lands on
//! the client's reply queue while the slow call's future is still parked. That
//! is the interleaving that used to strand a waiter.

use endurox_rs::{
    ubf_fields, AtmiCtx, AtmiResult, ServerHooks, TpReturnStatus, TpSvcInfo, UbfValue,
};
use std::time::Duration;

const SLOW_DELAY: Duration = Duration::from_millis(600);

fn echo_back(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>, tag: &str) {
    let mut ubf = match svc.take_data_ubf() {
        Some(ubf) => ubf,
        None => return,
    };

    let request = ubf
        .bget_string(ubf_fields::T_STRING_FLD, 0)
        .unwrap_or_default();

    if ubf
        .bchg(
            ubf_fields::T_STRING_2_FLD,
            0,
            UbfValue::String(format!("{tag}:{request}")),
            true,
        )
        .is_err()
    {
        ctx.tpreturn_ubf(TpReturnStatus::Fail, 1, ubf, 0);
        return;
    }

    ctx.tpreturn_ubf(TpReturnStatus::Success, 0, ubf, 0);
}

fn rs_demux_slow(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    std::thread::sleep(SLOW_DELAY);
    echo_back(ctx, svc, "SLOW");
}

fn rs_demux_fast(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    echo_back(ctx, svc, "FAST");
}

fn rs_demux_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    ctx.tpadvertise("RS_DEMUX_SLOW", rs_demux_slow)?;
    ctx.tpadvertise("RS_DEMUX_FAST", rs_demux_fast)?;
    Ok(())
}

fn rs_demux_done(_ctx: &AtmiCtx) {}

fn main() {
    let ctx = match AtmiCtx::new() {
        Ok(ctx) => ctx,
        Err(err) => {
            eprintln!("failed to create AtmiCtx: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = ctx.tp_run(ServerHooks::new(rs_demux_init).done(rs_demux_done)) {
        eprintln!("tp_run failed: {err}");
        std::process::exit(1);
    }
}
