use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use endurox_rs::{
    AtmiCtx, TpQCtl, TypedBuffer, TypedUbf, UbfValue, TPQCORRID, TPQFAILUREQ, TPQGETBYCORRID,
    TPQMSGID, TPQPRIORITY, TPQREPLYQ,
};

#[test]
fn atmictx_init_integration() {
    let _guard = endurox_test_env();

    // new() now returns Result<Self, AtmiError>
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    // tpinit() returns AtmiResult<()>
    ctx.tpinit().expect("tpinit failed");

    endurox_rs::ndrx_error!(ctx, "Context created...");

    // tpterm() returns AtmiResult<()>
    ctx.tpterm().expect("tpterm failed");
}

#[cfg(feature = "ctx-send")]
#[test]
fn ctx_send_context_can_move_to_another_thread() {
    fn assert_send<T: Send>() {}
    assert_send::<AtmiCtx>();

    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create detached AtmiCtx");
    std::thread::spawn(move || {
        ctx.tpinit().expect("Object API tpinit failed after move");
        let buffer = ctx
            .tpalloc_carray(b"moved context")
            .expect("Object API tpalloc failed after move");
        drop(buffer);
        ctx.tpterm().expect("Object API tpterm failed after move");
    })
    .join()
    .expect("moved context thread panicked");
}

#[cfg(all(feature = "ctx-send", feature = "async-io"))]
#[test]
fn async_io_adapter_preserves_movable_context_type() {
    fn assert_send<T: Send>() {}
    assert_send::<endurox_rs::AsyncIoAtmiCtx>();
}

#[test]
fn tpalloc_generic_and_cast_to_ubf() {
    let _guard = endurox_test_env();

    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    // generic typed buffer
    let tbuf: TypedBuffer<'_> = ctx.tpalloc("UBF", "", 0).expect("tpalloc failed");

    // "inherit" by casting to TypedUbf
    let mut ubf: TypedUbf<'_> = TypedUbf::from_typed(tbuf);

    assert!(ubf.bsizeof().expect("Bsizeof failed") > 0);

    //ctx.tpterm().expect("tpterm failed");
    ctx.tpinit().expect("Second init shall go OK");
}

#[test]
fn tpalloc_ubf() {
    let _guard = endurox_test_env();

    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    endurox_rs::ndrx_error!(ctx, ">>>>> About to alloc UBF...");
    let mut buf = ctx.tpalloc_ubf(1025).expect("Shall Alloc buffer OK");

    buf.bchg(1, 0, UbfValue::Long(5), false)
        .expect("Bchg failed");

    endurox_rs::ndrx_error!(ctx, ">>>>> About to free UBF...");
    drop(buf);
    drop(ctx);
}

#[cfg(all(feature = "async", not(endurox_pollable)))]
#[test]
fn async_adapters_report_non_pollable_endurox_backend() {
    let _guard = endurox_test_env();
    assert!(!AtmiCtx::ASYNC_SUPPORTED);

    #[cfg(feature = "tokio")]
    {
        assert!(!AtmiCtx::TOKIO_ASYNC_SUPPORTED);
        let ctx = AtmiCtx::new().expect("failed to create Tokio AtmiCtx");
        ctx.tpinit().expect("Tokio context tpinit failed");
        let error = match ctx.into_tokio() {
            Ok(_) => panic!("non-pollable Enduro/X backend accepted Tokio adapter"),
            Err(error) => error,
        };
        assert_eq!(error.code, endurox_rs::AtmiError::TPEINVAL);
        assert!(error.message.contains("EX_USE_EPOLL"));
    }

    #[cfg(feature = "async-io")]
    {
        let ctx = AtmiCtx::new().expect("failed to create async-io AtmiCtx");
        ctx.tpinit().expect("async-io context tpinit failed");
        let error = match ctx.into_async_io() {
            Ok(_) => panic!("non-pollable Enduro/X backend accepted async-io adapter"),
            Err(error) => error,
        };
        assert_eq!(error.code, endurox_rs::AtmiError::TPEINVAL);
        assert!(error.message.contains("EX_USE_EPOLL"));
    }
}

