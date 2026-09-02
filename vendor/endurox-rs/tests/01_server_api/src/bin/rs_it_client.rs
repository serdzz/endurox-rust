use std::collections::HashSet;

use endurox_rs::{ubf_fields, AtmiCtx, AtmiError, TypedUbf, UbfValue, TPGETANY};

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
        .unwrap_or_else(|| "tpcall".to_string());

    let ctx = AtmiCtx::new().map_err(|e| format!("failed to create AtmiCtx: {e}"))?;
    ctx.tpinit().map_err(|e| format!("tpinit failed: {e}"))?;

    let (svc, expected) = match scenario.as_str() {
        "tpcall" => ("RS_IT_ECHO", "RUST-SERVER:HELLO"),
        "tpforward" => ("RS_IT_FORWARD", "RUST-FORWARDED:HELLO"),
        "inner-ubf" => ("RS_IT_INNER_UBF", "RUST-INNER:HELLO-INNER"),
        "tpacall" => ("RS_IT_ECHO", "RUST-SERVER:HELLO"),
        "tpacall-getany" => return run_tpacall_getany(&ctx),
        "dispatch-threads" => return run_dispatch_threads(&ctx),
        "dynamic-advertise" => return run_dynamic_advertise(&ctx),
        other => return Err(format!("unknown integration scenario `{other}`")),
    };

    let req_fld = ubf_fields::T_STRING_FLD;
    let rsp_fld = ubf_fields::T_STRING_2_FLD;

    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;

    if scenario == "inner-ubf" {
        let inner_ubf_fld = ubf_fields::T_UBF_FLD;
        let inner_req_fld = ubf_fields::T_STRING_3_FLD;

        let mut inner = ctx
            .tpalloc_ubf(512)
            .map_err(|e| format!("failed to allocate inner UBF: {e}"))?;
        inner
            .bchg(
                inner_req_fld,
                0,
                UbfValue::String("HELLO-INNER".to_string()),
                true,
            )
            .map_err(|e| format!("failed to set inner request field: {e}"))?;
        buf.bchg(inner_ubf_fld, 0, UbfValue::Ubf(inner), true)
            .map_err(|e| format!("failed to set embedded UBF field: {e}"))?;
    } else {
        buf.bchg(req_fld, 0, UbfValue::String("HELLO".to_string()), true)
            .map_err(|e| format!("failed to set request field: {e}"))?;
    }

    if scenario == "tpacall" {
        let mut cd = ctx
            .tpacall(svc, &buf, 0)
            .map_err(|e| format!("tpacall failed: {e}"))?;
        ctx.tpgetrply(&mut cd, &mut buf, 0)
            .map_err(|e| format!("tpgetrply failed: {e}"))?;

        assert_response(&buf, rsp_fld, expected)?;
    } else {
        let mut rsp = ctx
            .tpalloc_ubf(1024)
            .map_err(|e| format!("reply tpalloc_ubf failed: {e}"))?;
        ctx.tpcall(svc, &buf, &mut rsp, 0)
            .map_err(|e| format!("tpcall failed: {e}"))?;

        assert_response(&rsp, rsp_fld, expected)?;
    }

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    Ok(())
}

