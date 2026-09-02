use endurox_rs::AtmiCtx;

const REQUEST_BYTES: [u8; 10] = [9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
const ITERATIONS: usize = 100;

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
    for iter in 0..ITERATIONS {
        let ctx = AtmiCtx::new().map_err(|e| format!("[iter {iter}] AtmiCtx::new: {e}"))?;
        ctx.tpinit()
            .map_err(|e| format!("[iter {iter}] tpinit: {e}"))?;

        let req = ctx
            .tpalloc_carray(&REQUEST_BYTES)
            .map_err(|e| format!("[iter {iter}] tpalloc_carray: {e}"))?;

        let req_info = req
            .tptypes()
            .map_err(|e| format!("[iter {iter}] request tptypes: {e}"))?;
        if req_info.type_name != "CARRAY" {
            return Err(format!(
                "[iter {iter}] unexpected request type=[{}] subtype=[{}]",
                req_info.type_name, req_info.subtype
            ));
        }

        let mut rsp = ctx
            .tpalloc_carray(&[])
            .map_err(|e| format!("[iter {iter}] reply tpalloc_carray: {e}"))?;

        println!("Sending: {:?}", req.as_bytes());

        ctx.tpcall("TESTSVC", &req, &mut rsp, 0)
            .map_err(|e| format!("[iter {iter}] tpcall: {e}"))?;

        let rsp_info = rsp
            .tptypes()
            .map_err(|e| format!("[iter {iter}] reply tptypes: {e}"))?;
        if rsp_info.type_name != "CARRAY" {
            return Err(format!(
                "[iter {iter}] unexpected reply type=[{}] subtype=[{}]",
                rsp_info.type_name, rsp_info.subtype
            ));
        }

        let bytes = rsp.as_bytes();
        let pretty = bytes
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "Got response (type=[{}] subtype=[{}] size={} len={}): {}",
            rsp_info.type_name,
            rsp_info.subtype,
            rsp_info.size,
            rsp.len(),
            pretty,
        );

        let expected: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        if bytes != expected {
            return Err(format!(
                "[iter {iter}] reply payload mismatch: got {bytes:?}, expected {expected:?}"
            ));
        }

        ctx.tpterm()
            .map_err(|e| format!("[iter {iter}] tpterm: {e}"))?;
    }

    Ok(())
}
