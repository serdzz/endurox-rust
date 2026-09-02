use endurox_rs::{AtmiCtx, AtmiResult, ServerHooks, TpReturnStatus, TpSvcInfo};

const RESPONSE_BYTES: [u8; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

fn testsvc(ctx: &AtmiCtx, svc: &mut TpSvcInfo<'_>) {
    let mut buf = match svc.take_data() {
        Some(b) => b,
        None => return,
    };

    let info = match buf.tptypes() {
        Ok(info) => info,
        Err(_) => {
            ctx.tpreturn(TpReturnStatus::Fail, 1, buf, 0);
            return;
        }
    };

    println!(
        "Incoming request type=[{}] subtype=[{}] size={} len={}: {:?}",
        info.type_name,
        info.subtype,
        info.size,
        buf.len(),
        buf.as_bytes(),
    );

    if info.type_name != "CARRAY" {
        eprintln!(
            "expected CARRAY buffer, got type=[{}] subtype=[{}]",
            info.type_name, info.subtype
        );
        ctx.tpreturn(TpReturnStatus::Fail, 2, buf, 0);
        return;
    }

    if let Err(err) = buf.tprealloc(128) {
        eprintln!("tprealloc failed: {err}");
        ctx.tpreturn(TpReturnStatus::Fail, 3, buf, 0);
        return;
    }

    if let Err(err) = buf.set_bytes(&RESPONSE_BYTES) {
        eprintln!("set_bytes failed: {err}");
        ctx.tpreturn(TpReturnStatus::Fail, 4, buf, 0);
        return;
    }

    ctx.tpreturn(TpReturnStatus::Success, 0, buf, 0);
}

fn rs_it_init(ctx: &AtmiCtx, _args: &[String]) -> AtmiResult<()> {
    ctx.tpadvertise("TESTSVC", testsvc)?;
    Ok(())
}

fn rs_it_done(_ctx: &AtmiCtx) {
    eprintln!("Server shutting down...");
}

fn main() {
    let ctx = match AtmiCtx::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to create context: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = ctx.tp_run(ServerHooks::new(rs_it_init).done(rs_it_done)) {
        eprintln!("server failed: {e}");
        std::process::exit(1);
    }
}
