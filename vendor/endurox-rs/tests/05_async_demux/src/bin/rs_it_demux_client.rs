//! Regression test for reply demultiplexing.
//!
//! Two calls are in flight on one context. `RS_DEMUX_SLOW` is submitted first
//! and lags; `RS_DEMUX_FAST` replies while the slow call's future is parked on
//! the reply fd.
//!
//! Before the demux, whichever future was polled first ran
//! `tpgetrply(own_cd, TPNOBLOCK)`, which pulled the *other* call's reply off the
//! OS queue and into Enduro/X's in-memory queue (`ndrx_add_to_memq`). The reply
//! fd then had nothing left to signal, and the future waiting on that reply
//! hung. With `tpgetrply(TPGETANY)` demultiplexing, every reply is accepted and
//! routed to its own descriptor, so both calls complete.

use endurox_rs::{ubf_fields, AtmiCtx, TokioAtmiCtx, UbfValue};
use std::time::{Duration, Instant};

/// Generous relative to the server's 600 ms delay, but far below any plausible
/// NDRX_TOUT, so a hang fails the test rather than stalling the suite.
const BUDGET: Duration = Duration::from_secs(20);

async fn call(ctx: &TokioAtmiCtx, svc: &str, payload: &str) -> Result<String, String> {
    let mut req = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("{svc}: tpalloc request failed: {e}"))?;
    req.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String(payload.to_owned()),
        true,
    )
    .map_err(|e| format!("{svc}: failed to set request field: {e}"))?;

    let mut rsp = ctx
        .tpalloc_ubf(4096)
        .map_err(|e| format!("{svc}: tpalloc response failed: {e}"))?;

    ctx.tpcall(svc, &req, &mut rsp, 0)
        .await
        .map_err(|e| format!("{svc}: tpcall failed: {e}"))?;

    rsp.bget_string(ubf_fields::T_STRING_2_FLD, 0)
        .map_err(|e| format!("{svc}: failed to read response field: {e}"))
}

async fn run() -> Result<(), String> {
    let ctx = AtmiCtx::new().map_err(|e| format!("failed to create AtmiCtx: {e}"))?;
    ctx.tpinit().map_err(|e| format!("tpinit failed: {e}"))?;
    let ctx = ctx
        .into_tokio()
        .map_err(|e| format!("into_tokio failed: {e}"))?;

    let started = Instant::now();

    // SLOW is listed first so it is polled first and parks first.
    let (slow, fast) = tokio::join!(
        call(&ctx, "RS_DEMUX_SLOW", "A"),
        call(&ctx, "RS_DEMUX_FAST", "B"),
    );

    let elapsed = started.elapsed();
    let slow = slow?;
    let fast = fast?;

    if slow != "SLOW:A" {
        return Err(format!("unexpected slow response `{slow}`"));
    }
    if fast != "FAST:B" {
        return Err(format!("unexpected fast response `{fast}`"));
    }
    if elapsed > BUDGET {
        return Err(format!(
            "calls completed but took {elapsed:.2?}, over budget"
        ));
    }

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    println!("demux ok: both replies routed in {elapsed:.2?}");
    Ok(())
}

/// `AtmiCtx` is `!Sync`, so its futures are `!Send`; a current-thread runtime is
/// required. It also makes the interleaving deterministic: exactly one thread
/// polls both calls, so the fast reply necessarily arrives while the slow
/// future is parked.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Belt and braces: if the demux regresses, the futures park forever and no
    // amount of waiting helps. Fail loudly instead of hanging the CI job.
    let outcome = match tokio::time::timeout(BUDGET, run()).await {
        Ok(outcome) => outcome,
        Err(_) => Err(format!(
            "timed out after {BUDGET:?} -- a reply was accepted but never routed \
             to its call descriptor"
        )),
    };

    if let Err(err) = outcome {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
