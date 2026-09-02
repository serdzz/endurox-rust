// Only the pollable variant of the test starts an Enduro/X domain, so the
// shared-domain lock is only needed there.
#[cfg(endurox_pollable)]
#[path = "common/endurox_domain_lock.rs"]
mod endurox_domain_lock;
#[cfg(endurox_pollable)]
use endurox_domain_lock::lock_endurox_domain;

#[cfg(endurox_pollable)]
use std::process::Command;

/// End-to-end proof that concurrent async calls on one context both complete.
///
/// Only meaningful on a pollable Enduro/X build: elsewhere `into_tokio()`
/// returns `TPEINVAL` and there is no reply fd to demultiplex, so the scenario
/// cannot be constructed at all.
#[cfg(endurox_pollable)]
#[test]
fn async_calls_are_demultiplexed_by_call_descriptor() {
    let _guard = lock_endurox_domain();

    let test_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/05_async_demux");
    let run_sh = format!("{test_dir}/run.sh");

    let output = Command::new("bash")
        .arg(&run_sh)
        .current_dir(test_dir)
        .output()
        .expect("failed to execute run.sh");

    if !output.status.success() {
        panic!(
            "async demux integration test failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[cfg(not(endurox_pollable))]
#[test]
fn async_calls_are_demultiplexed_by_call_descriptor() {
    eprintln!(
        "skipped: this Enduro/X build has no pollable reply queue \
         (needs EX_USE_EPOLL, or EX_USE_KQUEUE on FreeBSD)"
    );
}
