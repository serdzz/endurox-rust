use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use endurox_rs::{ubf_fields, AtmiCtx, UbfValue};

#[test]
fn bboolpr_prints_simple_field_comparison() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(42), false)
        .expect("Bchg failed");

    let tree = ctx.bboolco("T_LONG_FLD == 42").expect("Bboolco failed");
    let printed = ctx.bboolpr(&tree).expect("Bboolpr failed");

    assert!(!printed.is_empty(), "Bboolpr returned empty");
    assert!(
        printed.contains("T_LONG_FLD"),
        "expected field name in {printed:?}"
    );
    assert!(printed.contains("42"), "expected literal in {printed:?}");

    ctx.btreefree(tree);
}

#[test]
fn bboolpr_prints_compound_expression() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(100), false)
        .expect("long Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("hello".to_string()),
        false,
    )
    .expect("string Bchg failed");

    let tree = ctx
        .bboolco("(T_LONG_FLD > 50) && (T_STRING_FLD == 'hello')")
        .expect("Bboolco failed");

    assert!(ubf.bboolev(&tree));

    let printed = ctx.bboolpr(&tree).expect("Bboolpr failed");
    assert!(!printed.is_empty());
    assert!(printed.contains("T_LONG_FLD"));
    assert!(printed.contains("T_STRING_FLD"));
    assert!(printed.contains("hello"));

    ctx.btreefree(tree);
}

#[test]
fn bboolpr_roundtrips_through_bboolco() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    let tree1 = ctx
        .bboolco("T_LONG_FLD == 7")
        .expect("first Bboolco failed");
    let printed1 = ctx.bboolpr(&tree1).expect("first Bboolpr failed");
    ctx.btreefree(tree1);

    let tree2 = ctx
        .bboolco(printed1.trim())
        .expect("recompile of printed expression failed");
    let printed2 = ctx.bboolpr(&tree2).expect("second Bboolpr failed");
    ctx.btreefree(tree2);

    assert_eq!(printed1.trim(), printed2.trim());
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
    export CDPATH="${CDPATH:-}"
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}"
    export DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:-}"
    . "$HOME/ndrx_home"
fi
rm -f conf/app.ini conf/settest1
mkdir -p log
find log -type f -exec rm -f {} +
xadmin provision -d -vaddubf="$NDRX_RS_UNIT_UBF_FILE" >/dev/null
python3 -c "
import os, re
path = os.environ['NDRX_RS_UNIT_UBF_FILE']
d = os.path.dirname(path)
b = os.path.basename(path)
with open('conf/app.ini') as f:
    txt = f.read()
txt = re.sub(r'FIELDTBLS=Exfields,[^\n]+', 'FIELDTBLS=Exfields,' + b, txt)
txt = re.sub(r'(FLDTBLDIR=[^\n]+)', r'\1:' + d, txt)
with open('conf/app.ini', 'w') as f:
    f.write(txt)
"
. conf/settest1
export NDRX_CONFIG="$NDRX_RS_UNIT_TEST_DIR/conf/ndrxconfig.xml"
export FLDTBLDIR="$(dirname "$NDRX_RS_UNIT_UBF_FILE")"
export FIELDTBLS="$(basename "$NDRX_RS_UNIT_UBF_FILE")"
export NDRX_DEBUG_STR="file=$NDRX_RS_UNIT_TEST_DIR/log/boolexpr-print-tests.log ndrx=5"
env
"#,
            )
            .env("NDRX_RS_UNIT_TEST_DIR", &test_dir)
            .env("NDRX_RS_UNIT_UBF_FILE", &ubf_file)
            .output()
            .expect("failed to run xadmin provision for boolexpr_print tests");

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
