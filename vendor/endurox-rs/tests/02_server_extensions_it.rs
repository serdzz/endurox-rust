#[path = "common/endurox_domain_lock.rs"]
mod endurox_domain_lock;
use endurox_domain_lock::lock_endurox_domain;

use std::process::Command;

#[test]
fn xatmi_server_extensions_poller_and_before_poll_callbacks() {
    run_extension_scenario(None);
}

#[test]
fn xatmi_server_extensions_receive_oapi_main_context() {
    run_extension_scenario(Some("ctx-send"));
}

fn run_extension_scenario(feature: Option<&str>) {
    let _guard = lock_endurox_domain();
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_dir = manifest_dir.join("tests").join("02_server_extensions");
    let run_sh = test_dir.join("run.sh");

    assert!(
        run_sh.exists(),
        "missing integration script: {}",
        run_sh.display()
    );

    let output = Command::new("bash")
        .arg(&run_sh)
        .args(feature)
        .current_dir(&test_dir)
        .output()
        .expect("failed to execute run.sh");

    if !output.status.success() {
        panic!(
            "run.sh failed with status={}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