fn run_tpacall_getany(ctx: &AtmiCtx) -> Result<(), String> {
    let rsp_fld = ubf_fields::T_STRING_2_FLD;
    let mut first = build_echo_request(ctx, "FIRST")?;
    let second = build_echo_request(ctx, "SECOND")?;

    let first_cd = ctx
        .tpacall("RS_IT_ECHO", &first, 0)
        .map_err(|e| format!("first tpacall failed: {e}"))?;
    let second_cd = ctx
        .tpacall("RS_IT_ECHO", &second, 0)
        .map_err(|e| format!("second tpacall failed: {e}"))?;

    let mut pending = HashSet::from([first_cd, second_cd]);
    let mut expected = HashSet::from([
        "RUST-SERVER:FIRST".to_string(),
        "RUST-SERVER:SECOND".to_string(),
    ]);

    for _ in 0..2 {
        let mut cd = 0;
        ctx.tpgetrply(&mut cd, &mut first, TPGETANY)
            .map_err(|e| format!("tpgetrply TPGETANY failed: {e}"))?;

        if !pending.remove(&cd) {
            return Err(format!("unexpected async call descriptor returned: {cd}"));
        }

        let rsp = first
            .bget_string(rsp_fld, 0)
            .map_err(|e| format!("failed to read async response field: {e}"))?;
        if !expected.remove(&rsp) {
            return Err(format!("unexpected async response: `{rsp}`"));
        }
    }

    if !pending.is_empty() || !expected.is_empty() {
        return Err(format!(
            "async replies incomplete: pending={pending:?}, expected={expected:?}"
        ));
    }

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    Ok(())
}

fn run_dispatch_threads(ctx: &AtmiCtx) -> Result<(), String> {
    let rsp_fld = ubf_fields::T_STRING_2_FLD;
    let mut first = build_echo_request(ctx, "FIRST")?;
    let second = build_echo_request(ctx, "SECOND")?;

    ctx.tpacall("RS_IT_THREAD", &first, 0)
        .map_err(|e| format!("first threaded tpacall failed: {e}"))?;
    ctx.tpacall("RS_IT_THREAD", &second, 0)
        .map_err(|e| format!("second threaded tpacall failed: {e}"))?;

    let mut worker_threads = HashSet::new();
    for _ in 0..2 {
        let mut cd = 0;
        ctx.tpgetrply(&mut cd, &mut first, TPGETANY)
            .map_err(|e| format!("threaded tpgetrply failed: {e}"))?;
        let response = first
            .bget_string(rsp_fld, 0)
            .map_err(|e| format!("failed to read worker thread response: {e}"))?;
        let worker = response
            .split_once(':')
            .map(|(worker, _)| worker.to_owned())
            .ok_or_else(|| format!("invalid worker response `{response}`"))?;
        worker_threads.insert(worker);
    }

    if worker_threads.len() != 2 {
        return Err(format!(
            "expected two libatmisrv worker threads, got {worker_threads:?}"
        ));
    }

    // Each of those workers must also have run the Rust tpsvrthrinit hook, on
    // its own thread, with a usable worker context.
    let mut info = build_echo_request(ctx, "THRINFO")?;
    let mut rsp = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc for thread info failed: {e}"))?;
    ctx.tpcall("RS_IT_THRINFO", &info, &mut rsp, 0)
        .map_err(|e| format!("RS_IT_THRINFO tpcall failed: {e}"))?;
    let report = rsp
        .bget_string(rsp_fld, 0)
        .map_err(|e| format!("failed to read thread info: {e}"))?;
    for expect in ["thrinit=2", "thrinitctx=2"] {
        if !report.contains(expect) {
            return Err(format!(
                "expected `{expect}` in thread-hook report, got `{report}`"
            ));
        }
    }
    drop(info);

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    Ok(())
}