#[test]
fn tpqctl_sets_flags_and_bounded_fields() {
    let mut qctl = TpQCtl::default();

    qctl.set_flags(TPQCORRID | TPQPRIORITY)
        .add_flags(TPQREPLYQ | TPQFAILUREQ | TPQMSGID)
        .clear_flags(TPQPRIORITY);
    assert_eq!(qctl.flags(), TPQCORRID | TPQREPLYQ | TPQFAILUREQ | TPQMSGID);

    qctl.set_corrid(b"ORDER-1001").expect("set corrid failed");
    qctl.set_msgid(b"MSG-1").expect("set msgid failed");
    qctl.set_reply_queue("REPLYQ")
        .expect("set reply queue failed");
    qctl.set_failure_queue("ERRORQ")
        .expect("set failure queue failed");
    qctl.set_priority(50)
        .set_deq_time(30)
        .set_delivery_qos(2)
        .set_reply_qos(4)
        .set_exp_time(60)
        .set_urcode(7)
        .set_appkey(9);

    assert_eq!(qctl.corrid(), b"ORDER-1001");
    assert_eq!(qctl.msgid(), b"MSG-1");
    assert_eq!(qctl.reply_queue(), "REPLYQ");
    assert_eq!(qctl.failure_queue(), "ERRORQ");
    assert_eq!(qctl.priority(), 50);
    assert_eq!(qctl.deq_time(), 30);
    assert_eq!(qctl.delivery_qos(), 2);
    assert_eq!(qctl.reply_qos(), 4);
    assert_eq!(qctl.exp_time(), 60);
    assert_eq!(qctl.urcode(), 7);
    assert_eq!(qctl.appkey(), 9);
    assert_eq!(qctl.diagnostic(), 0);
    assert_eq!(qctl.diagmsg(), "");

    qctl.set_flags(TPQGETBYCORRID);
    assert_eq!(qctl.flags(), TPQGETBYCORRID);

    assert!(qctl.set_corrid(&[b'x'; 32]).is_err());
    assert!(qctl.set_reply_queue("1234567890123456").is_err());
    assert!(qctl.set_failure_queue("bad\0queue").is_err());
}

fn endurox_test_env() -> MutexGuard<'static, ()> {
    let guard = match endurox_test_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    provision_endurox_env();
    guard
}

fn endurox_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn provision_endurox_env() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let test_dir = manifest_dir.join("tests").join("00_unittest");
        let ubf_file = manifest_dir.join("tests").join("ubftab").join("test.fd");

        let output = Command::new("bash")
            .arg("-lc")
            .arg(
                r#"
set -euo pipefail
cd "$NDRX_RS_UNIT_TEST_DIR"
if [ -f "$HOME/ndrx_home" ]; then
    . "$HOME/ndrx_home"
fi
rm -f conf/app.ini conf/settest1
mkdir -p log
find log -type f -exec rm -f {} +
xadmin provision -d -vaddubf="$NDRX_RS_UNIT_UBF_FILE" >/dev/null
. conf/settest1
unset NDRX_DEBUG_CONF
export NDRX_DEBUG_STR="file=$NDRX_RS_UNIT_TEST_DIR/log/atmi-tests.log ndrx=5"
env
"#,
            )
            .env("NDRX_RS_UNIT_TEST_DIR", &test_dir)
            .env("NDRX_RS_UNIT_UBF_FILE", &ubf_file)
            .output()
            .expect("failed to run xadmin provision for atmictx tests");

        if !output.status.success() {
            panic!(
                "xadmin provision failed with status={}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some((key, value)) = line.split_once('=') {
                env::set_var(key, value);
            }
        }
    });
}
