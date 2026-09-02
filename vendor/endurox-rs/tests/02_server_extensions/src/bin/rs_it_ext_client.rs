use endurox_rs::{ubf_fields, AtmiCtx};
use std::thread;
use std::time::Duration;

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
    let ctx = AtmiCtx::new().map_err(|e| format!("failed to create AtmiCtx: {e}"))?;
    ctx.tpinit().map_err(|e| format!("tpinit failed: {e}"))?;

    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;

    let mut rsp = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("reply tpalloc_ubf failed: {e}"))?;

    ctx.tpcall("RS_EXT_INSTALL", &buf, &mut rsp, 0)
        .map_err(|e| format!("install tpcall failed: {e}"))?;

    let mut last_rsp = None;
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(100));

        ctx.tpcall("RS_EXT_STATUS", &buf, &mut rsp, 0)
            .map_err(|e| format!("status tpcall failed: {e}"))?;

        let status = rsp
            .bget_string(ubf_fields::T_STRING_2_FLD, 0)
            .map_err(|e| format!("failed to read extension status: {e}"))?;

        if status.contains("ok=true") {
            ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
            return Ok(());
        }

        last_rsp = Some(status);
    }

    ctx.tpcall("RS_EXT_STATUS", &buf, &mut rsp, 0)
        .map_err(|e| format!("final status tpcall failed: {e}"))?;

    let final_rsp = rsp
        .bget_string(ubf_fields::T_STRING_2_FLD, 0)
        .map_err(|e| format!("failed to read extension status: {e}"))?;

    if !final_rsp.contains("ok=true") {
        let last_rsp = last_rsp.unwrap_or(final_rsp);
        return Err(format!("extension callbacks did not fire: {last_rsp}"));
    }

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    Ok(())
}