fn run_dynamic_advertise(ctx: &AtmiCtx) -> Result<(), String> {
    const TARGET: &str = "RS_IT_DYNAMIC";
    let rsp_fld = ubf_fields::T_STRING_2_FLD;

    expect_no_entry(ctx, TARGET, "before advertise")?;

    let status = control_call(ctx, "advertise", TARGET)?;
    if status != "OK" {
        return Err(format!("control advertise failed with status `{status}`"));
    }

    let mut req = build_echo_request(ctx, "HELLO-DYN")?;
    let mut rsp = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("reply tpalloc_ubf failed: {e}"))?;
    ctx.tpcall(TARGET, &req, &mut rsp, 0)
        .map_err(|e| format!("tpcall to dynamically advertised service failed: {e}"))?;
    assert_response(&rsp, rsp_fld, "RUST-DYNAMIC:HELLO-DYN")?;

    let status = control_call(ctx, "unadvertise", TARGET)?;
    if status != "OK" {
        return Err(format!("control unadvertise failed with status `{status}`"));
    }

    expect_no_entry(ctx, TARGET, "after unadvertise")?;

    // sanity: re-advertise to confirm idempotency on re-entry
    let status = control_call(ctx, "advertise", TARGET)?;
    if status != "OK" {
        return Err(format!("re-advertise failed with status `{status}`"));
    }
    req.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("AGAIN".to_string()),
        true,
    )
    .map_err(|e| format!("failed to reset request field: {e}"))?;
    ctx.tpcall(TARGET, &req, &mut rsp, 0)
        .map_err(|e| format!("tpcall after re-advertise failed: {e}"))?;
    assert_response(&rsp, rsp_fld, "RUST-DYNAMIC:AGAIN")?;

    let status = control_call(ctx, "unadvertise", TARGET)?;
    if status != "OK" {
        return Err(format!("final unadvertise failed with status `{status}`"));
    }

    ctx.tpterm().map_err(|e| format!("tpterm failed: {e}"))?;
    Ok(())
}

fn control_call(ctx: &AtmiCtx, cmd: &str, target: &str) -> Result<String, String> {
    let mut req = ctx
        .tpalloc_ubf(512)
        .map_err(|e| format!("control tpalloc_ubf failed: {e}"))?;
    req.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String(cmd.to_string()),
        true,
    )
    .map_err(|e| format!("failed to set control command: {e}"))?;
    req.bchg(
        ubf_fields::T_STRING_2_FLD,
        0,
        UbfValue::String(target.to_string()),
        true,
    )
    .map_err(|e| format!("failed to set control target: {e}"))?;

    let mut rsp = ctx
        .tpalloc_ubf(512)
        .map_err(|e| format!("control reply tpalloc_ubf failed: {e}"))?;
    ctx.tpcall("RS_IT_CONTROL", &req, &mut rsp, 0)
        .map_err(|e| format!("RS_IT_CONTROL tpcall failed for `{cmd}`: {e}"))?;

    rsp.bget_string(ubf_fields::T_STRING_3_FLD, 0)
        .map_err(|e| format!("failed to read control result field: {e}"))
}

fn expect_no_entry(ctx: &AtmiCtx, svc: &str, phase: &str) -> Result<(), String> {
    let req = ctx
        .tpalloc_ubf(256)
        .map_err(|e| format!("tpalloc_ubf for negative tpcall failed: {e}"))?;
    let mut rsp = ctx
        .tpalloc_ubf(256)
        .map_err(|e| format!("reply tpalloc_ubf for negative tpcall failed: {e}"))?;
    match ctx.tpcall(svc, &req, &mut rsp, 0) {
        Ok(()) => Err(format!(
            "expected `{svc}` to be unadvertised {phase}, but tpcall succeeded"
        )),
        Err(e) if e.code == AtmiError::TPENOENT => Ok(()),
        Err(e) => Err(format!(
            "expected TPENOENT for `{svc}` {phase}, got code {} ({e})",
            e.code
        )),
    }
}

fn build_echo_request<'a>(ctx: &'a AtmiCtx, value: &str) -> Result<TypedUbf<'a>, String> {
    let mut buf = ctx
        .tpalloc_ubf(1024)
        .map_err(|e| format!("tpalloc_ubf failed: {e}"))?;
    buf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String(value.to_string()),
        true,
    )
    .map_err(|e| format!("failed to set request field: {e}"))?;
    Ok(buf)
}

fn assert_response(buf: &TypedUbf<'_>, rsp_fld: i32, expected: &str) -> Result<(), String> {
    let rsp = buf
        .bget_string(rsp_fld, 0)
        .map_err(|e| format!("failed to read response field: {e}"))?;

    if rsp != expected {
        return Err(format!(
            "unexpected response: expected `{expected}`, got `{rsp}`"
        ));
    }

    Ok(())
}
